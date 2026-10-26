use persona_builder::crew::{NewMember, new_member};
use persona_builder_ingest::private::{self, lock_state, write_private};
use std::{
    fs,
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
            .env("PATH", std::env::var_os("PATH").unwrap())
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
        let before = files(&f.state());
        let lock = lock_state(&f.home, "wren").unwrap();
        refused(&f.run(command, args), private::BUSY, command);
        assert_eq!(files(&f.state()), before, "{command:?}: files changed");
        drop(lock);
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
