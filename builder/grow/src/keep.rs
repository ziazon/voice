//! Check an answer before keeping a signal and its new test in private state.
use crate::{
    record::*,
    rules::CaseBook,
    store::{Prefix, Store},
    text,
};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_ingest::filters::Filters;
use persona_builder_steps::{book::book, text_check::hidden};
use serde_json::Value;

#[derive(Debug)]
pub struct Kept {
    pub signal: Signal,
    pub case: Case,
}

pub fn keep(
    store: &Store,
    member: &Member,
    filters: &Filters,
    input: &[u8],
    now: &str,
) -> Result<Kept> {
    let invalid = || Error::Invalid(text::INPUT_SHAPE.into());
    if input.len() > 64 * 1024 {
        return Err(Error::Invalid(text::TOO_BIG.into()));
    }
    let mut value: Value = serde_json::from_slice(input).map_err(|_| invalid())?;
    let object = value.as_object_mut().ok_or_else(invalid)?;
    for key in object.keys() {
        if ["schema", "id", "persona", "at"].contains(&key.as_str()) {
            return Err(Error::Invalid(text::FILLED_IN.into()));
        }
        if ![
            "kind",
            "area",
            "situation",
            "wanted",
            "persona_said",
            "verdict",
            "source",
            "given_by",
        ]
        .contains(&key.as_str())
        {
            return Err(Error::Invalid(text::unknown_key(key)));
        }
    }
    object.insert("schema".into(), SIGNAL_V1.into());
    object.insert("id".into(), "sig-pending".into());
    object.insert("persona".into(), member.slug.clone().into());
    object.insert("at".into(), now.into());
    object
        .entry("given_by")
        .or_insert(Value::String("subject".into()));
    let mut signal: Signal = serde_json::from_value(value).map_err(|_| invalid())?;
    let categories = book(member)?.categories;
    for (field, words, limit) in [
        ("area", Some(signal.area.as_str()), 100),
        ("situation", Some(signal.situation.as_str()), 2000),
        ("wanted", Some(signal.wanted.as_str()), 2000),
        ("persona_said", signal.persona_said.as_deref(), 2000),
        ("source", signal.source.as_deref(), 100),
    ] {
        if let Some(words) = words {
            if words.trim().is_empty()
                || words.chars().count() > limit
                || words
                    .chars()
                    .any(|c| (c != '\n' && c != '\t') && hidden(c) && c != '<')
            {
                return Err(Error::Invalid(text::invalid_text(field, limit)));
            }
            if let Some(kind) = filters.kind(words) {
                return Err(Error::Invalid(text::left_out(field, kind.as_str())));
            }
        }
    }
    if !categories.contains(&signal.area) {
        return Err(Error::Invalid(text::unknown_area(&categories)));
    }
    signal.check()?;
    if store.slug() != member.slug {
        return Err(invalid());
    }
    let mut case = Case {
        schema: CASE_V1.into(),
        id: "case-pending".into(),
        persona: signal.persona.clone(),
        created: now.into(),
        from_signals: vec![signal.id.clone()],
        area: signal.area.clone(),
        situation: signal.situation.clone(),
        options: None,
        expected: signal.wanted.clone(),
        r#match: Match::Rubric,
        status: Status::New,
    };
    case.check()?;
    let _lock = store.lock()?;
    let day = &now[..10]; // Signal::check has checked the timestamp's ASCII date prefix.
    signal.id = store.next_id(Prefix::Signal, day)?;
    case.id = store.next_id(Prefix::Case, day)?;
    case.from_signals = vec![signal.id.clone()];
    signal.check()?;
    case.check()?;
    let signals = store.signals()?;
    let mut cases = store.case_lines()?;
    if signals.iter().any(|old| old.id == signal.id) {
        return Err(Error::Invalid(text::already_kept(&signal.id)));
    }
    cases.push(case.clone());
    CaseBook::new(&cases)?;
    store.append_signal_unlocked(&signal)?;
    store.append_case_unlocked(&case)?;
    Ok(Kept { signal, case })
}
