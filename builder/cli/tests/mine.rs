use persona_builder::{
    crew::{NewMember, new_member},
    member::{read_member, write_member},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const TEXT: &str = "Use the small patch.";
const SUMMARY: &str = "mined 0 questions (0 held out) and 1 messages, 2026-01-01 → 2026-01-01; held out from nothing\ndropped 0 excluded folders, 0 personal records, 0 secret records\n";
struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-mine-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
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
        let folder = home.join(".claude/projects/projects-demo");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("session.jsonl"), format!(r#"{{"type":"user","timestamp":"2026-01-01","origin":{{"kind":"human"}},"message":{{"content":"{TEXT}"}}}}"#)).unwrap();
        Self { home, member }
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
    fn refused(&self, args: &[&str], message: &str) {
        let output = self.run("mine", args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            output.stderr,
            format!("persona-builder: {message}\n").as_bytes()
        );
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(TEXT));
        assert!(!self.home.join(".claude/state").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
#[test]
fn dry_run_prints_summary_without_creating_state() {
    let fixture = Fixture::new();
    let output = fixture.run("mine", &["--dry-run"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        output.stdout,
        format!("{SUMMARY}dry run: nothing written\n").as_bytes()
    );
    assert!(output.stderr.is_empty());
    assert!(!fixture.home.join(".claude/state").exists());
}
#[test]
fn mine_writes_private_jsonl_and_prints_saved_path() {
    let fixture = Fixture::new();
    let output = fixture.run("mine", &[]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        output.stdout,
        format!("{SUMMARY}saved to ~/.claude/state/wren/decisions-claude-code.jsonl\n").as_bytes()
    );
    assert!(output.stderr.is_empty());
    let folder = fixture.home.join(".claude/state/wren");
    let file = folder.join("decisions-claude-code.jsonl");
    assert!(fs::read_to_string(&file).unwrap().contains(TEXT));
    assert_eq!(
        fs::metadata(file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(folder).unwrap().permissions().mode() & 0o777,
        0o700
    );
}
#[test]
fn state_elsewhere_is_refused_even_in_dry_run() {
    let fixture = Fixture::new();
    let config = fixture.member.join("member.json");
    let mut member = read_member(&config).unwrap();
    member.state_dir = "~/elsewhere".into();
    write_member(&config, &member).unwrap();
    fixture.refused(&["--dry-run"], "Persona Builder keeps your history only in ~/.claude/state/<slug>. This assistant's member.json says ~/elsewhere: change it to ~/.claude/state/<slug> and try again");
    assert!(!fixture.home.join("elsewhere").exists());
}
#[test]
fn output_name_is_validated_even_in_dry_run() {
    let fixture = Fixture::new();
    for name in ["../x.jsonl", ".x.jsonl", "x.txt", "a/x.jsonl"] {
        fixture.refused(
            &["--dry-run", "--out", name],
            "the file name must be a plain name ending .jsonl",
        );
    }
}
#[test]
fn holdout_must_be_a_finite_fraction() {
    let fixture = Fixture::new();
    for value in ["2", "-0.1", "NaN", "inf", "no"] {
        fixture.refused(
            &["--holdout", value],
            "--holdout must be a number from 0 to 1",
        );
    }
}
#[test]
fn split_options_conflict_is_a_usage_error() {
    let fixture = Fixture::new();
    let output = fixture.run("mine", &["--holdout", "0.5", "--cutoff", "2026"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(TEXT));
    assert!(!fixture.home.join(".claude/state").exists());
}
#[test]
fn folders_prints_rows_and_totals_and_expands_home() {
    let fixture = Fixture::new();
    let output = fixture.run("folders", &["--projects", "~/.claude/projects"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        output.stdout,
        b"mined\t1\tdemo\tprojects-demo\n1 mined folders (1 sessions), 0 excluded (0 sessions)\n"
    );
    assert!(output.stderr.is_empty());
}
#[test]
fn missing_history_has_a_plain_message() {
    let fixture = Fixture::new();
    for command in ["mine", "folders"] {
        let output = fixture.run(command, &["--projects", "~/missing"]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            output.stderr,
            format!(
                "persona-builder: {} doesn't exist: there is no Claude Code history there\n",
                fixture.home.join("missing").display()
            )
            .as_bytes()
        );
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(TEXT));
    }
}
