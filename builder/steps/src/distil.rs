//! `distil-in`: the labelled training records, grouped by the codebook's groups, as the rows a
//! distiller reads.
use crate::{
    book::Book,
    input::{Record, Records, first},
    labels::category,
};
use persona_builder::error::{Error, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// The most characters of an option's description a row keeps.
pub const DESCRIPTION_CHARS: usize = 150;

pub struct Group {
    pub name: String,
    pub rows: Vec<Map<String, Value>>,
}

pub fn distil_in(
    book: &Book,
    corpus_name: &str,
    records: &Records,
    labels_name: &str,
    labels: &Map<String, Value>,
) -> Result<Vec<Group>> {
    // A category in two groups goes to the later one, as the Python's dict comprehension does.
    let mut group_of = HashMap::new();
    for (at, (_, categories)) in book.groups.iter().enumerate() {
        for c in categories {
            group_of.insert(c.as_str(), at);
        }
    }
    let mut groups: Vec<_> = book
        .groups
        .iter()
        .map(|(name, _)| Group {
            name: name.clone(),
            rows: Vec::new(),
        })
        .collect();
    for r in Record::all(corpus_name, records) {
        if r.get("split")? != "train" {
            continue;
        }
        let id = r.id()?;
        let label = match labels.get(id) {
            Some(Value::Object(label)) => label,
            Some(_) => {
                return Err(Error::Invalid(format!(
                    "{labels_name}: the label for {id} is not a JSON object"
                )));
            }
            None => return Err(Error::Invalid(format!("{labels_name}: no label for {id}"))),
        };
        let name = category(labels_name, labels, id)?;
        let Some(&at) = group_of.get(name) else {
            return Err(Error::Invalid(format!(
                "category {name} is in no codebook group"
            )));
        };
        groups[at].rows.push(row(r, label, labels_name, id)?);
    }
    Ok(groups)
}

fn row(
    r: Record,
    label: &Map<String, Value>,
    labels_name: &str,
    id: &str,
) -> Result<Map<String, Value>> {
    let from_label = |key: &str| {
        label.get(key).cloned().ok_or_else(|| {
            Error::Invalid(format!("{labels_name}: the label for {id} has no {key}"))
        })
    };
    let mut row = Map::new();
    row.insert("date".into(), Value::from(first(r.str("timestamp")?, 10)));
    row.insert("project".into(), r.get("project")?.clone());
    row.insert("category".into(), from_label("category")?);
    if let Some(domain) = label.get("domain") {
        row.insert("domain".into(), domain.clone());
    }
    if r.get("kind")? == "question" {
        row.insert("q".into(), r.get("question")?.clone());
        let options: Vec<Value> = r
            .options()?
            .into_iter()
            .map(|(label, description)| {
                Value::from(if description.is_empty() {
                    label.to_owned()
                } else {
                    format!("{label} — {}", first(description, DESCRIPTION_CHARS))
                })
            })
            .collect();
        row.insert("options".into(), Value::from(options));
        row.insert("picks".into(), r.get("picks")?.clone());
        row.insert("own".into(), r.get("own_words")?.clone());
        row.insert("notes".into(), r.get("notes")?.clone());
        row.insert("outcome".into(), r.get("outcome")?.clone());
    } else {
        row.insert("msg".into(), r.get("text")?.clone());
        row.insert("signal".into(), from_label("signal")?);
    }
    row.insert("reason".into(), from_label("reason")?);
    Ok(row)
}

/// One `<group> <rows>` line per group.
pub fn summary(groups: &[Group]) -> String {
    groups
        .iter()
        .map(|g| format!("{} {}\n", g.name, g.rows.len()))
        .collect()
}
