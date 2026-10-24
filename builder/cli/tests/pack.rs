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
            "pb5-pack-cli-{}-{}",
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
fn pack_prints_exact_project_instructions() {
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
    fs::write(member.join("decisions.md"), "## Calibration (the blind backtest)\n\n| Kind | High |\n|---|---|\n| scope | 6/6 |\n| sequencing | 5/6 |\n\n## By kind of question\n\n### scope\nKeep it small.\nGrow after feedback.\n\n### sequencing\nStart with the unknown.\nPolish later.\n\n### writing-voice\nUse plain words.\nLead with the choice.\n").unwrap();
    let out = scratch.0.join("project pack");
    let output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", scratch.0.join("home"))
        .env("CLAUDE_CONFIG_DIR", scratch.0.join("home/.claude"))
        .arg("pack")
        .arg(&member)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "Made Ari's claude.ai Project pack in {}:\n  instructions.md\n  decisions-scope.md\n  decisions-sequencing.md\n  decisions-writing-voice.md\nTo use it: in claude.ai, make a new Project, paste instructions.md into \"Set project\ninstructions\", and add the other files to the Project's knowledge with the + button.\n",
            out.display()
        )
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_dir(&out).unwrap().count(), 4);
}
