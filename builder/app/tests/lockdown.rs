include!("../src/command_names.rs");

use persona_builder_api::parity::PARITY;
use serde_json::Value;
use std::path::Path;

fn source(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap()
}

fn json(name: &str) -> Value {
    serde_json::from_str(&source(name)).unwrap()
}

#[test]
fn bundle_is_an_ad_hoc_signed_app_only() {
    let config = json("tauri.conf.json");
    assert_eq!(config["bundle"]["targets"], serde_json::json!(["app"]));
    assert_eq!(config["bundle"]["macOS"]["signingIdentity"], "-");
}

fn list_after(text: &str, marker: &str) -> Vec<String> {
    text.split_once(marker)
        .unwrap_or_else(|| panic!("missing {marker}"))
        .1
        .split_once(']')
        .unwrap()
        .0
        .split(',')
        .map(|entry| {
            entry
                .trim()
                .trim_matches('"')
                .trim_start_matches("commands::")
                .to_owned()
        })
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[test]
fn lockdown_1_command_lists_agree_in_order() {
    let expected = COMMANDS;
    assert_eq!(
        expected,
        [
            "app_info",
            "list_assistants",
            "new_assistant",
            "show_assistant_folder",
            "consent_text",
            "agree_consent",
            "claude_check",
            "claude_sign_in",
            "claude_sign_in_cancel",
            "save_options",
            "open_link",
            "github_sign_in_start",
            "github_sign_in_finish",
            "github_sign_in_cancel",
            "github_sign_out",
            "save_choose",
            "save_push",
            "claude_code_folders",
            "read_claude_code",
            "choose_claudeai_export",
            "claudeai_export_status",
            "list_claudeai_chats",
            "mark_claudeai_chats",
            "read_claudeai",
            "merge_histories",
            "claude_code_status",
            "turn_on",
            "turn_off",
            "claude_ai_files",
            "make_claude_ai_files",
            "show_claude_ai_files",
            "forget_preview",
            "forget",
            "build_assistant",
            "stop_build",
            "spend_today",
            "spending_limit",
            "set_spending_limit",
            "interview_status",
            "draft_interview",
            "save_interview_answers",
            "keep_interview_answers",
            "show_report",
            "review_status",
            "keep_answer",
            "draft_refresh",
            "answer_refresh",
            "apply_refresh",
            "convene_panel",
            "resume_panel",
            "stop_panel",
            "list_panels",
            "panel",
        ]
    );
    let capability = json("capabilities/default.json");
    let allowed: Vec<_> = capability["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .filter_map(|permission| permission.strip_prefix("allow-"))
        .map(|permission| permission.replace('-', "_"))
        .collect();
    assert_eq!(allowed, expected);
    assert_eq!(
        list_after(&source("src/main.rs"), "generate_handler!["),
        expected
    );
    assert_eq!(
        PARITY.iter().map(|row| row.app).collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn assistant_commands_run_off_the_main_thread() {
    let commands = source("src/commands.rs");
    for command in &COMMANDS[1..] {
        assert!(
            commands.contains(&format!("#[tauri::command(async)]\npub fn {command}(")),
            "{command} must run on the async runtime",
        );
    }
}

#[test]
fn lockdown_2_capability_grants_only_app_commands() {
    let capability = json("capabilities/default.json");
    assert_eq!(capability["identifier"], "default");
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    let expected: Vec<_> = COMMANDS
        .iter()
        .map(|command| format!("allow-{}", command.replace('_', "-")))
        .collect();
    assert_eq!(capability["permissions"], serde_json::json!(expected));
}

#[test]
fn lockdown_3_webview_has_no_extra_authority() {
    let main = source("src/main.rs");
    assert!(main.contains("is_allowed_navigation(url, tauri::is_dev())"));
    assert!(!main.contains("debug_assertions"));
    let config = json("tauri.conf.json");
    assert_eq!(config["app"]["withGlobalTauri"], false);
    let windows = config["app"]["windows"].as_array().unwrap();
    assert!(!windows.is_empty());
    for window in windows {
        assert_eq!(window["dragDropEnabled"], false);
    }
    assert!(config.get("plugins").is_none());
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    assert!(!csp.is_empty());
    for directive in csp.split(';').filter(|part| !part.trim().is_empty()) {
        let mut words = directive.split_whitespace();
        let name = words.next().unwrap();
        for origin in words {
            assert!(
                matches!(
                    origin,
                    "'self'" | "'none'" | "ipc:" | "http://ipc.localhost"
                ) || (origin == "'unsafe-inline'" && name == "style-src")
                    || (origin == "data:" && name == "img-src"),
                "disallowed {origin} in {name}"
            );
        }
    }
}

fn is_plugin_line(line: &str) -> bool {
    line.contains("tauri-plugin")
}

#[test]
fn plugin_keys_include_both_toml_quote_styles() {
    for line in [
        "tauri-plugin-fs",
        "\"tauri-plugin-fs\"",
        "'tauri-plugin-fs'",
        "[dependencies.tauri-plugin-fs]",
        "[dependencies.\"tauri-plugin-fs\"]",
        "[dependencies.'tauri-plugin-fs']",
        "foo = { package = \"tauri-plugin-fs\", version = \"2\" }",
    ] {
        assert!(is_plugin_line(line), "missed plugin line: {line}");
    }
    assert!(!is_plugin_line("tauri"));
    assert!(!is_plugin_line("[build-dependencies]"));
}

#[test]
fn lockdown_4_no_external_tauri_plugins() {
    // Check whole lines to catch renamed packages as well as dependency keys and tables.
    for line in source("Cargo.toml").lines() {
        assert!(!is_plugin_line(line), "{line}");
    }
}

#[test]
fn app_exit_stops_a_pending_claude_sign_in() {
    let main = source("src/main.rs");
    assert!(main.contains(".build(tauri::generate_context!())"));
    assert!(main.contains("tauri::RunEvent::Exit"));
    assert!(main.contains("persona_builder_api::claude_sign_in_shutdown(Duration::from_secs(1))"));
}
