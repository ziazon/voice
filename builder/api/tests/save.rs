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
use voice_api::{dto::*, *};
use voice_core::{assistants, error::Result};
use voice_save::{
    flow::MemberPaths,
    git::Git,
    github::{Endpoints, Github, Http, HttpRequest, HttpResponse},
    keychain::{MemoryStore, TokenStore},
    record::{SaveRecord, read_record, write_record},
    secret::Secret,
    text,
};

const FIND: &str = "Voice couldn't find that assistant. Close it and open it again.";
const CHANGED: &str =
    "The places to keep your assistant have changed. Look at them again, then choose.";
const SAVE: &str =
    "Voice couldn't save your assistant. Try again, or keep it on this computer only.";
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
            "voice",
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
            role: voice_save::record::Role::Builder,
            builder: None,
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
            format!("Voice couldn't open your web browser. Go to {page} yourself.")
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
        member_computer: None,
    };
    assert_eq!(
        serde_json::to_value(options).unwrap(),
        json!({"choices":[{"kind":"github","label":"label"}],"selected":0,"note":null,"chosen":null,"signedIn":false,"suggestedName":"ari-assistant","memberComputer":null})
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

#[test]
fn saved_list_without_token_is_signed_out_and_does_no_http() {
    let home = Home::new();
    let http = FakeHttp::new(vec![]);
    let github = github(&http);
    let answer = saved_on_github_in(&home.0, Some(&github), &MemoryStore::new()).unwrap();
    assert!(!answer.signed_in);
    assert!(answer.items.is_empty());
    assert!(http.requests.borrow().is_empty());
}

#[test]
fn member_computer_options_carry_the_save_refusal() {
    let home = Home::new();
    let member = home.make();
    write_record(
        &member.state_dir,
        &SaveRecord::Github {
            owner: "example-owner".into(),
            name: "ari-assistant".into(),
            branch: "main".into(),
            role: voice_save::record::Role::Member,
            builder: Some("Studio".into()),
        },
    )
    .unwrap();
    let options = save_options_in(&home.0, slug("ari"), &git(), &MemoryStore::new()).unwrap();
    assert_eq!(
        options.member_computer,
        Some(text::member_computer("Ari", Some("Studio")))
    );
}

fn open_fixture(home: &Home) -> Git {
    use std::os::unix::fs::PermissionsExt;
    let source = home.0.join("source");
    let made = assistants::make(
        &source,
        &assistants::New {
            name: "Ari",
            pronouns: "they/them",
            principal: "Rowan Vale",
            principal_pronouns: "they/them",
        },
        "2026-11-13",
        "voice",
    )
    .unwrap();
    let repo = source.join(".claude");
    run_git(&repo, &["init", "-q", "--initial-branch=main"]);
    run_git(&repo, &["add", "crew"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.name=Voice",
            "-c",
            "user.email=voice@localhost",
            "commit",
            "-qm",
            "Save Ari\n\nSaved-on: Studio\n",
        ],
    );
    assert!(made.dir.join("member.json").exists());
    let wrapper = home.0.join("git-wrapper");
    let real = serde_json::to_string(real_git().to_str().unwrap()).unwrap();
    let rewrite = serde_json::to_string(&format!(
        "url.file://{}.insteadOf=https://github.com/example-owner/ari-assistant.git",
        repo.display()
    ))
    .unwrap();
    fs::write(&wrapper, format!("#!/usr/bin/python3\nimport os, sys\nrewrite = ['-c', {rewrite}] if any(verb in sys.argv[1:] for verb in ('push', 'fetch')) else []\nos.execv({real}, [{real}] + rewrite + sys.argv[1:])\n")).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    Git::at(wrapper)
}
fn open_request() -> OpenSavedRequest {
    OpenSavedRequest {
        owner: "example-owner".into(),
        repo: "ari-assistant".into(),
        slug: "ari".into(),
    }
}
fn open_http() -> FakeHttp {
    FakeHttp::new(vec![
        account("repo"),
        reply(
            json!({"private":true,"visibility":"private","owner":{"login":"example-owner"},"name":"ari-assistant"}),
        ),
    ])
}
fn assert_no_staging(home: &Path) {
    let mut entries = match fs::read_dir(home.join(".claude/crew")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("{error}"),
    };
    assert!(entries.all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("voice-opening")
    }));
}

#[test]
fn open_saved_removes_new_crew_after_a_state_refusal() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let claude = home.0.join(".claude");
    fs::create_dir_all(claude.join("state")).unwrap();
    fs::write(claude.join("state/ari"), "older state file\n").unwrap();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let http = open_http();
    let github = github(&http);
    assert!(
        open_saved_in(
            &home.0,
            &claude,
            open_request(),
            &git,
            Some(&github),
            &tokens,
        )
        .is_err()
    );
    assert_no_staging(&home.0);
    assert!(!claude.join("crew").exists());
}

#[test]
fn open_saved_backs_up_older_assistant_and_clears_permission() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let claude = home.0.join(".claude");
    fs::create_dir_all(claude.join("skills/ari")).unwrap();
    fs::write(claude.join("skills/ari/SKILL.md"), "An older assistant.\n").unwrap();
    fs::write(claude.join("CLAUDE.md"), "Ask /ari before a choice.\n").unwrap();
    fs::create_dir_all(claude.join("state/ari")).unwrap();
    fs::write(claude.join("state/ari/decisions.jsonl"), "older history\n").unwrap();
    write_consent(&home.0, "ari").unwrap();
    assert!(read_consent(&home.0, "ari").agreed);
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let http = open_http();
    let github = github(&http);
    let opened = open_saved_in(
        &home.0,
        &claude,
        open_request(),
        &git,
        Some(&github),
        &tokens,
    )
    .unwrap();
    let backup = opened.backup.unwrap();
    let backup_dir = home.0.join(backup.strip_prefix("~/").unwrap());
    assert_eq!(
        fs::read(backup_dir.join("state/decisions.jsonl")).unwrap(),
        b"older history\n"
    );
    assert_eq!(
        fs::read(backup_dir.join("skill/SKILL.md")).unwrap(),
        b"An older assistant.\n"
    );
    assert!(backup_dir.join("CLAUDE.md").is_file());
    assert_eq!(
        fs::read(claude.join("state/ari/decisions-before-voice.jsonl")).unwrap(),
        b"older history\n"
    );
    assert_eq!(
        opened.message,
        format!(
            "Opened Ari from example-owner/ari-assistant. Voice asks for your permission again on this computer before Ari reads anything. Voice backed up the older Ari first, in {backup}/."
        )
    );
    assert!(!opened.assistant.consent.agreed);
    assert!(!read_consent(&home.0, "ari").agreed);
    assert_no_staging(&home.0);
    http.drained();
}

#[test]
fn open_saved_refuses_existing_voice_assistant_and_removes_staging() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let member = home.make();
    let original = fs::read(member.dir.join("member.json")).unwrap();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let http = open_http();
    let github = github(&http);
    let error = open_saved_in(
        &home.0,
        &home.0.join(".claude"),
        open_request(),
        &git,
        Some(&github),
        &tokens,
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "You already have an assistant called Ari on this computer, so Voice opened nothing. Forget that one first, then open the saved one again."
    );
    assert_eq!(fs::read(member.dir.join("member.json")).unwrap(), original);
    assert_no_staging(&home.0);
}

#[test]
fn open_saved_masks_internal_errors_and_rejects_extra_request_keys() {
    let home = Home::new();
    let error = open_saved_in(
        &home.0,
        &home.0.join(".claude"),
        OpenSavedRequest {
            slug: "../ari".into(),
            ..open_request()
        },
        &git(),
        None,
        &MemoryStore::new(),
    )
    .unwrap_err();
    assert_eq!(error.message, voice_api::save::OPEN_SAVED_FAILED);
    assert!(
        serde_json::from_value::<OpenSavedRequest>(
            json!({"owner":"example-owner","repo":"ari-assistant","slug":"ari","extra":true})
        )
        .is_err()
    );
}

#[test]
fn permission_clear_failure_undoes_the_new_saved_copy() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let claude = home.0.join(".claude");
    let state = claude.join("state/ari");
    fs::create_dir_all(state.join("consent.json")).unwrap();
    fs::write(state.join("decisions.jsonl"), "older history\n").unwrap();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let http = open_http();
    let github = github(&http);
    assert!(
        open_saved_in(
            &home.0,
            &claude,
            open_request(),
            &git,
            Some(&github),
            &tokens
        )
        .is_err()
    );
    assert!(!claude.join("crew/ari").exists());
    assert!(!state.join("save.json").exists());
    assert!(!state.join("backup.git").exists());
    assert_eq!(
        fs::read(state.join("decisions.jsonl")).unwrap(),
        b"older history\n"
    );
    assert!(state.join("consent.json").is_dir());
    assert_no_staging(&home.0);
}

#[test]
fn saved_list_marks_the_assistant_already_on_this_computer() {
    let home = Home::new();
    home.make();
    let http = FakeHttp::new(vec![
        reply(
            json!([{"private":true,"description":"My AI assistant, saved by Voice","owner":{"login":"example-owner"},"name":"ari-assistant"}]),
        ),
        reply(json!([{"type":"dir","name":"ari"}])),
        reply(
            json!({"type":"file","encoding":"base64","content":"eyJzbHVnIjoiYXJpIiwibmFtZSI6IkFyaSJ9"}),
        ),
    ]);
    let github = github(&http);
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let answer = saved_on_github_in(&home.0, Some(&github), &tokens).unwrap();
    assert!(answer.signed_in);
    assert_eq!(
        answer.items,
        vec![SavedAssistant {
            owner: "example-owner".into(),
            repo: "ari-assistant".into(),
            slug: "ari".into(),
            name: "Ari".into(),
            here: true
        }]
    );
    http.drained();
}

#[test]
fn open_saved_explains_an_older_state_refusal_and_cleans_staging() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let claude = home.0.join(".claude");
    fs::create_dir_all(claude.join("state")).unwrap();
    fs::write(claude.join("state/ari"), "older state file\n").unwrap();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let http = open_http();
    let github = github(&http);
    let error = open_saved_in(
        &home.0,
        &claude,
        open_request(),
        &git,
        Some(&github),
        &tokens,
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "~/.claude/state/ari is a file or a link, not a folder, so Voice opened nothing. Move it somewhere else, then try again."
    );
    assert_eq!(
        fs::read(claude.join("state/ari")).unwrap(),
        b"older state file\n"
    );
    assert_no_staging(&home.0);
}

#[test]
fn open_saved_reports_a_member_that_fails_the_final_check() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    // Change the staged config at the log call, after the privacy scan, so the
    // final API check is the one that refuses it.
    let wrapper = fixture.0.join("git-wrapper");
    let source = fs::read_to_string(&wrapper).unwrap();
    let intercept = "import json\nif 'log' in sys.argv[1:]:\n    git_dir = next(arg.split('=', 1)[1] for arg in sys.argv[1:] if arg.startswith('--git-dir='))\n    path = os.path.join(os.path.dirname(git_dir), 'member', 'member.json')\n    with open(path) as file: value = json.load(file)\n    value['filters']['personal'] = '['\n    with open(path, 'w') as file: json.dump(value, file)\n";
    fs::write(
        &wrapper,
        source.replace("os.execv(", &(intercept.to_owned() + "os.execv(")),
    )
    .unwrap();
    let home = Home::new();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let http = open_http();
    let github = github(&http);
    let error = open_saved_in(
        &home.0,
        &home.0.join(".claude"),
        open_request(),
        &git,
        Some(&github),
        &tokens,
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "The saved copy in example-owner/ari-assistant has a member.json Voice can't use, so it opened nothing."
    );
    assert_no_staging(&home.0);
}

#[test]
fn take_newest_non_member_keeps_the_explanation() {
    let home = Home::new();
    home.make();
    let error = take_newest_in(
        &home.0,
        &home.0.join(".claude"),
        slug("ari"),
        &git(),
        None,
        &MemoryStore::new(),
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "This computer saves Ari, so there's no newer version to take here."
    );
}

fn newer_fixture(fixture: &Home, home: &Home, git: &Git, tokens: &MemoryStore) -> PathBuf {
    let http = open_http();
    let github = github(&http);
    open_saved_in(
        &home.0,
        &home.0.join(".claude"),
        open_request(),
        git,
        Some(&github),
        tokens,
    )
    .unwrap();
    http.drained();
    let repo = fixture.0.join("source/.claude");
    let skill = repo.join("crew/ari/SKILL.md");
    let mut bytes = fs::read_to_string(&skill).unwrap();
    bytes.push_str("\nUse the newest saved guidance.\n");
    fs::write(&skill, bytes).unwrap();
    run_git(&repo, &["add", "crew"]);
    run_git(
        &repo,
        &[
            "-c",
            "user.name=Voice",
            "-c",
            "user.email=voice@localhost",
            "commit",
            "-qm",
            "Save Ari",
        ],
    );
    skill
}

#[test]
fn take_newest_refreshes_the_assistant_turned_on_in_claude_code() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    let skill = newer_fixture(&fixture, &home, &git, &tokens);
    let places = voice_core::install::Places {
        home: home.0.clone(),
        claude_home: home.0.join(".claude"),
    };
    let on = voice_core::install::turn_on(&home.0.join(".claude/crew/ari"), &places).unwrap();
    let before = fs::read(on.skill.join("SKILL.md")).unwrap();
    let http = open_http();
    let github = github(&http);
    let result = take_newest_in(
        &home.0,
        &places.claude_home,
        slug("ari"),
        &git,
        Some(&github),
        &tokens,
    );
    assert!(result.is_ok(), "{result:?}");
    let answer = result.unwrap();
    assert_eq!(answer.changed, 1);
    assert_eq!(
        answer.message,
        "Took the newest version of Ari from example-owner/ari-assistant: 1 file changed. Voice refreshed Ari in Claude Code too."
    );
    assert_ne!(fs::read(on.skill.join("SKILL.md")).unwrap(), before);
    assert!(
        fs::read_to_string(on.skill.join("SKILL.md"))
            .unwrap()
            .contains("Use the newest saved guidance.")
    );
    assert_eq!(
        fs::read(home.0.join(".claude/crew/ari/SKILL.md")).unwrap(),
        fs::read(skill).unwrap()
    );
    http.drained();
}

#[test]
fn take_newest_when_turned_off_does_not_refresh() {
    let fixture = Home::new();
    let git = open_fixture(&fixture);
    let home = Home::new();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("test-token".into())).unwrap();
    newer_fixture(&fixture, &home, &git, &tokens);
    let http = open_http();
    let github = github(&http);
    let result = take_newest_in(
        &home.0,
        &home.0.join(".claude"),
        slug("ari"),
        &git,
        Some(&github),
        &tokens,
    );
    assert!(result.is_ok(), "{result:?}");
    let answer = result.unwrap();
    assert_eq!(answer.changed, 1);
    assert_eq!(
        answer.message,
        "Took the newest version of Ari from example-owner/ari-assistant: 1 file changed."
    );
    assert!(!home.0.join(".claude/skills/ari").exists());
    http.drained();
}

#[test]
fn take_newest_unknown_assistant_does_not_create_state() {
    let home = Home::new();
    let error = take_newest_in(
        &home.0,
        &home.0.join(".claude"),
        slug("wrne"),
        &git(),
        None,
        &MemoryStore::new(),
    )
    .unwrap_err();
    assert_eq!(error.message, FIND);
    assert!(!home.0.join(".claude/state/wrne").exists());
}

#[test]
fn take_newest_without_state_is_not_a_member_and_creates_nothing() {
    let home = Home::new();
    let member = home.make();
    if member.state_dir.exists() {
        fs::remove_dir_all(&member.state_dir).unwrap();
    }
    let error = take_newest_in(
        &home.0,
        &home.0.join(".claude"),
        slug("ari"),
        &git(),
        None,
        &MemoryStore::new(),
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "This computer saves Ari, so there's no newer version to take here."
    );
    assert!(!member.state_dir.exists());
}

#[test]
fn take_newest_while_locked_says_to_wait() {
    let home = Home::new();
    home.make();
    let _lock = voice_ingest::private::lock_state(&home.0, "ari").unwrap();
    let error = take_newest_in(
        &home.0,
        &home.0.join(".claude"),
        slug("ari"),
        &git(),
        None,
        &MemoryStore::new(),
    )
    .unwrap_err();
    assert_eq!(
        error.message,
        "Voice is already working on Ari. Wait for it to finish, then try again."
    );
}
