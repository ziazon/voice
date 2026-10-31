#[path = "../../core/tests/common/mod.rs"]
mod common;

use common::{Scratch, repo};
use std::{
    fs,
    process::{Command, Output},
};

fn run(home: &Scratch, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env_clear()
        .env("HOME", &home.0)
        .current_dir(&home.0)
        .args(args)
        .output()
        .unwrap()
}
fn refused(output: Output, shown: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "persona-builder: {shown} is a kind of file that holds passwords or keys, so Persona Builder never opens it. Choose another file.\n"
        )
    );
}

#[test]
fn claudeai_inspect_refuses_export_under_aws() {
    let home = Scratch::new("deny-cli-export");
    fs::create_dir(home.0.join(".aws")).unwrap();
    let path = home.0.join(".aws/export.zip");
    fs::write(&path, "canary").unwrap();
    refused(
        run(
            &home,
            &["claudeai", "inspect", "--export", path.to_str().unwrap()],
        ),
        "~/.aws/export.zip",
    );
}

#[test]
fn shingle_refuses_document_under_ssh() {
    let home = Scratch::new("deny-cli-shingle");
    let member = home.0.join("member");
    fs::create_dir(&member).unwrap();
    let mut config: serde_json::Value =
        serde_json::from_slice(&fs::read(repo().join("crew/_template/member.json")).unwrap())
            .unwrap();
    config["slug"] = serde_json::json!("pebble");
    config["state_dir"] = serde_json::json!("~/.claude/state/pebble");
    fs::write(member.join("member.json"), config.to_string()).unwrap();
    fs::create_dir(home.0.join(".ssh")).unwrap();
    let path = home.0.join(".ssh/draft.md");
    fs::write(&path, "canary").unwrap();
    refused(
        run(
            &home,
            &[
                "shingle",
                member.to_str().unwrap(),
                "--doc",
                path.to_str().unwrap(),
                "--corpus",
                "decisions.jsonl",
            ],
        ),
        "~/.ssh/draft.md",
    );
}
