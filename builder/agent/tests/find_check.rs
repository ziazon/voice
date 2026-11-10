mod common;

use common::{Scratch, Stub};
use std::{
    ffi::OsString,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    time::Duration,
};
use voice_agent::{
    agents::{AGENTS, agent},
    claude::{Ready, fallbacks, find_in},
    text,
};

fn executable(path: &Path) {
    fs::write(path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn path_of(folders: &[&Path]) -> OsString {
    std::env::join_paths(folders).unwrap()
}

#[test]
fn sign_in_starts_only_where_the_platform_allows_it() {
    use voice_agent::claude::{SignIn, starts};

    assert!(starts(SignIn::Subscription));
    #[cfg(target_os = "macos")]
    assert!(!starts(SignIn::Console));
    #[cfg(not(target_os = "macos"))]
    assert!(starts(SignIn::Console));
}

#[test]
fn the_agent_list_has_claude_code_then_the_model_server() {
    assert_eq!(AGENTS.len(), 2);
    assert_eq!(AGENTS[1].id, "self-hosted");
    assert_eq!(AGENTS[1].name, "A model server on your own machines");
    assert_eq!(AGENTS[1].reach, voice_agent::provider::Reach::OwnMachines);
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

const READY_BODY: &str = r#"case "$3" in
--version) echo "2.1.292 (Claude Code)";;
auth) echo '{"loggedIn":true,"authMethod":"api_key","email":"someone@example.com"}';;
esac"#;

#[test]
fn check_reads_the_version_and_the_sign_in() {
    let stub = Stub::new(READY_BODY);
    let home = Scratch::new("check-home");
    let ready = stub.claude().check(&home.0).unwrap();
    assert_eq!(
        ready,
        Ready {
            subscription: false,
            version: "2.1.292".into(),
        }
    );
    assert_eq!(
        stub.argv(0).unwrap(),
        ["--setting-sources", "project,local", "--version"]
    );
    assert_eq!(
        stub.argv(1).unwrap(),
        [
            "--setting-sources",
            "project,local",
            "auth",
            "status",
            "--json"
        ]
    );
    let cwd = PathBuf::from(stub.file("cwd").trim());
    assert_eq!(
        cwd.parent().unwrap(),
        voice_agent::sandbox::runs_folder(&stub.profile())
            .canonicalize()
            .unwrap()
    );
    assert!(!cwd.exists());
    assert!(!format!("{ready:?}").contains("example.com"));
}

#[test]
fn check_says_when_claude_does_not_run() {
    let home = Scratch::new("check-broken");
    for body in [
        "exit 3",
        "echo 'Claude Code'",
        "echo '2.x (Claude Code)'",
        "case \"$3\" in --version) sleep 30;; esac",
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
            "case \"$3\" in --version) echo '2.1.292 (Claude Code)';; auth) {status};; esac"
        ));
        let error = stub.claude().check(&home.0).unwrap_err();
        assert_eq!(error.to_string(), text::SIGNED_OUT, "{status}");
    }
}

#[test]
fn subscription_profile_is_ready_only_in_testers_builds() {
    let home = Scratch::new("check-subscription");
    for method in ["claude.ai", "oauth_token"] {
        let stub = Stub::new(&format!(
            r#"case "$3" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{{"loggedIn":true,"authMethod":"{method}"}}';;
esac"#
        ));
        let result = stub.claude().check(&home.0);
        if voice_agent::claude::SUBSCRIPTION_OFFERED {
            assert_eq!(
                result.unwrap(),
                Ready {
                    subscription: true,
                    version: "2.1.292".into()
                }
            );
        } else {
            assert_eq!(result.unwrap_err().to_string(), text::SUBSCRIPTION_REFUSED);
        }
    }
}

#[test]
fn console_sign_in_uses_only_console_arguments_or_is_refused_on_mac() {
    sign_in_with_kind(voice_agent::claude::SignIn::Console);
}

#[test]
fn subscription_sign_in_obeys_the_build_feature_before_launching() {
    sign_in_with_kind(voice_agent::claude::SignIn::Subscription);
}

fn sign_in_with_kind(kind: voice_agent::claude::SignIn) {
    use std::sync::atomic::AtomicBool;
    use voice_agent::claude::{SUBSCRIPTION_OFFERED, SignIn};
    let home = Scratch::new("sign-in-kind");
    let stub = Stub::new("exit 0");
    let result = stub
        .claude()
        .sign_in_with(&home.0, &AtomicBool::new(false), kind, &|_| {
            panic!("stub never opens a page")
        });
    if cfg!(target_os = "macos") && kind == SignIn::Console {
        assert_eq!(result.unwrap_err().to_string(), text::CONSOLE_NOT_AVAILABLE);
        assert!(!stub.ran(), "refused Console sign-in started the child");
        assert!(!voice_agent::sandbox::runs_folder(&stub.profile()).exists());
    } else if kind == SignIn::Subscription && !SUBSCRIPTION_OFFERED {
        assert_eq!(
            result.unwrap_err().to_string(),
            text::SUBSCRIPTION_NOT_OFFERED
        );
        assert!(!stub.ran(), "refused subscription started the child");
        assert!(
            !voice_agent::sandbox::runs_folder(&stub.profile()).exists(),
            "refused subscription prepared a launch"
        );
    } else {
        result.unwrap();
        let flag = match kind {
            SignIn::Console => "--console",
            #[cfg(feature = "claude-subscription")]
            SignIn::Subscription => "--claudeai",
            #[cfg(not(feature = "claude-subscription"))]
            SignIn::Subscription => panic!("subscription must have been refused"),
        };
        assert_eq!(
            stub.argv(0).unwrap(),
            ["--setting-sources", "project,local", "auth", "login", flag]
        );
        assert!(stub.argv(1).is_none());
    }
}

#[test]
fn status_subscription_tracks_auth_method_and_feature() {
    let home = Scratch::new("status-subscription");
    for method in ["api_key", "claude.ai", "oauth_token"] {
        for logged_in in [true, false] {
            let stub = Stub::new(&format!(
                r#"case "$3" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{{"loggedIn":{logged_in},"authMethod":"{method}"}}';;
esac"#
            ));
            let status = stub.claude().status(&home.0).unwrap();
            let expected =
                logged_in && method != "api_key" && voice_agent::claude::SUBSCRIPTION_OFFERED;
            assert_eq!(
                status.subscription, expected,
                "{method}, loggedIn={logged_in}"
            );
            if status.signed_in {
                assert_eq!(stub.claude().check(&home.0).unwrap().subscription, expected);
            }
        }
    }
}
