/// The app manifest, capability, handler list and parity table must name exactly these; tests/lockdown.rs checks it.
pub const COMMANDS: &[&str] = &[
    "app_info",
    "list_assistants",
    "new_assistant",
    "show_assistant_folder",
    "consent_text",
    "agree_consent",
    "claude_check",
];
