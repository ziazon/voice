use persona_builder::crew::{NewMember, new_member};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb1-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn member_show_prints_exact_file_bytes() {
    let scratch = Scratch::new();
    let target = scratch.0.join("ari");
    let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template");
    new_member(
        &template,
        &target,
        &NewMember {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/projects/voice",
            date: "2026-10-24",
        },
    )
    .unwrap();
    let path = target.join("member.json");
    let output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", scratch.0.join("home"))
        .env("CLAUDE_CONFIG_DIR", scratch.0.join("home/.claude"))
        .args(["member", "show"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, fs::read(path).unwrap());
    assert!(output.stderr.is_empty());
}

#[test]
fn member_show_missing_path_exits_one_with_prefix() {
    let scratch = Scratch::new();
    let path = scratch.0.join("member.json");
    let output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", scratch.0.join("home"))
        .env("CLAUDE_CONFIG_DIR", scratch.0.join("home/.claude"))
        .args(["member", "show"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.starts_with("persona-builder: "), "{error}");
    assert!(error.contains(&path.display().to_string()), "{error}");
}
