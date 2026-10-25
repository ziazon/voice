use persona_builder::{assistants, error::Result};
use persona_builder_api::{dto::*, *};
use persona_builder_save::{
    flow::MemberPaths,
    git::Git,
    github::{Endpoints, Github, Http, HttpRequest, HttpResponse},
    keychain::{MemoryStore, TokenStore},
    record::{SaveRecord, read_record, write_record},
    secret::Secret,
    text,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const FIND: &str = "Persona Builder couldn't find that assistant. Close it and open it again.";
const CHANGED: &str =
    "The places to keep your assistant have changed. Look at them again, then choose.";
const SAVE: &str =
    "Persona Builder couldn't save your assistant. Try again, or keep it on this computer only.";
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-api-save-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn make(&self) -> MemberPaths {
        let made = assistants::make(
            &self.0,
            &assistants::New {
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
            },
            "2026-10-25",
        )
        .unwrap();
        MemberPaths::load(&made.dir.join("member.json"), &self.0).unwrap()
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn real_git() -> &'static Path {
    static GIT: OnceLock<PathBuf> = OnceLock::new();
    GIT.get_or_init(|| {
        let out = Command::new("/bin/sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        assert!(out.status.success());
        PathBuf::from(String::from_utf8(out.stdout).unwrap().trim())
    })
}
fn git() -> Git {
    Git::at(real_git().into())
}
fn run_git(dir: &Path, args: &[&str]) {
    let out = Command::new(real_git())
        .env_clear()
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn slug(value: &str) -> SlugRequest {
    SlugRequest { slug: value.into() }
}
fn choice(index: u32, name: Option<&str>) -> SaveChoiceRequest {
    SaveChoiceRequest {
        slug: "ari".into(),
        choice: index,
        kind: if index == 0 {
            SaveKind::Github
        } else {
            SaveKind::Local
        },
        name: name.map(str::to_owned),
    }
}
struct FakeHttp {
    replies: RefCell<VecDeque<HttpResponse>>,
    requests: RefCell<Vec<String>>,
}
impl FakeHttp {
    fn new(replies: Vec<HttpResponse>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            requests: RefCell::new(vec![]),
        }
    }
    fn drained(&self) {
        assert!(self.replies.borrow().is_empty(), "unused HTTP reply");
    }
}
impl Http for FakeHttp {
    fn send(&self, request: &HttpRequest<'_>) -> Result<HttpResponse> {
        self.requests.borrow_mut().push(request.url.clone());
        Ok(self
            .replies
            .borrow_mut()
            .pop_front()
            .expect("unexpected HTTP request"))
    }
}
fn reply(body: Value) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: vec![],
        body: serde_json::to_vec(&body).unwrap(),
    }
}
fn code() -> HttpResponse {
    reply(
        json!({"device_code":"private-device-code", "user_code":"ABCD-EFGH", "verification_uri":"https://github.com/untrusted-path", "interval":1, "expires_in":900}),
    )
}
fn token() -> HttpResponse {
    reply(json!({"access_token":"test-token", "token_type":"bearer", "scope":"repo"}))
}
fn account(scope: &str) -> HttpResponse {
    let mut response = reply(json!({"login":"example-owner", "id":42}));
    response
        .headers
        .push(("x-oauth-scopes".into(), scope.into()));
    response
}
fn github(http: &dyn Http) -> Github<'_> {
    Github::new(http, Endpoints::github(), "test.client").unwrap()
}

#[test]
fn options_describe_choices_saved_choice_and_sign_in() {
    let home = Home::new();
    home.make();
    let tokens = MemoryStore::new();
    let options = save_options_in(&home.0, slug("ari"), &git(), &tokens).unwrap();
    assert_eq!(
        options.choices,
        vec![
            SaveOption {
                kind: SaveKind::Github,
                label: text::CHOICE_GITHUB.into()
            },
            SaveOption {
                kind: SaveKind::Local,
                label: text::CHOICE_LOCAL.into()
            }
        ]
    );
    assert_eq!(options.selected, 0);
    assert_eq!(options.note.as_deref(), Some(text::NO_GITHUB_ACCOUNT));
    assert_eq!(options.chosen, None);
    assert!(!options.signed_in);
    assert_eq!(options.suggested_name, "ari-assistant");
    save_choose_in(&home.0, choice(1, Some("ignored")), &git(), None, &tokens).unwrap();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let options = save_options_in(&home.0, slug("ari"), &git(), &tokens).unwrap();
    assert_eq!(options.chosen.as_deref(), Some(text::CHOSE_LOCAL));
    assert!(options.signed_in);
    let claude = home.0.join(".claude");
    run_git(&claude, &["init", "-q", "--initial-branch=main"]);
    run_git(
        &claude,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example-owner/setup.git",
        ],
    );
    let options = save_options_in(&home.0, slug("ari"), &git(), &tokens).unwrap();
    assert_eq!(
        options.choices,
        vec![
            SaveOption {
                kind: SaveKind::Existing,
                label: text::choice_existing("~/.claude")
            },
            SaveOption {
                kind: SaveKind::Local,
                label: text::CHOICE_LOCAL.into()
            }
        ]
    );
    assert_eq!(options.note, None);
}

#[test]
fn every_slug_command_refuses_unlisted_and_bad_slugs() {
    let home = Home::new();
    home.make();
    // An existing folder without a valid listed member is still unlisted.
    fs::create_dir_all(home.0.join(".claude/crew/absent")).unwrap();
    let tokens = MemoryStore::new();
    for value in ["absent", "../x", ""] {
        assert_eq!(
            save_options_in(&home.0, slug(value), &git(), &tokens)
                .unwrap_err()
                .message,
            FIND,
            "options {value:?}"
        );
        let mut request = choice(1, None);
        request.slug = value.into();
        assert_eq!(
            save_choose_in(&home.0, request, &git(), None, &tokens)
                .unwrap_err()
                .message,
            FIND,
            "choose {value:?}"
        );
        assert_eq!(
            save_push_in(&home.0, slug(value), &git(), None, &tokens)
                .unwrap_err()
                .message,
            FIND,
            "push {value:?}"
        );
    }
}

#[test]
fn choose_local_persists_and_out_of_range_is_changed() {
    let home = Home::new();
    let member = home.make();
    let tokens = MemoryStore::new();
    assert_eq!(
        save_choose_in(&home.0, choice(1, None), &git(), None, &tokens)
            .unwrap()
            .message,
        text::CHOSE_LOCAL
    );
    assert_eq!(
        read_record(&member.state_dir).unwrap(),
        Some(SaveRecord::Local)
    );
    assert!(member.state_dir.join("save.json").is_file());
    for index in [2, u32::MAX] {
        assert_eq!(
            save_choose_in(&home.0, choice(index, None), &git(), None, &tokens)
                .unwrap_err()
                .message,
            CHANGED
        );
    }
}

#[test]
fn choose_github_requires_name_sign_in_and_client_id() {
    let home = Home::new();
    home.make();
    let tokens = MemoryStore::new();
    let http = FakeHttp::new(vec![]);
    let github = github(&http);
    assert_eq!(
        save_choose_in(&home.0, choice(0, None), &git(), Some(&github), &tokens)
            .unwrap_err()
            .message,
        text::BAD_REPO_NAME
    );
    assert_eq!(
        save_choose_in(
            &home.0,
            choice(0, Some("ari-assistant")),
            &git(),
            Some(&github),
            &tokens
        )
        .unwrap_err()
        .message,
        text::SIGN_IN_FIRST
    );
    assert_eq!(
        save_choose_in(
            &home.0,
            choice(0, Some("ari-assistant")),
            &git(),
            None,
            &tokens
        )
        .unwrap_err()
        .message,
        text::NOT_SET_UP
    );
    assert!(http.requests.borrow().is_empty());
}

#[test]
fn push_requires_a_choice_and_keeps_local() {
    let home = Home::new();
    home.make();
    let tokens = MemoryStore::new();
    assert_eq!(
        save_push_in(&home.0, slug("ari"), &git(), None, &tokens)
            .unwrap_err()
            .message,
        text::CHOOSE_FIRST
    );
    save_choose_in(&home.0, choice(1, None), &git(), None, &tokens).unwrap();
    assert_eq!(
        save_push_in(&home.0, slug("ari"), &git(), None, &tokens)
            .unwrap()
            .message,
        text::KEPT_LOCAL
    );
}

#[test]
fn push_reports_privacy_hits_before_network() {
    let home = Home::new();
    let member = home.make();
    let tokens = MemoryStore::new();
    write_record(
        &member.state_dir,
        &SaveRecord::Github {
            owner: "example-owner".into(),
            name: "ari-assistant".into(),
            branch: "main".into(),
        },
    )
    .unwrap();
    fs::write(member.dir.join("notes.md"), "my doctor\n").unwrap();
    let http = FakeHttp::new(vec![]);
    let github = github(&http);
    assert_eq!(
        save_push_in(&home.0, slug("ari"), &git(), Some(&github), &tokens)
            .unwrap_err()
            .message,
        text::privacy_hits("notes.md:1 (personal)")
    );
    assert!(http.requests.borrow().is_empty());
}

#[test]
fn damaged_record_uses_fallback_not_internal_error() {
    let home = Home::new();
    let member = home.make();
    let tokens = MemoryStore::new();
    write_record(&member.state_dir, &SaveRecord::Local).unwrap();
    fs::write(member.state_dir.join("save.json"), "not JSON").unwrap();
    assert_eq!(
        save_push_in(&home.0, slug("ari"), &git(), None, &tokens)
            .unwrap_err()
            .message,
        SAVE
    );
    assert_eq!(
        save_options_in(&home.0, slug("ari"), &git(), &tokens)
            .unwrap()
            .chosen,
        None
    );
}

#[test]
fn start_exposes_only_user_code_and_finish_saves_scoped_token() {
    let http = FakeHttp::new(vec![
        code(),
        reply(json!({"error":"authorization_pending"})),
        token(),
        account("read:user, repo"),
    ]);
    let github = github(&http);
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    let answer = github_sign_in_start_in(&sign_ins, Some(&github)).unwrap();
    assert_eq!(
        serde_json::to_value(answer).unwrap(),
        json!({"userCode":"ABCD-EFGH"})
    );
    let mut slept = vec![];
    let answer = github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |duration| {
        slept.push(duration)
    })
    .unwrap();
    assert_eq!(
        serde_json::to_value(answer).unwrap(),
        json!({"message":text::signed_in("example-owner")})
    );
    assert_eq!(tokens.load().unwrap().unwrap().expose(), "test-token");
    assert!(!slept.is_empty());
    assert!(slept.iter().all(|d| *d <= Duration::from_millis(250)));
    assert_eq!(slept.iter().sum::<Duration>(), Duration::from_secs(2));
    http.drained();
}

#[test]
fn finish_requires_start_and_start_requires_client_id() {
    let http = FakeHttp::new(vec![]);
    let github = github(&http);
    let tokens = MemoryStore::new();
    let sign_ins = SignIns::new();
    assert_eq!(
        github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |_| panic!(
            "unexpected sleep"
        ))
        .unwrap_err()
        .message,
        "Start signing in to GitHub again."
    );
    assert_eq!(
        github_sign_in_start_in(&sign_ins, None)
            .unwrap_err()
            .message,
        text::NOT_SET_UP
    );
}

#[test]
fn cancel_after_last_sleep_prevents_another_request() {
    let http = FakeHttp::new(vec![
        code(),
        reply(json!({"error":"authorization_pending"})),
    ]);
    let github = github(&http);
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    github_sign_in_start_in(&sign_ins, Some(&github)).unwrap();
    let mut steps = 0;
    let error = github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |duration| {
        assert_eq!(duration, Duration::from_millis(250));
        steps += 1;
        if steps == 8 {
            github_sign_in_cancel_in(&sign_ins).unwrap();
        }
    })
    .unwrap_err();
    assert_eq!(error.message, text::SIGN_IN_CANCELLED);
    assert_eq!(steps, 8);
    assert_eq!(http.requests.borrow().len(), 2);
    assert!(tokens.load().unwrap().is_none());
    http.drained();
    assert_eq!(
        github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |_| {})
            .unwrap_err()
            .message,
        "Start signing in to GitHub again."
    );
}

#[test]
fn second_start_cancels_the_finish_holding_old_code() {
    let first = FakeHttp::new(vec![code()]);
    let first_github = github(&first);
    let second = FakeHttp::new(vec![code(), token(), account("repo")]);
    let second_github = github(&second);
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    github_sign_in_start_in(&sign_ins, Some(&first_github)).unwrap();
    let mut started = false;
    assert_eq!(
        github_sign_in_finish_in(&sign_ins, Some(&first_github), &tokens, &mut |_| {
            if !started {
                started = true;
                github_sign_in_start_in(&sign_ins, Some(&second_github)).unwrap();
            }
        })
        .unwrap_err()
        .message,
        text::SIGN_IN_CANCELLED
    );
    assert_eq!(first.requests.borrow().len(), 1);
    assert!(tokens.load().unwrap().is_none());
    assert_eq!(
        github_sign_in_finish_in(&sign_ins, Some(&second_github), &tokens, &mut |_| {})
            .unwrap()
            .message,
        text::signed_in("example-owner")
    );
    first.drained();
    second.drained();
}

#[test]
fn finish_refuses_token_without_repo_scope() {
    let http = FakeHttp::new(vec![code(), token(), account("read:user")]);
    let github = github(&http);
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    github_sign_in_start_in(&sign_ins, Some(&github)).unwrap();
    assert_eq!(
        github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |_| {})
            .unwrap_err()
            .message,
        text::NO_REPO_PERMISSION
    );
    assert!(tokens.load().unwrap().is_none());
    http.drained();
}

#[test]
fn sign_out_forgets_token() {
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    assert_eq!(
        github_sign_out_in(&sign_ins, &tokens).unwrap().message,
        text::SIGNED_OUT
    );
    assert!(tokens.load().unwrap().is_none());
}

#[test]
fn links_use_fixed_pages_and_page_specific_failure() {
    for (link, page) in [
        (Link::GithubSignup, "https://github.com/signup"),
        (Link::GithubDevice, "https://github.com/login/device"),
    ] {
        open_link_in(LinkRequest { link: link.clone() }, |url| {
            assert_eq!(url, page);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            open_link_in(LinkRequest { link }, |url| {
                assert_eq!(url, page);
                Err(std::io::Error::other("private opener detail"))
            })
            .unwrap_err()
            .message,
            format!("Persona Builder couldn't open your web browser. Go to {page} yourself.")
        );
    }
}

#[test]
fn dto_json_uses_camel_case_and_requests_refuse_extra_keys() {
    let options = SaveOptions {
        choices: vec![SaveOption {
            kind: SaveKind::Github,
            label: "label".into(),
        }],
        selected: 0,
        note: None,
        chosen: None,
        signed_in: false,
        suggested_name: "ari-assistant".into(),
    };
    assert_eq!(
        serde_json::to_value(options).unwrap(),
        json!({"choices":[{"kind":"github","label":"label"}],"selected":0,"note":null,"chosen":null,"signedIn":false,"suggestedName":"ari-assistant"})
    );
    for (kind, value) in [
        (SaveKind::Existing, "existing"),
        (SaveKind::Github, "github"),
        (SaveKind::Local, "local"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(value));
    }
    for (link, value) in [
        (Link::GithubSignup, "githubSignup"),
        (Link::GithubDevice, "githubDevice"),
    ] {
        assert_eq!(
            serde_json::to_value(LinkRequest { link }).unwrap(),
            json!({"link":value})
        );
    }
    assert_eq!(
        serde_json::to_value(choice(1, None)).unwrap(),
        json!({"slug":"ari","choice":1,"kind":"local","name":null})
    );
    assert!(
        serde_json::from_value::<SaveChoiceRequest>(
            json!({"slug":"ari","choice":0,"kind":"github","name":null,"extra":1})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<SlugRequest>(json!({"slug":"ari","extra":1})).is_err());
    assert!(
        serde_json::from_value::<LinkRequest>(json!({"link":"githubDevice","extra":1})).is_err()
    );
    assert!(serde_json::from_value::<LinkRequest>(json!({"link":"https://example.com"})).is_err());
}

#[test]
fn cancel_during_token_reply_never_saves() {
    struct CancelOnToken<'a> {
        http: FakeHttp,
        sign_ins: &'a SignIns,
    }
    impl Http for CancelOnToken<'_> {
        fn send(&self, request: &HttpRequest<'_>) -> Result<HttpResponse> {
            let response = self.http.send(request)?;
            if request.url.ends_with("/oauth/access_token") {
                github_sign_in_cancel_in(self.sign_ins).unwrap();
            }
            Ok(response)
        }
    }
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    let http = CancelOnToken {
        http: FakeHttp::new(vec![code(), token(), account("repo")]),
        sign_ins: &sign_ins,
    };
    let github = github(&http);
    github_sign_in_start_in(&sign_ins, Some(&github)).unwrap();
    let answer = github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |_| {});
    assert!(tokens.load().unwrap().is_none());
    assert_eq!(answer.unwrap_err().message, text::SIGN_IN_CANCELLED);
    http.http.drained();
}

#[test]
fn sign_out_during_waiting_finish_cancels_and_keeps_store_empty() {
    let http = FakeHttp::new(vec![code(), token(), account("repo")]);
    let github = github(&http);
    let sign_ins = SignIns::new();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("previous-token".into())).unwrap();
    github_sign_in_start_in(&sign_ins, Some(&github)).unwrap();
    let mut signed_out = false;
    let answer = github_sign_in_finish_in(&sign_ins, Some(&github), &tokens, &mut |_| {
        if !signed_out {
            signed_out = true;
            assert_eq!(
                github_sign_out_in(&sign_ins, &tokens).unwrap().message,
                text::SIGNED_OUT
            );
            assert!(tokens.load().unwrap().is_none());
        }
    });
    assert!(signed_out);
    assert!(tokens.load().unwrap().is_none());
    assert_eq!(answer.unwrap_err().message, text::SIGN_IN_CANCELLED);
    assert_eq!(http.requests.borrow().len(), 1);
}

#[test]
fn mismatched_choice_kind_is_changed_without_writing_a_record() {
    let home = Home::new();
    let member = home.make();
    let tokens = MemoryStore::new();
    for kind in [SaveKind::Github, SaveKind::Existing] {
        let mut request = choice(1, None);
        request.kind = kind;
        let answer = save_choose_in(&home.0, request, &git(), None, &tokens);
        assert!(!member.state_dir.join("save.json").exists());
        assert_eq!(answer.unwrap_err().message, CHANGED);
    }
}
