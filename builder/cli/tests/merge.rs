use persona_builder::{
    crew::{NewMember, new_member},
    member::{read_member, write_member},
};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{PermissionsExt, symlink},
    },
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-merge-cli-{}-{}",
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
        Self { home, member }
    }
    fn state(&self) -> PathBuf {
        self.home.join(".claude/state/wren")
    }
    fn seed(&self) {
        fs::create_dir_all(self.state()).unwrap();
        for (name, id, split) in [
            ("decisions-history.jsonl", "h", None),
            ("decisions-interview.jsonl", "i", Some("heldout")),
            ("decisions-round-a.jsonl", "r", Some("heldout")),
        ] {
            let split = split
                .map(|s| format!(",\"split\":\"{s}\""))
                .unwrap_or_default();
            let record = format!(
                r#"{{"id":"{id}","kind":"question","timestamp":"2026","project":"kit","header":"Scope","question":"Keep sentinel small?","multiSelect":false,"options":[],"recommended":null,"wren_pick":null,"picks":[],"own_words":"","notes":"","outcome":"no_rec"{split}}}"#
            );
            fs::write(self.state().join(name), record).unwrap();
        }
        fs::write(self.state().join("decisions.jsonl"), b"not an input").unwrap();
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", std::env::var_os("PATH").unwrap())
            .current_dir(&self.home)
            .arg(command)
            .arg(&self.member)
            .args(args)
            .output()
            .unwrap()
    }
    fn refused(&self, command: &str, args: &[&str], message: &str) {
        let out = self.run(command, args);
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            out.stderr,
            format!("persona-builder: {message}\n").as_bytes()
        );
        assert!(!String::from_utf8_lossy(&out.stderr).contains("sentinel"));
        assert!(out.stdout.is_empty());
    }
    fn snapshot(&self) -> Vec<(std::ffi::OsString, Vec<u8>)> {
        let mut entries: Vec<_> = fs::read_dir(self.state())
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (e.file_name(), fs::read(e.path()).unwrap())
            })
            .collect();
        entries.sort();
        entries
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
#[test]
fn discovers_modes_prints_lines_and_writes_private_files_then_splits() {
    let f = Fixture::new();
    f.seed();
    let invalid_name = f.state().join(std::ffi::OsString::from_vec(
        b"decisions-\xff.jsonl".to_vec(),
    ));
    match fs::write(invalid_name, b"ignored") {
        Ok(()) => {}
        Err(_) => {
            eprintln!("invalid UTF-8 filename probe unavailable: filesystem refused the name");
        }
    }
    let out = f.run("merge", &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout,b"merging: decisions-history.jsonl, decisions-interview.jsonl (always training), decisions-round-a.jsonl (keeps its split)\nmerged 3 questions (2 held out) and 0 messages from 3 files; 0 duplicates dropped; held out from 2026\ndropped 0 personal records, 0 secret records\nsaved to ~/.claude/state/wren/decisions.jsonl\n");
    assert_eq!(
        fs::metadata(f.state().join("decisions.jsonl"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let out = f.run("split", &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout,b"split 2 held-out questions\nsaved to ~/.claude/state/wren/heldout-questions.jsonl and heldout-answers.jsonl\n");
    for name in ["heldout-questions.jsonl", "heldout-answers.jsonl"] {
        let path = f.state().join(name);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(path).unwrap().lines().count(), 2);
    }
}
#[test]
fn named_inputs_repeat_and_dry_runs_change_nothing() {
    let f = Fixture::new();
    f.seed();
    let before = f.snapshot();
    let out = f.run(
        "merge",
        &[
            "--in",
            "decisions-history.jsonl",
            "--in",
            "decisions-history.jsonl",
            "--train",
            "decisions-interview.jsonl",
            "--rounds",
            "decisions-round-a.jsonl",
            "--out",
            "dry.jsonl",
            "--dry-run",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("from 4 files; 1 duplicates dropped"));
    assert!(text.ends_with("dry run: nothing written\n"));
    assert_eq!(f.snapshot(), before);
    assert!(f.run("merge", &[]).status.success());
    let before = f.snapshot();
    let out = f.run(
        "split",
        &[
            "--questions",
            "dry-q.jsonl",
            "--answers",
            "dry-a.jsonl",
            "--dry-run",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        out.stdout,
        b"split 2 held-out questions\ndry run: nothing written\n"
    );
    assert_eq!(f.snapshot(), before);
}
#[test]
fn validation_and_missing_inputs_never_write() {
    let f = Fixture::new();
    let missing = "nothing to merge yet: ~/.claude/state/wren has no decisions-*.jsonl";
    f.refused("merge", &["--dry-run"], missing);
    assert!(!f.home.join(".claude/state").exists());
    fs::create_dir_all(f.state()).unwrap();
    f.refused("merge", &["--dry-run"], missing);
    assert!(fs::read_dir(f.state()).unwrap().next().is_none());
    f.seed();
    let before = f.snapshot();
    f.refused(
        "merge",
        &[
            "--in",
            "decisions-history.jsonl",
            "--out",
            "decisions-history.jsonl",
        ],
        "the output can't also be one of the inputs",
    );
    for command in ["merge", "split"] {
        for name in ["../x.jsonl", ".x.jsonl", "x.txt", "a/b.jsonl"] {
            f.refused(
                command,
                &[
                    if command == "merge" {
                        "--out"
                    } else {
                        "--corpus"
                    },
                    name,
                    "--dry-run",
                ],
                "the file name must be a plain name ending .jsonl",
            );
        }
    }
    for value in ["2", "-0.1", "NaN", "inf", "no"] {
        f.refused(
            "merge",
            &["--holdout", value],
            "--holdout must be a number from 0 to 1",
        );
    }
    assert_eq!(
        f.run("merge", &["--holdout", "0.5", "--cutoff", "2026"])
            .status
            .code(),
        Some(2)
    );
    for args in [
        ["--corpus", "same.jsonl", "--questions", "same.jsonl"],
        ["--questions", "same.jsonl", "--answers", "same.jsonl"],
        ["--corpus", "same.jsonl", "--answers", "same.jsonl"],
    ] {
        f.refused(
            "split",
            &args,
            "--corpus, --questions and --answers must be three different files",
        );
    }
    assert_eq!(f.snapshot(), before);
    let config = f.member.join("member.json");
    let mut m = read_member(&config).unwrap();
    m.state_dir = "~/elsewhere".into();
    write_member(&config, &m).unwrap();
    for command in ["merge", "split"] {
        f.refused(command,&["--dry-run"],"Persona Builder keeps your history only in ~/.claude/state/<slug>. This assistant's member.json says ~/elsewhere: change it to ~/.claude/state/<slug> and try again");
    }
}
#[test]
fn links_and_bad_corpus_leave_existing_outputs_unchanged() {
    let f = Fixture::new();
    f.seed();
    let target = f.home.join("target.jsonl");
    fs::write(&target, b"sentinel").unwrap();
    symlink(&target, f.state().join("decisions-link.jsonl")).unwrap();
    let out = f.run("merge", &["--in", "decisions-link.jsonl"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("sentinel"));
    assert_eq!(
        fs::read(f.state().join("decisions.jsonl")).unwrap(),
        b"not an input"
    );
    let bad = r#"{"kind":"question","split":"heldout","id":"sentinel","timestamp":"now","project":"kit","header":"scope","question":"safe","multiSelect":false,"options":[],"recommended":null}"#;
    fs::write(f.state().join("decisions.jsonl"), bad).unwrap();
    fs::write(f.state().join("heldout-questions.jsonl"), b"old questions").unwrap();
    fs::write(f.state().join("heldout-answers.jsonl"), b"old answers").unwrap();
    let before = f.snapshot();
    f.refused(
        "split",
        &[],
        "decisions.jsonl:1: a record has an unexpected shape (wren_pick)",
    );
    assert_eq!(f.snapshot(), before);
}
