mod common;

use common::*;
use serde_json::json;
use std::collections::HashSet;
use voice_agent::guard::Guard;
use voice_agent::{
    agents::{self, AGENTS},
    claude::Claude,
    provider::{Brief, Provider, Reach},
};
use voice_core::ceilings::Ceilings;

#[test]
fn claude_reports_its_identity_and_reach_through_the_trait() {
    let home = Scratch::new("provider-identity");
    let claude = Claude::at(home.0.join("claude"));
    let provider: &dyn Provider = &claude;
    assert_eq!(provider.id(), "claude-code");
    assert_eq!(provider.name(), "Claude, through your own Claude Code");
    assert_eq!(provider.reach(), Reach::Anthropic);
}

#[test]
fn the_registry_matches_the_provider_and_has_distinct_ids() {
    let home = Scratch::new("provider-registry");
    let claude = Claude::at(home.0.join("claude"));
    let provider: &dyn Provider = &claude;
    let agent = agents::agent("claude-code").unwrap();
    assert_eq!(agent.id, provider.id());
    assert_eq!(agent.name, provider.name());
    assert_eq!(agent.reach, provider.reach());
    let ids: HashSet<_> = AGENTS.iter().map(|agent| agent.id).collect();
    assert_eq!(ids.len(), AGENTS.len());
    assert_eq!(AGENTS.len(), 2);
}

#[test]
fn reach_is_ordered_from_own_machines_to_third_parties() {
    assert!(Reach::OwnMachines < Reach::Anthropic);
    assert!(Reach::Anthropic < Reach::ThirdParty);
}

#[test]
fn a_trait_call_uses_claude_and_leaves_one_ledger_line() {
    let home = Scratch::new("provider-call");
    let output = json!({"labels": ["scope"]});
    let stub = Stub::replying(&ok_reply(&output), 0);
    let claude = stub.claude();
    let provider: &dyn Provider = &claude;
    let session = session(&home.0);
    let schema = schema();
    let reply = Guard::new(vec![provider], Ceilings::defaults())
        .call(
            &session,
            &Brief {
                sources: &[voice_core::ceilings::Source::ClaudeHistory],
                step: "label",
                system: "Label each record.",
                prompt: "record one\n",
                model: "sonnet",
                schema: &schema,
            },
        )
        .unwrap();
    assert_eq!(reply.output, output);
    assert!(stub.ran());
    assert!(stub.argv(1).is_none());
    let lines = ledger_lines(&home.0);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["agent"], "claude-code");
    assert_eq!(lines[0]["outcome"], "ok");
}

#[test]
fn shared_checks_run_before_claude_model_checks() {
    let home = Scratch::new("shared-checks");
    let claude = Claude::at(home.0.join("does-not-run"));
    let session = session(&home.0);
    let schema = schema();
    let brief = Brief {
        model: "BAD MODEL",
        prompt: " ",
        ..call("records", &schema)
    };
    let error = Guard::new(vec![&claude], Ceilings::defaults())
        .call(&session, &brief)
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "a step's records must be there and at most 8 MiB"
    );
    assert!(ledger_lines(&home.0).is_empty());
}
