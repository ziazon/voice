use std::process::Command;

#[test]
fn prints_its_version() {
    let scratch = std::env::temp_dir().join(format!("pb5-cli-version-{}", std::process::id()));
    std::fs::create_dir(&scratch).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", &scratch)
        .env("CLAUDE_CONFIG_DIR", scratch.join(".claude"))
        .arg("--version")
        .output()
        .expect("run persona-builder");
    std::fs::remove_dir(scratch).unwrap();
    assert!(out.status.success(), "exit: {:?}", out.status);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("persona-builder {}\n", env!("CARGO_PKG_VERSION"))
    );
}
