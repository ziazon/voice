use std::process::Command;

fn help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env_clear()
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn assert_commands(help: &str, expected: &[(&str, &str)]) {
    let commands: Vec<_> = help
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .collect();
    assert_eq!(commands.len(), expected.len(), "{help}");
    for (words, (name, description)) in commands.iter().zip(expected) {
        assert_eq!(words[0], *name, "{help}");
        assert_eq!(words[1..].join(" "), *description, "{name}: {help}");
    }
}

#[test]
fn save_help_describes_all_six_subcommands() {
    assert_commands(
        &help(&["save", "--help"]),
        &[
            ("options", "Show where your assistant can be kept."),
            (
                "sign-in",
                "Sign in to GitHub, so your assistant can be saved there.",
            ),
            ("sign-out", "Sign out of GitHub on this computer."),
            ("choose", "Choose where your assistant is kept."),
            ("push", "Save your assistant where you chose."),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
    );
}

#[test]
fn choose_help_describes_each_destination() {
    assert_commands(
        &help(&["save", "choose", "member.json", "--help"]),
        &[
            ("local", "Keep it on this computer only."),
            (
                "here",
                "Save it in a GitHub repo your AI setup already uses.",
            ),
            (
                "github",
                "Save it to a new private GitHub repo with this name.",
            ),
            (
                "help",
                "Print this message or the help of the given subcommand(s)",
            ),
        ],
    );
}
