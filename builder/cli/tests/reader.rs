use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_api::{
    dto::*,
    reader::{MAX_LINE, REQUEST, Step},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "voice-reader-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        voice_api::new_assistant_in(
            &path,
            &path.join(".claude"),
            NewAssistantRequest {
                name: "Ari".into(),
                pronouns: "they/them".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        voice_api::agree_consent_in(&path, SlugRequest { slug: "ari".into() }).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str], input: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_voice"))
            .env_clear()
            .env("HOME", &self.0)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn lines(output: &Output) -> Vec<Value> {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.ends_with(b"\n"));
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(|line| {
            assert!(line.len() <= MAX_LINE);
            let value: Value = serde_json::from_str(line).unwrap();
            assert_eq!(value.as_object().unwrap().len(), 1);
            value
        })
        .collect()
}
#[test]
fn hidden_in_help_but_has_its_own_help() {
    let home = Home::new();
    let output = home.run(&["--help"], b"");
    assert!(output.status.success());
    assert!(!String::from_utf8(output.stdout).unwrap().contains("reader"));
    let output = home.run(&["reader", "--help"], b"");
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Read for the Voice app, inside its sandbox (not for use in a terminal)")
    );
}
#[test]
fn accepts_every_step_and_refuses_unknown_steps() {
    let home = Home::new();
    for step in Step::ALL {
        let output = home.run(&["reader", step.name()], b"");
        assert_eq!(output.status.code(), Some(1), "{}", step.name());
        assert_eq!(lines(&output), vec![json!({"error": REQUEST})]);
    }
    let output = home.run(&["reader", "unknown"], b"");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
#[test]
fn history_request_streams_progress_then_result() {
    let home = Home::new();
    let folder = home.0.join(".claude/projects/project-one");
    fs::create_dir_all(&folder).unwrap();
    let record = json!({"type":"user","timestamp":"2026-01-03T10:00:00.000Z","origin":{"kind":"human"},"message":{"content":"Start with the smallest useful change."}});
    fs::write(folder.join("session.jsonl"), format!("{record}\n")).unwrap();
    let request = ReadRequest {
        slug: "ari".into(),
        dry_run: false,
    };
    let expected = voice_api::read_claude_code_in(&home.0, request.clone(), &mut |_| {}).unwrap();
    let output = home.run(
        &["reader", "read-claude-code"],
        &serde_json::to_vec(&request).unwrap(),
    );
    assert!(output.status.success());
    let lines = lines(&output);
    assert!(lines.len() >= 2);
    assert!(
        lines[..lines.len() - 1]
            .iter()
            .all(|line| line.get("progress").is_some())
    );
    assert_eq!(lines.last().unwrap(), &json!({"result": expected}));
}
#[test]
fn missing_or_empty_home_is_a_protocol_error() {
    for empty in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_voice"));
        command.env_clear();
        if empty {
            command.env("HOME", "");
        }
        let output = command
            .args(["reader", "skills"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            lines(&output),
            vec![
                json!({"error": "Voice couldn't find that assistant. Close it and open it again."})
            ]
        );
    }
}
