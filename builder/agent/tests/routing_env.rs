//! One test in its own binary: environment mutation follows tests/env.rs.
mod common;
use common::*;
use serde_json::json;
use voice_agent::{guard::Guard, provider::Brief};
use voice_core::ceilings::{Ceilings, Source};

#[test]
fn provider_switches_and_base_urls_are_removed_and_proxy_is_private() {
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
    body.push_str(&format!("case \"$3\" in\n--version) echo '2.1.292 (Claude Code)';;\nauth) echo '{{\"loggedIn\":true,\"authMethod\":\"api_key\"}}';;\n-p) echo '{}';;\nesac", ok_reply(&json!({}))));
    let stub = Stub::new(&body);
    let home = Scratch::new("routing-env");
    let claude = stub.claude();
    claude.check(&home.0).unwrap();
    #[cfg(any(not(target_os = "macos"), feature = "claude-subscription"))]
    claude
        .sign_in_with(
            &home.0,
            &std::sync::atomic::AtomicBool::new(false),
            {
                #[cfg(target_os = "macos")]
                {
                    voice_agent::claude::SignIn::Subscription
                }
                #[cfg(not(target_os = "macos"))]
                {
                    voice_agent::claude::SignIn::Console
                }
            },
            &|_| panic!("no browser requested"),
        )
        .unwrap();
    // Default Mac builds refuse sign-in before launching; logout exercises the same
    // ordinary launch environment while feature builds still cover the sign-in launch.
    #[cfg(all(target_os = "macos", not(feature = "claude-subscription")))]
    claude.sign_out(&home.0).unwrap();
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
    for n in 0..4 {
        assert_launch_env(&stub, n, &stub.profile());
    }
    assert_eq!(
        stub.file("proxy-env").lines().collect::<Vec<_>>(),
        vec!["set"; 4]
    );
}
