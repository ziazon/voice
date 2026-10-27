//! Record invented decisions only in the assistant's existing private state.
use persona_builder::crew::{Decisions, NewMember, new_member};
use persona_builder_ingest::{mine::to_jsonl, private};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output, Stdio},
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
    fn stdin(&self, args: &[&str], input: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", &self.home)
            .arg("record")
            .arg(&self.member)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // A usage error can close stdin before it has been written.
        let _ = child.stdin.take().unwrap().write_all(input);
        child.wait_with_output().unwrap()
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

const HOSTILE: &str = "$(touch pwned-dollar) `touch pwned-tick` ${HOME} \"double\" 'single' back\\slash café\nEND_OF_TEXT\nend";

fn stdin_row() -> Value {
    json!({"work": HOSTILE, "kind": "decided", "category": HOSTILE,
        "tier": "high", "question": HOSTILE, "pick": HOSTILE})
}

fn without_timestamp(line: &str) -> String {
    let row: Value = serde_json::from_str(line).unwrap();
    line.replace(row["timestamp"].as_str().unwrap(), "TIMESTAMP")
}

#[test]
fn stdin_records_match_flags_and_python_bytes() {
    if Command::new("python3").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("PERSONA_BUILDER_REQUIRE_PARITY").as_deref(),
            Ok("1"),
            "python3 required for parity"
        );
        eprintln!("skipped: python3 not found");
        return;
    }
    for optional in [false, true] {
        let f = Fixture::new();
        f.seed();
        let mut input = stdin_row();
        let mut flags = vec![
            "--work",
            HOSTILE,
            "--kind",
            "decided",
            "--category",
            HOSTILE,
            "--tier",
            "high",
            "--question",
            HOSTILE,
            "--ari-pick",
            HOSTILE,
        ];
        if optional {
            input["rule"] = json!(HOSTILE);
            input["answer"] = json!(HOSTILE);
            flags.extend(["--rule", HOSTILE, "--robin-answer", HOSTILE]);
        }
        let stdin = f.stdin(&["--stdin"], input.to_string().as_bytes());
        success(&stdin);
        let first = fs::read_to_string(f.log()).unwrap();
        let flag = f.run(&flags);
        success(&flag);
        assert_eq!(stdin.stdout, flag.stdout);
        let both = fs::read_to_string(f.log()).unwrap();
        assert_eq!(
            without_timestamp(&first),
            without_timestamp(&format!("{}\n", both.lines().nth(1).unwrap()))
        );
        let python_log = f.home.join("python-stdin.jsonl");
        let mut child = Command::new("python3")
            .arg("-I")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/crew_decisions.py"))
            .arg("--member")
            .arg(f.member.join("member.json"))
            .arg("record")
            .arg("--log")
            .arg(&python_log)
            .arg("--stdin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let _ = child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes());
        let python = child.wait_with_output().unwrap();
        success(&python);
        assert_eq!(stdin.stdout, python.stdout);
        assert_eq!(
            without_timestamp(&first),
            without_timestamp(&fs::read_to_string(python_log).unwrap())
        );
        let row: Value = serde_json::from_str(&first).unwrap();
        for key in ["work", "question", "category", "ari_pick"] {
            assert_eq!(row[key], HOSTILE);
        }
        assert_eq!(
            row["rule"],
            if optional {
                json!(HOSTILE)
            } else {
                Value::Null
            }
        );
        assert_eq!(
            row["robin_answer"],
            if optional {
                json!(HOSTILE)
            } else {
                Value::Null
            }
        );
    }
}

#[test]
fn stdin_record_refusals_are_usage_errors_and_do_not_write() {
    let valid = stdin_row();
    let mut cases: Vec<(Vec<&str>, Vec<u8>, &str)> = vec![
        (
            vec!["--stdin", "--work", "kit"],
            valid.to_string().into_bytes(),
            "--work",
        ),
        (vec![], vec![], "required"),
        (vec!["--stdin"], b"not JSON".to_vec(), "one JSON object"),
        (vec!["--stdin"], b"[]".to_vec(), "one JSON object"),
        (vec!["--stdin"], vec![b'x'; 65537], "at most 64 KiB"),
        (vec!["--stdin"], vec![0xff], "UTF-8"),
        (
            vec!["--stdin"],
            br#"{"work":"kit","kind":"decided","category":"scope","tier":"high","question":"Keep it small?","pick":"\ud800"}"#.to_vec(),
            "pick",
        ),
        (
            vec!["--stdin"],
            b"{\"pick\":\"one\",\"pick\":\"two\"}".to_vec(),
            "pick",
        ),
    ];
    for (key, value) in [
        ("extra", json!("bad")),
        ("pick", json!(42)),
        ("rule", json!(42)),
        ("answer", json!(42)),
        ("kind", json!("maybe")),
        ("tier", json!("top")),
    ] {
        let mut row = valid.clone();
        row[key] = value;
        cases.push((vec!["--stdin"], row.to_string().into_bytes(), key));
    }
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("pick");
    cases.push((vec!["--stdin"], missing.to_string().into_bytes(), "pick"));
    for existing in [false, true] {
        for (flags, input, problem) in &cases {
            let f = Fixture::new();
            f.seed();
            if existing {
                private::write_private(&f.home, "ari", "live.jsonl", b"existing\n").unwrap();
            }
            let out = f.stdin(flags, input);
            assert_eq!(
                out.status.code(),
                Some(2),
                "{problem}: {}",
                text(&out.stderr)
            );
            assert!(
                text(&out.stderr).contains(problem),
                "{problem}: {}",
                text(&out.stderr)
            );
            if existing {
                assert_eq!(fs::read(f.log()).unwrap(), b"existing\n");
            } else {
                assert!(!f.log().exists());
            }
        }
    }
}

#[test]
fn stdin_record_accepts_null_optional_fields() {
    let f = Fixture::new();
    f.seed();
    let mut input = stdin_row();
    input["rule"] = Value::Null;
    input["answer"] = Value::Null;
    success(&f.stdin(&["--stdin"], input.to_string().as_bytes()));
    let row: Value = serde_json::from_slice(&fs::read(f.log()).unwrap()).unwrap();
    assert!(row["rule"].is_null());
    assert!(row["robin_answer"].is_null());
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
        (None, "kit", "Garder le café \"petit\"?"),
        (
            Some(("Le café d’abord.", "Oui, merci.")),
            "kit",
            "Garder le café \"petit\"?",
        ),
        (None, "- fix login", "-5 or 5?"),
        (None, "-x y", "short dash with a space"),
        (Some(("-r x", "-a b")), "- fix login", "-5 or 5?"),
        (None, "-5", "negative integer"),
        (None, "-", "single dash"),
        (None, "--kind", "explicit equals"),
        (None, "--rule=keep it short", "explicit equals with a space"),
        (None, "-.5", "negative fraction"),
        (None, "-5.25", "negative decimal"),
    ] {
        let f = Fixture::new();
        f.seed();
        let mut arguments = args();
        arguments[1] = work;
        arguments[9] = question;
        if let Some((rule, answer)) = optional {
            arguments.extend(["--rule", rule, "--robin-answer", answer]);
        }
        if work == "--kind" {
            arguments.splice(0..2, ["--work=--kind"]);
            // Exercise several joined tokens before and after separate values.
            arguments.splice(1..3, ["--kind=decided"]);
            arguments.extend(["--rule=--rule", "--robin-answer=--work"]);
        } else if work == "--rule=keep it short" {
            arguments.splice(0..2, ["--work=--rule=keep it short"]);
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
        if optional.is_none() && work != "--kind" {
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

#[test]
fn rule_without_value_before_answer() {
    let f = Fixture::new();
    f.seed();
    let mut arguments = args();
    arguments.extend(["--rule", "--robin-answer"]);
    let out = f.run(&arguments);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("error:"),
        "{}",
        text(&out.stderr)
    );
    assert!(!f.log().exists());
}

#[test]
fn pick_without_value_before_rule() {
    let f = Fixture::new();
    f.seed();
    let mut arguments = args();
    arguments.truncate(11);
    arguments.push("--rule");
    let out = f.run(&arguments);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("error:"),
        "{}",
        text(&out.stderr)
    );
    assert!(!f.log().exists());
}

#[test]
fn work_without_value_before_two_dashes_with_spaces() {
    for value in ["--rule=keep it short", "--ques=what now?", "--x y"] {
        let f = Fixture::new();
        f.seed();
        let mut arguments = args();
        arguments.drain(0..2);
        arguments.extend(["--work", value]);
        let out = f.run(&arguments);
        assert_eq!(out.status.code(), Some(2), "{value}: {}", text(&out.stderr));
        assert!(!f.log().exists(), "{value}: wrote a record");
    }
}

#[test]
fn work_without_value_before_help() {
    let f = Fixture::new();
    f.seed();
    let mut arguments = args();
    arguments.drain(0..2);
    arguments.extend(["--work", "--help"]);
    let out = f.run(&arguments);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("error:"),
        "{}",
        text(&out.stderr)
    );
    assert!(!f.log().exists());
}

#[test]
fn work_without_value_before_separator() {
    let f = Fixture::new();
    f.seed();
    let mut arguments = args();
    arguments.drain(0..2);
    arguments.extend(["--work", "--"]);
    let out = f.run(&arguments);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("error:"),
        "{}",
        text(&out.stderr)
    );
    assert!(!f.log().exists());
}

#[test]
fn work_without_value_before_unknown_short_flag() {
    let f = Fixture::new();
    f.seed();
    let mut arguments = args();
    arguments.drain(0..2);
    arguments.extend(["--work", "-x"]);
    let out = f.run(&arguments);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("error:"),
        "{}",
        text(&out.stderr)
    );
    assert!(!f.log().exists());
}

#[test]
fn record_long_help_without_member() {
    let f = Fixture::new();
    let out = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", &f.home)
        .args(["record", "--help"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    for expected in [
        "Keep one decision the assistant made, or your answer to one it handed back.",
        "--<assistant>-pick",
        "--<you>-answer",
        "--work",
        "--kind",
        "--category",
        "--tier",
        "--question",
        "--rule",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    assert!(!f.state().exists());
}

#[test]
fn record_short_help_without_member() {
    let f = Fixture::new();
    let out = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", &f.home)
        .args(["record", "-h"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    for expected in [
        "Keep one decision the assistant made, or your answer to one it handed back.",
        "--<assistant>-pick",
        "--<you>-answer",
        "--work",
        "--kind",
        "--category",
        "--tier",
        "--question",
        "--rule",
    ] {
        assert!(stdout.contains(expected), "missing {expected}: {stdout}");
    }
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
    assert!(!f.state().exists());
}
