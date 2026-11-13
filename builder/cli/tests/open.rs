use std::process::Command;

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
    assert!(output.stderr.is_empty());
}

#[test]
fn opening_requires_both_the_repo_and_the_slug() {
    let output = run(&["open", "rowan-vale/wren-assistant"]);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("<SLUG>"), "{error}");
}
