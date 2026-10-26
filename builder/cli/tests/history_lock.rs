use persona_builder::crew::{NewMember, new_member};
use persona_builder_ingest::private::{self, lock_state, write_private};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const NO_REVIEW: &str = "there's no review file yet: run persona-builder claudeai titles first";
const NOTHING_YET: &str = "~/.claude/state/wren isn't there yet, so there's nothing to read. Start with persona-builder mine, persona-builder claudeai titles or persona-builder interview draft.";
const COMMANDS: &[(&[&str], &[&str])] = &[
    (&["mine"], &[]),
    (&["claudeai", "titles"], &["--export", "export.json"]),
    (&["claudeai", "mark"], &["--all", "--as", "y"]),
    (&["claudeai", "mine"], &["--export", "export.json"]),
    (&["merge"], &["--in", "decisions-history.jsonl"]),
    (&["split"], &[]),
    (&["chunk"], &[]),
    (&["distil-in"], &["--labels", "labels.json"]),
    (
        &["round"],
        &[
            "--name",
            "pilot",
            "--questions",
            "questions.json",
            "--answers",
            "answers.json",
        ],
    ),
    (
        &["interview", "answer"],
        &["--questions", "questions.json", "--answers", "answers.json"],
    ),
];

struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-history-lock-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let member = home.join("crew/wren");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            &member,
            &NewMember {
                name: "Wren",
                pronouns: "she/her",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-10-24",
            },
        )
        .unwrap();
        let projects = home.join(".claude/projects/projects-demo");
        fs::create_dir_all(&projects).unwrap();
        fs::write(projects.join("session.jsonl"), r#"{"type":"user","timestamp":"2026-01-01","origin":{"kind":"human"},"message":{"content":"Use the small patch."}}"#).unwrap();
        fs::write(home.join("export.json"), r#"[{"uuid":"c-1","name":"Reading list","created_at":"2026-03-01T09:00:00Z","chat_messages":[{"sender":"human","created_at":"2026-03-01T09:00:00Z","text":"Keep the reading list short."}]}]"#).unwrap();
        Self { home, member }
    }
    fn state(&self) -> PathBuf {
        self.home.join(".claude/state/wren")
    }
    fn run(&self, command: &[&str], args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env_clear()
            .env("HOME", &self.home)
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.home.join("bin").display()),
            )
            .current_dir(&self.home)
            .args(command)
            .arg(&self.member)
            .args(args)
            .output()
            .unwrap()
    }
    fn seed(&self) {
        let record = r#"{"id":"question:1","kind":"question","timestamp":"2026-01-01","project":"kit","header":"Scope","question":"Keep the queue?","multiSelect":false,"options":[{"label":"Yes","description":""},{"label":"No","description":""}],"recommended":null,"wren_pick":null,"picks":["Yes"],"own_words":"","notes":"","outcome":"no_rec","split":"train"}"#;
        for (name, text) in [
            ("decisions-history.jsonl", record),
            ("decisions.jsonl", record),
            (
                "labels.json",
                r#"{"question:1":{"category":"scope","reason":"Keep the queue small"}}"#,
            ),
            (
                "questions.json",
                r#"{"questions":[{"n":1,"header":"scope","question":"Keep the queue?","options":[{"label":"Yes"},{"label":"No"}]}]}"#,
            ),
            (
                "answers.json",
                r#"{"answered":"2026-10-24","answers":{"1":{"pick":"A"}}}"#,
            ),
        ] {
            write_private(&self.home, "wren", name, text.as_bytes()).unwrap();
        }
        // Use the same review format the titles command writes, without running that command.
        let review = persona_builder_ingest::claudeai::review_text(&[
            persona_builder_ingest::claudeai::ReviewRow {
                uuid: "c-1".into(),
                title: "Reading list".into(),
                created: "2026-03-01T09:00:00Z".into(),
                project: String::new(),
                mine: "y".into(),
                updated: String::new(),
                messages: "1".into(),
            },
        ]);
        write_private(&self.home, "wren", "claudeai-review.tsv", review.as_bytes()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn files(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push((path.clone(), fs::read(path).unwrap()));
        }
    }
    found.sort();
    found
}

fn refused(output: &Output, message: &str, command: &[&str]) {
    assert_eq!(output.status.code(), Some(1), "{command:?}");
    assert_eq!(
        output.stderr,
        format!("persona-builder: {message}\n").as_bytes(),
        "{command:?}"
    );
    assert!(output.stdout.is_empty(), "{command:?}");
}

#[test]
fn history_commands_refuse_busy_state_then_succeed_after_unlock() {
    for &(command, args) in COMMANDS {
        let f = Fixture::new();
        f.seed();
        if command[0] == "claudeai" {
            write_private(&f.home, "wren", "claudeai-review.tsv", b"junk\n").unwrap();
        }
        let before = files(&f.state());
        let lock = lock_state(&f.home, "wren").unwrap();
        refused(&f.run(command, args), private::BUSY, command);
        assert_eq!(files(&f.state()), before, "{command:?}: files changed");
        drop(lock);
        if command[0] == "claudeai" {
            f.seed();
        }
        let output = f.run(command, args);
        assert!(
            output.status.success(),
            "{command:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{command:?}");
    }
}

#[test]
fn existing_history_commands_never_recreate_missing_state() {
    for &(command, args) in &COMMANDS[2..] {
        let f = Fixture::new();
        let output = f.run(command, args);
        assert!(!f.state().exists(), "{command:?}: recreated missing state");
        let message = if command[0] == "claudeai" {
            NO_REVIEW
        } else {
            NOTHING_YET
        };
        refused(&output, message, command);
    }
}

fn failed_input_does_not_create_state(command: &[&str], args: &[&str]) {
    let f = Fixture::new();
    let output = f.run(command, args);
    assert_eq!(output.status.code(), Some(1), "{command:?}");
    assert!(!f.state().exists(), "{command:?}: created empty state");
}

#[test]
fn missing_projects_do_not_create_state() {
    failed_input_does_not_create_state(&["mine"], &["--projects", "missing-projects"]);
}

#[test]
fn missing_export_does_not_create_state() {
    failed_input_does_not_create_state(
        &["claudeai", "titles"],
        &["--export", "missing-export.json"],
    );
}

// This test's lock creates the folder. mine.rs's
// dry_run_prints_summary_without_creating_state pins that a dry run creates nothing;
// this test pins that a dry run is not refused while the lock is held.
#[test]
fn mine_dry_run_ignores_busy_state_and_writes_nothing() {
    let f = Fixture::new();
    let lock = lock_state(&f.home, "wren").unwrap();
    let output = f.run(&["mine"], &["--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .ends_with("dry run: nothing written\n")
    );
    assert!(files(&f.state()).is_empty());
    drop(lock);
}

impl Fixture {
    fn claude(&self, auth: &str) {
        let bin = self.home.join("bin");
        fs::create_dir(&bin).unwrap();
        let program = bin.join("claude");
        fs::write(
            &program,
            format!(
                r#"#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd -P)"
case "$1" in
    --version) echo '2.1.292 (Claude Code)';;
    auth) {auth}; echo '{{"loggedIn":true}}';;
    *) cat > "$here/stdin"; echo x >> "$here/calls"; cat "$here/reply";;
esac
"#
            ),
        )
        .unwrap();
        fs::set_permissions(program, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn build_refuses_history_forgotten_during_claude_check() {
    forgotten_during_check(&["build"]);
}

#[test]
fn draft_refuses_history_forgotten_during_claude_check() {
    forgotten_during_check(&["interview", "draft"]);
}

fn forgotten_during_check(command: &[&str]) {
    let f = Fixture::new();
    f.seed();
    f.claude(r#"rm -rf "$HOME/.claude/state/wren""#);
    let before = files(&f.member);
    let output = f.run(command, &[]);
    refused(&output, NOTHING_YET, command);
    assert!(!f.state().exists(), "recreated forgotten state");
    assert!(!f.home.join("bin/calls").exists(), "called Claude");
    assert_eq!(files(&f.member), before, "changed member files");
}

#[test]
fn keep_locks_before_reading_then_succeeds_after_unlock() {
    let f = Fixture::new();
    let lock = lock_state(&f.home, "wren").unwrap();
    let command = &["keep"][..];
    let args = &["--signal", "answer.json"][..];
    refused(&f.run(command, args), private::BUSY, command);
    drop(lock);
    write_private(&f.home, "wren", "answer.json", br#"{"kind":"instruction","area":"scope","situation":"Choose the next lesson to prepare.","wanted":"Prepare the short lesson first."}"#).unwrap();
    let output = f.run(command, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for name in ["signals.jsonl", "cases.jsonl"] {
        let rows = fs::read_to_string(f.state().join("growth").join(name)).unwrap();
        assert_eq!(rows.lines().count(), 1);
    }
}

#[test]
fn keep_never_creates_missing_state() {
    let f = Fixture::new();
    refused(
        &f.run(&["keep"], &["--signal", "answer.json"]),
        NOTHING_YET,
        &["keep"],
    );
    assert!(!f.state().exists());
}
