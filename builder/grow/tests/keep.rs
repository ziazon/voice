mod common;
use common::*;
use persona_builder_grow::{keep::keep, record::*, store::Store};
use persona_builder_ingest::filters::Filters;
use serde_json::{Value, json};
use std::fs;
fn run(
    f: &Fixture,
    input: &Value,
) -> persona_builder::error::Result<persona_builder_grow::keep::Kept> {
    keep(
        &Store::open(&f.home, &f.member).unwrap(),
        &f.member,
        &Filters::compile(&f.member.filters).unwrap(),
        &serde_json::to_vec(input).unwrap(),
        NOW,
    )
}
#[test]
fn keeps_linked_records_and_goldens_with_default_subject() {
    let f = Fixture::new();
    let kept = run(&f, &input());
    assert!(kept.is_ok(), "{kept:?}");
    let kept = kept.unwrap();
    assert_eq!(kept.signal.given_by, GivenBy::Subject);
    assert_eq!(kept.case.from_signals, vec![kept.signal.id.clone()]);
    assert_eq!(kept.case.expected, kept.signal.wanted);
    assert_eq!(kept.case.situation, kept.signal.situation);
    assert_eq!(kept.case.r#match, Match::Rubric);
    assert_eq!(kept.case.status, Status::New);
    assert_eq!(kept.case.options, None);
    for (kind, actual) in [
        ("signal", serde_json::to_value(&kept.signal).unwrap()),
        ("case", serde_json::to_value(&kept.case).unwrap()),
    ] {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/golden/{kind}.json"));
        assert_eq!(
            actual,
            serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()
        );
    }
    let store = Store::open(&f.home, &f.member).unwrap();
    assert_eq!(store.signals().unwrap(), vec![kept.signal]);
    assert_eq!(store.case_lines().unwrap(), vec![kept.case]);
    store.check_all().unwrap();
}
#[test]
fn answers_get_distinct_daily_ids_and_preserve_staff() {
    let f = Fixture::new();
    let first = run(&f, &input()).unwrap();
    let mut v = input();
    v["given_by"] = json!("staff");
    let second = run(&f, &v).unwrap();
    assert_eq!(first.signal.id, "sig-2026-03-04-001");
    assert_eq!(second.signal.id, "sig-2026-03-04-002");
    assert_eq!(second.case.id, "case-2026-03-04-002");
    assert_eq!(second.signal.given_by, GivenBy::Staff);
}
fn refused(v: Value) {
    let f = Fixture::new();
    let result = run(&f, &v);
    assert!(result.is_err(), "must refuse {v:?}");
    let message = result.unwrap_err().to_string();
    if ["schema", "id", "persona", "at"]
        .iter()
        .any(|key| v.get(key).is_some())
    {
        assert_eq!(message, persona_builder_grow::text::FILLED_IN);
    }
    assert!(!message.contains("zebracorn"), "{message}");
    assert!(
        !f.home.join(".claude").exists(),
        "refusal created private state"
    );
}
macro_rules! refusal {
    ($name:ident,$edit:expr) => {
        #[test]
        fn $name() {
            let mut v = input();
            ($edit)(&mut v);
            refused(v);
        }
    };
}
refusal!(filled_schema, |v: &mut Value| v["schema"] =
    json!("zebracorn"));
refusal!(filled_id, |v: &mut Value| v["id"] = json!("zebracorn"));
refusal!(filled_persona, |v: &mut Value| v["persona"] =
    json!("zebracorn"));
refusal!(filled_at, |v: &mut Value| v["at"] = json!("zebracorn"));
#[test]
fn unknown_key() {
    let f = Fixture::new();
    let mut v = input();
    v["app_extra"] = json!("zebracorn");
    assert_eq!(
        run(&f, &v).unwrap_err().to_string(),
        persona_builder_grow::text::unknown_key("app_extra")
    );
    assert!(!f.home.join(".claude").exists());
}
refusal!(unknown_area, |v: &mut Value| v["area"] = json!("zebracorn"));
refusal!(too_long, |v: &mut Value| v["wanted"] =
    json!(format!("zebracorn{}", "a".repeat(2001))));
refusal!(empty, |v: &mut Value| v["situation"] = json!(" \t\n"));
refusal!(control, |v: &mut Value| v["wanted"] = json!("zebracorn\r"));
refusal!(hidden, |v: &mut Value| v["persona_said"] =
    json!("zebracorn\u{200b}"));
refusal!(personal, |v: &mut Value| v["situation"] =
    json!("zebracorn my doctor"));
refusal!(secret, |v: &mut Value| v["wanted"] =
    json!("zebracorn password"));
#[test]
fn too_big() {
    let f = Fixture::new();
    let mut v = input();
    v["wanted"] = json!("zebracorn".repeat(8000));
    assert_eq!(
        run(&f, &v).unwrap_err().to_string(),
        persona_builder_grow::text::TOO_BIG
    );
    assert!(!f.home.join(".claude").exists());
}
refusal!(bad_kind, |v: &mut Value| v["kind"] = json!("zebracorn"));
refusal!(null_optional, |v: &mut Value| v["source"] = Value::Null);
refusal!(verdict_missing, |v: &mut Value| v["kind"] =
    json!("verdict"));
refusal!(verdict_extra, |v: &mut Value| v["verdict"] = json!("agree"));
#[test]
fn every_text_field_is_checked() {
    for field in ["area", "situation", "wanted", "persona_said", "source"] {
        for words in ["zebracorn my doctor", "zebracorn password"] {
            let mut f = Fixture::new();
            let mut v = input();
            v[field] = json!(words);
            if field == "area" {
                f.member
                    .codebook
                    .as_mut()
                    .unwrap()
                    .categories
                    .push(words.into());
            }
            let result = run(&f, &v);
            assert!(result.is_err(), "{field}");
            assert!(!result.unwrap_err().to_string().contains("zebracorn"));
            assert!(!f.growth().exists());
        }
    }
}
#[test]
fn text_limits_are_in_characters_and_allow_newlines_tabs() {
    let f = Fixture::new();
    let mut v = input();
    v["wanted"] = json!("é".repeat(2000));
    v["situation"] = json!("A quiz\nnext\tweek");
    v["source"] = json!("é".repeat(100));
    assert!(run(&f, &v).is_ok());
}
#[test]
fn all_optional_text_limits_and_empty_values_are_checked() {
    for field in ["situation", "wanted", "persona_said", "source"] {
        for bad in [
            " \t".to_string(),
            "zebracorn\u{202e}".into(),
            "x".repeat(if field == "source" { 101 } else { 2001 }),
        ] {
            let mut v = input();
            v[field] = json!(bad);
            refused(v);
        }
    }
}
#[test]
fn malformed_input_and_bad_time_write_nothing() {
    let f = Fixture::new();
    let store = Store::open(&f.home, &f.member).unwrap();
    let filters = Filters::compile(&f.member.filters).unwrap();
    for bytes in [b"{zebracorn".as_slice(), b"[]", b"{}"] {
        assert!(keep(&store, &f.member, &filters, bytes, NOW).is_err());
    }
    assert!(
        keep(
            &store,
            &f.member,
            &filters,
            &serde_json::to_vec(&input()).unwrap(),
            "zebracorn"
        )
        .is_err()
    );
    assert!(!f.growth().exists());
}

#[test]
fn keeping_refuses_an_already_held_lock_without_writing() {
    let f = Fixture::new();
    let _lock = persona_builder_ingest::private::lock_state(&f.home, &f.member.slug).unwrap();
    assert!(run(&f, &input()).is_err());
    assert!(!f.growth().exists());
}

#[test]
fn summary_counts_statuses_in_category_order_without_words() {
    use persona_builder_grow::{rules::CaseBook, text};
    let mut base: Case = serde_json::from_str(include_str!("golden/case.json")).unwrap();
    base.situation = "zebracorn".into();
    let mut active = base.clone();
    active.id = "case-active".into();
    active.area = "scope".into();
    active.status = Status::Active;
    let mut retired = base.clone();
    retired.id = "case-retired".into();
    retired.status = Status::Retired;
    let book = CaseBook::new(&[base, active, retired]).unwrap();
    assert_eq!(
        text::cases_summary(&book, &["scope".into(), "communication".into()]),
        "Tests made from your answers: 1 waiting for a refresh, 1 in use, 1 retired.\nscope: 0 waiting for a refresh, 1 in use, 0 retired.\ncommunication: 1 waiting for a refresh, 0 in use, 1 retired."
    );
}

#[test]
fn unknown_keys_are_capped_at_forty_characters() {
    let f = Fixture::new();
    let mut value = input();
    let key = format!("{}zebracorn", "q".repeat(40));
    value[&key] = json!("private words");
    let message = run(&f, &value).unwrap_err().to_string();
    assert!(message.contains(&"q".repeat(40)));
    assert!(!message.contains("zebracorn"));
    assert!(!message.contains("private words"));
    assert!(!f.home.join(".claude").exists());
}

#[test]
fn next_answer_recovers_when_an_earlier_signal_has_no_case() {
    let f = Fixture::new();
    let store = Store::open(&f.home, &f.member).unwrap();
    let orphan: Signal = serde_json::from_str(include_str!("golden/signal.json")).unwrap();
    store.append_signal(&orphan).unwrap();
    let result = run(&f, &input());
    assert!(
        result.is_ok(),
        "a signal left by a failed case write must not block the next answer: {result:?}"
    );
    let kept = result.unwrap();
    assert_eq!(kept.signal.id, "sig-2026-03-04-002");
    assert_eq!(kept.case.id, "case-2026-03-04-001");
    assert_eq!(kept.case.from_signals, vec![kept.signal.id]);
    assert_eq!(store.signals().unwrap().len(), 2);
    assert_eq!(store.case_lines().unwrap().len(), 1);
    store.check_all().unwrap();
}
