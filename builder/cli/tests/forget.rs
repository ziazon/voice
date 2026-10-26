use persona_builder::crew::{NewMember, new_member};
use persona_builder_ingest::private::{self, lock_state};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
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

fn fixture(scratch: &Scratch) -> (PathBuf, PathBuf, PathBuf) {
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
    for folder in [
        home.join(".claude"),
        home.join(".claude/state"),
        state.clone(),
    ] {
        fs::set_permissions(folder, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fs::write(state.join("history.json"), "invented history").unwrap();
    fs::write(state.join("drafts/notes.md"), "invented draft").unwrap();
    (member, home, state)
}

#[test]
fn forget_prints_exact_preview_delete_and_missing_text() {
    let scratch = Scratch::new();
    let (member, home, state) = fixture(&scratch);
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

#[test]
fn forget_refuses_busy_state_then_deletes_after_unlock() {
    let scratch = Scratch::new();
    let (member, home, state) = fixture(&scratch);
    let lock = lock_state(&home, "ari").unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", &home)
            .env("CLAUDE_CONFIG_DIR", scratch.0.join("other-claude"))
            .arg("forget")
            .arg(&member)
            .arg("--yes")
            .output()
            .unwrap()
    };
    let output = run();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(private::BUSY));
    assert_eq!(
        fs::read_to_string(state.join("history.json")).unwrap(),
        "invented history"
    );
    assert_eq!(
        fs::read_to_string(state.join("drafts/notes.md")).unwrap(),
        "invented draft"
    );
    drop(lock);
    let output = run();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!state.exists());
}

#[test]
fn forget_missing_state_never_creates_it() {
    let scratch = Scratch::new();
    let (member, home, state) = fixture(&scratch);
    fs::remove_dir_all(home.join(".claude")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", scratch.0.join("other-claude"))
        .arg("forget")
        .arg(&member)
        .arg("--yes")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("Nothing to forget: {} is not there.\n", state.display())
    );
    assert!(output.stderr.is_empty());
    assert!(!state.exists());
    assert!(!home.join(".claude").exists());
}
