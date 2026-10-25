/// Each app command and the `persona-builder` commands that do the same thing.
pub struct Parity {
    pub app: &'static str,
    pub cli: &'static [&'static [&'static str]],
}
pub const PARITY: &[Parity] = &[
    Parity {
        app: "app_info",
        cli: &[&["--version"], &["agents"]],
    },
    Parity {
        app: "list_assistants",
        cli: &[&["assistants"]],
    },
    Parity {
        app: "new_assistant",
        cli: &[&["new"]],
    },
    Parity {
        app: "show_assistant_folder",
        cli: &[&["assistants"]],
    },
    Parity {
        app: "consent_text",
        cli: &[&["consent", "show"]],
    },
    Parity {
        app: "agree_consent",
        cli: &[&["consent", "agree"]],
    },
    Parity {
        app: "claude_check",
        cli: &[&["claude", "check"]],
    },
    Parity {
        app: "save_options",
        cli: &[&["save", "options"]],
    },
    Parity {
        app: "open_link",
        cli: &[&["save", "options"], &["save", "sign-in"]],
    },
    Parity {
        app: "github_sign_in_start",
        cli: &[&["save", "sign-in"]],
    },
    Parity {
        app: "github_sign_in_finish",
        cli: &[&["save", "sign-in"]],
    },
    Parity {
        app: "github_sign_in_cancel",
        cli: &[&["save", "sign-in"]],
    },
    Parity {
        app: "github_sign_out",
        cli: &[&["save", "sign-out"]],
    },
    Parity {
        app: "save_choose",
        cli: &[&["save", "choose"]],
    },
    Parity {
        app: "save_push",
        cli: &[&["save", "push"]],
    },
];
