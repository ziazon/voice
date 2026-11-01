//! Panels through the app's public boundary, with invented assistants only.
mod panel_support;
use panel_support::Home;
use std::fs;
use voice_api::{dto::*, *};

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
        Some("Voice needs your permission before it reads anything. Give it in the first step.")
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

fn convene_with_progress(h: &Home) -> (PanelRan, Vec<PanelProgress>) {
    let mut progress = Vec::new();
    let ran = convene_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        h.request(),
        &mut |p| progress.push(p),
    )
    .unwrap();
    assert_eq!(ran.status, "closed");
    (ran, progress)
}
fn manifest(h: &Home) -> std::path::PathBuf {
    h.path.join(".claude/state/pebble/mirror/.manifest.json")
}
fn notes_hash(h: &Home) -> serde_json::Value {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(manifest(h)).unwrap()).unwrap();
    value["skills"]["notes"]["SKILL.md"].clone()
}
fn refresh_notes(h: &Home) {
    refresh_skills_in(
        &h.path,
        SlugRequest {
            slug: "pebble".into(),
        },
    )
    .unwrap();
}

#[test]
fn convene_refreshes_agreed_skills_and_leaves_unused_mirror_absent() {
    let h = Home::new(true);
    h.agree_notes();
    assert!(!manifest(&h).parent().unwrap().exists());
    let (_, progress) = convene_with_progress(&h);
    assert!(
        manifest(&h).is_file(),
        "agreed skill needs a fresh manifest"
    );
    assert!(notes_hash(&h).as_str().is_some_and(|hash| hash.len() == 64));
    assert_eq!(
        fs::read_to_string(manifest(&h).parent().unwrap().join("notes/SKILL.md")).unwrap(),
        "Write short notes.\n"
    );
    assert!(!h.path.join(".claude/state/quill/mirror").exists());
    assert!(
        progress
            .iter()
            .all(|p| !p.line.starts_with("Voice couldn't bring"))
    );
}

#[test]
fn convene_updates_stale_skill_copy_and_hash() {
    let h = Home::new(true);
    let source = h.agree_notes();
    refresh_notes(&h);
    let before = notes_hash(&h);
    fs::write(source, "Write clear notes.\n").unwrap();
    convene_with_progress(&h);
    assert_ne!(notes_hash(&h), before);
    assert_eq!(
        fs::read_to_string(manifest(&h).parent().unwrap().join("notes/SKILL.md")).unwrap(),
        "Write clear notes.\n"
    );
}

#[test]
fn failed_skill_refresh_is_private_progress_before_first_round_and_panel_closes() {
    if panel_support::running_as_root() {
        return;
    }
    let h = Home::new(true);
    h.agree_notes();
    let _mode = panel_support::UnreadablePlaceholders::new(&h);
    let (_, progress) = convene_with_progress(&h);
    let failures: Vec<_> = progress
        .iter()
        .enumerate()
        .filter(|(_, p)| p.line == panel_support::SKILLS_NOT_REFRESHED)
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].1.round, 1);
    assert!(
        failures[0].0
            < progress
                .iter()
                .position(|p| p.line == "Round 1 is starting.")
                .unwrap()
    );
    for p in progress {
        for private in ["notes", "placeholders", "~/"] {
            assert!(!p.line.contains(private), "{}", p.line);
        }
    }
    assert!(!manifest(&h).exists());
}

#[test]
fn resume_refreshes_stale_skill_copy() {
    let h = Home::new(true);
    let source = h.agree_notes();
    refresh_notes(&h);
    let (ran, _) = convene_with_progress(&h);
    let before = notes_hash(&h);
    fs::write(source, "Write clear notes.\n").unwrap();
    let resumed = resume_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        ResumePanelRequest {
            id: ran.id,
            steer: Some("Try a smaller trial.".into()),
            time_box: None,
        },
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(resumed.status, "closed");
    assert_ne!(notes_hash(&h), before);
}

#[test]
fn refused_resume_leaves_skill_manifest_unchanged() {
    let h = Home::new(true);
    let source = h.agree_notes();
    refresh_notes(&h);
    let (ran, _) = convene_with_progress(&h);
    let before = fs::read(manifest(&h)).unwrap();
    fs::write(source, "Write clear notes.\n").unwrap();
    let result = resume_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        ResumePanelRequest {
            id: ran.id,
            steer: None,
            time_box: None,
        },
        &mut |_| {},
    );
    assert_eq!(
        result.unwrap_err().message,
        voice_panel_runner::text::NEED_STEER
    );
    assert_eq!(fs::read(manifest(&h)).unwrap(), before);
}
