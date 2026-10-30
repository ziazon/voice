//! Panels through the app's public boundary, with invented assistants only.
mod panel_support;
use panel_support::Home;
use persona_builder_api::{dto::*, *};
use std::fs;

#[test]
fn convene_streams_rounds_and_closes_through_claude_code() {
    let h = Home::new(true);
    let mut progress = Vec::new();
    let result = convene_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        h.request(),
        &mut |p| progress.push(p),
    );
    assert!(result.is_ok(), "panel should close: {result:?}");
    let result = result.unwrap();
    assert_eq!(result.status, "closed");
    assert_eq!(result.outcome.as_deref(), Some("agreed"));
    assert!(result.id.starts_with("dl-"));
    assert!(!progress.is_empty(), "round progress was not delivered");
    assert!(
        progress
            .iter()
            .all(|p| p.id == result.id && p.round > 0 && !p.line.is_empty())
    );
    assert!(
        h.path
            .join(".claude/voice/panels")
            .join(result.id)
            .join("result.md")
            .is_file()
    );
}

#[test]
fn list_and_detail_return_the_closed_panels_dtos() {
    let h = Home::new(true);
    let ran = convene_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        h.request(),
        &mut |_| {},
    );
    assert!(ran.is_ok(), "fixture panel should close: {ran:?}");
    let ran = ran.unwrap();
    let summaries = list_panels_in(&h.path);
    assert!(summaries.is_ok(), "listing should succeed: {summaries:?}");
    let summaries = summaries.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, ran.id);
    assert_eq!(summaries[0].question, "Which trial should we try?");
    assert_eq!(summaries[0].status, "closed");
    assert_eq!(summaries[0].outcome.as_deref(), Some("agreed"));
    assert!(summaries[0].round > 0 && !summaries[0].updated_at.is_empty());
    let detail = panel_in(&h.path, PanelIdRequest { id: ran.id.clone() });
    assert!(detail.is_ok(), "detail should be readable: {detail:?}");
    let detail = detail.unwrap();
    assert_eq!(detail.summary, summaries[0]);
    assert_eq!(detail.kind, "list:1");
    assert_eq!(detail.seats, ["pebble", "quill"]);
    assert_eq!(detail.time_box.as_deref(), Some("15m"));
    assert!(!detail.asked);
    assert!(!detail.rounds.is_empty());
    assert!(
        detail
            .rounds
            .iter()
            .all(|r| r.withheld.is_empty() && r.failed.is_empty())
    );
    assert!(detail.result.is_some(), "closed panel needs a result");
    let result = detail.result.unwrap();
    assert_eq!(result.outcome, "agreed");
    assert_eq!(result.answer_items, ["A short trial"]);
    assert!(result.answer_text.is_none());
    assert!(result.dissent.is_empty());
    assert!(!result.groups.is_empty());
    let markdown = fs::read_to_string(
        h.path
            .join(".claude/voice/panels")
            .join(ran.id)
            .join("result.md"),
    )
    .unwrap();
    assert_eq!(result.markdown, markdown);
    let json = serde_json::to_value(detail.summary).unwrap();
    assert!(json.get("updatedAt").is_some());
    assert!(json.get("updated_at").is_none());
}

#[test]
fn a_panel_requires_each_assistants_permission_before_writing() {
    let h = Home::new(true);
    fs::remove_file(h.path.join(".claude/state/quill/consent.json")).unwrap();
    let mut progress = Vec::new();
    let result = convene_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        h.request(),
        &mut |p| progress.push(p),
    );
    assert_eq!(
        result.err().map(|e| e.message).as_deref(),
        Some(
            "Persona Builder needs your permission before it reads anything. Give it in the first step."
        )
    );
    assert!(progress.is_empty());
    assert!(!h.path.join(".claude/voice/panels").exists());
    for slug in ["pebble", "quill"] {
        assert!(
            !h.path
                .join(".claude/state")
                .join(slug)
                .join("runs.jsonl")
                .exists()
        );
    }
}

#[test]
fn panel_requests_reject_unknown_fields_and_accept_camel_case() {
    let request: ConvenePanelRequest = serde_json::from_str(r#"{"question":"Which trial?","background":"","kind":"text","seats":["pebble","quill"],"timeBox":"15m"}"#).unwrap();
    assert_eq!(request.time_box.as_deref(), Some("15m"));
    for value in [
        serde_json::json!({"id":"dl-example","extra":true}),
        serde_json::json!({"id":"dl-example","steer":null,"timeBox":null,"extra":true}),
    ] {
        assert!(serde_json::from_value::<PanelIdRequest>(value.clone()).is_err());
        assert!(serde_json::from_value::<ResumePanelRequest>(value).is_err());
    }
    let mut value = serde_json::to_value(request).unwrap();
    value["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ConvenePanelRequest>(value).is_err());
}
