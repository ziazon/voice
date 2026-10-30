//! Panel commands through the binary, with invented assistants and a local Claude substitute.
#[path = "../../api/tests/panel_support/mod.rs"]
mod panel_support;
use panel_support::Home;
use persona_builder_api::{Panels, convene_panel_in};
use std::{
    fs,
    process::{Command, Output},
};

fn run(h: &Home, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env("HOME", &h.path)
        .env("PATH", format!("{}:/usr/bin:/bin", h.bin.display()))
        .env_remove("ANTHROPIC_API_KEY")
        .args(args)
        .output()
        .unwrap()
}
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
fn closed(h: &Home) -> String {
    let result = convene_panel_in(
        &Panels::new(),
        &h.path,
        Some(h.bin.as_os_str()),
        &[],
        h.request(),
        &mut |_| {},
    );
    assert!(result.is_ok(), "fixture panel must close: {result:?}");
    let result = result.unwrap();
    assert_eq!(result.status, "closed");
    result.id
}

#[test]
fn panel_convene_prints_id_rounds_outcome_and_result_path() {
    let h = Home::new(true);
    let question = h.path.join("question.md");
    fs::write(
        &question,
        "\nWhich trial should we try?\n\nKeep it easy to undo.\n",
    )
    .unwrap();
    let out = run(
        &h,
        &[
            "panel",
            "convene",
            "--question-file",
            question.to_str().unwrap(),
            "--kind",
            "list:1",
            "--seat",
            "pebble",
            "--seat",
            "quill",
            "--time-box",
            "15m",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    let id = text.lines().next().unwrap_or("");
    assert!(
        id.starts_with("dl-"),
        "first line must be the panel id: {text}"
    );
    assert!(text.contains("closed: agreed"), "{text}");
    let result = h
        .path
        .join(".claude/voice/panels")
        .join(id)
        .join("result.md");
    assert!(text.contains(&result.display().to_string()), "{text}");
    assert!(result.is_file());
    assert!(text.lines().count() >= 4, "round progress missing: {text}");
    let convene: serde_json::Value =
        serde_json::from_slice(&fs::read(result.parent().unwrap().join("convene.json")).unwrap())
            .unwrap();
    assert_eq!(convene["question"], "Which trial should we try?");
    assert!(
        convene["background"]
            .as_str()
            .is_some_and(|s| s.contains("Keep it easy to undo."))
    );
}

#[test]
fn panel_status_json_reports_the_closed_panel() {
    let h = Home::new(true);
    let id = closed(&h);
    let out = run(&h, &["panel", "status", &id, "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let parsed = serde_json::from_slice::<serde_json::Value>(&out.stdout);
    assert!(
        parsed.is_ok(),
        "status must be readable JSON: {}",
        stdout(&out)
    );
    let value = parsed.unwrap();
    // Status uses the same detail DTO as the app, so identity and state agree.
    assert_eq!(value["summary"]["id"], id);
    assert_eq!(value["summary"]["status"], "closed");
    assert_eq!(value["summary"]["outcome"], "agreed");
    assert!(
        value["rounds"]
            .as_array()
            .is_some_and(|rounds| !rounds.is_empty())
    );
}

#[test]
fn panel_list_shows_the_saved_panel() {
    let h = Home::new(true);
    let id = closed(&h);
    let out = run(&h, &["panel", "list"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains(&id), "{text}");
    assert!(text.contains("Which trial should we try?"), "{text}");
    assert!(text.contains("closed"), "{text}");
}

#[test]
fn panel_bad_kind_is_bad_input_and_writes_nothing() {
    let h = Home::new(true);
    let question = h.path.join("question.md");
    fs::write(&question, "Which trial should we try?").unwrap();
    for kind in ["list:0", "list:13", "list:+1", "list:-1", "list:", "other"] {
        let out = run(
            &h,
            &[
                "panel",
                "convene",
                "--question-file",
                question.to_str().unwrap(),
                "--kind",
                kind,
                "--seat",
                "pebble",
                "--seat",
                "quill",
            ],
        );
        assert_eq!(out.status.code(), Some(2), "{kind}: {}", stderr(&out));
    }
    assert!(!h.path.join(".claude/voice/panels").exists());
}

#[test]
fn panel_help_describes_every_subcommand_and_argument() {
    let h = Home::new(true);
    for (args, words) in [
        (
            vec!["panel", "--help"],
            vec!["convene", "resume", "status", "list"],
        ),
        (
            vec!["panel", "convene", "--help"],
            vec!["--question-file", "--kind", "--seat", "--time-box"],
        ),
        (
            vec!["panel", "resume", "--help"],
            vec!["<ID>", "--steer-file", "--time-box"],
        ),
        (vec!["panel", "status", "--help"], vec!["<ID>", "--json"]),
    ] {
        let out = run(&h, &args);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        for word in words {
            assert!(text.contains(word), "missing {word}: {text}");
        }
    }
}

#[test]
fn panel_parsing_requires_inputs_and_rejects_bad_time_boxes() {
    let h = Home::new(true);
    for args in [
        vec!["panel", "convene"],
        vec!["panel", "resume"],
        vec!["panel", "status"],
        vec!["panel", "resume", "dl-example", "--time-box", "tomorrow"],
        vec![
            "panel",
            "convene",
            "--question-file",
            "question.md",
            "--kind",
            "text",
        ],
    ] {
        let out = run(&h, &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", stderr(&out));
    }
}
