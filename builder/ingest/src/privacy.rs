use crate::filters::{Filters, Kind};
use persona_builder::error::{Error, Result};
use serde_json::Value;

fn lacks(key: &str) -> Error {
    Error::Invalid(format!("a record lacks {key}"))
}
fn string<'a>(record: &'a Value, key: &str) -> Result<&'a str> {
    record
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| lacks(key))
}
fn array<'a>(record: &'a Value, key: &str) -> Result<&'a [Value]> {
    record
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| lacks(key))
}

/// The exact single-space joins used by Python's searchable() and filtered().
pub fn filtered_text(record: &Value) -> Result<String> {
    let kind = string(record, "kind")?;
    let searchable = if kind == "message" {
        string(record, "text")?.to_owned()
    } else {
        let question = string(record, "question")?;
        let labels = array(record, "options")?
            .iter()
            .map(|o| string(o, "label"))
            .collect::<Result<Vec<_>>>()?
            .join(" ");
        let picks = array(record, "picks")?
            .iter()
            .map(|p| p.as_str().ok_or_else(|| lacks("picks")))
            .collect::<Result<Vec<_>>>()?
            .join(" ");
        [
            question,
            &labels,
            &picks,
            string(record, "own_words")?,
            string(record, "notes")?,
        ]
        .join(" ")
    };
    let optional = |key| match record.get(key) {
        None => Ok(""),
        Some(value) => value.as_str().ok_or_else(|| lacks(key)),
    };
    let mut text = [
        searchable.as_str(),
        optional("context")?,
        optional("project")?,
    ]
    .join(" ");
    if kind == "question" {
        let header = string(record, "header")?;
        let descriptions = array(record, "options")?
            .iter()
            .map(|o| string(o, "description"))
            .collect::<Result<Vec<_>>>()?
            .join(" ");
        text.push_str(&format!(" {header} {descriptions}"));
    }
    Ok(text)
}

pub fn privacy(filters: &Filters, record: &Value) -> Result<Option<Kind>> {
    Ok(filters.kind(&filtered_text(record)?))
}
