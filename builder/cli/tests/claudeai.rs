use persona_builder::crew::{NewMember, new_member};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const TEXT: &str = "Keep the reading list short.";
const REVIEW: &str = "~/.claude/state/wren/claudeai-review.tsv";
const NO_REVIEW: &str = "there's no review file yet: run persona-builder claudeai titles first";

struct Fixture {
    home: PathBuf,
    member: PathBuf,
    export: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-claudeai-cli-{}-{}",
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
        let export = home.join("Downloads/conversations.json");
        fs::create_dir_all(export.parent().unwrap()).unwrap();
        let conversations = format!(
            r#"[{{"uuid": "c-1", "name": "Reading list", "created_at": "2026-03-01T09:00:00Z",
                "chat_messages": [{{"sender": "human", "created_at": "2026-03-01T09:00:00Z", "text": "{TEXT}"}}]}},
               {{"uuid": "c-2", "name": "Someone else's chat", "created_at": "2026-03-02T09:00:00Z",
                "chat_messages": [{{"sender": "human", "created_at": "2026-03-02T09:00:00Z", "text": "not mine"}}]}}]"#
        );
        fs::write(&export, conversations).unwrap();
        Self {
            home,
            member,
            export,
        }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", std::env::var_os("PATH").unwrap())
            .current_dir(&self.home)
            .arg("claudeai")
            .args(args)
            .output()
            .unwrap()
    }
    fn step(&self, step: &str, args: &[&str]) -> Output {
        let mut all = vec![step, self.member.to_str().unwrap()];
        if step != "mark" {
            all.extend(["--export", "~/Downloads/conversations.json"]);
        }
        all.extend(args);
        self.run(&all)
    }
    fn ok(&self, step: &str, args: &[&str]) -> String {
        let output = self.step(step, args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    }
    fn refused(&self, step: &str, args: &[&str], message: &str) {
        let output = self.step(step, args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("persona-builder: {message}\n")
        );
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(TEXT));
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
fn mode(path: PathBuf) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn titles_mark_and_mine_end_to_end() {
    let f = Fixture::new();
    assert_eq!(
        f.ok("titles", &[]),
        format!("listed 2 conversations in {REVIEW} (0 already marked)\n")
    );
    let review = f.state().join("claudeai-review.tsv");
    assert_eq!(mode(review.clone()), 0o600);
    assert_eq!(mode(f.state()), 0o700);
    assert!(!fs::read_to_string(&review).unwrap().contains(TEXT));
    assert_eq!(
        f.ok("mark", &["--uuid", "c-1", "--as", "y"]),
        "marked 1 conversations y\n"
    );
    assert_eq!(
        f.ok("mark", &["--title", "(?i)someone", "--as", "n"]),
        "marked 1 conversations n\n"
    );
    assert_eq!(
        f.ok("titles", &[]),
        format!("listed 2 conversations in {REVIEW} (2 already marked)\n")
    );
    assert_eq!(
        f.ok("mine", &[]),
        "read 1 conversations marked mine; skipped 1 not mine and 0 unmarked\n\
         mined 1 messages; dropped 0 personal records, 0 secret records\n\
         saved to ~/.claude/state/wren/decisions-claudeai.jsonl\n"
    );
    let corpus = f.state().join("decisions-claudeai.jsonl");
    assert_eq!(mode(corpus.clone()), 0o600);
    let text = fs::read_to_string(corpus).unwrap();
    assert!(text.contains(TEXT));
    assert!(!text.contains("not mine"));
    assert!(text.contains(r#""project": "claude.ai""#));
}
#[test]
fn dry_runs_write_nothing() {
    let f = Fixture::new();
    assert_eq!(
        f.ok("titles", &["--dry-run"]),
        format!(
            "listed 2 conversations in {REVIEW} (0 already marked)\ndry run: nothing written\n"
        )
    );
    assert!(!f.home.join(".claude/state").exists());
    f.ok("titles", &[]);
    f.ok("mark", &["--all", "--as", "y"]);
    let before = fs::read(f.state().join("claudeai-review.tsv")).unwrap();
    assert!(
        f.ok("mine", &["--dry-run"])
            .ends_with("dry run: nothing written\n")
    );
    assert!(!f.state().join("decisions-claudeai.jsonl").exists());
    assert_eq!(
        fs::read(f.state().join("claudeai-review.tsv")).unwrap(),
        before
    );
}
#[test]
fn names_are_checked_first() {
    let f = Fixture::new();
    for name in ["x.jsonl", "a/x.tsv", ".x.tsv"] {
        f.refused(
            "titles",
            &["--review", name],
            "the review file name must be a plain name ending .tsv",
        );
        f.refused(
            "mark",
            &["--review", name, "--all", "--as", "y"],
            "the review file name must be a plain name ending .tsv",
        );
    }
    f.refused(
        "mine",
        &["--out", "x.tsv"],
        "the file name must be a plain name ending .jsonl",
    );
    assert!(!f.home.join(".claude/state").exists());
}
#[test]
fn mark_and_mine_need_a_review_file() {
    let f = Fixture::new();
    f.refused("mark", &["--all", "--as", "y"], NO_REVIEW);
    f.refused("mine", &[], NO_REVIEW);
    assert!(!f.home.join(".claude/state").exists());
}
#[test]
fn mark_needs_a_selector_and_a_readable_pattern() {
    let f = Fixture::new();
    f.ok("titles", &[]);
    let selector =
        "mark needs a selector: --uuid, --project, --title, --before, --after, --unmarked or --all";
    f.refused("mark", &["--as", "y"], selector);
    f.refused("mark", &["--project", "", "--as", "y"], selector);
    f.refused(
        "mark",
        &["--title", "(?=x)", "--as", "y"],
        "--title is not a valid pattern, or it uses look-around or a backreference, which Persona Builder can't read",
    );
    let usage = f.step("mark", &["--all", "--as", "maybe"]);
    assert_eq!(usage.status.code(), Some(2));
}
#[test]
fn holdout_and_cutoff() {
    let f = Fixture::new();
    f.ok("titles", &[]);
    f.refused(
        "mine",
        &["--holdout", "2"],
        "--holdout must be a number from 0 to 1",
    );
    let both = f.step("mine", &["--holdout", "0.1", "--cutoff", "2026"]);
    assert_eq!(both.status.code(), Some(2));
}
#[test]
fn a_link_in_the_state_folder_is_refused() {
    let f = Fixture::new();
    f.ok("titles", &[]);
    let review = f.state().join("claudeai-review.tsv");
    let elsewhere = f.home.join("elsewhere.tsv");
    fs::rename(&review, &elsewhere).unwrap();
    symlink(&elsewhere, &review).unwrap();
    let output = f.step("mine", &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("link"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!f.state().join("decisions-claudeai.jsonl").exists());
}
#[test]
fn inspect_shows_the_shape_and_names_missing_fields() {
    let f = Fixture::new();
    let output = f.run(&["inspect", "--export", f.export.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("files: 1\nconversations.json: list of 2\n"));
    assert!(stdout.ends_with("ok: every field the adapter reads is present\n"));
    assert!(!stdout.contains(TEXT) && !stdout.contains("Reading list"));
    let broken = f.home.join("broken.json");
    fs::write(&broken, r#"[{"uuid": "c-1", "chat_messages": []}]"#).unwrap();
    let output = f.run(&["inspect", "--export", broken.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "files: 1\nconversations.json: list of 1\n  .chat_messages: list (1/1)\n  .uuid: str (1/1)\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "persona-builder: the export is missing fields Persona Builder reads: .name, .created_at, .chat_messages[].sender, .chat_messages[].created_at, .chat_messages[].text or .chat_messages[].content[].text\n"
    );
}
