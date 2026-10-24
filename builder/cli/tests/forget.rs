use persona_builder::crew::{NewMember, new_member};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "pb5-forget-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn forget_prints_exact_preview_delete_and_missing_text() {
    let scratch = Scratch::new();
    let member = scratch.0.join("ari");
    new_member(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
        &member,
        &NewMember {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/voice",
            date: "2026-10-24",
        },
    )
    .unwrap();
    let home = scratch.0.join("home");
    let state = home.join(".claude/state/ari");
    fs::create_dir_all(state.join("drafts")).unwrap();
    fs::write(state.join("history.json"), "invented history").unwrap();
    fs::write(state.join("drafts/notes.md"), "invented draft").unwrap();
    for (yes, expected) in [
        (
            false,
            format!(
                "This deletes 2 files in {}: the history Ari learned from and the drafts of each build.\nAri's own files in {} stay.\nTo go ahead: persona-builder forget {} --yes\n",
                state.display(),
                member.display(),
                member.display()
            ),
        ),
        (
            true,
            format!(
                "Deleted 2 files in {}. Ari's own files in {} are untouched.\n",
                state.display(),
                member.display()
            ),
        ),
        (
            false,
            format!("Nothing to forget: {} is not there.\n", state.display()),
        ),
        (
            true,
            format!("Nothing to forget: {} is not there.\n", state.display()),
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_persona-builder"));
        command
            .env("HOME", &home)
            .env("CLAUDE_CONFIG_DIR", scratch.0.join("other-claude"))
            .arg("forget")
            .arg(&member);
        if yes {
            command.arg("--yes");
        }
        let output = command.output().unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        if !yes && state.exists() {
            assert_eq!(
                fs::read_to_string(state.join("history.json")).unwrap(),
                "invented history"
            );
            assert_eq!(
                fs::read_to_string(state.join("drafts/notes.md")).unwrap(),
                "invented draft"
            );
        } else {
            assert!(!state.exists());
        }
        assert!(member.join("member.json").is_file());
        assert!(member.join("SKILL.md").is_file());
    }
}
