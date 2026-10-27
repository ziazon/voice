mod common;

use common::{Scratch, Stub};
use persona_builder_agent::{
    agents::{AGENTS, agent},
    claude::{Ready, fallbacks, find_in},
    text,
};
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    time::Duration,
};

fn executable(path: &Path) {
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn path_of(folders: &[&Path]) -> OsString {
    std::env::join_paths(folders).unwrap()
}

#[test]
fn the_agent_list_has_claude_code_only() {
    assert_eq!(AGENTS.len(), 1);
    assert_eq!(AGENTS[0].id, "claude-code");
    assert_eq!(AGENTS[0].name, "Claude, through your own Claude Code");
    assert_eq!(agent("claude-code").unwrap().id, "claude-code");
    assert!(agent("other").is_none());
}

#[test]
fn finds_claude_in_path_order_then_the_usual_places() {
    let scratch = Scratch::new("find");
    let (a, b) = (scratch.0.join("a"), scratch.0.join("b"));
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    executable(&b.join("claude"));
    executable(&a.join("claude"));
    let found = find_in(&scratch.0, Some(&path_of(&[&a, &b])), &[]).unwrap();
    assert_eq!(found.program(), a.join("claude"));

    let fallback = scratch.0.join("fallback-claude");
    executable(&fallback);
    let found = find_in(
        &scratch.0,
        Some(&path_of(&[&scratch.0.join("missing")])),
        std::slice::from_ref(&fallback),
    )
    .unwrap();
    assert_eq!(found.program(), fallback);
    let found = find_in(
        &scratch.0,
        None,
        &[scratch.0.join("nope"), fallback.clone()],
    )
    .unwrap();
    assert_eq!(found.program(), fallback);
}

#[test]
fn skips_relative_and_empty_path_entries_and_odd_files() {
    let scratch = Scratch::new("find-skip");
    let cwd = std::env::current_dir().unwrap();
    let relative = PathBuf::from(format!(
        "{}",
        pathdiff(&scratch.0.join("rel"), &cwd).display()
    ));
    fs::create_dir_all(scratch.0.join("rel")).unwrap();
    executable(&scratch.0.join("rel/claude"));
    let raw = OsString::from(format!("{}::", relative.display()));
    let error = find_in(&scratch.0, Some(&raw), &[]).unwrap_err();
    assert_eq!(error.to_string(), text::NOT_INSTALLED);

    let plain = scratch.0.join("plain");
    fs::create_dir_all(&plain).unwrap();
    fs::write(plain.join("claude"), "not executable").unwrap();
    let folder = scratch.0.join("folder");
    fs::create_dir_all(folder.join("claude")).unwrap();
    let error = find_in(&scratch.0, Some(&path_of(&[&plain, &folder])), &[]).unwrap_err();
    assert_eq!(error.to_string(), text::NOT_INSTALLED);

    let linked = scratch.0.join("linked");
    fs::create_dir_all(&linked).unwrap();
    executable(&scratch.0.join("real-claude"));
    symlink(scratch.0.join("real-claude"), linked.join("claude")).unwrap();
    let found = find_in(&scratch.0, Some(&path_of(&[&linked])), &[]).unwrap();
    assert_eq!(found.program(), linked.join("claude"));
}

/// `target` relative to `base`, through `..` steps.
fn pathdiff(target: &Path, base: &Path) -> PathBuf {
    let ups = base.components().count() - 1;
    let mut out = PathBuf::new();
    for _ in 0..ups {
        out.push("..");
    }
    out.join(target.strip_prefix("/").unwrap())
}

#[test]
fn the_usual_places_are_the_installers() {
    let home = Path::new("/Users/someone");
    assert_eq!(
        fallbacks(home),
        vec![
            PathBuf::from("/Users/someone/.local/bin/claude"),
            PathBuf::from("/opt/homebrew/bin/claude"),
            PathBuf::from("/usr/local/bin/claude"),
        ]
    );
}

const READY_BODY: &str = r#"case "$1" in
--version) echo "2.1.292 (Claude Code)";;
auth) echo '{"loggedIn":true,"authMethod":"claude.ai","email":"someone@example.invalid"}';;
esac"#;

#[test]
fn check_reads_the_version_and_the_sign_in() {
    let stub = Stub::new(READY_BODY);
    let home = Scratch::new("check-home");
    let ready = stub.claude().check(&home.0).unwrap();
    assert_eq!(
        ready,
        Ready {
            version: "2.1.292".into(),
            api_key_set: std::env::var_os("ANTHROPIC_API_KEY").is_some_and(|v| !v.is_empty()),
        }
    );
    assert_eq!(stub.argv(0).unwrap(), ["--version"]);
    assert_eq!(stub.argv(1).unwrap(), ["auth", "status", "--json"]);
    assert_eq!(stub.file("cwd").trim(), home.0.to_str().unwrap());
    assert!(!format!("{ready:?}").contains("example.invalid"));
}

#[test]
fn check_says_when_claude_does_not_run() {
    let home = Scratch::new("check-broken");
    for body in [
        "exit 3",
        "echo 'Claude Code'",
        "echo '2.x (Claude Code)'",
        "case \"$1\" in --version) sleep 30;; esac",
    ] {
        let stub = Stub::new(body);
        let error = stub
            .claude()
            .with_timeout(Duration::from_secs(1))
            .check(&home.0)
            .unwrap_err();
        assert_eq!(error.to_string(), text::DOES_NOT_RUN, "{body}");
    }
}

#[test]
fn check_says_when_claude_is_signed_out() {
    let home = Scratch::new("check-signed-out");
    for status in [
        "echo '{\"loggedIn\":false}'",
        "echo '{\"authMethod\":\"none\"}'",
        "echo 'not json'",
        "echo '{\"loggedIn\":\"true\"}'",
        "exit 1",
    ] {
        let stub = Stub::new(&format!(
            "case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) {status};; esac"
        ));
        let error = stub.claude().check(&home.0).unwrap_err();
        assert_eq!(error.to_string(), text::SIGNED_OUT, "{status}");
    }
}
