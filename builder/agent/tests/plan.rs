mod common;

use common::*;
use persona_builder_agent::{
    ledger::{Cap, Session, local_day},
    provider::{Provider, Usage},
    text,
};
use serde_json::{Value, json};
use std::time::Duration;

fn stream(lines: &[String]) -> String {
    format!("{}\n", lines.join("\n"))
}

fn refused(session: &Session, stub: &Stub) {
    let schema = schema();
    assert_eq!(
        stub.claude()
            .call(session, &call("records", &schema))
            .unwrap_err()
            .to_string(),
        text::PLAN_STOPPED
    );
    assert!(stub.argv(1).is_none());
}

#[test]
fn a_call_reads_the_result_from_a_stream() {
    let home = Scratch::new("plan-stream");
    let output = json!({"labels": ["small"]});
    let stub = Stub::replying(
        &stream(&[
            json!({"type":"system","subtype":"init"}).to_string(),
            json!({"type":"assistant","message":{}}).to_string(),
            ok_reply(&output),
        ]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    let reply = stub
        .claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    assert_eq!(reply.output, output);
    assert_eq!(reply.cost_usd, 0.0123);
    assert_eq!(
        reply.usage,
        Usage {
            input_tokens: 1200,
            output_tokens: 300,
            cache_creation_input_tokens: 7,
            cache_read_input_tokens: 9
        }
    );
    let lines = ledger_lines(&home.0);
    assert_eq!(lines[0]["outcome"], "ok");
    assert!(lines[0].get("plan").is_none());
}

#[test]
fn a_warning_keeps_this_answer_and_stops_the_next_call() {
    let home = Scratch::new("plan-warning");
    let output = json!({"labels": []});
    let stub = Stub::replying(
        &stream(&[rate_limit_line("allowed_warning"), ok_reply(&output)]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    assert_eq!(
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap()
            .output,
        output
    );
    assert_eq!(ledger_lines(&home.0)[0]["plan"], "allowed_warning");
    refused(&session, &stub);
    assert_eq!(ledger_lines(&home.0).len(), 1);
}

#[test]
fn a_rejected_plan_explains_the_failed_call() {
    let home = Scratch::new("plan-rejected");
    let result = json!({"type":"result","is_error":true,"result":"You've hit your limit","total_cost_usd":0.0});
    let stub = Stub::replying(
        &stream(&[rate_limit_line("rejected"), result.to_string()]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    assert_eq!(
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap_err()
            .to_string(),
        text::PLAN_STOPPED
    );
    let lines = ledger_lines(&home.0);
    assert_eq!(lines[0]["outcome"], "plan-limit");
    assert_eq!(lines[0]["plan"], "rejected");
    refused(&session, &stub);
}

#[test]
fn a_rejected_plan_explains_any_failed_reply() {
    for lines in [
        vec![rate_limit_line("rejected")],
        vec![
            rate_limit_line("rejected"),
            json!({"type":"result","subtype":"success","is_error":false,"result":"","total_cost_usd":0.01}).to_string(),
        ],
    ] {
        let home = Scratch::new("plan-rejected-reply");
        let stub = Stub::replying(&stream(&lines), 0);
        let session = session(&home.0);
        let schema = schema();
        assert_eq!(
            stub.claude()
                .call(&session, &call("records", &schema))
                .unwrap_err()
                .to_string(),
            text::PLAN_STOPPED
        );
        let lines = ledger_lines(&home.0);
        assert_eq!(lines[0]["outcome"], "plan-limit");
        assert_eq!(lines[0]["plan"], "rejected");
        refused(&session, &stub);
    }
}

#[test]
fn allowed_and_unknown_statuses_change_nothing() {
    let home = Scratch::new("plan-allowed");
    let stub = Stub::replying(
        &stream(&[
            rate_limit_line("allowed"),
            rate_limit_line("weird"),
            json!({"type":"rate_limit_event","rate_limit_info":{}}).to_string(),
            json!({"type":"rate_limit_event"}).to_string(),
            ok_reply(&json!({})),
        ]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    assert!(ledger_lines(&home.0)[0].get("plan").is_none());
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    assert!(stub.argv(1).is_some());
}

#[test]
fn the_worst_status_wins() {
    for (statuses, expected) in [
        (["allowed_warning", "allowed"], "allowed_warning"),
        (["rejected", "allowed_warning"], "rejected"),
    ] {
        let home = Scratch::new("plan-worst");
        let stub = Stub::replying(
            &stream(&[
                rate_limit_line(statuses[0]),
                rate_limit_line(statuses[1]),
                ok_reply(&json!({})),
            ]),
            0,
        );
        let session = session(&home.0);
        let schema = schema();
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap();
        assert_eq!(ledger_lines(&home.0)[0]["plan"], expected);
    }
}

#[test]
fn the_stop_lasts_only_the_day() {
    let home = Scratch::new("plan-tomorrow");
    let stub = Stub::replying(
        &stream(&[rate_limit_line("allowed_warning"), ok_reply(&json!({}))]),
        0,
    );
    let schema = schema();
    let first = session(&home.0);
    stub.claude()
        .call(&first, &call("records", &schema))
        .unwrap();
    drop(first);
    let tomorrow = Session::open_with_clock(
        &home.0,
        "wren",
        Cap::default(),
        Box::new(|| fixed_time() + Duration::from_secs(86_400)),
    )
    .unwrap();
    stub.claude()
        .call(&tomorrow, &call("records", &schema))
        .unwrap();
    assert!(stub.argv(1).is_some());
}

#[test]
fn a_warning_in_unreadable_output_still_stops() {
    let home = Scratch::new("plan-unreadable");
    let stub = Stub::replying(
        &stream(&[rate_limit_line("allowed_warning"), "not json".into()]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    assert_eq!(
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap_err()
            .to_string(),
        text::FAILED
    );
    let lines = ledger_lines(&home.0);
    assert_eq!(lines[0]["outcome"], "unreadable");
    assert_eq!(lines[0]["plan"], "allowed_warning");
    refused(&session, &stub);
}

#[test]
fn a_stream_without_a_result_is_unreadable() {
    let home = Scratch::new("plan-no-result");
    let stub = Stub::replying(
        &stream(&[json!({"type":"system","subtype":"init"}).to_string()]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    assert_eq!(
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap_err()
            .to_string(),
        text::FAILED
    );
    assert_eq!(ledger_lines(&home.0)[0]["outcome"], "unreadable");
}

#[test]
fn the_last_result_line_is_the_reply() {
    let home = Scratch::new("plan-last");
    let output = json!({"answer":"B"});
    let stub = Stub::replying(
        &stream(&[ok_reply(&json!({"answer":"A"})), ok_reply(&output)]),
        0,
    );
    let session = session(&home.0);
    let schema = schema();
    assert_eq!(
        stub.claude()
            .call(&session, &call("records", &schema))
            .unwrap()
            .output,
        output
    );
}

#[test]
fn an_unknown_plan_value_in_the_ledger_stops_spending() {
    for plan in [json!("maybe"), Value::Null] {
        let home = Scratch::new("plan-damaged");
        let mut line: Value =
            serde_json::from_str(&spent_line(&local_day(fixed_time()), 0.0)).unwrap();
        line["plan"] = plan;
        seed_ledger(&home.0, &[line.to_string()]);
        let session = session(&home.0);
        let expected = text::ledger_damaged(&state(&home.0).join("runs.jsonl"));
        assert_eq!(session.spent_today().unwrap_err().to_string(), expected);
        let stub = Stub::replying(&ok_reply(&json!({})), 0);
        let schema = schema();
        assert_eq!(
            stub.claude()
                .call(&session, &call("records", &schema))
                .unwrap_err()
                .to_string(),
            expected
        );
        assert!(!stub.ran());
    }
}
