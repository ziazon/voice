use persona_builder::crew::{NewMember, new_member};
use persona_builder_agent::ledger::{Cap, Session};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

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
                decisions: persona_builder::crew::Decisions::Python,
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
        fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_persona-builder"));
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
        "claude-code  Claude, through your own Claude Code\n"
    );
}

#[test]
fn claude_check_says_ready_or_what_to_do() {
    let f = Fixture::new();
    f.stub(
        "case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":true,\"email\":\"someone@example.invalid\"}';; esac",
    );
    let out = f.run(&["claude", "check"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Claude Code 2.1.292 is installed and signed in.\n"
    );
    assert!(!text(&out.stdout).contains("example.invalid"));
    let out = f
        .command(&["claude", "check"])
        .env("ANTHROPIC_API_KEY", "not-a-real-key")
        .output()
        .unwrap();
    assert_eq!(
        text(&out.stdout),
        format!(
            "Claude Code 2.1.292 is installed and signed in.\n{}\n",
            persona_builder_agent::text::API_KEY_SET
        )
    );
    assert!(!text(&out.stdout).contains("not-a-real-key"));

    f.stub("case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":false}';; esac");
    let out = f.run(&["claude", "check"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        format!(
            "persona-builder: {}\n",
            persona_builder_agent::text::SIGNED_OUT
        )
    );
}

fn day_of(home: &Path) -> String {
    let _ = home;
    persona_builder_agent::ledger::local_day(SystemTime::now())
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
    let earlier = persona_builder_agent::ledger::local_day(
        UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000),
    );
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
    assert_eq!(out.status.code(), Some(1));

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
        "0",
        "--step-usd",
        "0",
    ]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("the daily limit must be at least $0.01"));
    assert!(text(&out.stderr).contains("no more than the daily limit, both in whole cents"));
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
    assert!(stderr.contains("This assistant has no private folder yet, so there's nowhere to keep its limit. Run `persona-builder consent agree` first."), "{stderr}");
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
    assert!(text(&out.stderr).contains("no more than the daily limit"));
    assert!(!f.state().exists());
}
