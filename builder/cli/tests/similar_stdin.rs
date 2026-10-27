//! Stdin texts stay data and agree with the guided build's command.
use persona_builder::crew::{Decisions, NewMember, new_member};
use persona_builder_ingest::private;
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const HOSTILE: &str = "$(touch pwned-dollar) `touch pwned-tick` ${HOME} \"double\" 'single' back\\slash café\nEND_OF_TEXT\nafter";
struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-similar-stdin-{}-{}",
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
        drop(private::lock_state(&home, "ari").unwrap());
        private::write_private(&home, "ari", "decisions.jsonl", b"{\"kind\":\"message\",\"split\":\"train\",\"timestamp\":\"2026-10-25T00:00:00Z\",\"project\":\"garden\",\"text\":\"Choose durable garden benches before adding decorative flower pots outdoors.\"}\n").unwrap();
        Self { home, member }
    }
    fn rust(&self, args: &[&str], input: &[u8]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_persona-builder"));
        command
            .env("HOME", &self.home)
            .arg("similar")
            .arg(&self.member)
            .args(args);
        run(&mut command, input)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
fn run(command: &mut Command, input: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn stdin_matches_query_flags_with_and_without_json() {
    let f = Fixture::new();
    let query = format!("durable garden benches {HOSTILE}");
    let mut input = json!({"query": query}).to_string();
    // Leading and trailing whitespace count toward the byte cap and remain valid.
    input.insert(0, ' ');
    input.push_str(&" ".repeat(65536 - input.len()));
    for json_output in [false, true] {
        let mut stdin_args = vec!["--stdin"];
        let mut flags = vec!["--query", &query];
        if json_output {
            stdin_args.push("--json");
            flags.push("--json");
        }
        let stdin = f.rust(&stdin_args, input.as_bytes());
        success(&stdin);
        let flags = f.rust(&flags, b"");
        success(&flags);
        assert_eq!(stdin.stdout, flags.stdout);
        assert!(String::from_utf8_lossy(&stdin.stdout).contains("Choose durable garden benches"));
    }
}

#[test]
fn stdin_matches_python_with_and_without_json() {
    if Command::new("python3").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("PERSONA_BUILDER_REQUIRE_PARITY").as_deref(),
            Ok("1"),
            "python3 required for parity"
        );
        eprintln!("skipped: python3 not found");
        return;
    }
    let f = Fixture::new();
    let input = json!({"query": format!("durable garden benches {HOSTILE}")}).to_string();
    for json_output in [false, true] {
        let mut args = vec!["--stdin"];
        if json_output {
            args.push("--json");
        }
        let rust = f.rust(&args, input.as_bytes());
        success(&rust);
        let mut command = Command::new("python3");
        command
            .env("HOME", &f.home)
            .arg("-I")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/crew_decisions.py"))
            .arg("--member")
            .arg(f.member.join("member.json"))
            .arg("similar")
            .args(&args);
        let python = run(&mut command, input.as_bytes());
        success(&python);
        assert_eq!(rust.stdout, python.stdout);
    }
}

#[test]
fn invalid_stdin_is_a_usage_error_before_searching() {
    let f = Fixture::new();
    fs::remove_file(f.home.join(".claude/state/ari/decisions.jsonl")).unwrap();
    for (input, problem) in [
        (b"not JSON".to_vec(), "one JSON object"),
        (b"[]".to_vec(), "one JSON object"),
        (br#"["x"]"#.to_vec(), "one JSON object"),
        (br#"{"query":"x","extra":"x"}"#.to_vec(), "extra"),
        (br#"{"query":"x","query":"y"}"#.to_vec(), "query"),
        (b"{}".to_vec(), "query"),
        (br#"{"query":"x"} {}"#.to_vec(), "one JSON object"),
        (br#"{"query":4}"#.to_vec(), "query"),
        (br#"{"query":"\ud800"}"#.to_vec(), "query"),
        (vec![b'x'; 65537], "at most 64 KiB"),
        (vec![0xff], "UTF-8"),
    ] {
        let out = f.rust(&["--stdin"], &input);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(problem), "missing {problem}: {stderr}");
        assert!(
            stderr.contains("Usage: persona-builder similar"),
            "{stderr}"
        );
        assert!(out.stdout.is_empty());
    }
    for args in [vec![], vec!["--stdin", "--query", "x"]] {
        let out = f.rust(&args, b"{\"query\":\"x\"}");
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    assert!(!f.home.join(".claude/state/ari/live.jsonl").exists());
}
