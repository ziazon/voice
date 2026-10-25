use persona_builder_grow::record::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

fn spec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/growth-loop")
}
fn example(kind: &str) -> Value {
    serde_json::from_slice(&fs::read(spec().join(format!("examples/{kind}.json"))).unwrap())
        .unwrap()
}
fn same_keys(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
            for key in b.keys() {
                same_keys(&a[key], &b[key]);
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                same_keys(a, b);
            }
        }
        _ => {}
    }
}
fn round_trip<T: Record + Serialize + DeserializeOwned>() {
    let original = example(T::KIND);
    let parsed = parse::<T>(&serde_json::to_vec(&original).unwrap());
    assert!(
        parsed.is_ok(),
        "{}: {parsed_error}",
        T::KIND,
        parsed_error = parsed.err().map(|e| e.to_string()).unwrap_or_default()
    );
    let record = parse::<T>(&serde_json::to_vec(&original).unwrap()).unwrap();
    record.check().unwrap();
    let pretty = to_pretty(&record);
    assert!(pretty.ends_with('\n'));
    let actual: Value = serde_json::from_str(&pretty).unwrap();
    assert_eq!(actual, original);
    // The signal example predates the table's wanted/persona_said order.
    let mut expected_keys: Vec<_> = original.as_object().unwrap().keys().cloned().collect();
    if T::KIND == "signal" {
        let wanted = expected_keys
            .iter()
            .position(|key| key == "wanted")
            .unwrap();
        let said = expected_keys
            .iter()
            .position(|key| key == "persona_said")
            .unwrap();
        expected_keys.swap(wanted, said);
    }
    assert_eq!(
        actual
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        expected_keys
    );
    for key in original.as_object().unwrap().keys() {
        same_keys(&actual[key], &original[key]);
    }
    let line = to_line(&record);
    assert_eq!(line.lines().count(), 1);
    assert!(line.ends_with('\n'));
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap(), original);
}

#[test]
fn examples_round_trip() {
    round_trip::<Signal>();
    round_trip::<Case>();
    round_trip::<Proposal>();
    round_trip::<Approval>();
    round_trip::<Measurement>();
}

fn schema(kind: &str) -> Value {
    serde_json::from_slice(&fs::read(spec().join(format!("{kind}.schema.json"))).unwrap()).unwrap()
}
fn parity(value: &Value, field: &str, kind: &str) {
    match value {
        Value::Object(object) => {
            if let Some(pattern) = object.get("pattern") {
                let expected = match field {
                    "timestamp" => TIMESTAMP.to_owned(),
                    "persona" => PERSONA.to_owned(),
                    "file" => FILE.to_owned(),
                    "id" => id_pattern(match kind {
                        "signal" => "sig",
                        "case" => "case",
                        "proposal" => "prop",
                        "approval" => "appr",
                        "measurement" => "meas",
                        _ => panic!("unmapped kind"),
                    }),
                    "from_signals" => id_pattern("sig"),
                    "cases" | "case" => id_pattern("case"),
                    "proposal" => id_pattern("prop"),
                    "approval" => id_pattern("appr"),
                    _ => panic!("unmapped pattern at {field}"),
                };
                assert_eq!(pattern, &json!(expected), "pattern at {field}");
            }
            if let Some(values) = object.get("enum") {
                let expected = match field {
                    "given_by" | "role" => json!([GivenBy::Subject, GivenBy::Staff]),
                    "kind" => json!([
                        SignalKind::Pick,
                        SignalKind::Correction,
                        SignalKind::Verdict,
                        SignalKind::Instruction
                    ]),
                    "verdict" => json!([Verdict::Agree, Verdict::Disagree]),
                    "match" => json!([Match::Choice, Match::Rubric]),
                    "status" => json!([Status::New, Status::Active, Status::Retired]),
                    "persona_kind" => json!([PersonaKind::Person, PersonaKind::Product]),
                    "decision" => json!([Decision::Approved, Decision::Rejected]),
                    "before" | "after" => json!([Grade::Pass, Grade::Fail]),
                    "outcome" => json!([KeepOrRevert::Keep, KeepOrRevert::Revert]),
                    _ => panic!("unmapped enum at {field}"),
                };
                assert_eq!(values, &expected, "enum at {field}");
            }
            for (key, child) in object {
                parity(child, if key == "items" { field } else { key }, kind);
            }
        }
        Value::Array(items) => {
            for item in items {
                parity(item, field, kind);
            }
        }
        _ => {}
    }
}

#[test]
fn schemas_match_patterns_and_enums() {
    for kind in ["signal", "case", "proposal", "approval", "measurement"] {
        parity(&schema(kind), "", kind);
    }
}

fn refused<T: Record>(mut value: Value, field: &str) {
    let private_field = match T::KIND {
        "signal" | "case" => {
            if field == "situation" {
                "area"
            } else {
                "situation"
            }
        }
        "proposal" => {
            if field == "summary" {
                "base"
            } else {
                "summary"
            }
        }
        "approval" => "note",
        _ => {
            if field == "reason" {
                "candidate"
            } else {
                "reason"
            }
        }
    };
    if field != private_field {
        value[private_field] = json!("zebracorn");
    }
    let result = parse::<T>(&serde_json::to_vec(&value).unwrap());
    assert!(result.is_err(), "{} {field} was accepted", T::KIND);
    let error = result.err().unwrap();
    assert!(matches!(error, persona_builder::error::Error::Invalid(_)));
    let message = error.to_string();
    assert!(message.contains(T::KIND), "{message}");
    assert!(
        message.starts_with(&format!("the {}\'s {field} ", T::KIND)),
        "{message}"
    );
    assert!(!message.contains("zebracorn"), "{message}");
}

#[test]
fn signal_schema_version() {
    let mut value = example("signal");
    value["schema"] = json!("zebracorn");
    refused::<Signal>(value, "schema");
}

#[test]
fn signal_id_pattern() {
    let mut value = example("signal");
    value["id"] = json!("zebracorn!");
    refused::<Signal>(value, "id");
}

#[test]
fn signal_persona_pattern() {
    let mut value = example("signal");
    value["persona"] = json!("zebracorn!");
    refused::<Signal>(value, "persona");
}

#[test]
fn signal_at_pattern() {
    let mut value = example("signal");
    value["at"] = json!("zebracorn!");
    refused::<Signal>(value, "at");
}

#[test]
fn signal_area_nonempty() {
    let mut value = example("signal");
    value["area"] = json!("");
    refused::<Signal>(value, "area");
}

#[test]
fn signal_situation_nonempty() {
    let mut value = example("signal");
    value["situation"] = json!("");
    refused::<Signal>(value, "situation");
}

#[test]
fn signal_wanted_nonempty() {
    let mut value = example("signal");
    value["wanted"] = json!("");
    refused::<Signal>(value, "wanted");
}

#[test]
fn signal_persona_said_nonempty() {
    let mut value = example("signal");
    value["persona_said"] = json!("");
    refused::<Signal>(value, "persona_said");
}

#[test]
fn signal_source_nonempty() {
    let mut value = example("signal");
    value["source"] = json!("");
    refused::<Signal>(value, "source");
}

#[test]
fn case_schema_version() {
    let mut value = example("case");
    value["schema"] = json!("zebracorn");
    refused::<Case>(value, "schema");
}

#[test]
fn case_id_pattern() {
    let mut value = example("case");
    value["id"] = json!("zebracorn!");
    refused::<Case>(value, "id");
}

#[test]
fn case_persona_pattern() {
    let mut value = example("case");
    value["persona"] = json!("zebracorn!");
    refused::<Case>(value, "persona");
}

#[test]
fn case_created_pattern() {
    let mut value = example("case");
    value["created"] = json!("zebracorn!");
    refused::<Case>(value, "created");
}

#[test]
fn case_from_signals_minimum() {
    let mut value = example("case");
    value["from_signals"] = json!([]);
    refused::<Case>(value, "from_signals");
}

#[test]
fn case_from_signals_unique() {
    let mut value = example("case");
    value["from_signals"] = json!(["sig-zebracorn", "sig-zebracorn"]);
    refused::<Case>(value, "from_signals");
}

#[test]
fn case_from_signals_item_pattern() {
    let mut value = example("case");
    value["from_signals"] = json!(["zebracorn!"]);
    refused::<Case>(value, "from_signals");
}

#[test]
fn case_area_nonempty() {
    let mut value = example("case");
    value["area"] = json!("");
    refused::<Case>(value, "area");
}

#[test]
fn case_situation_nonempty() {
    let mut value = example("case");
    value["situation"] = json!("");
    refused::<Case>(value, "situation");
}

#[test]
fn case_options_minimum() {
    let mut value = example("case");
    value["options"] = json!(["zebracorn"]);
    value["expected"] = json!("zebracorn");
    refused::<Case>(value, "options");
}

#[test]
fn case_options_unique() {
    let mut value = example("case");
    value["options"] = json!(["zebracorn", "zebracorn"]);
    value["expected"] = json!("zebracorn");
    refused::<Case>(value, "options");
}

#[test]
fn case_options_item_nonempty() {
    let mut value = example("case");
    value["options"] = json!(["", "zebracorn"]);
    refused::<Case>(value, "options");
}

#[test]
fn case_expected_nonempty() {
    let mut value = example("case");
    value["match"] = json!("rubric");
    value["expected"] = json!("");
    refused::<Case>(value, "expected");
}

#[test]
fn proposal_schema_version() {
    let mut value = example("proposal");
    value["schema"] = json!("zebracorn");
    refused::<Proposal>(value, "schema");
}

#[test]
fn proposal_id_pattern() {
    let mut value = example("proposal");
    value["id"] = json!("zebracorn!");
    refused::<Proposal>(value, "id");
}

#[test]
fn proposal_persona_pattern() {
    let mut value = example("proposal");
    value["persona"] = json!("zebracorn!");
    refused::<Proposal>(value, "persona");
}

#[test]
fn proposal_created_pattern() {
    let mut value = example("proposal");
    value["created"] = json!("zebracorn!");
    refused::<Proposal>(value, "created");
}

#[test]
fn proposal_base_nonempty() {
    let mut value = example("proposal");
    value["base"] = json!("");
    refused::<Proposal>(value, "base");
}

#[test]
fn proposal_summary_nonempty() {
    let mut value = example("proposal");
    value["summary"] = json!("");
    refused::<Proposal>(value, "summary");
}

#[test]
fn proposal_changes_minimum() {
    let mut value = example("proposal");
    value["changes"] = json!([]);
    refused::<Proposal>(value, "changes");
}

#[test]
fn proposal_entry_file_pattern() {
    let mut value = example("proposal");
    value["changes"][0]["file"] = json!("zebracorn!");
    refused::<Proposal>(value, "changes[].file");
}

#[test]
fn proposal_entry_diff_nonempty() {
    let mut value = example("proposal");
    value["changes"][0]["diff"] = json!("");
    refused::<Proposal>(value, "changes[].diff");
}

#[test]
fn proposal_entry_why_nonempty() {
    let mut value = example("proposal");
    value["changes"][0]["why"] = json!("");
    refused::<Proposal>(value, "changes[].why");
}

#[test]
fn proposal_cases_minimum() {
    let mut value = example("proposal");
    value["cases"] = json!([]);
    refused::<Proposal>(value, "cases");
}

#[test]
fn proposal_cases_unique() {
    let mut value = example("proposal");
    value["cases"] = json!(["case-zebracorn", "case-zebracorn"]);
    refused::<Proposal>(value, "cases");
}

#[test]
fn proposal_cases_item_pattern() {
    let mut value = example("proposal");
    value["cases"] = json!(["zebracorn!"]);
    refused::<Proposal>(value, "cases");
}

#[test]
fn approval_schema_version() {
    let mut value = example("approval");
    value["schema"] = json!("zebracorn");
    refused::<Approval>(value, "schema");
}

#[test]
fn approval_id_pattern() {
    let mut value = example("approval");
    value["id"] = json!("zebracorn!");
    refused::<Approval>(value, "id");
}

#[test]
fn approval_proposal_pattern() {
    let mut value = example("approval");
    value["proposal"] = json!("zebracorn!");
    refused::<Approval>(value, "proposal");
}

#[test]
fn approval_at_pattern() {
    let mut value = example("approval");
    value["at"] = json!("zebracorn!");
    refused::<Approval>(value, "at");
}

#[test]
fn approval_note_nonempty() {
    let mut value = example("approval");
    value["note"] = json!("");
    refused::<Approval>(value, "note");
}

#[test]
fn measurement_schema_version() {
    let mut value = example("measurement");
    value["schema"] = json!("zebracorn");
    refused::<Measurement>(value, "schema");
}

#[test]
fn measurement_id_pattern() {
    let mut value = example("measurement");
    value["id"] = json!("zebracorn!");
    refused::<Measurement>(value, "id");
}

#[test]
fn measurement_proposal_pattern() {
    let mut value = example("measurement");
    value["proposal"] = json!("zebracorn!");
    refused::<Measurement>(value, "proposal");
}

#[test]
fn measurement_approval_pattern() {
    let mut value = example("measurement");
    value["approval"] = json!("zebracorn!");
    refused::<Measurement>(value, "approval");
}

#[test]
fn measurement_at_pattern() {
    let mut value = example("measurement");
    value["at"] = json!("zebracorn!");
    refused::<Measurement>(value, "at");
}

#[test]
fn measurement_base_nonempty() {
    let mut value = example("measurement");
    value["base"] = json!("");
    refused::<Measurement>(value, "base");
}

#[test]
fn measurement_candidate_nonempty() {
    let mut value = example("measurement");
    value["candidate"] = json!("");
    refused::<Measurement>(value, "candidate");
}

#[test]
fn measurement_results_minimum() {
    let mut value = example("measurement");
    value["results"] = json!([]);
    refused::<Measurement>(value, "results");
}

#[test]
fn measurement_entry_case_pattern() {
    let mut value = example("measurement");
    value["results"][0]["case"] = json!("zebracorn!");
    refused::<Measurement>(value, "results[].case");
}

#[test]
fn measurement_reason_nonempty() {
    let mut value = example("measurement");
    value["reason"] = json!("");
    refused::<Measurement>(value, "reason");
}

#[test]
fn verdict_requires_kind() {
    let mut value = example("signal");
    value["verdict"] = json!("agree");
    refused::<Signal>(value, "verdict");
}

#[test]
fn choice_requires_options() {
    let mut value = example("case");
    value.as_object_mut().unwrap().remove("options");
    refused::<Case>(value, "options");
}

#[test]
fn expected_is_a_choice() {
    let mut value = example("case");
    value["expected"] = json!("zebracorn");
    refused::<Case>(value, "expected");
}

#[test]
fn case_once() {
    let mut value = example("measurement");
    let item = value["results"][0].clone();
    value["results"].as_array_mut().unwrap().push(item);
    refused::<Measurement>(value, "results");
}

#[test]
fn optional_fields_reject_null() {
    for (kind, fields) in [
        ("signal", vec!["persona_said", "verdict", "source"]),
        ("case", vec!["options"]),
        ("approval", vec!["note"]),
    ] {
        for field in fields {
            let mut value = example(kind);
            value[field] = Value::Null;
            let bytes = serde_json::to_vec(&value).unwrap();
            let error = match kind {
                "signal" => parse::<Signal>(&bytes).err(),
                "case" => parse::<Case>(&bytes).err(),
                _ => parse::<Approval>(&bytes).err(),
            };
            assert!(error.is_some(), "{kind} {field} accepted null");
            assert!(error.unwrap().to_string().contains("shape"));
        }
    }
}

#[test]
fn shape_errors_never_quote_unknown_keys_or_values() {
    fn check<T: Record>(mut value: Value) {
        value["zebracorn"] = json!("zebracorn");
        let message = parse::<T>(&serde_json::to_vec(&value).unwrap())
            .err()
            .unwrap()
            .to_string();
        assert!(!message.contains("zebracorn"), "{message}");
        assert!(
            message.starts_with(&format!(
                "the {} isn't in the growth loop's {} shape (line ",
                T::KIND,
                T::KIND
            )),
            "{message}"
        );
        assert!(message.contains(", column "));
    }
    check::<Signal>(example("signal"));
    check::<Case>(example("case"));
    check::<Proposal>(example("proposal"));
    check::<Approval>(example("approval"));
    check::<Measurement>(example("measurement"));
    for (kind, array) in [("proposal", "changes"), ("measurement", "results")] {
        let mut value = example(kind);
        value[array][0]["zebracorn"] = json!("zebracorn");
        let bytes = serde_json::to_vec(&value).unwrap();
        let error = if kind == "proposal" {
            parse::<Proposal>(&bytes).err().unwrap()
        } else {
            parse::<Measurement>(&bytes).err().unwrap()
        };
        assert!(!error.to_string().contains("zebracorn"), "{error}");
    }
    let mut value = example("signal");
    value["kind"] = json!("zebracorn");
    let error = parse::<Signal>(&serde_json::to_vec(&value).unwrap())
        .err()
        .unwrap();
    assert!(!error.to_string().contains("zebracorn"), "{error}");
}

#[test]
fn verdict_kind_requires_verdict_and_rubric_needs_no_options() {
    let mut value = example("signal");
    value["kind"] = json!("verdict");
    refused::<Signal>(value.clone(), "verdict");
    value["verdict"] = json!("agree");
    assert!(parse::<Signal>(&serde_json::to_vec(&value).unwrap()).is_ok());
    let mut value = example("case");
    value["match"] = json!("rubric");
    value.as_object_mut().unwrap().remove("options");
    assert!(parse::<Case>(&serde_json::to_vec(&value).unwrap()).is_ok());
    let mut value = example("signal");
    value.as_object_mut().unwrap().remove("persona_said");
    value.as_object_mut().unwrap().remove("source");
    let signal = parse::<Signal>(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&to_line(&signal)).unwrap(),
        value
    );
}

#[test]
fn schemas_pin_id_prefixes() {
    for kind in ["signal", "case", "proposal", "approval", "measurement"] {
        let mut changed = schema(kind);
        changed["properties"]["id"]["pattern"] = json!("^other-[a-z0-9]+(-[a-z0-9]+)*$");
        assert!(
            std::panic::catch_unwind(|| parity(&changed, "", kind)).is_err(),
            "{kind} prefix drift was accepted"
        );
    }
}
