use std::{fs, path::PathBuf, process::Command};
use voice_core::crew::{NewMember, new_member};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_voice"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn open_help_explains_the_saved_assistant_command() {
    let output = run(&["open", "--help"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("Open an assistant you saved on GitHub, on this computer"));
    assert!(help.contains("[REPO] [SLUG]"));
    assert!(help.contains("<owner>/<repo>"));
    assert!(help.contains("The assistant's folder name"));
    assert!(help.contains("--update"), "{help}");
    assert!(
        help.contains("Take the newest saved version of this assistant"),
        "{help}"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn opening_requires_both_the_repo_and_the_slug() {
    let output = run(&["open", "rowan-vale/wren-assistant"]);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("<SLUG>"), "{error}");
}

#[test]
fn updating_refuses_a_repository_in_place_of_a_slug() {
    let output = run(&["open", "--update", "rowan-vale/wren-assistant"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("Use the assistant's folder name"), "{error}");
}

#[test]
fn updating_requires_a_slug() {
    let output = run(&["open", "--update"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("<REPO>"), "{error}");
}

#[test]
fn updating_prints_the_refusal_and_exits_one() {
    let home = std::env::temp_dir().join(format!("pb89-cli-update-{}", std::process::id()));
    fs::create_dir(&home).unwrap();
    let member = home.join(".claude/crew/wren");
    new_member(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
        &member,
        &NewMember {
            decisions: voice_core::crew::Decisions::Python,
            name: "Wren",
            pronouns: "she/her",
            principal: "Rowan Vale",
            principal_pronouns: "they/them",
            repo_shown: "~/ai",
            date: "2026-11-13",
        },
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_voice"))
        .env_clear()
        .env("HOME", &home)
        .env("PATH", std::env::var_os("PATH").unwrap())
        .args(["open", "--update", "wren"])
        .output()
        .unwrap();
    fs::remove_dir_all(&home).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: This computer saves Wren, so there's no newer version to take here.\n"
    );
}
