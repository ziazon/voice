use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_core::crew::{NewMember, new_member};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
    home: PathBuf,
    claude: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "pb5-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("home");
        let claude = home.join(".claude");
        fs::create_dir_all(&claude).unwrap();
        let root = root.canonicalize().unwrap();
        let home = home.canonicalize().unwrap();
        let claude = claude.canonicalize().unwrap();
        let member = root.join("crew/ari");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            &member,
            &NewMember {
                decisions: voice_core::crew::Decisions::Python,
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/voice",
                date: "2026-10-24",
            },
        )
        .unwrap();
        Self {
            root,
            home,
            claude,
            member,
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_voice"));
        command.env("HOME", &self.home).env("CLAUDE_CONFIG_DIR", "");
        command
    }
    fn run(&self, args: &[&str]) -> String {
        let output = self.command().args(args).output().unwrap();
        success(output)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}
#[test]
fn on_status_off_print_exact_text() {
    let f = Fixture::new();
    assert_eq!(
        f.run(&["on", f.member.to_str().unwrap()]),
        format!(
            "Ari is on in Claude Code.\nAdded the skill /ari in {0}/skills/ari.\nAdded one line to {0}/CLAUDE.md:\n  @~/.claude/voice/assistants/ari/RULES.md\nStart a new Claude Code session to use Ari. To turn Ari off: voice off\n",
            f.claude.display()
        )
    );
    assert_eq!(
        f.run(&["status"]),
        format!(
            "Ari is on in Claude Code.\nThe line in CLAUDE.md: there\n{0}/skills/ari/SKILL.md: there\n{0}/voice/assistants/ari/RULES.md: there\n",
            f.claude.display()
        )
    );
    fs::write(f.member.join("SKILL.md"), "updated").unwrap();
    assert!(f.run(&["status"]).ends_with(&format!(
        "Ari has changed since it was turned on. To update it: voice on {}\n",
        f.member.display()
    )));
    assert_eq!(
        f.run(&["off"]),
        "Ari is off. Everything Voice added is removed.\n"
    );
    for command in ["off", "status"] {
        assert_eq!(f.run(&[command]), "No assistant is on in Claude Code.\n");
    }
}
#[test]
fn folder_option_and_environment_select_the_folder() {
    let f = Fixture::new();
    let custom = f.home.join("custom");
    let other = f.home.join("other");
    fs::create_dir(&custom).unwrap();
    fs::create_dir(&other).unwrap();
    let output = f
        .command()
        .env("CLAUDE_CONFIG_DIR", &other)
        .args([
            "on",
            f.member.to_str().unwrap(),
            "--claude-home",
            custom.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(success(output).contains("@~/custom/voice/assistants/ari/RULES.md"));
    assert!(!other.join("CLAUDE.md").exists());
    assert!(!f.claude.join("CLAUDE.md").exists());
    assert!(
        f.run(&["status", "--claude-home", custom.to_str().unwrap()])
            .starts_with("Ari is on")
    );
    f.run(&["off", "--claude-home", custom.to_str().unwrap()]);
    let output = f
        .command()
        .env("CLAUDE_CONFIG_DIR", &custom)
        .args(["on", f.member.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(success(output).contains("@~/custom/voice/assistants/ari/RULES.md"));
    assert!(
        success(
            f.command()
                .env("CLAUDE_CONFIG_DIR", &custom)
                .arg("status")
                .output()
                .unwrap()
        )
        .starts_with("Ari is on")
    );
    assert_eq!(
        success(
            f.command()
                .env("CLAUDE_CONFIG_DIR", &custom)
                .arg("off")
                .output()
                .unwrap()
        ),
        "Ari is off. Everything Voice added is removed.\n"
    );
}
#[test]
fn missing_or_empty_home_is_a_plain_error() {
    let f = Fixture::new();
    for unset in [true, false] {
        let mut command = f.command();
        if unset {
            command.env_remove("HOME");
        } else {
            command.env("HOME", "");
        }
        let output = command.arg("status").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"voice: HOME is not set\n");
    }
}
#[test]
fn off_reports_every_left_path_exactly() {
    let f = Fixture::new();
    f.run(&["on", f.member.to_str().unwrap()]);
    fs::write(f.claude.join("skills/ari/SKILL.md"), "user edit").unwrap();
    assert_eq!(
        f.run(&["off"]),
        format!(
            "Ari is off. These were left in place:\n  {0}/skills/ari/SKILL.md: you changed it after it was turned on\n  {0}/skills/ari: it still holds other files\n  {0}/skills: it still holds other files\n",
            f.claude.display()
        )
    );
}

#[test]
fn old_skill_requires_explicit_replacement_and_keeps_a_backup() {
    let f = Fixture::new();
    let skill = f.claude.join("skills/ari");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "older skill").unwrap();
    fs::write(f.claude.join("CLAUDE.md"), "keep\n/ari\n").unwrap();
    let output = f
        .command()
        .args(["on", f.member.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        error,
        "voice: Claude Code already has a skill called ari that Voice didn't add, so Voice didn't turn Ari on. Run voice on <member_dir> --replace-old to back it up and move it aside.\n"
    );
    assert_eq!(fs::read(skill.join("SKILL.md")).unwrap(), b"older skill");
    let text = f.run(&["on", f.member.to_str().unwrap(), "--replace-old"]);
    let backup = PathBuf::from(
        text.lines()
            .next()
            .unwrap()
            .strip_prefix("backup: ")
            .unwrap(),
    );
    assert!(backup.starts_with(f.home.join(".claude/voice-backups/ari")));
    assert_eq!(
        fs::read(backup.join("skill/SKILL.md")).unwrap(),
        b"older skill"
    );
    assert_eq!(fs::read(backup.join("CLAUDE.md")).unwrap(), b"keep\n/ari\n");
    assert!(text.contains("Ari is on in Claude Code."));
    f.run(&["off"]);
    assert!(!skill.exists());
    assert_eq!(fs::read(f.claude.join("CLAUDE.md")).unwrap(), b"keep\n");
}
#[test]
fn stale_replace_flag_still_turns_on() {
    let f = Fixture::new();
    let text = f.run(&["on", f.member.to_str().unwrap(), "--replace-old"]);
    assert!(text.starts_with("Ari is on in Claude Code."));
    assert!(!f.claude.join("voice-backups").exists());
}

#[test]
fn plain_on_ignores_old_one_find_size_error() {
    let f = Fixture::new();
    let original = vec![b' '; 4 * 1024 * 1024 + 1];
    fs::write(f.claude.join("CLAUDE.md"), &original).unwrap();
    let text = f.run(&["on", f.member.to_str().unwrap()]);
    assert!(text.starts_with("Ari is on in Claude Code."));
    assert!(
        fs::read(f.claude.join("CLAUDE.md"))
            .unwrap()
            .starts_with(&original)
    );
    assert!(!f.claude.join("voice-backups").exists());
}
