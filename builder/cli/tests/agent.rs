use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use voice_agent::ledger::{Cap, Session};
use voice_core::crew::{NewMember, new_member};

static NEXT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn claude_help_lists_check_then_sign_in_then_sign_out() {
    let fixture = Fixture::new();
    let out = fixture.run(&["claude", "--help"]);
    assert!(out.status.success());
    let help = text(&out.stdout);
    assert!(help.find("  check ").unwrap() < help.find("  sign-in ").unwrap());
    assert!(help.find("  sign-in ").unwrap() < help.find("  sign-out ").unwrap());
}

struct Fixture {
    home: PathBuf,
    member: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-agent-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let member = home.join("crew/wren");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            &member,
            &NewMember {
                decisions: voice_core::crew::Decisions::Python,
                name: "Wren",
                pronouns: "she/her",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-10-24",
            },
        )
        .unwrap();
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        Self { home, member, bin }
    }
    fn stub(&self, body: &str) {
        let program = self.bin.join("claude");
        fs::write(&program, format!("#!/bin/sh\nshift 2\nprintf '%s\\n' \"${{ANTHROPIC_API_KEY+set}}\" >> \"${{0%/*}}/api-key\"\n{body}\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_voice"));
        command
            .env("HOME", &self.home)
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env_remove("ANTHROPIC_API_KEY")
            .args(args);
        command
    }
    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }
    fn state(&self) -> PathBuf {
        self.home.join(".claude/state/wren")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn agents_lists_claude_code() {
    let f = Fixture::new();
    let out = f.run(&["agents"]);
    assert!(out.status.success());
    assert_eq!(
        text(&out.stdout),
        "claude-code  Claude, through your own Claude Code\nself-hosted  A model server on your own machines\n"
    );
}

#[test]
fn claude_check_says_ready_or_what_to_do() {
    let f = Fixture::new();
    f.stub(
        "case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":true,\"authMethod\":\"api_key\",\"email\":\"someone@example.com\"}';; esac",
    );
    let out = f.run(&["claude", "check"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Claude Code 2.1.292 is installed and signed in.\n"
    );
    assert!(!text(&out.stdout).contains("example.com"));
    let out = f
        .command(&["claude", "check"])
        .env("ANTHROPIC_API_KEY", "not-a-real-key")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Claude Code 2.1.292 is installed and signed in.\n"
    );
    assert!(!text(&out.stdout).contains("billed separately"));
    assert_eq!(
        fs::read_to_string(f.bin.join("api-key")).unwrap(),
        "\n\n\n\n"
    );
    assert!(!text(&out.stdout).contains("not-a-real-key"));

    f.stub("case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":false}';; esac");
    let out = f.run(&["claude", "check"]);
    assert_eq!(out.status.code(), Some(1));
    let expected = if !cfg!(target_os = "macos") {
        "voice: Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with an Anthropic Console account. Run voice claude sign-in to sign in.\n"
    } else if voice_agent::claude::SUBSCRIPTION_OFFERED {
        "voice: Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with your Claude subscription. Run voice claude sign-in --subscription to sign in.\n"
    } else {
        "voice: Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.\n"
    };
    assert_eq!(text(&out.stderr), expected);
}

#[test]
fn claude_sign_in_skips_login_when_already_signed_in() {
    let f = Fixture::new();
    f.stub(
        r#"here="$(dirname "$0")"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth status') echo '{"loggedIn":true,"authMethod":"api_key"}';;
    'auth login') touch "$here/login";;
esac"#,
    );
    let out = f.run(&["claude", "sign-in"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Claude Code 2.1.292 is installed and signed in.\n"
    );
    assert!(!f.bin.join("login").exists());
}

#[cfg(not(target_os = "macos"))]
#[test]
fn claude_sign_in_uses_its_profile_then_checks_the_result() {
    let f = Fixture::new();
    f.stub(r#"here="$(dirname "$0")"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth status') if [ -f "$here/login" ]; then echo '{"loggedIn":true,"authMethod":"api_key"}'; else echo '{"loggedIn":false}'; fi;;
    'auth login') printf '%s\n' "$@" > "$here/login"; printf '%s' "$CLAUDE_CONFIG_DIR" > "$here/profile";;
esac"#);
    let out = f.run(&["claude", "sign-in"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        fs::read_to_string(f.bin.join("login")).unwrap(),
        "auth\nlogin\n--console\n"
    );
    #[cfg(target_os = "macos")]
    let profile = f.home.join("Library/Application Support/Voice/claude-code");
    #[cfg(not(target_os = "macos"))]
    let profile = f.home.join(".local/share/voice/claude-code");
    assert_eq!(
        fs::read_to_string(f.bin.join("profile")).unwrap(),
        profile.to_str().unwrap()
    );
    assert_eq!(
        text(&out.stdout),
        "Finish signing in on the page that opened in your browser. Voice carries on when you're done.\nClaude Code 2.1.292 is installed and signed in.\n"
    );
}

#[cfg(feature = "claude-subscription")]
#[test]
fn claude_subscription_sign_in_uses_its_profile_then_checks_the_result() {
    let f = Fixture::new();
    f.stub(r#"here="$(dirname "$0")"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth status') if [ -f "$here/login" ]; then echo '{"loggedIn":true,"authMethod":"api_key"}'; else echo '{"loggedIn":false}'; fi;;
    'auth login') printf '%s\n' "$@" > "$here/login"; printf '%s' "$CLAUDE_CONFIG_DIR" > "$here/profile";;
esac"#);
    let out = f.run(&["claude", "sign-in", "--subscription"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        fs::read_to_string(f.bin.join("login")).unwrap(),
        "auth\nlogin\n--claudeai\n"
    );
    #[cfg(target_os = "macos")]
    let profile = f.home.join("Library/Application Support/Voice/claude-code");
    #[cfg(not(target_os = "macos"))]
    let profile = f.home.join(".local/share/voice/claude-code");
    assert_eq!(
        fs::read_to_string(f.bin.join("profile")).unwrap(),
        profile.to_str().unwrap()
    );
    assert_eq!(
        text(&out.stdout),
        "Finish signing in on the page that opened in your browser. Voice carries on when you're done.\nClaude Code 2.1.292 is installed and signed in.\n"
    );
}

#[cfg(all(not(target_os = "macos"), not(feature = "claude-subscription")))]
#[test]
fn claude_sign_in_reports_a_subscription_account_as_refused() {
    let f = Fixture::new();
    f.stub(r#"here="$(dirname "$0")"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth status') if [ -f "$here/login" ]; then echo '{"loggedIn":true,"authMethod":"claude.ai"}'; else echo '{"loggedIn":false}'; fi;;
    'auth login') printf '%s\n' "$@" > "$here/login";;
esac"#);
    let out = f.run(&["claude", "sign-in"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stderr).contains(voice_agent::text::SUBSCRIPTION_REFUSED),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(f.bin.join("login")).unwrap(),
        "auth\nlogin\n--console\n"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn claude_sign_in_fails_when_login_does_not_sign_in() {
    let f = Fixture::new();
    f.stub(r#"case "$1" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{"loggedIn":false}';; esac"#);
    let out = f.run(&["claude", "sign-in"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        "voice: Claude Code didn't finish signing in. Try again. If no sign-in page opened in your browser, check that this computer has a default browser.\n"
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn claude_sign_in_stops_the_child_on_ctrl_c() {
    let f = Fixture::new();
    f.stub(
        r#"here="${0%/*}"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth status') echo '{"loggedIn":false}';;
    'auth login') echo "$$" > "$here/pid"; sleep 30;;
esac"#,
    );
    let mut cli = f
        .command(&["claude", "sign-in"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let pid = loop {
        if let Ok(text) = fs::read_to_string(f.bin.join("pid"))
            && let Ok(pid) = text.trim().parse::<u32>()
        {
            break pid;
        }
        if cli.try_wait().unwrap().is_some() {
            let out = cli.wait_with_output().unwrap();
            panic!("sign-in did not start: {}", text(&out.stderr));
        }
        if start.elapsed() >= Duration::from_secs(5) {
            cli.kill().unwrap();
            let _ = cli.wait();
            panic!("sign-in did not start");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let start = Instant::now();
    assert!(
        Command::new("/bin/kill")
            .args(["-INT", &cli.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    while cli.try_wait().unwrap().is_none() {
        if start.elapsed() >= Duration::from_secs(5) {
            cli.kill().unwrap();
            let _ = cli.wait();
            panic!("sign-in did not stop after Ctrl-C");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let out = cli.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert_eq!(
        text(&out.stderr),
        format!("voice: {}\n", voice_agent::text::SIGN_IN_STOPPED)
    );
    assert!(
        !Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn claude_sign_in_stops_on_ctrl_c_during_the_first_check() {
    let f = Fixture::new();
    f.stub(
        r#"here="${0%/*}"
case "$1 $2" in
    '--version ') echo "$$" > "$here/pid"; sleep 2; echo '2.1.292 (Claude Code)';;
    'auth status') echo '{"loggedIn":false}';;
    'auth login') touch "$here/login"; sleep 30;;
esac"#,
    );
    let mut cli = f
        .command(&["claude", "sign-in"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Ok(text) = fs::read_to_string(f.bin.join("pid"))
            && text.trim().parse::<u32>().is_ok()
        {
            break;
        }
        if cli.try_wait().unwrap().is_some() {
            let out = cli.wait_with_output().unwrap();
            panic!("first check did not start: {}", text(&out.stderr));
        }
        if start.elapsed() >= Duration::from_secs(5) {
            cli.kill().unwrap();
            let _ = cli.wait();
            panic!("first check did not start");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let start = Instant::now();
    assert!(
        Command::new("/bin/kill")
            .args(["-INT", &cli.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    while cli.try_wait().unwrap().is_none() {
        if start.elapsed() >= Duration::from_secs(10) {
            cli.kill().unwrap();
            let _ = cli.wait();
            panic!("sign-in did not stop after Ctrl-C during the first check");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let out = cli.wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(out.status.code().is_some());
    assert_eq!(
        text(&out.stderr),
        format!("voice: {}\n", voice_agent::text::SIGN_IN_STOPPED)
    );
    assert!(!f.bin.join("login").exists());
    #[cfg(target_os = "macos")]
    let profile = f.home.join("Library/Application Support/Voice/claude-code");
    #[cfg(not(target_os = "macos"))]
    let profile = f.home.join(".local/share/voice/claude-code");
    match fs::read_dir(voice_agent::sandbox::runs_folder(&profile)) {
        Ok(mut entries) => assert!(entries.next().is_none()),
        Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
    }
}

fn day_of(home: &Path) -> String {
    let _ = home;
    voice_agent::ledger::local_day(SystemTime::now())
}

#[test]
fn spend_reports_today_against_the_cap() {
    let f = Fixture::new();
    let member = f.member.to_str().unwrap().to_owned();
    let out = f.run(&["spend", &member]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Today, about $0.00 of the $50.00 daily limit, as Claude Code estimates it.\n"
    );

    let today = day_of(&f.home);
    let earlier =
        voice_agent::ledger::local_day(UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000));
    let line = |day: &str, cost: f64| {
        format!(
            r#"{{"at":"x","day":"{day}","agent":"claude-code","step":"label","model":"sonnet","outcome":"ok","seconds":1.0,"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"cost_usd":{cost},"cost_from":"claude"}}"#
        )
    };
    fs::write(
        f.state().join("runs.jsonl"),
        format!("{}\n{}\n", line(&today, 3.2), line(&earlier, 9.0)),
    )
    .unwrap();
    let out = f.run(&["spend", &member, "--daily-cap-usd", "20"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Today, about $3.20 of the $20.00 daily limit, as Claude Code estimates it.\n"
    );

    let out = f.run(&["spend", &member, "--daily-cap-usd", "0"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Today, about $3.20, with no daily limit, as Claude Code estimates it.\n"
    );

    let held = Session::open(&f.home, "wren", Cap::default()).unwrap();
    let out = f.run(&["spend", &member]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stderr).contains("already working on this assistant"),
        "{}",
        text(&out.stderr)
    );
    drop(held);
}

#[test]
fn limit_show_prints_defaults_without_creating_state() {
    let f = Fixture::new();
    let out = f.run(&["limit", "show", f.member.to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Daily limit: about $50.00 a day, and at most $5.00 for one step, as Claude Code estimates it.\n"
    );
    assert!(!f.state().exists());
}

#[test]
fn saved_limit_is_used_by_spend_and_flags_still_override() {
    let f = Fixture::new();
    let member = f.member.to_str().unwrap();
    drop(Session::open(&f.home, "wren", Cap::default()).unwrap());
    let out = f.run(&[
        "limit",
        "set",
        member,
        "--daily-usd",
        "20",
        "--step-usd",
        "2.5",
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Daily limit: about $20.00 a day, and at most $2.50 for one step, as Claude Code estimates it.\n"
    );
    let out = f.run(&["limit", "show", member]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("$20.00 a day, and at most $2.50"));
    for (flag, expected) in [(None, "20.00"), (Some("30"), "30.00"), (Some("1"), "1.00")] {
        let mut args = vec!["spend", member];
        if let Some(value) = flag {
            args.extend(["--daily-cap-usd", value]);
        }
        let out = f.run(&args);
        assert!(out.status.success(), "{}", text(&out.stderr));
        assert!(text(&out.stdout).contains(&format!("of the ${expected} daily limit")));
    }
}

#[test]
fn invalid_limit_set_writes_nothing() {
    let f = Fixture::new();
    let out = f.run(&[
        "limit",
        "set",
        f.member.to_str().unwrap(),
        "--daily-usd",
        "1000.01",
        "--step-usd",
        "0",
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("Each amount is 0 for no limit, or from $0.01 to $1,000 in whole cents, like 2.50. When both are set, the most one step may use can't be more than the daily limit."));
    assert!(!f.state().exists());
}

#[test]
fn limit_set_needs_existing_state_and_refuses_a_running_session() {
    let f = Fixture::new();
    let args = [
        "limit",
        "set",
        f.member.to_str().unwrap(),
        "--daily-usd",
        "20",
        "--step-usd",
        "2.5",
    ];
    let out = f.run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    let stderr = text(&out.stderr);
    assert!(stderr.contains("This assistant has no private folder yet, so there's nowhere to keep its limit. Run `voice consent agree` first."), "{stderr}");
    assert!(!stderr.contains(f.home.to_str().unwrap()), "{stderr}");
    assert!(!stderr.contains("os error"), "{stderr}");
    assert!(!f.state().exists());
    let held = Session::open(&f.home, "wren", Cap::default()).unwrap();
    let out = f.run(&args);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("already working on this assistant"));
    assert!(!f.state().join("limit.json").exists());
    drop(held);
}

#[test]
fn explicit_call_cap_above_the_day_is_refused() {
    let f = Fixture::new();
    let out = f.run(&[
        "refresh",
        "propose",
        f.member.to_str().unwrap(),
        "--daily-cap-usd",
        "10",
        "--call-cap-usd",
        "20",
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("the most one step may use can't be more than the daily limit")
    );
    assert!(!f.state().exists());
}

#[test]
fn no_limit_is_saved_shown_and_used_by_spend() {
    let f = Fixture::new();
    let member = f.member.to_str().unwrap();
    drop(Session::open(&f.home, "wren", Cap::default()).unwrap());
    let out = f.run(&[
        "limit",
        "set",
        member,
        "--daily-usd",
        "0",
        "--step-usd",
        "0",
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let expected = "No limit: Voice doesn't stop for the day or for one step.\n";
    assert_eq!(text(&out.stdout), expected);
    let out = f.run(&["limit", "show", member]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), expected);
    let out = f.run(&["spend", member]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Today, about $0.00, with no daily limit, as Claude Code estimates it.\n"
    );
}

#[test]
fn no_daily_override_preserves_the_saved_step() {
    daily_override_budget("50", "5", "0", "5.00");
}

#[test]
fn daily_override_preserves_a_saved_unlimited_step() {
    daily_override_budget("50", "0", "20", "20.00");
}

fn daily_override_budget(saved_day: &str, saved_step: &str, daily: &str, expected_budget: &str) {
    use serde_json::json;
    let f = Fixture::new();
    let member = f.member.to_str().unwrap();
    drop(Session::open(&f.home, "wren", Cap::default()).unwrap());
    let out = f.run(&[
        "limit",
        "set",
        member,
        "--daily-usd",
        saved_day,
        "--step-usd",
        saved_step,
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let kinds = ["scope", "sequencing", "methods", "verification", "go-no-go"];
    let questions: Vec<_> = (1..=20)
        .map(|n| {
            json!({
                "n": n, "header": kinds[(n - 1) % kinds.len()],
                "question": format!("Situation {n}: what first?"), "multiSelect": false,
                "recommended": "A", "options": [
                    {"label": "Cut it", "description": "Less work"},
                    {"label": "Keep it", "description": "More work"}
                ]
            })
        })
        .collect();
    fs::write(
        f.bin.join("reply"),
        json!({
            "type": "result", "subtype": "success", "is_error": false,
            "result": "", "total_cost_usd": 0.02,
            "structured_output": {"questions": questions}
        })
        .to_string(),
    )
    .unwrap();
    f.stub(
        r#"here="$(cd "$(dirname "$0")" && pwd -P)"
case "$1" in
    --version) echo '2.1.292 (Claude Code)';;
    auth) echo '{"loggedIn":true,"authMethod":"api_key"}';;
    *) printf '%s\n' "$@" > "$here/argv"; cat > "$here/stdin"; cat "$here/reply";;
esac"#,
    );
    let out = f.run(&["interview", "draft", member, "--daily-cap-usd", daily]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let argv = fs::read_to_string(f.bin.join("argv")).unwrap();
    let args: Vec<_> = argv.lines().collect();
    let at = args
        .iter()
        .position(|arg| *arg == "--max-budget-usd")
        .expect("the saved step or remaining daily limit must bound the call");
    assert_eq!(args[at + 1], expected_budget);
    let saved = Cap::load(&f.home, "wren").unwrap();
    assert_eq!(saved.daily_usd(), saved_day.parse::<f64>().unwrap());
    assert_eq!(saved.call_usd(), saved_step.parse::<f64>().unwrap());
}

#[cfg(not(feature = "claude-subscription"))]
#[test]
fn public_sign_in_refuses_the_subscription_flag() {
    let fixture = Fixture::new();
    let out = fixture.run(&["claude", "sign-in", "--subscription"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("unexpected argument '--subscription'"),
        "{}",
        text(&out.stderr)
    );
    let help = fixture.run(&["claude", "sign-in", "--help"]);
    assert!(help.status.success());
    assert!(!text(&help.stdout).contains("--subscription"));
}

#[cfg(feature = "claude-subscription")]
#[test]
fn testers_sign_in_help_offers_the_subscription_flag() {
    let fixture = Fixture::new();
    let out = fixture.run(&["claude", "sign-in", "--help"]);
    assert!(out.status.success());
    assert!(text(&out.stdout).contains("--subscription"));
    assert!(
        text(&out.stdout).contains("Sign in with a Claude subscription (testers' builds only).")
    );
}

#[test]
fn claude_sign_out_prints_success_after_claude_logs_out() {
    let f = Fixture::new();
    f.stub(
        r#"here="${0%/*}"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth logout') touch "$here/logged-out";;
esac"#,
    );
    let out = f.run(&["claude", "sign-out"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "Signed out of Claude Code in Voice.\n");
    assert!(f.bin.join("logged-out").exists());
}

#[test]
fn claude_sign_out_failure_has_no_success_line() {
    let f = Fixture::new();
    f.stub("exit 1");
    let out = f.run(&["claude", "sign-out"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stdout).is_empty());
}

#[cfg(target_os = "macos")]
#[test]
fn console_sign_in_reports_the_mac_limitation_without_starting_login() {
    let f = Fixture::new();
    f.stub(
        r#"here="${0%/*}"
case "$1 $2" in
    '--version ') echo '2.1.292 (Claude Code)';;
    'auth status') echo '{"loggedIn":false}';;
    'auth login') touch "$here/login"; exit 1;;
esac"#,
    );
    let out = f.run(&["claude", "sign-in"]);
    assert_eq!(out.status.code(), Some(1));
    let expected =
        "Signing in with a Console account doesn't work on a Mac yet. A later update adds it.";
    #[cfg(feature = "claude-subscription")]
    let expected = format!("{expected} Use Sign in with a Claude subscription instead.");
    assert!(text(&out.stdout).is_empty());
    assert_eq!(text(&out.stderr), format!("voice: {expected}\n"));
    assert!(!f.bin.join("login").exists());
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_spend_reports_todays_calls_and_tokens() {
    let f = Fixture::new();
    f.stub(r#"case "$1" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{"loggedIn":true,"authMethod":"claude.ai"}';; esac"#);
    {
        let _session = Session::open(&f.home, "wren", Cap::default()).unwrap();
    }
    let today = voice_agent::ledger::local_day(SystemTime::now());
    voice_ingest::private::write_private(&f.home, "wren", "runs.jsonl", format!("{}\n{}\n{}\n",
        serde_json::json!({"day":today,"cost_usd":1,"input_tokens":100,"output_tokens":200,"cache_creation_input_tokens":300,"cache_read_input_tokens":250}),
        serde_json::json!({"day":today,"cost_usd":5,"outcome":"started"}),
        serde_json::json!({"day":"2000-01-01","cost_usd":1,"input_tokens":999})).as_bytes()).unwrap();
    let out = f.run(&["spend", f.member.to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Today, 2 calls to Claude and 850 tokens, on your Claude subscription.\n"
    );
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_limits_show_and_save_console_amounts() {
    for method in ["claude.ai", "api_key"] {
        let f = Fixture::new();
        f.stub(&format!(r#"case "$1" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{{"loggedIn":true,"authMethod":"{method}"}}';; esac"#));
        let member = f.member.to_str().unwrap();
        let out = f.run(&["limit", "show", member]);
        assert!(out.status.success(), "{}", text(&out.stderr));
        let expected = if method == "claude.ai" {
            "Signed in with your Claude subscription, Voice has no daily limit: it stops for the day when Claude Code says your plan is nearly used up, and leaves the rest for you. The daily limit is for an Anthropic Console sign-in only."
        } else {
            "Daily limit: about $50.00 a day, and at most $5.00 for one step, as Claude Code estimates it."
        };
        assert_eq!(text(&out.stdout), format!("{expected}\n"));
        drop(Session::open(&f.home, "wren", Cap::default()).unwrap());
        let out = f.run(&[
            "limit",
            "set",
            member,
            "--daily-usd",
            "20",
            "--step-usd",
            "2",
        ]);
        assert!(out.status.success(), "{}", text(&out.stderr));
        let expected = if method == "claude.ai" {
            "Saved for an Anthropic Console sign-in. Signed in with your Claude subscription, Voice has no daily limit: it stops for the day when Claude Code says your plan is nearly used up, and leaves the rest for you."
        } else {
            "Daily limit: about $20.00 a day, and at most $2.00 for one step, as Claude Code estimates it."
        };
        assert_eq!(text(&out.stdout), format!("{expected}\n"));
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(f.state().join("limit.json")).unwrap()).unwrap();
        assert_eq!(saved, serde_json::json!({"daily_usd":20.0,"call_usd":2.0}));
    }
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_interview_ignores_daily_limit_keeps_step_budget() {
    for method in ["claude.ai", "api_key"] {
        let f = Fixture::new();
        let member = f.member.to_str().unwrap();
        {
            let _session = Session::open(&f.home, "wren", Cap::default()).unwrap();
            Cap::new(1.0, 0.5).unwrap().save(&f.home, "wren").unwrap();
        }
        voice_ingest::private::write_private(
            &f.home,
            "wren",
            "runs.jsonl",
            format!("{{\"day\":\"{}\",\"cost_usd\":2}}\n", day_of(&f.home)).as_bytes(),
        )
        .unwrap();
        let kinds = ["scope", "sequencing", "methods", "verification", "go-no-go"];
        let questions: Vec<_> = (1..=20).map(|n| serde_json::json!({"n":n,"header":kinds[(n-1)%5],"question":format!("Situation {n}: what first?"),"multiSelect":false,"recommended":"A","options":[{"label":"Cut it","description":"Less work"},{"label":"Keep it","description":"More work"}]})).collect();
        fs::write(f.bin.join("reply"), serde_json::json!({"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.02,"structured_output":{"questions":questions}}).to_string()).unwrap();
        f.stub(&format!(
            r#"here="${{0%/*}}"
case "$1" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{{"loggedIn":true,"authMethod":"{method}"}}';;
*) printf '%s\n' "$@" > "$here/argv"; cat > "$here/stdin"; cat "$here/reply";;
esac"#
        ));
        let out = f.run(&["interview", "draft", member, "--daily-cap-usd", "1"]);
        if method == "claude.ai" {
            assert!(out.status.success(), "{}", text(&out.stderr));
            let argv = fs::read_to_string(f.bin.join("argv")).unwrap();
            let args: Vec<_> = argv.lines().collect();
            let at = args.iter().position(|s| *s == "--max-budget-usd").unwrap();
            assert_eq!(args[at + 1], "0.50");
        } else {
            assert_eq!(
                text(&out.stderr),
                "voice: Today's spending limit for this assistant is reached: about $2.00 of $1.00, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit.\n"
            );
            assert!(!f.bin.join("argv").exists());
        }
    }
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_interview_ignores_explicit_daily_flags() {
    for (method, explicit_step) in [("claude.ai", false), ("claude.ai", true), ("api_key", true)] {
        let f = Fixture::new();
        let member = f.member.to_str().unwrap();
        {
            let _session = Session::open(&f.home, "wren", Cap::default()).unwrap();
            Cap::new(50.0, 5.0).unwrap().save(&f.home, "wren").unwrap();
        }
        voice_ingest::private::write_private(
            &f.home,
            "wren",
            "runs.jsonl",
            format!("{{\"day\":\"{}\",\"cost_usd\":2}}\n", day_of(&f.home)).as_bytes(),
        )
        .unwrap();
        let kinds = ["scope", "sequencing", "methods", "verification", "go-no-go"];
        let questions: Vec<_> = (1..=20).map(|n| serde_json::json!({"n":n,"header":kinds[(n-1)%5],"question":format!("Situation {n}: what first?"),"multiSelect":false,"recommended":"A","options":[{"label":"Cut it","description":"Less work"},{"label":"Keep it","description":"More work"}]})).collect();
        fs::write(f.bin.join("reply"), serde_json::json!({"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.02,"structured_output":{"questions":questions}}).to_string()).unwrap();
        f.stub(&format!(
            r#"here="${{0%/*}}"
case "$1" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{{"loggedIn":true,"authMethod":"{method}"}}';;
*) printf '%s\n' "$@" > "$here/argv"; cat > "$here/stdin"; cat "$here/reply";;
esac"#
        ));
        let mut args = vec!["interview", "draft", member, "--daily-cap-usd", "2"];
        if explicit_step {
            args.extend(["--call-cap-usd", "5"]);
        }
        let out = f.run(&args);
        if method == "claude.ai" {
            assert!(out.status.success(), "{}", text(&out.stderr));
            let argv = fs::read_to_string(f.bin.join("argv")).unwrap();
            let args: Vec<_> = argv.lines().collect();
            let at = args.iter().position(|s| *s == "--max-budget-usd").unwrap();
            assert_eq!(args[at + 1], "5.00");
        } else {
            assert_eq!(
                text(&out.stderr),
                "voice: Each amount is 0 for no limit, or from $0.01 to $1,000 in whole cents, like 2.50. When both are set, the most one step may use can't be more than the daily limit.\n"
            );
            assert!(!f.bin.join("argv").exists());
        }
    }
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_interview_new_session_ignores_daily_limit() {
    for method in ["claude.ai", "api_key"] {
        let f = Fixture::new();
        let member = f.member.to_str().unwrap();
        fs::write(
            f.bin.join("runs-seed"),
            format!(
                "{{\"day\":\"{}\",\"cost_usd\":2}}\n",
                voice_agent::ledger::local_day(SystemTime::now())
            ),
        )
        .unwrap();
        let kinds = ["scope", "sequencing", "methods", "verification", "go-no-go"];
        let questions: Vec<_> = (1..=20).map(|n| serde_json::json!({"n":n,"header":kinds[(n-1)%5],"question":format!("Situation {n}: what first?"),"multiSelect":false,"recommended":"A","options":[{"label":"Cut it","description":"Less work"},{"label":"Keep it","description":"More work"}]})).collect();
        fs::write(f.bin.join("reply"), serde_json::json!({"type":"result","subtype":"success","is_error":false,"total_cost_usd":0.02,"structured_output":{"questions":questions}}).to_string()).unwrap();
        f.stub(&format!(
            r#"here="${{0%/*}}"
case "$1" in
--version) echo '2.1.292 (Claude Code)';;
auth) mkdir -p "$HOME/.claude/state/wren"; chmod 700 "$HOME/.claude/state/wren"; cp "$here/runs-seed" "$HOME/.claude/state/wren/runs.jsonl"; printf '%s' '{{"daily_usd":1,"call_usd":0.5}}' > "$HOME/.claude/state/wren/limit.json"; chmod 600 "$HOME/.claude/state/wren/"*.json*; echo '{{"loggedIn":true,"authMethod":"{method}"}}';;
*) printf '%s\n' "$@" > "$here/argv"; cat > "$here/stdin"; cat "$here/reply";;
esac"#
        ));
        let out = f.run(&["interview", "draft", member, "--daily-cap-usd", "1"]);
        if method == "claude.ai" {
            assert!(out.status.success(), "{}", text(&out.stderr));
            let argv = fs::read_to_string(f.bin.join("argv")).unwrap();
            let args: Vec<_> = argv.lines().collect();
            let at = args.iter().position(|s| *s == "--max-budget-usd").unwrap();
            assert_eq!(args[at + 1], "0.50");
        } else {
            assert_eq!(
                text(&out.stderr),
                "voice: Today's spending limit for this assistant is reached: about $2.00 of $1.00, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit.\n"
            );
            assert!(!f.bin.join("argv").exists());
        }
    }
}
