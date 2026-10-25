/// The app manifest, capability, handler list and parity table must name exactly these; tests/lockdown.rs checks it.
pub const COMMANDS: &[&str] = &[
    "app_info",
    "list_assistants",
    "new_assistant",
    "show_assistant_folder",
    "consent_text",
    "agree_consent",
    "claude_check",
    "save_options",
    "open_link",
    "github_sign_in_start",
    "github_sign_in_finish",
    "github_sign_in_cancel",
    "github_sign_out",
    "save_choose",
    "save_push",
];
