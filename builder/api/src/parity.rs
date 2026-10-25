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
];
