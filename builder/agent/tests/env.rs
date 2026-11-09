//! Alone in its own test binary: environment changes precede every thread and child.
mod common;

use common::*;
use serde_json::json;
use std::sync::atomic::AtomicBool;
use voice_agent::guard::Guard;
use voice_core::ceilings::Ceilings;

#[test]
fn every_launch_gets_only_its_own_environment() {
    // SAFETY: this binary's only test sets these before it starts any thread or child.
    unsafe {
        for name in [
            "ANTHROPIC_API_KEY",
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_USE_BEDROCK",
            "PB_AGENT_TEST_KEEP",
            "CLAUDE_CONFIG_DIR",
        ] {
            std::env::set_var(name, "invented-parent-value");
        }
    }
    let home = Scratch::new("env");
    let stub = Stub::new(&format!(
        r#"case "$3" in
--version) echo '2.1.292 (Claude Code)';;
auth) echo '{{"loggedIn":true,"authMethod":"api_key"}}';;
-p) echo '{}';;
esac"#,
        ok_reply(&json!({}))
    ));
    let claude = stub.claude();
    let schema = schema();
    Guard::new(vec![&claude], Ceilings::defaults())
        .call(&session(&home.0), &call("records", &schema))
        .unwrap();
    claude.check(&home.0).unwrap();
    claude
        .sign_in_with(
            &home.0,
            &AtomicBool::new(false),
            voice_agent::claude::SignIn::Console,
            &|_| panic!("no browser requested"),
        )
        .unwrap();
    for n in 0..4 {
        assert_launch_env(&stub, n, &stub.profile());
    }
    assert!(std::env::var("CLAUDE_CONFIG_DIR").is_ok_and(|value| value == "invented-parent-value"));
}
