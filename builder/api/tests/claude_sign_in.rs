//! Sign-in uses only invented stubs and disposable homes, never a real Claude account.
use persona_builder_agent::text;
use persona_builder_api::{dto::ClaudeStatus, *};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
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
here="$(cd "$(dirname "$0")" && pwd -P)"
printf '%s\n' "$*" >> "$here/runs"
case "$1 $2" in
'--version ') echo '2.1.292 (Claude Code)';;
'auth status') if [ -f "$here/ready" ]; then echo '{{"loggedIn":true}}'; else echo '{{"loggedIn":false}}'; fi;;
'auth login') touch "$here/login"; {login};;
esac
"#)).unwrap();
        fs::set_permissions(program, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn sign_in(&self, state: &ClaudeSignIns) -> ClaudeStatus {
        claude_sign_in_in(&self.0, None, &[self.0.join("claude")], state).unwrap()
    }
    fn runs(&self) -> String {
        fs::read_to_string(self.0.join("runs")).unwrap_or_default()
    }
    fn ready(&self) {
        fs::write(self.0.join("ready"), "").unwrap();
    }
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
#[test]
fn successful_login_is_checked_again_and_releases_the_slot() {
    let stub = Stub::new("touch \"$here/ready\"");
    let state = ClaudeSignIns::new();
    let answer = stub.sign_in(&state);
    assert!(answer.ready);
    assert!(!answer.can_sign_in);
    assert!(stub.runs().contains("auth login --claudeai"));
    assert!(stub.sign_in(&state).ready);
}
#[test]
fn exit_zero_without_sign_in_is_failed_and_can_try_again() {
    let stub = Stub::new("exit 0");
    let state = ClaudeSignIns::new();
    for _ in 0..2 {
        let answer = stub.sign_in(&state);
        assert!(!answer.ready);
        assert!(answer.can_sign_in);
        assert_eq!(answer.message, text::SIGN_IN_FAILED);
    }
    assert_eq!(stub.runs().matches("auth login").count(), 2);
}
#[test]
fn failed_login_rechecks_status_and_accepts_a_completed_sign_in() {
    let stub = Stub::new("touch \"$here/ready\"; exit 1");
    assert!(stub.sign_in(&ClaudeSignIns::new()).ready);
}
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
#[test]
fn busy_cancel_and_retry_never_check_status_after_cancel() {
    let stub = Stub::new("sleep 3");
    let state = ClaudeSignIns::new();
    thread::scope(|scope| {
        let first = scope.spawn(|| stub.sign_in(&state));
        stub.wait_for_login();
        let busy = claude_sign_in_in(&stub.0, None, &[], &state);
        // Cancel even if the busy guard regresses, so no waiting child leaks from this test.
        claude_sign_in_cancel_in(&state).unwrap();
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
fn unavailable_or_broken_program_does_not_offer_or_run_login() {
    let stub = Stub::new("exit 1");
    let state = ClaudeSignIns::new();
    let missing = claude_sign_in_in(&stub.0, None, &[], &state).unwrap();
    assert!(!missing.can_sign_in);
    assert_eq!(missing.message, text::NOT_INSTALLED);
    fs::write(stub.0.join("claude"), "#!/bin/sh\necho private-detail\n").unwrap();
    let broken = stub.sign_in(&state);
    assert!(!broken.can_sign_in);
    assert_eq!(broken.message, text::DOES_NOT_RUN);
    assert!(!stub.runs().contains("auth login"));
    stub.program("touch \"$here/ready\"");
    assert!(stub.sign_in(&state).ready);
}
