pub mod common;

use common::{FakeHttp, reply};
use persona_builder_save::{
    github::{DeviceCode, Endpoints, Github},
    keychain::store_line,
    secret::Secret,
};
use serde_json::json;

// Invented token: a GitHub OAuth prefix followed by 36 base62 characters.
const TOKEN: &str = "gho_0123456789abcdefghijklmnopqrstuvwxyz";

fn device(github: &Github<'_>) -> DeviceCode {
    github.start_sign_in().unwrap()
}

fn start_reply() -> persona_builder_save::github::HttpResponse {
    reply(
        200,
        json!({
            "device_code": "invented-device",
            "user_code": "ABCD-1234",
            "verification_uri": "https://github.com/login/device"
        }),
    )
}

#[test]
fn finish_sign_in_returns_github_oauth_token() {
    let http = FakeHttp::new(vec![
        start_reply(),
        reply(200, json!({"access_token": TOKEN})),
    ]);
    let github = Github::new(&http, Endpoints::github(), "Iv1.invented").unwrap();
    let result = github.finish_sign_in(&device(&github), &mut |_| {});
    assert_eq!(
        result.as_ref().map(|token| token.expose()).ok(),
        Some(TOKEN)
    );
    http.assert_drained();
}

#[test]
fn store_line_builds_exact_github_oauth_line() {
    let line = store_line("persona-builder.test", &Secret::new(TOKEN.into()));
    assert_eq!(
        line.as_ref().map(|line| line.expose()).ok(),
        Some(
            "add-generic-password -U -s persona-builder.test -a persona-builder -w \"gho_0123456789abcdefghijklmnopqrstuvwxyz\"\n"
        )
    );
}

use common::{
    Scratch,
    pb6::{git, init, real_git, seed},
};
use persona_builder::error::Error;
use persona_builder_save::git::{Env, Git, Identity, commit_folder};
use std::{fs, path::Path};

fn checkout_state(repo: &Path) -> (String, Vec<u8>, String, Vec<u8>) {
    let index = git(repo, &["rev-parse", "--git-path", "index"]);
    (
        git(repo, &["rev-parse", "HEAD"]),
        fs::read(repo.join(index)).unwrap(),
        git(repo, &["status", "--porcelain=v1"]),
        fs::read(repo.join("keep.md")).unwrap(),
    )
}

fn refuses_checked_out_branch(linked: bool) {
    let scratch = Scratch::new("verifier-branch");
    let repo = scratch.0.join("repo");
    init(&repo, false);
    seed(&repo);
    let checkout = if linked {
        let checkout = scratch.0.join("linked");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-qb",
                "persona/wren",
                checkout.to_str().unwrap(),
            ],
        );
        checkout
    } else {
        git(&repo, &["checkout", "-qb", "persona/wren"]);
        repo.clone()
    };
    fs::write(checkout.join("keep.md"), "staged\n").unwrap();
    git(&checkout, &["add", "keep.md"]);
    fs::write(checkout.join("keep.md"), "unstaged\n").unwrap();
    fs::write(checkout.join("untracked.md"), "untracked\n").unwrap();
    let folder = scratch.0.join("member");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("assistant.md"), "assistant\n").unwrap();
    let before_repo = checkout_state(&repo);
    let before_checkout = checkout_state(&checkout);
    let before_ref = git(&repo, &["rev-parse", "refs/heads/persona/wren"]);
    let result = commit_folder(
        &Git::at(real_git().into()),
        Env::User,
        &repo.join(".git"),
        "persona/wren",
        &folder,
        "crew/wren",
        &Identity {
            name: "Rowan Vale".into(),
            email: "rowan@example.invalid".into(),
        },
        "Save Wren",
    );
    assert!(
        matches!(&result, Err(Error::Invalid(message)) if message == "The branch persona/wren is open in your repo right now, so Persona Builder didn't save. Switch to another branch, then try again."),
        "expected branch-in-use error"
    );
    assert_eq!(checkout_state(&repo), before_repo);
    assert_eq!(checkout_state(&checkout), before_checkout);
    assert_eq!(
        git(&repo, &["rev-parse", "refs/heads/persona/wren"]),
        before_ref
    );
    assert_eq!(
        fs::read(checkout.join("untracked.md")).unwrap(),
        b"untracked\n"
    );
}

#[test]
fn refuses_branch_checked_out_in_main_worktree() {
    refuses_checked_out_branch(false);
}

#[test]
fn refuses_branch_checked_out_only_in_linked_worktree() {
    refuses_checked_out_branch(true);
}

use persona_builder_save::text;

#[test]
fn poll_400_pending_then_token() {
    let http = FakeHttp::new(vec![
        start_reply(),
        reply(400, json!({"error": "authorization_pending"})),
        reply(200, json!({"access_token": TOKEN})),
    ]);
    let github = Github::new(&http, Endpoints::github(), "Iv1.invented").unwrap();
    let mut sleeps = Vec::new();
    let result = github.finish_sign_in(&device(&github), &mut |delay| sleeps.push(delay.as_secs()));
    assert_eq!(
        result.as_ref().map(|token| token.expose()).ok(),
        Some(TOKEN)
    );
    assert_eq!(sleeps, [5, 5]);
    http.assert_drained();
}

#[test]
fn poll_400_denied_is_cancelled() {
    let http = FakeHttp::new(vec![
        start_reply(),
        reply(400, json!({"error": "access_denied"})),
    ]);
    let github = Github::new(&http, Endpoints::github(), "Iv1.invented").unwrap();
    let result = github.finish_sign_in(&device(&github), &mut |_| {});
    assert_eq!(result.unwrap_err().to_string(), text::SIGN_IN_CANCELLED);
    http.assert_drained();
}

#[test]
fn poll_400_with_token_is_unexpected() {
    let http = FakeHttp::new(vec![
        start_reply(),
        reply(
            400,
            json!({"access_token": TOKEN, "token_type": "bearer", "scope": "repo"}),
        ),
    ]);
    let github = Github::new(&http, Endpoints::github(), "Iv1.invented").unwrap();
    let result = github.finish_sign_in(&device(&github), &mut |_| {});
    assert_eq!(
        result.err().map(|error| error.to_string()),
        Some(text::sign_in_failed("unexpected reply"))
    );
    http.assert_drained();
}

#[test]
fn poll_401_is_unexpected_even_with_error_body() {
    let http = FakeHttp::new(vec![
        start_reply(),
        reply(401, json!({"error": "access_denied"})),
    ]);
    let github = Github::new(&http, Endpoints::github(), "Iv1.invented").unwrap();
    let result = github.finish_sign_in(&device(&github), &mut |_| {});
    assert_eq!(
        result.unwrap_err().to_string(),
        text::sign_in_failed("unexpected reply")
    );
    http.assert_drained();
}

use persona_builder_save::flow::MemberPaths;

#[test]
fn relative_state_dir_is_refused_without_creating_anything() {
    let scratch = Scratch::new("verifier-state-dir");
    let folder = scratch.0.join("member");
    common::pb6::member(&folder);
    fs::write(
        folder.join("private.json"),
        r#"{"state_dir":"relative/state"}"#,
    )
    .unwrap();
    let home = scratch.0.join("home");
    let before = fs::read_dir(&scratch.0).unwrap().count();
    let result = MemberPaths::load(&folder.join("member.json"), &home);
    assert!(
        matches!(result, Err(Error::Invalid(message)) if message == "member config: state_dir must start with ~/ or /")
    );
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), before);
    assert!(!home.exists());
    assert!(!scratch.0.join("relative").exists());
    assert!(!folder.join("relative").exists());
    assert!(!Path::new("relative/state").exists());
}
