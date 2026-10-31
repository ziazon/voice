//! Alone in its own test binary: it sets environment variables, which no other thread may read meanwhile.
mod common;

use common::*;
use persona_builder_agent::provider::Provider;
use serde_json::json;

#[test]
fn the_session_markers_do_not_reach_claude_and_the_rest_does() {
    // SAFETY: this binary's only test sets these before it starts any thread or child.
    unsafe {
        std::env::set_var("CLAUDECODE", "1");
        std::env::set_var("CLAUDE_CODE_ENTRYPOINT", "cli");
        std::env::set_var("PB_AGENT_TEST_KEEP", "kept");
        std::env::set_var("ANTHROPIC_API_KEY", "not-a-real-key");
        std::env::set_var("CLAUDE_CONFIG_DIR", "invented-parent-profile");
    }
    let home = Scratch::new("env");
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let session = common::session(&home.0);
    let schema = schema();
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    assert_eq!(stub.file("claudecode").trim(), "unset");
    assert_eq!(stub.file("entrypoint").trim(), "unset");
    assert_eq!(stub.file("keep"), "kept");
    assert_eq!(
        stub.file("profiles").trim(),
        "-p invented-parent-profile unset"
    );
    assert!(stub.claude().profile().is_none());

    let check = Stub::new(
        "case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":true}';; esac",
    );
    assert!(check.claude().check(&home.0).unwrap().api_key_set);
    assert_eq!(check.file("claudecode").trim(), "unset");
    assert_eq!(check.file("entrypoint").trim(), "unset");

    let profiled = Stub::new(&format!(
        r#"case "$1" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{{"loggedIn":true}}';;
-p) echo '{}';;
esac"#,
        ok_reply(&json!({}))
    ));
    let claude =
        persona_builder_agent::claude::find_in(&home.0, None, &[profiled.program()]).unwrap();
    claude.check(&home.0).unwrap();
    claude.call(&session, &call("records", &schema)).unwrap();
    claude
        .sign_in(&home.0, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    let folder = persona_builder_agent::claude::profile_folder(&home.0);
    let expected = ["--version", "auth", "-p", "auth"]
        .map(|arg| format!("{arg} {} unset", folder.display()))
        .join("\n");
    assert_eq!(profiled.file("profiles").trim(), expected);
    assert_eq!(
        std::env::var("CLAUDE_CONFIG_DIR").unwrap(),
        "invented-parent-profile"
    );
}
