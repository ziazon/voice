//! `chunk`: the corpus in parts small enough to label, the held-out questions without answers.
use crate::input::{Record, Records};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::mine::to_jsonl;
use serde_json::{Map, Value};

/// The held-out question fields a labeller sees (the Python's `QUESTION_FIELDS`).
pub const QUESTION_FIELDS: [&str; 8] = [
    "id",
    "timestamp",
    "project",
    "header",
    "question",
    "multiSelect",
    "options",
    "recommended",
];

pub struct Chunks {
    pub train: Vec<Vec<Map<String, Value>>>,
    pub heldout: Vec<Vec<Map<String, Value>>>,
    pub train_records: usize,
    pub held_questions: usize,
}

pub fn chunk(name: &str, records: &Records, size: usize) -> Result<Chunks> {
    if size == 0 {
        return Err(Error::Invalid("the chunk size must be at least 1".into()));
    }
    let mut train = Vec::new();
    for r in Record::all(name, records) {
        if r.get("split")? == "train" {
            train.push(r.map.clone());
        }
    }
    let mut held = Vec::new();
    for r in Record::all(name, records) {
        if r.get("split")? == "heldout" && r.get("kind")? == "question" {
            let mut question = Map::from_iter([("kind".to_owned(), Value::from("question"))]);
            for key in QUESTION_FIELDS {
                question.insert(key.to_owned(), r.get(key)?.clone());
            }
            held.push(question);
        }
    }
    let parts = |records: Vec<Map<String, Value>>| -> Vec<Vec<Map<String, Value>>> {
        let mut records = records.into_iter().peekable();
        let mut parts = Vec::new();
        while records.peek().is_some() {
            parts.push(records.by_ref().take(size).collect());
        }
        parts
    };
    Ok(Chunks {
        train_records: train.len(),
        held_questions: held.len(),
        train: parts(train),
        heldout: parts(held),
    })
}

impl Chunks {
    /// Each part's file name and JSONL text: `train-01.jsonl`, …, then `heldout-01.jsonl`, ….
    pub fn files(&self) -> Vec<(String, String)> {
        [("train", &self.train), ("heldout", &self.heldout)]
            .into_iter()
            .flat_map(|(kind, parts)| {
                parts
                    .iter()
                    .enumerate()
                    .map(move |(n, part)| (format!("{kind}-{:02}.jsonl", n + 1), to_jsonl(part)))
            })
            .collect()
    }
    pub fn summary(&self) -> String {
        format!(
            "{} train records in {} chunks, {} held-out questions in {} chunks\n",
            self.train_records,
            self.train.len(),
            self.held_questions,
            self.heldout.len()
        )
    }
}
