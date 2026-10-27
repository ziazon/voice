use persona_builder_api::parity::PARITY;
use std::process::Command;

#[test]
fn every_app_command_has_working_cli_equivalents() {
    assert!(!PARITY.is_empty(), "the app must declare CLI parity");
    for row in PARITY {
        assert!(!row.cli.is_empty(), "{} has no CLI equivalent", row.app);
        for argv in row.cli {
            assert!(!argv.is_empty(), "{} has an empty CLI invocation", row.app);
            let output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
                .args(*argv)
                .arg("--help")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}: {:?}: {}",
                row.app,
                argv,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[test]
fn claude_sign_in_commands_share_the_terminal_sign_in_command() {
    for command in ["claude_sign_in", "claude_sign_in_cancel"] {
        let row = PARITY.iter().find(|row| row.app == command).unwrap();
        assert_eq!(row.cli, &[&["claude", "sign-in"][..]]);
    }
}
