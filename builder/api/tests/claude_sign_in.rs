//! Sign-in uses only invented stubs and disposable homes, never a real Claude account.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
use voice_agent::text;
use voice_api::{
    dto::{ClaudeSignInRequest, ClaudeStatus},
    *,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Stub(PathBuf);
impl Stub {
    fn new(body: &str) -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-login-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let stub = Self(home);
        stub.program(body);
        stub
    }
    fn program(&self, login: &str) {
        let program = self.0.join("claude");
        fs::write(&program, format!(r#"#!/bin/sh
shift 2
here="$(cd "$(dirname "$0")" && pwd -P)"
printf '%s\n' "$*" >> "$here/runs"
case "$1 $2" in
'--version ') echo '2.1.292 (Claude Code)';;
'auth status') if [ -f "$here/ready" ]; then echo '{{"loggedIn":true,"authMethod":"api_key"}}'; else echo '{{"loggedIn":false}}'; fi;;
'auth logout') rm -f "$here/ready";;
'auth login') touch "$here/login"; {login};;
esac
"#)).unwrap();
        fs::set_permissions(program, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn logout(&self, body: &str) {
        let program = self.0.join("claude");
        let script = fs::read_to_string(&program).unwrap();
        fs::write(&program, script.replace("rm -f \"$here/ready\"", body)).unwrap();
    }
    fn wait_for_logout(&self) {
        let begun = Instant::now();
        while !self.0.join("logout").exists() && begun.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(self.0.join("logout").exists(), "logout did not start");
    }
    fn sign_out(&self, state: &ClaudeSignIns) -> std::result::Result<ClaudeStatus, dto::Failure> {
        claude_sign_out_in(&self.0, None, &[self.0.join("claude")], state)
    }
    fn sign_in(&self, state: &ClaudeSignIns) -> ClaudeStatus {
        claude_sign_in_in(
            &self.0,
            None,
            &[self.0.join("claude")],
            state,
            &ClaudeSignInRequest {
                subscription: cfg!(all(target_os = "macos", feature = "claude-subscription")),
            },
        )
        .unwrap()
    }
    fn runs(&self) -> String {
        fs::read_to_string(self.0.join("runs")).unwrap_or_default()
    }
    fn ready(&self) {
        fs::write(self.0.join("ready"), "").unwrap();
    }
    #[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
    fn wait_for_login(&self) {
        let begun = Instant::now();
        while !self.0.join("login").exists() && begun.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(self.0.join("login").exists(), "login did not start");
    }
}
impl Drop for Stub {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn already_signed_in_never_starts_login_and_releases_the_slot() {
    let stub = Stub::new("exit 1");
    stub.ready();
    let state = ClaudeSignIns::new();
    for _ in 0..2 {
        let answer = stub.sign_in(&state);
        assert!(answer.ready);
        assert!(!answer.can_sign_in);
        assert_eq!(answer.message, text::ready("2.1.292"));
    }
    assert!(!stub.runs().contains("auth login"));
}
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn successful_login_is_checked_again_and_releases_the_slot() {
    let stub = Stub::new("touch \"$here/ready\"");
    let state = ClaudeSignIns::new();
    let answer = stub.sign_in(&state);
    assert!(answer.ready);
    assert!(!answer.can_sign_in);
    let login = if cfg!(all(target_os = "macos", feature = "claude-subscription")) {
        "auth login --claudeai"
    } else {
        "auth login --console"
    };
    assert!(stub.runs().contains(login));
    assert!(stub.sign_in(&state).ready);
}
#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_request_uses_subscription_login() {
    let stub = Stub::new("touch \"$here/ready\"");
    let answer = claude_sign_in_in(
        &stub.0,
        None,
        &[stub.0.join("claude")],
        &ClaudeSignIns::new(),
        &ClaudeSignInRequest { subscription: true },
    )
    .unwrap();
    assert!(answer.ready);
    let runs = stub.runs();
    assert!(runs.contains("auth login --claudeai"), "{runs}");
    assert!(!runs.contains("--console"), "{runs}");
}

#[cfg(all(not(target_os = "macos"), not(feature = "claude-subscription")))]
#[test]
fn sign_in_reports_a_subscription_account_as_refused() {
    let stub = Stub::new("touch \"$here/ready\"");
    let program = stub.0.join("claude");
    let script = fs::read_to_string(&program).unwrap();
    fs::write(&program, script.replace("api_key", "claude.ai")).unwrap();
    let answer = stub.sign_in(&ClaudeSignIns::new());
    assert_eq!(answer.message, text::SUBSCRIPTION_REFUSED);
    assert!(!answer.ready);
    assert!(answer.can_sign_in);
    assert!(!answer.can_sign_in_with_subscription);
    assert_eq!(answer.note, None);
    assert!(stub.runs().contains("auth login --console"));
}

#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn exit_zero_without_sign_in_is_failed_and_can_try_again() {
    let stub = Stub::new("exit 0");
    let state = ClaudeSignIns::new();
    for _ in 0..2 {
        let answer = stub.sign_in(&state);
        assert!(!answer.ready);
        assert!(answer.can_sign_in);
        assert_eq!(
            answer.can_sign_in_with_subscription,
            voice_agent::claude::SUBSCRIPTION_OFFERED
        );
        assert_eq!(answer.message, text::SIGN_IN_FAILED);
    }
    assert_eq!(stub.runs().matches("auth login").count(), 2);
}
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn failed_login_rechecks_status_and_accepts_a_completed_sign_in() {
    let stub = Stub::new("touch \"$here/ready\"; exit 1");
    assert!(stub.sign_in(&ClaudeSignIns::new()).ready);
}
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn failed_login_uses_safe_text_and_releases_the_slot() {
    let stub = Stub::new("echo private-login-link; exit 1");
    let state = ClaudeSignIns::new();
    for _ in 0..2 {
        let answer = stub.sign_in(&state);
        assert_eq!(answer.message, text::SIGN_IN_FAILED);
        assert!(answer.can_sign_in);
    }
}
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn busy_cancel_and_retry_never_check_status_after_cancel() {
    let stub = Stub::new("sleep 3");
    let state = ClaudeSignIns::new();
    thread::scope(|scope| {
        let first = scope.spawn(|| stub.sign_in(&state));
        stub.wait_for_login();
        let busy = claude_sign_in_in(
            &stub.0,
            None,
            &[],
            &state,
            &ClaudeSignInRequest {
                subscription: false,
            },
        );
        #[cfg(not(feature = "claude-subscription"))]
        let refused = claude_sign_in_in(
            &stub.0,
            None,
            &[],
            &state,
            &ClaudeSignInRequest { subscription: true },
        );
        // Cancel even if the busy guard regresses, so no waiting child leaks from this test.
        claude_sign_in_cancel_in(&state).unwrap();
        #[cfg(not(feature = "claude-subscription"))]
        assert_eq!(refused.unwrap().message, text::SUBSCRIPTION_NOT_OFFERED);
        assert_eq!(busy.unwrap_err().message, SIGN_IN_BUSY);
        let begun = Instant::now();
        let answer = first.join().unwrap();
        assert!(begun.elapsed() < Duration::from_secs(1));
        assert_eq!(answer.message, text::SIGN_IN_STOPPED);
        assert!(answer.can_sign_in);
        assert!(!answer.ready);
    });
    assert_eq!(stub.runs().matches("auth status").count(), 1);
    stub.program("touch \"$here/ready\"");
    assert!(stub.sign_in(&state).ready);
    claude_sign_in_cancel_in(&state).unwrap();
}
#[test]
fn cancel_during_status_never_starts_login() {
    let stub = Stub::new("exit 0");
    fs::write(
        stub.0.join("claude"),
        r#"#!/bin/sh
shift 2
here="$(cd "$(dirname "$0")" && pwd -P)"
case "$1 $2" in
'--version ') echo '2.1.292 (Claude Code)';;
'auth status')
    touch "$here/status-started"
    sleep 1
    touch "$here/status-finished"
    echo '{"loggedIn":false}';;
'auth login') echo login >> "$here/calls"; sleep 1;;
esac
"#,
    )
    .unwrap();
    let state = ClaudeSignIns::new();
    thread::scope(|scope| {
        let pending = scope.spawn(|| stub.sign_in(&state));
        let begun = Instant::now();
        while !stub.0.join("status-started").exists() && begun.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            stub.0.join("status-started").exists(),
            "status did not start"
        );
        assert!(!stub.0.join("status-finished").exists());
        claude_sign_in_cancel_in(&state).unwrap();
        // A spawned login can be killed before its shell writes `calls`. Removing the
        // program makes even that attempted spawn observable as DOES_NOT_RUN.
        fs::remove_file(stub.0.join("claude")).unwrap();
        let answer = pending.join().unwrap();
        assert_eq!(answer.message, text::SIGN_IN_STOPPED);
        assert!(!answer.ready);
        assert_eq!(answer.note, None);
        assert!(answer.can_sign_in);
    });
    assert!(
        !stub.0.join("calls").exists(),
        "auth login ran after cancel"
    );
}
#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn unavailable_or_broken_program_does_not_offer_or_run_login() {
    let stub = Stub::new("exit 1");
    let state = ClaudeSignIns::new();
    let missing = claude_sign_in_in(
        &stub.0,
        None,
        &[],
        &state,
        &ClaudeSignInRequest {
            subscription: false,
        },
    )
    .unwrap();
    assert!(!missing.can_sign_in);
    assert_eq!(missing.message, text::NOT_INSTALLED);
    fs::write(
        stub.0.join("claude"),
        "#!/bin/sh\nshift 2\necho private-detail\n",
    )
    .unwrap();
    let broken = stub.sign_in(&state);
    assert!(!broken.can_sign_in);
    assert_eq!(broken.message, text::DOES_NOT_RUN);
    assert!(!stub.runs().contains("auth login"));
    stub.program("touch \"$here/ready\"");
    assert!(stub.sign_in(&state).ready);
}

#[test]
fn subscription_status_obeys_the_build_feature() {
    let stub = Stub::new("exit 1");
    fs::write(
        stub.0.join("claude"),
        r#"#!/bin/sh
shift 2
case "$1" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{"loggedIn":true,"authMethod":"claude.ai"}';;
esac
"#,
    )
    .unwrap();
    let status = claude_check_in(&stub.0, None, &[stub.0.join("claude")]).unwrap();
    if voice_agent::claude::SUBSCRIPTION_OFFERED {
        assert!(status.ready);
        assert!(!status.can_sign_in);
        assert!(!status.can_sign_in_with_subscription);
        assert_eq!(status.message, text::ready("2.1.292"));
    } else {
        assert!(!status.ready);
        assert!(status.can_sign_in);
        assert!(!status.can_sign_in_with_subscription);
        assert_eq!(status.message, text::SUBSCRIPTION_REFUSED);
    }
}

#[cfg(not(feature = "claude-subscription"))]
#[test]
fn unavailable_subscription_never_finds_or_starts_claude() {
    let stub = Stub::new("exit 1");
    let state = ClaudeSignIns::new();
    let request = ClaudeSignInRequest { subscription: true };
    for fallbacks in [vec![], vec![stub.0.join("claude")]] {
        let status = claude_sign_in_in(&stub.0, None, &fallbacks, &state, &request).unwrap();
        assert!(!status.ready);
        assert!(status.can_sign_in);
        assert!(!status.can_sign_in_with_subscription);
        assert_eq!(status.note, None);
        assert_eq!(status.message, text::SUBSCRIPTION_NOT_OFFERED);
    }
    assert!(stub.runs().is_empty());
    assert!(!voice_agent::claude::profile_folder(&stub.0).exists());
}

#[test]
fn signed_out_status_offers_subscription_only_in_testers_builds() {
    let stub = Stub::new("exit 1");
    let status = claude_check_in(&stub.0, None, &[stub.0.join("claude")]).unwrap();
    assert!(!status.ready);
    assert!(status.can_sign_in);
    assert_eq!(
        status.can_sign_in_with_subscription,
        voice_agent::claude::SUBSCRIPTION_OFFERED
    );
}

#[test]
fn sign_in_request_requires_a_boolean_and_refuses_unknown_fields() {
    use serde_json::{from_value, json, to_value};
    for subscription in [false, true] {
        let value = json!({"subscription": subscription});
        let request: ClaudeSignInRequest = from_value(value.clone()).unwrap();
        assert_eq!(request.subscription, subscription);
        assert_eq!(to_value(request).unwrap(), value);
    }
    for value in [
        json!({}),
        json!({"subscription": null}),
        json!({"subscription": "true"}),
        json!({"subscription": false, "extra": true}),
    ] {
        assert!(from_value::<ClaudeSignInRequest>(value).is_err());
    }
}

#[cfg(target_os = "macos")]
#[test]
fn console_sign_in_reports_the_mac_limitation_without_starting_login() {
    let stub = Stub::new("exit 1");
    let answer = claude_sign_in_in(
        &stub.0,
        None,
        &[stub.0.join("claude")],
        &ClaudeSignIns::new(),
        &ClaudeSignInRequest {
            subscription: false,
        },
    )
    .unwrap();
    let expected =
        "Signing in with a Console account doesn't work on a Mac yet. A later update adds it.";
    #[cfg(feature = "claude-subscription")]
    let expected = format!("{expected} Use Sign in with a Claude subscription instead.");
    assert_eq!(answer.message, expected);
    assert!(!answer.ready);
    assert!(answer.can_sign_in);
    assert!(!stub.runs().contains("auth login"));
}

#[test]
fn sign_out_returns_fresh_signed_out_status() {
    let stub = Stub::new("exit 1");
    stub.ready();
    let state = ClaudeSignIns::new();
    let status = claude_sign_out_in(&stub.0, None, &[stub.0.join("claude")], &state).unwrap();
    assert!(!status.ready);
    assert!(status.can_sign_in);
    assert_eq!(status.message, text::signed_out_at("2.1.292", "~/claude"));
    assert_eq!(
        status.note.as_deref(),
        Some("Signed out of Claude Code in Voice.")
    );
    assert!(stub.runs().contains("auth logout"));
    assert!(!stub.0.join("ready").exists());
}

#[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
#[test]
fn sign_out_is_refused_while_sign_in_is_running() {
    let stub = Stub::new("sleep 3");
    let state = ClaudeSignIns::new();
    thread::scope(|scope| {
        let login = scope.spawn(|| stub.sign_in(&state));
        stub.wait_for_login();
        let answer = claude_sign_out_in(&stub.0, None, &[], &state);
        claude_sign_in_cancel_in(&state).unwrap();
        assert_eq!(
            answer.unwrap_err().message,
            "Wait for the sign-in to finish or cancel it, then sign out."
        );
        assert_eq!(login.join().unwrap().message, text::SIGN_IN_STOPPED);
    });
    assert!(!stub.runs().contains("auth logout"));
}

#[test]
fn sign_out_passes_only_fixed_failure_text() {
    let stub = Stub::new("exit 1");
    let program = stub.0.join("claude");
    let script = fs::read_to_string(&program).unwrap();
    fs::write(
        &program,
        script.replace("rm -f \"$here/ready\"", "echo private-detail; exit 1"),
    )
    .unwrap();
    let error = claude_sign_out_in(&stub.0, None, &[program], &ClaudeSignIns::new()).unwrap_err();
    assert_eq!(error.message, text::SIGN_OUT_FAILED);
}

#[test]
fn sign_out_reports_a_login_file_left_behind_without_removing_it() {
    let stub = Stub::new("exit 1");
    let profile = voice_agent::claude::profile_folder(&stub.0);
    fs::create_dir_all(&profile).unwrap();
    let file = profile.join(".credentials.json");
    fs::write(&file, "").unwrap();
    let error = claude_sign_out_in(
        &stub.0,
        None,
        &[stub.0.join("claude")],
        &ClaudeSignIns::new(),
    )
    .unwrap_err();
    assert_eq!(error.message, text::SIGN_OUT_FILE_LEFT);
    assert!(file.exists());
}

#[test]
fn sign_in_is_refused_at_once_while_sign_out_runs() {
    let stub = Stub::new("exit 1");
    stub.logout("touch \"$here/logout\"; sleep 3");
    let state = ClaudeSignIns::new();
    thread::scope(|scope| {
        let logout = scope.spawn(|| stub.sign_out(&state));
        stub.wait_for_logout();
        let begun = Instant::now();
        let result = claude_sign_in_in(
            &stub.0,
            None,
            &[],
            &state,
            &ClaudeSignInRequest {
                subscription: false,
            },
        );
        assert!(
            begun.elapsed() < Duration::from_secs(1),
            "sign-in waited for logout"
        );
        assert_eq!(
            result.unwrap_err().message,
            "Wait for the sign-out to finish, then sign in."
        );
        logout.join().unwrap().unwrap();
    });
    assert!(!stub.runs().contains("auth login"));
}

#[test]
fn a_second_sign_out_is_refused_at_once() {
    let stub = Stub::new("exit 1");
    stub.logout("touch \"$here/logout\"; sleep 3");
    let state = ClaudeSignIns::new();
    thread::scope(|scope| {
        let logout = scope.spawn(|| stub.sign_out(&state));
        stub.wait_for_logout();
        let begun = Instant::now();
        let result = stub.sign_out(&state);
        assert!(
            begun.elapsed() < Duration::from_secs(1),
            "second sign-out waited for logout"
        );
        assert_eq!(result.unwrap_err().message, "Voice is already signing out.");
        logout.join().unwrap().unwrap();
    });
    assert_eq!(stub.runs().matches("auth logout").count(), 1);
}

#[test]
fn cancel_and_shutdown_do_not_wait_for_sign_out() {
    let stub = Stub::new("exit 1");
    stub.logout("touch \"$here/logout\"; sleep 3");
    let state = claude_sign_ins();
    thread::scope(|scope| {
        let logout = scope.spawn(|| stub.sign_out(state));
        stub.wait_for_logout();
        let begun = Instant::now();
        claude_sign_in_cancel_in(state).unwrap();
        claude_sign_in_shutdown(Duration::from_secs(5));
        assert!(
            begun.elapsed() < Duration::from_secs(1),
            "cancel/shutdown waited for logout"
        );
        assert!(!logout.is_finished(), "logout must still be running");
        logout.join().unwrap().unwrap();
    });
}

#[test]
fn failed_sign_out_releases_the_marker_for_sign_in_and_sign_out() {
    let stub = Stub::new("exit 1");
    stub.logout("exit 1");
    let state = ClaudeSignIns::new();
    assert_eq!(
        stub.sign_out(&state).unwrap_err().message,
        text::SIGN_OUT_FAILED
    );
    stub.ready();
    assert!(stub.sign_in(&state).ready);
    stub.program("exit 1");
    assert!(!stub.sign_out(&state).unwrap().ready);
}

#[test]
fn subscription_status_is_only_true_for_ready_subscription() {
    for method in ["claude.ai", "api_key"] {
        let stub = Stub::new("");
        stub.ready();
        let program = stub.0.join("claude");
        fs::write(
            &program,
            fs::read_to_string(&program)
                .unwrap()
                .replace("api_key", method),
        )
        .unwrap();
        let status = claude_check_in(&stub.0, None, std::slice::from_ref(&program)).unwrap();
        assert_eq!(
            status.subscription,
            method == "claude.ai" && cfg!(feature = "claude-subscription")
        );
        fs::remove_file(stub.0.join("ready")).unwrap();
        let status = claude_check_in(&stub.0, None, &[program]).unwrap();
        assert!(!status.ready);
        assert!(!status.subscription);
    }
    let stub = Stub::new("");
    fs::remove_file(stub.0.join("claude")).unwrap();
    assert!(
        !claude_check_in(&stub.0, Some(std::ffi::OsStr::new("")), &[])
            .unwrap()
            .subscription
    );
}
