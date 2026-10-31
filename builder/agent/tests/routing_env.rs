//! One test in its own binary: environment mutation follows tests/env.rs.
mod common;
use common::*;
use persona_builder::ceilings::{Ceilings, Source};
use persona_builder_agent::{guard::Guard, provider::Brief};
use serde_json::json;

#[test]
fn provider_switches_and_base_urls_are_removed_but_https_proxy_reaches_claude() {
    const REMOVED: &[&str] = &[
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_USE_MANTLE",
        "CLAUDE_CODE_USE_ANTHROPIC_AWS",
        "CLAUDE_CODE_USE_FUTURE",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_BEDROCK_BASE_URL",
        "ANTHROPIC_BEDROCK_MANTLE_BASE_URL",
        "ANTHROPIC_VERTEX_BASE_URL",
        "ANTHROPIC_FOUNDRY_BASE_URL",
        "ANTHROPIC_AWS_BASE_URL",
        "ANTHROPIC_FUTURE_BASE_URL",
        "ANTHROPIC_FOUNDRY_RESOURCE",
    ];
    // SAFETY: this binary has only this test, and mutations precede every child.
    unsafe {
        for name in REMOVED {
            std::env::set_var(name, "invented-override");
        }
        std::env::set_var("HTTPS_PROXY", "http://proxy.example.com:8080");
    }
    // Only presence is recorded; no inherited environment values are printed.
    let mut body = String::new();
    for name in REMOVED {
        body.push_str(&format!("if [ -n \"${{{name}+x}}\" ]; then echo set; else echo unset; fi >> \"$here/routing-env\"\n"));
    }
    body.push_str("if [ -n \"${HTTPS_PROXY+x}\" ]; then echo set; else echo unset; fi >> \"$here/proxy-env\"\n");
    body.push_str(&format!("case \"$1\" in\n--version) echo '2.1.292 (Claude Code)';;\nauth) echo '{{\"loggedIn\":true}}';;\n-p) echo '{}';;\nesac", ok_reply(&json!({}))));
    let stub = Stub::new(&body);
    let home = Scratch::new("routing-env");
    let claude = stub.claude();
    claude.check(&home.0).unwrap();
    claude
        .sign_in(&home.0, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    let schema = schema();
    Guard::new(vec![&claude], Ceilings::defaults())
        .call(
            &session(&home.0),
            &Brief {
                sources: &[Source::ClaudeHistory],
                ..call("records", &schema)
            },
        )
        .unwrap();
    assert_eq!(
        stub.file("routing-env").lines().collect::<Vec<_>>(),
        vec!["unset"; REMOVED.len() * 4]
    );
    assert_eq!(
        stub.file("proxy-env").lines().collect::<Vec<_>>(),
        vec!["set"; 4]
    );
}
