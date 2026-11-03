use std::process::Command;
use voice_api::parity::PARITY;

#[test]
fn every_app_command_has_working_cli_equivalents() {
    assert!(!PARITY.is_empty(), "the app must declare CLI parity");
    for row in PARITY {
        assert!(!row.cli.is_empty(), "{} has no CLI equivalent", row.app);
        for argv in row.cli {
            assert!(!argv.is_empty(), "{} has an empty CLI invocation", row.app);
            let output = Command::new(env!("CARGO_BIN_EXE_voice"))
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
fn claude_sign_in_actions_share_the_terminal_login() {
    for command in ["claude_sign_in", "claude_sign_in_cancel"] {
        let row = PARITY.iter().find(|row| row.app == command).unwrap();
        assert_eq!(row.cli, &[&["claude", "sign-in"][..]]);
    }
}

#[test]
fn skills_commands_share_the_terminal_skills_commands() {
    let expected = [
        ("list_skills", "list"),
        ("agree_skills", "agree"),
        ("withdraw_skills", "withdraw"),
        ("refresh_skills", "refresh"),
    ];
    let skills: Vec<_> = PARITY
        .iter()
        .filter(|row| row.app.ends_with("_skills"))
        .collect();
    assert_eq!(skills.len(), expected.len());
    for (row, (app, action)) in skills.iter().zip(expected) {
        assert_eq!(row.app, app);
        assert_eq!(row.cli, &[&["skills", action][..]]);
    }
}

#[test]
fn permissions_commands_share_the_terminal_commands() {
    let expected = [
        ("list_permissions", "grants", "show"),
        ("give_permission", "grants", "give"),
        ("turn_off_permission", "grants", "revoke"),
        ("choose_calendar_export", "calendar", "choose"),
        ("audit_log", "audit", "show"),
    ];
    let rows = &PARITY[PARITY.len().saturating_sub(5)..];
    assert_eq!(rows.len(), expected.len());
    for (row, (app, group, action)) in rows.iter().zip(expected) {
        assert_eq!(row.app, app);
        assert_eq!(row.cli, &[&[group, action][..]]);
    }
}
