//! Keep fictional Robin's answers privately, and count their tests without printing their words.
use persona_builder::crew::{NewMember, new_member};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const KEPT: &str =
    "Kept your answer. It's now one of the tests your assistant is checked on at its next refresh.";
const NO_CASES: &str = "You haven't given your assistant any answers to learn from yet.";

struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-keep-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let member = home.join("crew/robin");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            &member,
            &NewMember {
                name: "Robin",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-03-04",
            },
        )
        .unwrap();
        Self { home, member }
    }
    fn state(&self) -> PathBuf {
        self.home.join(".claude/state/robin")
    }
    fn write(&self, name: &str, value: &Value) {
        let path = self.state().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::set_permissions(self.state(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&path, value.to_string()).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", &self.home)
            .arg(command)
            .arg(&self.member)
            .args(args)
            .output()
            .unwrap()
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
fn answer(area: &str) -> Value {
    json!({"kind":"instruction", "area":area,
        "situation":"Choose the next lesson to prepare.",
        "wanted":"Prepare the short lesson first. SENTINEL"})
}
fn lines(f: &Fixture, file: &str) -> Vec<Value> {
    fs::read_to_string(f.state().join("growth").join(file))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn keep_saves_a_signal_and_case_privately_and_prints_where() {
    let f = Fixture::new();
    f.write("answer.json", &answer("scope"));
    let out = f.run("keep", &["--signal", "answer.json"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        format!("{KEPT}\nsaved to ~/.claude/state/robin/growth/\n")
    );
    let growth = f.state().join("growth");
    assert_eq!(
        fs::metadata(&growth).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in ["signals.jsonl", "cases.jsonl"] {
        assert_eq!(
            fs::metadata(growth.join(file))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let signals = lines(&f, "signals.jsonl");
    let cases = lines(&f, "cases.jsonl");
    assert_eq!(signals.len(), 1);
    assert_eq!(cases.len(), 1);
    assert_eq!(signals[0]["persona"], "robin");
    assert_eq!(signals[0]["wanted"], answer("scope")["wanted"]);
    assert_eq!(cases[0]["from_signals"], json!([signals[0]["id"]]));
    assert_eq!(cases[0]["expected"], signals[0]["wanted"]);
    assert_eq!(cases[0]["created"], signals[0]["at"]);
    assert!(!text(&out.stdout).contains("SENTINEL"));
    assert!(!f.member.join("growth").exists());
}

#[test]
fn keep_reads_a_json_file_from_a_private_subfolder() {
    let f = Fixture::new();
    f.write("inbox/answer.json", &answer("scope"));
    let out = f.run("keep", &["--signal", "inbox/answer.json"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(lines(&f, "signals.jsonl").len(), 1);
}

#[test]
fn refused_answer_exits_nonzero_writes_nothing_and_hides_its_words() {
    let f = Fixture::new();
    f.write("answer.json", &answer("unknown-area"));
    let out = f.run("keep", &["--signal", "answer.json"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(!text(&out.stderr).contains("SENTINEL"));
    assert!(text(&out.stdout).is_empty());
    assert!(!f.state().join("growth/signals.jsonl").exists());
    assert!(!f.state().join("growth/cases.jsonl").exists());
}

#[test]
fn keep_refuses_non_json_traversal_and_linked_input() {
    for path in [
        "answer.txt",
        "answer.jsonl",
        "../answer.json",
        "linked.json",
    ] {
        let f = Fixture::new();
        f.write("answer.json", &answer("scope"));
        fs::copy(f.state().join("answer.json"), f.state().join("answer.txt")).unwrap();
        fs::copy(
            f.state().join("answer.json"),
            f.state().join("answer.jsonl"),
        )
        .unwrap();
        symlink(f.state().join("answer.json"), f.state().join("linked.json")).unwrap();
        let out = f.run("keep", &["--signal", path]);
        assert_eq!(out.status.code(), Some(1), "{path}: {}", text(&out.stderr));
        assert!(!f.state().join("growth/signals.jsonl").exists());
    }
}

#[test]
fn cases_shows_none_then_counts_two_answers_in_category_order_without_their_words() {
    let f = Fixture::new();
    let out = f.run("cases", &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), format!("{NO_CASES}\n"));
    // Reverse entry order proves the summary follows the member's categories.
    for area in ["sequencing", "scope"] {
        f.write("answer.json", &answer(area));
        let out = f.run("keep", &["--signal", "answer.json"]);
        assert!(out.status.success(), "{}", text(&out.stderr));
    }
    let out = f.run("cases", &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert_eq!(shown.lines().count(), 3, "{shown}");
    assert!(
        shown.starts_with(
            "Tests made from your answers: 2 waiting for a refresh, 0 in use, 0 retired.\n"
        ),
        "{shown}"
    );
    assert!(
        shown.lines().nth(1).unwrap().starts_with("scope:"),
        "{shown}"
    );
    assert!(
        shown.lines().nth(2).unwrap().starts_with("sequencing:"),
        "{shown}"
    );
    assert!(!shown.contains("SENTINEL"));
    assert!(!shown.contains("Prepare the short lesson"));
    assert_eq!(lines(&f, "signals.jsonl").len(), 2);
    assert_eq!(lines(&f, "cases.jsonl").len(), 2);
}

#[test]
fn help_lists_the_new_commands_in_plain_words() {
    let out = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    let shown = text(&out.stdout);
    assert!(
        shown.contains("Keep an answer you gave your assistant, so it learns from it"),
        "{shown}"
    );
    assert!(
        shown.contains("Show how many tests your answers have made"),
        "{shown}"
    );
}
