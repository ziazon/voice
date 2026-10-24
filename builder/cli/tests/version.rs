use std::process::Command;

#[test]
fn prints_its_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .arg("--version")
        .output()
        .expect("run persona-builder");
    assert!(out.status.success(), "exit: {:?}", out.status);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("persona-builder {}\n", env!("CARGO_PKG_VERSION"))
    );
}
