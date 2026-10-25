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
    Parity {
        app: "claude_code_folders",
        cli: &[&["folders"]],
    },
    Parity {
        app: "read_claude_code",
        cli: &[&["mine"]],
    },
    Parity {
        app: "choose_claudeai_export",
        cli: &[&["claudeai", "inspect"]],
    },
    Parity {
        app: "claudeai_export_status",
        cli: &[&["claudeai", "inspect"]],
    },
    Parity {
        app: "list_claudeai_chats",
        cli: &[&["claudeai", "titles"]],
    },
    Parity {
        app: "mark_claudeai_chats",
        cli: &[&["claudeai", "mark"]],
    },
    Parity {
        app: "read_claudeai",
        cli: &[&["claudeai", "mine"]],
    },
    Parity {
        app: "merge_histories",
        cli: &[&["merge"]],
    },
    Parity {
        app: "claude_code_status",
        cli: &[&["status"]],
    },
    Parity {
        app: "turn_on",
        cli: &[&["on"]],
    },
    Parity {
        app: "turn_off",
        cli: &[&["off"]],
    },
    Parity {
        app: "claude_ai_files",
        cli: &[&["pack"]],
    },
    Parity {
        app: "make_claude_ai_files",
        cli: &[&["pack"]],
    },
    Parity {
        app: "show_claude_ai_files",
        cli: &[&["pack"]],
    },
    Parity {
        app: "forget_preview",
        cli: &[&["forget"]],
    },
    Parity {
        app: "forget",
        cli: &[&["forget"]],
    },
];
