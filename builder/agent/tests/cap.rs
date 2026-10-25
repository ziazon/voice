mod common;

use common::*;
use persona_builder_agent::{
    ledger::{Cap, local_day},
    text,
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::Arc,
    time::{Duration, SystemTime},
};

fn today() -> String {
    local_day(fixed_time())
}

fn yesterday() -> String {
    local_day(fixed_time() - Duration::from_secs(86_400))
}

#[test]
fn caps_must_be_sensible() {
    assert_eq!(
        Cap::default(),
        Cap::new(Cap::DEFAULT_DAILY_USD, Cap::DEFAULT_CALL_USD).unwrap()
    );
    let cap = Cap::new(20.0, 2.5).unwrap();
    assert_eq!((cap.daily_usd(), cap.call_usd()), (20.0, 2.5));
    Cap::new(1000.0, 1000.0).unwrap();
    for (daily, call) in [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, -1.0),
        (10.0, 11.0),
        (1000.5, 1.0),
        (f64::NAN, 1.0),
        (10.0, f64::INFINITY),
    ] {
        assert!(Cap::new(daily, call).is_err(), "{daily} {call}");
    }
}

#[test]
fn a_call_that_could_pass_the_daily_cap_never_starts() {
    let home = Scratch::new("cap-full");
    seed_ledger(
        &home.0,
        &[spent_line(&today(), 40.0), spent_line(&today(), 6.0)],
    );
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let session = common::session(&home.0);
    assert_eq!(session.spent_today().unwrap(), 46.0);
    let schema = schema();
    let error = stub
        .claude()
        .call(&session, &call("records", &schema))
        .unwrap_err();
    assert_eq!(error.to_string(), text::cap_reached(46.0, 50.0));
    assert!(error.to_string().contains("$46.00 of $50.00"));
    assert!(!stub.ran());
    assert_eq!(ledger_lines(&home.0).len(), 2);
}

#[test]
fn only_today_counts_and_a_call_up_to_the_cap_runs() {
    let home = Scratch::new("cap-room");
    seed_ledger(
        &home.0,
        &[spent_line(&yesterday(), 49.0), spent_line(&today(), 44.99)],
    );
    let stub = Stub::replying(&ok_reply(&json!({})), 0);
    let session = common::session(&home.0);
    assert_eq!(session.spent_today().unwrap(), 44.99);
    let schema = schema();
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
    let lines = ledger_lines(&home.0);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["cost_usd"], 49.0);
    assert_eq!(lines[1]["cost_usd"], 44.99);
    assert_eq!(lines[2]["day"], today());
    assert_eq!(
        fs::metadata(state(&home.0).join("runs.jsonl"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(state(&home.0)).unwrap().permissions().mode() & 0o777,
        0o700
    );

    let home = Scratch::new("cap-exact");
    seed_ledger(&home.0, &[spent_line(&today(), 45.0)]);
    let session = common::session(&home.0);
    stub.claude()
        .call(&session, &call("records", &schema))
        .unwrap();
}

#[test]
fn calls_in_flight_hold_their_budget() {
    let home = Scratch::new("cap-race");
    seed_ledger(&home.0, &[spent_line(&today(), 41.0)]);
    let stub = Arc::new(Stub::new(&format!(
        "sleep 1\ncat <<'EOF'\n{}\nEOF",
        ok_reply(&json!({}))
    )));
    let session = Arc::new(session(&home.0));
    let schema = Arc::new(schema());
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let (stub, session, schema) = (stub.clone(), session.clone(), schema.clone());
            std::thread::spawn(move || {
                stub.claude()
                    .call(&session, &call("records", &schema))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    let refused = results.iter().find_map(|r| r.clone().err()).unwrap();
    assert_eq!(refused, text::cap_reached(46.0, 50.0));
    assert_eq!(ledger_lines(&home.0).len(), 2);
}

#[test]
fn a_damaged_ledger_stops_all_spending() {
    let mut no_day: serde_json::Value =
        serde_json::from_str(&spent_line("2026-01-01", 1.0)).unwrap();
    no_day.as_object_mut().unwrap().remove("day");
    for bad in [
        "not json".to_owned(),
        spent_line(&today(), 1.0).replace("\"cost_usd\":1.0", "\"cost_usd\":\"1.0\""),
        no_day.to_string(),
        "[1]".to_owned(),
        String::new(),
    ] {
        let home = Scratch::new("cap-damaged");
        seed_ledger(&home.0, &[spent_line(&today(), 1.0), bad.clone()]);
        if bad.is_empty() {
            let text = format!(
                "{}\n\n{}\n",
                spent_line(&today(), 1.0),
                spent_line(&today(), 1.0)
            );
            fs::write(state(&home.0).join("runs.jsonl"), text).unwrap();
        }
        let stub = Stub::replying(&ok_reply(&json!({})), 0);
        let session = common::session(&home.0);
        let expected = text::ledger_damaged(&state(&home.0).join("runs.jsonl"));
        assert_eq!(
            session.spent_today().unwrap_err().to_string(),
            expected,
            "{bad}"
        );
        let schema = schema();
        let error = stub
            .claude()
            .call(&session, &call("records", &schema))
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
        assert!(!stub.ran());
    }
}

#[test]
fn the_day_is_the_local_calendar_day() {
    let out = std::process::Command::new("date")
        .arg("+%F")
        .output()
        .unwrap();
    let expected = String::from_utf8(out.stdout).unwrap();
    assert_eq!(local_day(SystemTime::now()), expected.trim());
}
