pub mod common;

use common::{
    FakeHttp, Scratch,
    pb6::{Wrapper, git, init, member, seed},
    reply,
};
use persona_builder::error::Result;
use persona_builder_save::{
    flow::{self, MemberPaths, PrivacyCheck, RemoteCheck, Saver},
    git::Git,
    github::{Endpoints, Github},
    keychain::{MemoryStore, TokenStore},
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
fn tokens() -> MemoryStore {
    let store = MemoryStore::new();
    store.save(&Secret::new("invented-token".into())).unwrap();
    store
}
fn paths(home: &Path) -> MemberPaths {
    let dir = home.join("ai/crew/wren");
    let member = member(&dir);
    MemberPaths {
        member,
        dir: dir.canonicalize().unwrap(),
        state_dir: home.join(".claude/state/wren"),
    }
}
fn existing(paths: &MemberPaths, root: &Path) {
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.state_dir.join("save.json"), serde_json::to_vec(
        &json!({"version":1,"mode":"existing","repo":root,"remote":"origin","branch":"persona/wren"})
    ).unwrap()).unwrap();
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
fn assert_error<T>(result: Result<T>, expected: &str) {
    assert_eq!(
        result.err().expect("expected refusal").to_string(),
        expected
    );
}
fn assert_no_push(wrapper: &Wrapper) {
    assert!(!wrapper.calls().iter().any(|call| {
        call["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "push")
    }));
}

#[test]
fn choose_existing_without_github_refuses_before_git() {
    let scratch = Scratch::new("choose-existing-no-github");
    let member = paths(&scratch.0);
    let repo = repo_at(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let tokens = tokens();
    let remotes = Approve::new();
    assert_error(
        Saver {
            git: &saving,
            github: None,
            tokens: &tokens,
        }
        .choose_existing(&member, &scratch.0, &repo, &remotes),
        text::NOT_SET_UP,
    );
    assert!(wrapper.calls().is_empty());
}

#[test]
fn choose_existing_local_remote_is_not_offered_and_writes_nothing() {
    let scratch = Scratch::new("choose-existing-local-remote");
    let member = paths(&scratch.0);
    let repo = repo_at(&scratch.0);
    let remote = scratch.0.join("local.git");
    init(&remote, true);
    git(
        &repo,
        &["remote", "set-url", "origin", remote.to_str().unwrap()],
    );
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![]);
    let github = client(&http);
    let tokens = tokens();
    let remotes = Approve::new();
    assert_error(
        Saver {
            git: &saving,
            github: Some(&github),
            tokens: &tokens,
        }
        .choose_existing(&member, &scratch.0, &repo, &remotes),
        text::NOT_OFFERED,
    );
    assert!(
        flow::detect(&saving, &scratch.0, &member.dir)
            .unwrap()
            .is_empty()
    );
    assert!(http.requests.borrow().is_empty());
    assert!(!member.state_dir.join("save.json").exists());
}

#[test]
fn choose_existing_without_token_refuses_before_http() {
    let scratch = Scratch::new("choose-existing-no-token");
    let member = paths(&scratch.0);
    let repo = repo_at(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
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
        .choose_existing(&member, &scratch.0, &repo, &remotes),
        text::SIGN_IN_FIRST,
    );
    assert!(http.requests.borrow().is_empty());
    assert!(remotes.urls.borrow().is_empty());
}

fn bad_github_name(name: &str) {
    let scratch = Scratch::new("choose-github-bad-name");
    let member = paths(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![]);
    let github = client(&http);
    let tokens = tokens();
    assert_error(
        Saver {
            git: &saving,
            github: Some(&github),
            tokens: &tokens,
        }
        .choose_github(&member, name),
        text::BAD_REPO_NAME,
    );
    assert!(http.requests.borrow().is_empty());
    assert!(wrapper.calls().is_empty());
}
#[test]
fn choose_github_empty_name_refuses_before_http_or_git() {
    bad_github_name("");
}
#[test]
fn choose_github_dot_git_name_refuses_before_http_or_git() {
    bad_github_name("x.git");
}
#[test]
fn choose_github_slash_name_refuses_before_http_or_git() {
    bad_github_name("a/b");
}

#[test]
fn choose_github_without_token_refuses_before_http() {
    let scratch = Scratch::new("choose-github-no-token");
    let member = paths(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let http = FakeHttp::new(vec![]);
    let github = client(&http);
    let tokens = MemoryStore::new();
    assert_error(
        Saver {
            git: &saving,
            github: Some(&github),
            tokens: &tokens,
        }
        .choose_github(&member, "wren-assistant"),
        text::SIGN_IN_FIRST,
    );
    assert!(http.requests.borrow().is_empty());
}

fn save_existing_after_repo_changed(deleted: bool) {
    let scratch = Scratch::new("save-existing-changed");
    let member = paths(&scratch.0);
    // Keep the recorded repo separate so deleting it leaves the member intact.
    let repo = scratch.0.join("setup");
    init(&repo, false);
    seed(&repo);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/rowan-vale/wren-assistant.git",
        ],
    );
    existing(&member, &repo);
    if deleted {
        fs::remove_dir_all(&repo).unwrap();
    } else {
        git(&repo, &["remote", "remove", "origin"]);
    }
    let wrapper = Wrapper::new(&scratch.0, false);
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
        .save_now(&member, &scratch.0, &Clean, &remotes),
        text::NOT_OFFERED,
    );
    let requests = http.requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].url, "https://api.github.com/user");
    http.assert_drained();
    assert!(remotes.urls.borrow().is_empty());
    assert_no_push(&wrapper);
}
#[test]
fn save_existing_deleted_repo_refuses_after_account_without_push() {
    save_existing_after_repo_changed(true);
}
#[test]
fn save_existing_removed_remote_is_not_offered() {
    save_existing_after_repo_changed(false);
}

#[test]
fn member_paths_keeps_absolute_private_state_dir() {
    let scratch = Scratch::new("member-paths-absolute");
    let member = paths(&scratch.0);
    let state = scratch.0.join("private-state");
    fs::write(
        member.dir.join("private.json"),
        serde_json::to_vec(&json!({"state_dir":state})).unwrap(),
    )
    .unwrap();
    let loaded = MemberPaths::load(&member.dir.join("member.json"), &scratch.0).unwrap();
    assert_eq!(loaded.state_dir, state);
}
#[test]
fn member_paths_expands_template_state_dir_under_home() {
    let scratch = Scratch::new("member-paths-tilde");
    let member = paths(&scratch.0);
    let loaded = MemberPaths::load(&member.dir.join("member.json"), &scratch.0).unwrap();
    assert_eq!(loaded.state_dir, scratch.0.join(".claude/state/wren"));
}
