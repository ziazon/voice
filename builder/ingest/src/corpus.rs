//! Combine filtered corpora and prepare the two sides of a held-out test.
use crate::{
    filters::{Filters, Kind},
    mine, privacy,
    python::py_space,
};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    History,
    Train,
    Round,
}
pub struct Input<'a> {
    pub name: &'a str,
    pub mode: Mode,
    pub bytes: &'a [u8],
}
#[derive(Debug, Default)]
pub struct Merged {
    pub records: Vec<Map<String, Value>>,
    pub questions: usize,
    pub held: usize,
    pub messages: usize,
    pub files: usize,
    pub duplicates: usize,
    pub cutoff: Option<String>,
    pub personal: usize,
    pub secret: usize,
}
fn invalid(name: &str, line: usize, message: &str) -> Error {
    Error::Invalid(format!("{name}:{line}: {message}"))
}
fn shape(name: &str, line: usize, key: &str) -> Error {
    invalid(
        name,
        line,
        &format!("a record has an unexpected shape ({key})"),
    )
}
/// True when every number in the value is a whole number serde_json keeps exactly.
pub fn exact_numbers(value: &Value) -> bool {
    match value {
        Value::Number(n) => !n.is_f64() && (n.as_i64().is_some() || n.as_u64().is_some()),
        Value::Array(a) => a.iter().all(exact_numbers),
        Value::Object(o) => o.values().all(exact_numbers),
        _ => true,
    }
}
pub fn read_records(name: &str, bytes: &[u8]) -> Result<Vec<(usize, Map<String, Value>)>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Invalid(format!("{name} is not UTF-8 text")))?;
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    text.split('\n')
        .enumerate()
        .filter(|(_, line)| !line.chars().all(py_space))
        .map(|(index, line)| {
            let line_number = index + 1;
            let value: Value = serde_json::from_str(line)
                .map_err(|_| invalid(name, line_number, "a line is not valid JSON"))?;
            if !value.is_object() {
                return Err(invalid(name, line_number, "a line is not a JSON object"));
            }
            if !exact_numbers(&value) {
                return Err(invalid(
                    name,
                    line_number,
                    "a record holds a number Persona Builder can't copy exactly",
                ));
            }
            let Value::Object(record) = value else {
                unreachable!()
            };
            Ok((line_number, record))
        })
        .collect()
}
pub fn merge(filters: &Filters, inputs: &[Input], split: &mine::Split) -> Result<Merged> {
    if inputs.is_empty() {
        return Err(Error::Invalid("merge needs at least one input".into()));
    }
    if matches!(split, mine::Split::Holdout(h) if !h.is_finite() || !(0.0..=1.0).contains(h)) {
        return Err(Error::Invalid(
            "--holdout must be a number from 0 to 1".into(),
        ));
    }
    let mut merged = Merged {
        files: inputs.len(),
        ..Merged::default()
    };
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    for mode in [Mode::History, Mode::Train, Mode::Round] {
        for input in inputs.iter().filter(|input| input.mode == mode) {
            for (line, record) in read_records(input.name, input.bytes)? {
                let fixed = match mode {
                    Mode::History => None,
                    Mode::Train => Some("train".to_owned()),
                    Mode::Round => match record.get("split").and_then(Value::as_str) {
                        Some(s @ ("train" | "heldout")) => Some(s.to_owned()),
                        _ => {
                            return Err(invalid(
                                input.name,
                                line,
                                "a round record's split must be train or heldout",
                            ));
                        }
                    },
                };
                let id = record
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| shape(input.name, line, "id"))?;
                if !seen.insert(id.to_owned()) {
                    merged.duplicates += 1;
                    continue;
                }
                if record.get("kind").and_then(Value::as_str).is_none() {
                    return Err(shape(input.name, line, "kind"));
                }
                let value = Value::Object(record);
                match privacy::privacy(filters, &value)
                    .map_err(|e| invalid(input.name, line, &e.to_string()))?
                {
                    Some(Kind::Personal) => {
                        merged.personal += 1;
                        continue;
                    }
                    Some(Kind::Secret) => {
                        merged.secret += 1;
                        continue;
                    }
                    None => {}
                }
                let Value::Object(record) = value else {
                    unreachable!()
                };
                if record.get("timestamp").and_then(Value::as_str).is_none() {
                    return Err(shape(input.name, line, "timestamp"));
                }
                kept.push((record, fixed));
            }
        }
    }
    kept.sort_by(|(a, _), (b, _)| {
        (a["timestamp"].as_str(), a["id"].as_str())
            .cmp(&(b["timestamp"].as_str(), b["id"].as_str()))
    });
    let mut history: Vec<_> = kept
        .iter_mut()
        .filter(|(_, fixed)| fixed.is_none())
        .map(|(record, _)| std::mem::take(record))
        .collect();
    merged.cutoff = mine::mark_split(&mut history, split);
    let mut history = history.into_iter();
    for (mut record, fixed) in kept {
        if let Some(fixed) = fixed {
            record.insert("split".into(), Value::from(fixed));
        } else {
            record = history
                .next()
                .expect("one marked record per history record");
        }
        if record["kind"] == "question" {
            merged.questions += 1;
            merged.held += usize::from(record["split"] == "heldout");
        } else {
            merged.messages += 1;
        }
        merged.records.push(record);
    }
    Ok(merged)
}
pub fn merge_summary(m: &Merged) -> String {
    format!(
        "merged {} questions ({} held out) and {} messages from {} files; {} duplicates dropped; held out from {}\ndropped {} personal records, {} secret records\n",
        m.questions,
        m.held,
        m.messages,
        m.files,
        m.duplicates,
        m.cutoff
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("nothing"),
        m.personal,
        m.secret
    )
}
pub fn discover(names: &[String], out: &str) -> Vec<(String, Mode)> {
    let mut result: Vec<_> = names
        .iter()
        .filter(|s| s.starts_with("decisions-") && s.ends_with(".jsonl") && s.as_str() != out)
        .map(|s| {
            let mode = if s == "decisions-interview.jsonl" {
                Mode::Train
            } else if s.starts_with("decisions-round-") {
                Mode::Round
            } else {
                Mode::History
            };
            (s.clone(), mode)
        })
        .collect();
    result.sort_by(|(a, am), (b, bm)| (*am as u8, a).cmp(&(*bm as u8, b)));
    result
}
#[derive(Debug)]
pub struct Heldout {
    pub questions: Vec<Map<String, Value>>,
    pub answers: Vec<Map<String, Value>>,
}
pub fn split(member: &Member, name: &str, bytes: &[u8]) -> Result<Heldout> {
    let mut heldout = Heldout {
        questions: vec![],
        answers: vec![],
    };
    let records = read_records(name, bytes)?;
    let mut held = Vec::new();
    for (line, record) in &records {
        let kind = record
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| shape(name, *line, "kind"))?;
        if kind == "question" {
            let split = record
                .get("split")
                .ok_or_else(|| shape(name, *line, "split"))?;
            if split == "heldout" {
                held.push((*line, record));
            }
        }
    }
    let project =
        |line, record: &Map<String, Value>, keys: &[&str]| -> Result<Map<String, Value>> {
            keys.iter()
                .map(|key| {
                    Ok((
                        (*key).to_owned(),
                        record
                            .get(*key)
                            .ok_or_else(|| shape(name, line, key))?
                            .clone(),
                    ))
                })
                .collect()
        };
    for (line, record) in &held {
        heldout.questions.push(project(
            *line,
            record,
            &[
                "id",
                "timestamp",
                "project",
                "header",
                "question",
                "multiSelect",
                "options",
                "recommended",
            ],
        )?);
    }
    for (line, record) in &held {
        heldout.answers.push(project(
            *line,
            record,
            &[
                "id",
                "recommended",
                &member.pick_field(),
                "picks",
                "own_words",
                "notes",
                "outcome",
                "multiSelect",
            ],
        )?);
    }
    Ok(heldout)
}
pub fn split_summary(heldout: &Heldout) -> String {
    format!("split {} held-out questions\n", heldout.questions.len())
}
