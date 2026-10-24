pub mod common;
use common::{
    FakeHttp, Scratch,
    pb6::{Wrapper, file_url, git, git_output, init, member, seed},
    reply,
};
use persona_builder::error::{Error, Result};
use persona_builder_save::{
    flow::{self, Choice, GithubRemotes, MemberPaths, PrivacyCheck, RemoteCheck, Saved, Saver},
    git::{FoundRepo, Git, Remote},
    github::{Endpoints, Github},
    keychain::{MemoryStore, TokenStore},
    record::{SaveRecord, read_record},
    secret::Secret,
    text,
};
use serde_json::json;
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
};

struct Approve {
    urls: RefCell<Vec<Vec<String>>>,
}
impl Approve {
    fn new() -> Self {
        Self {
            urls: RefCell::new(vec![]),
        }
    }
}
impl RemoteCheck for Approve {
    fn confirm(&self, _token: &Secret, urls: &[String]) -> Result<()> {
        self.urls.borrow_mut().push(urls.to_vec());
        Ok(())
    }
}
struct Clean;
impl PrivacyCheck for Clean {
    fn hits(&self, _folder: &Path) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
struct Hits;
impl PrivacyCheck for Hits {
    fn hits(&self, _folder: &Path) -> Result<Vec<String>> {
        Ok((1..=25)
            .map(|line| format!("decisions.md:{line}"))
            .collect())
    }
}
struct RefusePrivacy;
impl PrivacyCheck for RefusePrivacy {
    fn hits(&self, _folder: &Path) -> Result<Vec<String>> {
        Err(Error::Invalid("privacy fixture refused".into()))
    }
}
struct NeverPrivacy;
impl PrivacyCheck for NeverPrivacy {
    fn hits(&self, _folder: &Path) -> Result<Vec<String>> {
        panic!("privacy must not run for local/no record")
    }
}
fn client(http: &FakeHttp) -> Github<'_> {
    Github::new(http, Endpoints::github(), "Iv1.invented").unwrap()
}
fn account() -> persona_builder_save::github::HttpResponse {
    let mut response = reply(200, json!({"login":"rowan-vale","id":4242}));
    response
        .headers
        .push(("x-oauth-scopes".into(), "repo".into()));
    response
}
fn private() -> persona_builder_save::github::HttpResponse {
    reply(200, json!({"private":true,"visibility":"private"}))
}
fn tokens() -> MemoryStore {
    let store = MemoryStore::new();
    store.save(&Secret::new("invented-token".into())).unwrap();
    store
}
fn paths(home: &Path) -> MemberPaths {
    fs::create_dir_all(home).unwrap();
    let dir = home.join("ai/crew/wren");
    let member = member(&dir);
    MemberPaths {
        member,
        dir: dir.canonicalize().unwrap(),
        state_dir: home.join(".claude/state/wren"),
    }
}
fn record(paths: &MemberPaths, value: serde_json::Value) {
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(
        paths.state_dir.join("save.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}
fn existing(paths: &MemberPaths, root: &Path) {
    record(
        paths,
        json!({"version":1,"mode":"existing","repo":root,"remote":"origin","branch":"persona/wren"}),
    );
}
fn assert_error<T>(result: Result<T>, expected: &str) {
    assert_eq!(
        result.err().expect("expected refusal").to_string(),
        expected
    );
}
fn assert_pushed(saved: Saved, expected: &str) {
    match saved {
        Saved::Pushed { message } => assert_eq!(message, expected),
        Saved::KeptLocal => panic!("expected pushed"),
    }
}
fn repo_at(home: &Path) -> PathBuf {
    let root = home.join("ai");
    init(&root, false);
    seed(&root);
    git(
        &root,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/rowan-vale/wren-assistant.git",
        ],
    );
    root.canonicalize().unwrap()
}
const MEMBER_TREE: &str = "crew/wren/IDENTITY.md\ncrew/wren/SKILL.md\ncrew/wren/autonomy.md\ncrew/wren/briefs/backtest-brief.md\ncrew/wren/briefs/card-brief.md\ncrew/wren/briefs/codebook.md\ncrew/wren/briefs/distil-brief.md\ncrew/wren/briefs/label-brief.md\ncrew/wren/briefs/lens-brief.md\ncrew/wren/briefs/panel-seat.md\ncrew/wren/briefs/persona-backtest-brief.md\ncrew/wren/briefs/persona-model-brief.md\ncrew/wren/decisions.md\ncrew/wren/interview.md\ncrew/wren/measurements.md\ncrew/wren/member.json\ncrew/wren/report.md";

#[test]
fn member_paths_loads_template_member_and_expands_home() {
    let scratch = Scratch::new("member-paths");
    let expected = paths(&scratch.0);
    let loaded = MemberPaths::load(&expected.dir.join("member.json"), &scratch.0).unwrap();
    assert_eq!(loaded.member.name, "Wren");
    assert_eq!(loaded.member.principal, "Rowan Vale");
    assert_eq!(loaded.dir, expected.dir);
    assert_eq!(loaded.state_dir, expected.state_dir);
}

#[test]
fn options_without_repos_offer_github_then_local_and_note() {
    let scratch = Scratch::new("options-none");
    let options = flow::options(&[], &scratch.0);
    assert_eq!(options.selected, 0);
    assert_eq!(options.note, Some(text::NO_GITHUB_ACCOUNT));
    assert_eq!(options.choices.len(), 2);
    assert!(
        matches!(&options.choices[0], Choice::Github { label } if label == text::CHOICE_GITHUB)
    );
    assert!(matches!(&options.choices[1], Choice::Local { label } if label == text::CHOICE_LOCAL));
}

#[test]
fn detects_github_setup_drops_local_only_and_deduplicates_candidates() {
    let scratch = Scratch::new("detect");
    let home = scratch.0.canonicalize().unwrap();
    let wrapper = Wrapper::new(&home, false);
    let saving = Git::at(wrapper.program);
    let setup = home.join(".claude");
    let member_dir = home.join("member");
    fs::create_dir(&member_dir).unwrap();
    assert!(
        flow::detect(&saving, &home, &member_dir)
            .unwrap()
            .is_empty()
    );
    init(&setup, false);
    git(
        &setup,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:rowan-vale/setup.git",
        ],
    );
    let found = flow::detect(&saving, &home, &member_dir).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].root, setup);
    assert_eq!(found[0].remotes[0].name, "origin");
    let options = flow::options(&found, &home);
    assert_eq!(options.selected, 0);
    assert_eq!(options.note, None);
    assert_eq!(options.choices.len(), 2);
    assert!(
        matches!(&options.choices[0], Choice::Existing { root, remote, label } if root == &setup && remote == "origin" && label == &text::choice_existing("~/.claude"))
    );
    assert!(matches!(&options.choices[1], Choice::Local { label } if label == text::CHOICE_LOCAL));
    fs::create_dir(setup.join("nested")).unwrap();
    assert_eq!(
        flow::detect(&saving, &home, &setup.join("nested"))
            .unwrap()
            .len(),
        1
    );
    git(
        &setup,
        &["remote", "set-url", "origin", member_dir.to_str().unwrap()],
    );
    assert!(
        flow::detect(&saving, &home, &member_dir)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn options_prefer_origin_then_first_usable_remote_and_reject_mixed_push_urls() {
    let scratch = Scratch::new("options-remotes");
    let root = scratch.0.canonicalize().unwrap();
    let repos = [FoundRepo {
        root: root.clone(),
        git_dir: root.join(".git"),
        remotes: vec![
            Remote {
                name: "alpha".into(),
                push_urls: vec!["git@github.com:rowan-vale/a.git".into()],
            },
            Remote {
                name: "origin".into(),
                push_urls: vec!["git@github.com:rowan-vale/b.git".into()],
            },
        ],
    }];
    let options = flow::options(&repos, &root);
    assert!(
        matches!(&options.choices[0], Choice::Existing { remote, label, .. } if remote == "origin" && label == &text::choice_existing("~/."))
    );
    // Exercise remote eligibility through detect with mixed push URLs.
    let setup = root.join(".claude");
    init(&setup, false);
    git(
        &setup,
        &["remote", "add", "origin", "git@github.com:rowan-vale/b.git"],
    );
    git(
        &setup,
        &[
            "remote",
            "set-url",
            "--add",
            "--push",
            "origin",
            "git@github.com:rowan-vale/b.git",
        ],
    );
    git(
        &setup,
        &[
            "remote",
            "set-url",
            "--add",
            "--push",
            "origin",
            "file:///not-github",
        ],
    );
    git(
        &setup,
        &["remote", "add", "alpha", "git@github.com:rowan-vale/a.git"],
    );
    let wrapper = Wrapper::new(&root, false);
    let saving = Git::at(wrapper.program);
    let found = flow::detect(&saving, &root, &setup).unwrap();
    assert_eq!(found.len(), 1);
    assert!(
        matches!(&flow::options(&found, &root).choices[0], Choice::Existing { remote, .. } if remote == "alpha")
    );
}

#[test]
fn no_record_refuses_and_local_skips_privacy_git_and_http() {
    let scratch = Scratch::new("flow-local");
    let member = paths(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![]);
    let tokens = MemoryStore::new();
    let remotes = Approve::new();
    let saver = Saver {
        git: &saving,
        github: None,
        tokens: &tokens,
    };
    assert_error(
        saver.save_now(&member, &scratch.0, &NeverPrivacy, &remotes),
        text::CHOOSE_FIRST,
    );
    assert!(wrapper.calls().is_empty());
    saver.choose_local(&member).unwrap();
    assert_eq!(
        read_record(&member.state_dir).unwrap(),
        Some(SaveRecord::Local)
    );
    assert!(matches!(
        saver
            .save_now(&member, &scratch.0, &NeverPrivacy, &remotes)
            .unwrap(),
        Saved::KeptLocal
    ));
    assert!(wrapper.calls().is_empty());
    assert!(http.requests.borrow().is_empty());
    assert!(remotes.urls.borrow().is_empty());
}

#[test]
fn privacy_hits_are_limited_to_twenty_before_any_http_request() {
    let scratch = Scratch::new("flow-hits");
    let member = paths(&scratch.0);
    record(
        &member,
        json!({"version":1,"mode":"github","owner":"rowan-vale","name":"wren-assistant","branch":"main"}),
    );
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![]);
    let github = client(&http);
    let tokens = tokens();
    let remotes = Approve::new();
    let saver = Saver {
        git: &saving,
        github: Some(&github),
        tokens: &tokens,
    };
    assert_error(
        saver.save_now(&member, &scratch.0, &Hits, &remotes),
        &text::privacy_hits(
            "decisions.md:1, decisions.md:2, decisions.md:3, decisions.md:4, decisions.md:5, decisions.md:6, decisions.md:7, decisions.md:8, decisions.md:9, decisions.md:10, decisions.md:11, decisions.md:12, decisions.md:13, decisions.md:14, decisions.md:15, decisions.md:16, decisions.md:17, decisions.md:18, decisions.md:19, decisions.md:20 and 5 more",
        ),
    );
    assert!(http.requests.borrow().is_empty());
    assert!(wrapper.calls().is_empty());
    assert!(remotes.urls.borrow().is_empty());
}

#[test]
fn privacy_errors_and_not_built_precede_client_id_and_sign_in() {
    let scratch = Scratch::new("flow-privacy");
    let member = paths(&scratch.0);
    record(
        &member,
        json!({"version":1,"mode":"github","owner":"rowan-vale","name":"wren-assistant","branch":"main"}),
    );
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![]);
    let tokens = MemoryStore::new();
    let remotes = Approve::new();
    let saver = Saver {
        git: &saving,
        github: None,
        tokens: &tokens,
    };
    assert_error(
        saver.save_now(&member, &scratch.0, &RefusePrivacy, &remotes),
        "privacy fixture refused",
    );
    assert_error(
        saver.save_now(&member, &scratch.0, &Clean, &remotes),
        text::NOT_SET_UP,
    );
    assert!(http.requests.borrow().is_empty());
    assert!(wrapper.calls().is_empty());
}

#[test]
fn no_token_refuses_before_http_or_remote_checks() {
    let scratch = Scratch::new("flow-no-token");
    let member = paths(&scratch.0);
    existing(&member, &scratch.0.join("ai"));
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program);
    let http = FakeHttp::new(vec![]);
    let github = client(&http);
    let tokens = MemoryStore::new();
    let remotes = Approve::new();
    assert_error(
        Saver {
            git: &saving,
            github: Some(&github),
            tokens: &tokens,
        }
        .save_now(&member, &scratch.0, &Clean, &remotes),
        text::SIGN_IN_FIRST,
    );
    assert!(http.requests.borrow().is_empty());
    assert!(remotes.urls.borrow().is_empty());
}

#[test]
fn public_repo_is_refused_before_any_commit() {
    let scratch = Scratch::new("flow-public");
    let member = paths(&scratch.0);
    let repo = repo_at(&scratch.0);
    existing(&member, &repo);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program);
    let http = FakeHttp::new(vec![
        account(),
        reply(200, json!({"private":false,"visibility":"public"})),
    ]);
    let github = client(&http);
    let tokens = tokens();
    assert_error(
        Saver {
            git: &saving,
            github: Some(&github),
            tokens: &tokens,
        }
        .save_now(
            &member,
            &scratch.0,
            &Clean,
            &GithubRemotes { github: &github },
        ),
        &text::repo_public("rowan-vale/wren-assistant"),
    );
    assert!(
        !git_output(&repo, &["rev-parse", "--verify", "refs/heads/persona/wren"])
            .status
            .success()
    );
    assert_eq!(http.requests.borrow().len(), 2);
    http.assert_drained();
}

#[test]
fn github_remotes_refuses_file_urls_empty_lists_and_every_unconfirmed_url() {
    let http = FakeHttp::new(vec![]);
    let github = client(&http);
    let check = GithubRemotes { github: &github };
    let token = Secret::new("invented-token".into());
    assert_error(check.confirm(&token, &[]), text::NOT_ON_GITHUB);
    assert_error(
        check.confirm(&token, &["file:///invented/remote.git".into()]),
        text::NOT_ON_GITHUB,
    );
    assert!(http.requests.borrow().is_empty());
    let http = FakeHttp::new(vec![
        private(),
        reply(200, json!({"private":false,"visibility":"public"})),
    ]);
    let github = client(&http);
    assert_error(
        GithubRemotes { github: &github }.confirm(
            &token,
            &[
                "git@github.com:rowan-vale/a.git".into(),
                "git@github.com:rowan-vale/b.git".into(),
            ],
        ),
        &text::repo_public("rowan-vale/b"),
    );
    assert_eq!(http.requests.borrow().len(), 2);
    http.assert_drained();
}

#[test]
fn existing_save_pushes_only_member_files_preserves_worktree_and_reports_unchanged() {
    let scratch = Scratch::new("flow-existing");
    let home = scratch.0.canonicalize().unwrap();
    let member = paths(&home);
    let repo = repo_at(&home);
    let remote = home.join("remote.git");
    init(&remote, true);
    let wrapper = Wrapper::new(&home, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![account(), account(), account()]);
    let github = client(&http);
    let tokens = tokens();
    let remotes = Approve::new();
    let saver = Saver {
        git: &saving,
        github: Some(&github),
        tokens: &tokens,
    };
    let chosen = saver
        .choose_existing(&member, &home, &repo, &remotes)
        .unwrap();
    assert_eq!(
        chosen,
        SaveRecord::Existing {
            repo: repo.clone(),
            remote: "origin".into(),
            branch: "persona/wren".into()
        }
    );
    git(
        &repo,
        &[
            "config",
            &format!("url.{}.pushInsteadOf", file_url(&remote)),
            "https://github.com/rowan-vale/wren-assistant.git",
        ],
    );
    fs::write(repo.join("keep.md"), "dirty\n").unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let index = fs::read(repo.join(".git/index")).unwrap();
    let status = git_output(&repo, &["status", "--porcelain"]).stdout;
    assert_pushed(
        saver.save_now(&member, &home, &Clean, &remotes).unwrap(),
        &text::saved_existing("~/ai", "persona/wren"),
    );
    assert_eq!(
        git(&remote, &["ls-tree", "-r", "--name-only", "persona/wren"]),
        MEMBER_TREE
    );
    assert_eq!(
        git(
            &remote,
            &["show", "-s", "--format=%an <%ae>|%s", "persona/wren"]
        ),
        "rowan-vale <4242+rowan-vale@users.noreply.github.com>|Save Wren"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git_output(&repo, &["status", "--porcelain"]).stdout, status);
    assert_eq!(fs::read(repo.join("keep.md")).unwrap(), b"dirty\n");
    let commit = git(&remote, &["rev-parse", "persona/wren"]);
    assert_pushed(
        saver.save_now(&member, &home, &Clean, &remotes).unwrap(),
        &format!(
            "{} {}",
            text::saved_existing("~/ai", "persona/wren"),
            text::UP_TO_DATE
        ),
    );
    assert_eq!(git(&remote, &["rev-parse", "persona/wren"]), commit);
    assert_eq!(
        *remotes.urls.borrow(),
        vec![
            vec!["https://github.com/rowan-vale/wren-assistant.git".into()],
            vec![file_url(&remote)],
            vec![file_url(&remote)]
        ]
    );
    assert_eq!(
        wrapper
            .calls()
            .iter()
            .filter(|call| call["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|arg| arg == "push"))
            .count(),
        2,
        "an unchanged save must retry the push"
    );
    http.assert_drained();
}

#[test]
fn github_choice_creates_once_and_saves_to_main_then_refuses_changed_origin() {
    let scratch = Scratch::new("flow-github");
    let home = scratch.0.canonicalize().unwrap();
    let member = paths(&home);
    let remote = home.join("remote.git");
    init(&remote, true);
    let wrapper = Wrapper::new(&home, false);
    let saving = Git::at(wrapper.program);
    let http = FakeHttp::new(vec![
        account(),
        reply(
            201,
            json!({"owner":{"login":"rowan-vale"},"name":"wren-assistant"}),
        ),
        private(),
        account(),
        private(),
        account(),
        account(),
    ]);
    let github = client(&http);
    let tokens = tokens();
    let remotes = Approve::new();
    let saver = Saver {
        git: &saving,
        github: Some(&github),
        tokens: &tokens,
    };
    let chosen = saver.choose_github(&member, "wren-assistant").unwrap();
    assert_eq!(chosen.full(), "rowan-vale/wren-assistant");
    let backup = member.state_dir.join("backup.git");
    assert_eq!(git(&backup, &["rev-parse", "--is-bare-repository"]), "true");
    assert_eq!(
        git(&backup, &["remote", "get-url", "origin"]),
        "https://github.com/rowan-vale/wren-assistant.git"
    );
    assert_eq!(
        read_record(&member.state_dir).unwrap(),
        Some(SaveRecord::Github {
            owner: "rowan-vale".into(),
            name: "wren-assistant".into(),
            branch: "main".into()
        })
    );
    assert_eq!(
        saver.choose_github(&member, "another-name").unwrap(),
        chosen
    );
    {
        let calls = http.requests.borrow();
        assert_eq!(calls.iter().filter(|r| r.method == "POST").count(), 1);
        assert_eq!(calls[1].url, "https://api.github.com/user/repos");
        assert_eq!(calls[2].method, "GET");
        assert_eq!(
            calls[2].url,
            "https://api.github.com/repos/rowan-vale/wren-assistant"
        );
        assert_eq!(
            calls[1].json,
            Some(
                json!({"name":"wren-assistant","private":true,"description":"My AI assistant, saved by Persona Builder","auto_init":false,"has_issues":false,"has_projects":false,"has_wiki":false})
            )
        );
    }
    git(
        &backup,
        &[
            "config",
            &format!("url.{}.pushInsteadOf", file_url(&remote)),
            "https://github.com/rowan-vale/wren-assistant.git",
        ],
    );
    assert_pushed(
        saver.save_now(&member, &home, &Clean, &remotes).unwrap(),
        &text::saved_github("rowan-vale/wren-assistant"),
    );
    assert_eq!(
        git(&remote, &["ls-tree", "-r", "--name-only", "main"]),
        MEMBER_TREE
    );
    assert_eq!(*remotes.urls.borrow(), vec![vec![file_url(&remote)]]);
    git(
        &backup,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/rowan-vale/other.git",
        ],
    );
    let before = git(&remote, &["rev-parse", "main"]);
    assert_error(
        saver.save_now(&member, &home, &Clean, &remotes),
        &text::repo_unconfirmed("rowan-vale/wren-assistant"),
    );
    assert_eq!(git(&remote, &["rev-parse", "main"]), before);
    assert_eq!(remotes.urls.borrow().len(), 1);
    http.assert_drained();
}

#[test]
fn symlink_in_template_member_is_refused_before_commit_or_push() {
    let scratch = Scratch::new("flow-member-link");
    let home = scratch.0.canonicalize().unwrap();
    let member = paths(&home);
    let repo = repo_at(&home);
    existing(&member, &repo);
    std::os::unix::fs::symlink(home.join("outside"), member.dir.join("briefs/link.md")).unwrap();
    let remote = home.join("remote.git");
    init(&remote, true);
    git(
        &repo,
        &[
            "config",
            &format!("url.{}.pushInsteadOf", file_url(&remote)),
            "https://github.com/rowan-vale/wren-assistant.git",
        ],
    );
    let wrapper = Wrapper::new(&home, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![account()]);
    let github = client(&http);
    let tokens = tokens();
    let remotes = Approve::new();
    assert_error(
        Saver {
            git: &saving,
            github: Some(&github),
            tokens: &tokens,
        }
        .save_now(&member, &home, &Clean, &remotes),
        &text::links_in_folder("briefs/link.md"),
    );
    assert!(
        !git_output(&repo, &["rev-parse", "--verify", "refs/heads/persona/wren"])
            .status
            .success()
    );
    assert!(git_output(&remote, &["show-ref"]).stdout.is_empty());
    assert_eq!(*remotes.urls.borrow(), vec![vec![file_url(&remote)]]);
    assert!(!wrapper.calls().iter().any(|call| {
        call["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "push")
    }));
    http.assert_drained();
}
