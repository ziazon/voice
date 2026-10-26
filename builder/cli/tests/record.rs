//! Record invented decisions only in the assistant's existing private state.
use persona_builder::crew::{Decisions, NewMember, new_member};
use persona_builder_ingest::{mine::to_jsonl, private};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const TOO_LONG: &str = "That is too long to keep (over 64 KiB). Shorten it, then record it again.";
struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-record-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let member = home.join("crew/ari");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            &member,
            &NewMember {
                decisions: Decisions::Python,
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-10-26",
            },
        )
        .unwrap();
        Self { home, member }
    }
    fn state(&self) -> PathBuf {
        self.home.join(".claude/state/ari")
    }
    fn seed(&self) {
        drop(private::lock_state(&self.home, "ari").unwrap());
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", &self.home)
            .arg("record")
            .arg(&self.member)
            .args(args)
            .output()
            .unwrap()
    }
    fn log(&self) -> PathBuf {
        self.state().join("live.jsonl")
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
fn args() -> Vec<&'static str> {
    vec![
        "--work",
        "kit",
        "--kind",
        "decided",
        "--category",
        "scope",
        "--tier",
        "high",
        "--question",
        "Garder le café \"petit\"?",
        "--ari-pick",
        "Oui, café.",
    ]
}
fn success(out: &Output) {
    assert!(out.status.success(), "{}", text(&out.stderr));
}

#[test]
fn records_match_python_bytes_with_and_without_optional_fields() {
    if Command::new("python3").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("PERSONA_BUILDER_REQUIRE_PARITY").as_deref(),
            Ok("1"),
            "python3 required for parity"
        );
        eprintln!("skipped: python3 not found");
        return;
    }
    for (optional, work, question) in [
        (false, "kit", "Garder le café \"petit\"?"),
        (true, "kit", "Garder le café \"petit\"?"),
        (false, "- fix login", "-5 or 5?"),
    ] {
        let f = Fixture::new();
        f.seed();
        let mut arguments = args();
        arguments[1] = work;
        arguments[9] = question;
        if optional {
            arguments.extend([
                "--rule",
                "Le café d’abord.",
                "--robin-answer",
                "Oui, merci.",
            ]);
        }
        let out = f.run(&arguments);
        success(&out);
        assert_eq!(
            text(&out.stdout),
            format!("recorded a decided scope decision (high) for {work}\n")
        );
        let python_log = f.home.join("python.jsonl");
        let python = Command::new("python3")
            .arg("-I")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/crew_decisions.py"))
            .arg("--member")
            .arg(f.member.join("member.json"))
            .arg("record")
            .arg("--log")
            .arg(&python_log)
            .args(&arguments)
            .output()
            .unwrap();
        success(&python);
        let rust = fs::read_to_string(f.log()).unwrap();
        let python = fs::read_to_string(python_log).unwrap();
        let row: Value = serde_json::from_str(&rust).unwrap();
        let timestamp = row["timestamp"].as_str().unwrap();
        assert_eq!(timestamp.len(), 20);
        assert_eq!(&timestamp[10..11], "T");
        assert!(timestamp.ends_with('Z'));
        let python_row: Value = serde_json::from_str(&python).unwrap();
        assert_eq!(
            rust.replace(timestamp, "TIMESTAMP"),
            python.replace(python_row["timestamp"].as_str().unwrap(), "TIMESTAMP")
        );
        if !optional {
            assert!(row["rule"].is_null());
            assert!(row["robin_answer"].is_null());
        }
        assert_eq!(
            fs::metadata(f.log()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn help_shows_member_flags_without_writing() {
    let f = Fixture::new();
    f.seed();
    for help in ["--help", "-h"] {
        let out = f.run(&[help]);
        success(&out);
        let stdout = text(&out.stdout);
        for expected in [
            "--ari-pick",
            "--robin-answer",
            "Keep one decision the assistant made, or your answer to one it handed back.",
            "--work",
            "--kind",
            "--category",
            "--tier",
            "--question",
            "--rule",
        ] {
            assert!(stdout.contains(expected), "missing {expected}: {stdout}");
        }
        assert!(!f.log().exists());
    }
}
#[test]
fn appends_without_replacing_existing_lines() {
    let f = Fixture::new();
    f.seed();
    private::write_private(&f.home, "ari", "live.jsonl", b"existing\n").unwrap();
    for _ in 0..2 {
        success(&f.run(&args()));
    }
    let log = fs::read_to_string(f.log()).unwrap();
    assert!(log.starts_with("existing\n"));
    assert_eq!(log.lines().count(), 3);
}
#[test]
fn missing_state_is_not_recreated() {
    let f = Fixture::new();
    let out = f.run(&args());
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("Ari has nowhere to keep this yet: ~/.claude/state/ari isn't there. Open Persona Builder and give Ari permission, then try again."), "{}", text(&out.stderr));
    assert!(!f.state().exists());
}
#[test]
fn unsafe_log_targets_are_refused_unchanged() {
    for kind in ["symlink", "directory", "readable"] {
        let f = Fixture::new();
        f.seed();
        let target = f.home.join("target");
        fs::write(&target, b"untouched").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        match kind {
            "symlink" => symlink(&target, f.log()).unwrap(),
            "directory" => fs::create_dir(f.log()).unwrap(),
            _ => {
                fs::write(f.log(), b"untouched").unwrap();
                fs::set_permissions(f.log(), fs::Permissions::from_mode(0o644)).unwrap();
            }
        }
        let out = f.run(&args());
        assert_eq!(out.status.code(), Some(1), "{kind}: {}", text(&out.stderr));
        assert_eq!(fs::read(target).unwrap(), b"untouched");
        if kind == "directory" {
            assert_eq!(fs::read_dir(f.log()).unwrap().count(), 0);
        } else {
            assert_eq!(fs::read(f.log()).unwrap(), b"untouched");
        }
    }
}
#[test]
fn member_flags_and_choices_are_usage_errors_without_writing() {
    for change in [
        "kind",
        "tier",
        "missing-pick",
        "other-pick",
        "log",
        "unknown",
    ] {
        let f = Fixture::new();
        f.seed();
        let mut arguments = args();
        match change {
            "kind" => arguments[3] = "other",
            "tier" => arguments[7] = "other",
            "missing-pick" => {
                arguments.truncate(10);
            }
            "other-pick" => arguments[10] = "--robin-pick",
            "unknown" => arguments.extend(["--nope", "x"]),
            _ => arguments.extend(["--log", "elsewhere.jsonl"]),
        }
        let out = f.run(&arguments);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{change}: {}",
            text(&out.stderr)
        );
        assert!(!f.log().exists());
    }
}
#[test]
fn exact_line_size_limit_is_checked_before_appending() {
    let f = Fixture::new();
    f.seed();
    let row = json!({"timestamp":"2026-10-26T12:00:00Z", "work":"kit", "kind":"decided", "category":"scope", "tier":"high", "question":"", "ari_pick":"Oui, café.", "rule":null, "robin_answer":null});
    let overhead = to_jsonl(&[row.as_object().unwrap().clone()]).len();
    let mut arguments = args();
    let accepted = "x".repeat(64 * 1024 - overhead);
    arguments[9] = &accepted;
    success(&f.run(&arguments));
    let before = fs::read(f.log()).unwrap();
    assert_eq!(before.len(), 64 * 1024);
    let oversized = format!("{accepted}x");
    arguments[9] = &oversized;
    let out = f.run(&arguments);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stderr).contains(TOO_LONG),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(fs::read(f.log()).unwrap(), before);
}
#[test]
fn records_while_another_process_holds_the_state_lock() {
    let f = Fixture::new();
    let _lock = private::lock_state(&f.home, "ari").unwrap();
    // The CLI is a separate process from this lock holder.
    success(&f.run(&args()));
    assert_eq!(fs::read_to_string(f.log()).unwrap().lines().count(), 1);
}
#[test]
fn refuses_member_state_outside_private_folder() {
    let f = Fixture::new();
    f.seed();
    let path = f.member.join("member.json");
    let mut member: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    member["state_dir"] = json!("~/elsewhere");
    fs::write(path, member.to_string()).unwrap();
    let out = f.run(&args());
    assert_eq!(out.status.code(), Some(1));
    assert!(!f.log().exists());
    assert!(!f.home.join("elsewhere").exists());
}
