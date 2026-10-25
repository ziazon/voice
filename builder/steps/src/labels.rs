//! `check-labels` and `tally`: labels checked against the codebook and the corpus, and the outcomes
//! counted per category over the training split.
use crate::{
    book::{Book, one_of},
    input::{Record, Records, pretty},
    share::{percent, share},
};
use persona_builder::error::{Error, Result};
use persona_builder_ingest::python::py_space;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};

/// The most words a training label's reason may have.
pub const MAX_REASON_WORDS: usize = 25;
const MAX_PRINTED: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Train,
    Heldout,
}

/// Each record by id, the last of a repeated id winning, in the order ids first appear (the
/// Python's `{r["id"]: r for r in records}`).
struct ById<'a> {
    order: Vec<&'a str>,
    records: HashMap<&'a str, Record<'a>>,
}
impl<'a> ById<'a> {
    fn new(name: &'a str, records: &'a Records) -> Result<Self> {
        let mut by_id = ById {
            order: Vec::new(),
            records: HashMap::new(),
        };
        for r in Record::all(name, records) {
            let id = r.id()?;
            if by_id.records.insert(id, r).is_none() {
                by_id.order.push(id);
            }
        }
        Ok(by_id)
    }
    fn get(&self, id: &str) -> Option<Record<'a>> {
        self.records.get(id).copied()
    }
    fn contains(&self, id: &str) -> bool {
        self.records.contains_key(id)
    }
    fn iter(&self) -> impl Iterator<Item = (&'a str, Record<'a>)> + '_ {
        self.order.iter().map(|id| (*id, self.records[id]))
    }
}

fn words(text: &str) -> usize {
    text.split(py_space).filter(|w| !w.is_empty()).count()
}

/// The Python's `label_problems`: what is wrong with a labeller's answers for one chunk. A label
/// that is not an object, or an `escalates` that is not a list of strings, is reported, never fatal.
pub fn label_problems(
    book: &Book,
    chunk_name: &str,
    records: &Records,
    labels: &Map<String, Value>,
    mode: Mode,
) -> Result<Vec<String>> {
    let ids = ById::new(chunk_name, records)?;
    let mut problems = Vec::new();
    let missing = ids
        .order
        .iter()
        .filter(|i| !labels.contains_key(**i))
        .count();
    let extra = labels.keys().filter(|i| !ids.contains(i)).count();
    if missing > 0 || extra > 0 {
        problems.push(format!("ids differ: missing {missing}, extra {extra}"));
    }
    for (i, label) in labels {
        let Some(r) = ids.get(i) else { continue };
        let Some(label) = label.as_object() else {
            problems.push(format!("{i} label"));
            continue;
        };
        if !one_of(label.get("category"), &book.categories) {
            problems.push(format!("{i} category"));
        }
        if !book.domains.is_empty() && !one_of(label.get("domain"), &book.domains) {
            problems.push(format!("{i} domain"));
        }
        let kind = r.get("kind")?;
        if kind == "question" {
            let fits = match label.get("escalates") {
                None => true,
                Some(Value::Array(list)) => list.iter().all(|e| one_of(Some(e), &book.escalates)),
                Some(_) => false,
            };
            if !fits {
                problems.push(format!("{i} escalates"));
            }
        }
        if kind == "message" && !one_of(label.get("signal"), &book.signals) {
            problems.push(format!("{i} signal"));
        }
        let reason = label.get("reason");
        if mode == Mode::Train
            && !reason
                .and_then(Value::as_str)
                .is_some_and(|r| (1..=MAX_REASON_WORDS).contains(&words(r)))
        {
            problems.push(format!("{i} reason"));
        }
        if mode == Mode::Heldout && reason.is_some() {
            problems.push(format!("{i} has reason"));
        }
    }
    Ok(problems)
}

/// `OK`, or `PROBLEMS:` and the first 40 problems.
pub fn render_problems(problems: &[String]) -> String {
    if problems.is_empty() {
        return "OK\n".into();
    }
    let shown: Vec<_> = problems
        .iter()
        .take(MAX_PRINTED)
        .map(String::as_str)
        .collect();
    format!("PROBLEMS:\n{}\n", shown.join("\n"))
}

/// The Python's `check_labels`: every training record labelled, no label for an unknown record,
/// and no held-out label carrying a reason.
pub fn check_labels(
    corpus_name: &str,
    records: &Records,
    labels: &Map<String, Value>,
) -> Result<()> {
    let ids = ById::new(corpus_name, records)?;
    let mut missing = BTreeSet::new();
    for (i, r) in ids.iter() {
        if r.get("split")? == "train" && !labels.contains_key(i) {
            missing.insert(i);
        }
    }
    let unknown: BTreeSet<_> = labels
        .keys()
        .filter(|i| !ids.contains(i))
        .map(String::as_str)
        .collect();
    let mut peeked = BTreeSet::new();
    for (i, label) in labels {
        if let Some(r) = ids.get(i)
            && r.get("split")? == "heldout"
            && label.get("reason").is_some()
        {
            peeked.insert(i.as_str());
        }
    }
    let list = |set: &BTreeSet<&str>| set.iter().copied().collect::<Vec<_>>().join(", ");
    let mut problems = Vec::new();
    if !missing.is_empty() {
        problems.push(format!("unlabelled {}", list(&missing)));
    }
    if !unknown.is_empty() {
        problems.push(format!("unknown {}", list(&unknown)));
    }
    if !peeked.is_empty() {
        problems.push(format!(
            "held-out records carry a reason: {}",
            list(&peeked)
        ));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "labels do not match the corpus: {}",
            problems.join("; ")
        )))
    }
}

/// A label's category, which must be text.
pub(crate) fn category<'a>(
    labels_name: &str,
    labels: &'a Map<String, Value>,
    id: &str,
) -> Result<&'a str> {
    let label = labels
        .get(id)
        .ok_or_else(|| Error::Invalid(format!("{labels_name}: no label for {id}")))?;
    match label.get("category") {
        Some(Value::String(category)) => Ok(category),
        Some(_) => Err(Error::Invalid(format!(
            "{labels_name}: the label for {id} has a category that is not text"
        ))),
        None => Err(Error::Invalid(format!(
            "{labels_name}: the label for {id} has no category"
        ))),
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Row {
    pub questions: usize,
    pub took_rec: usize,
    pub other_option: usize,
    pub own_words: usize,
    pub no_rec: usize,
    pub messages: usize,
    pub rec_share: f64,
    pub last_seen: String,
}

/// Outcomes per category over the training split, the busiest category first.
pub fn tally(
    corpus_name: &str,
    records: &Records,
    labels_name: &str,
    labels: &Map<String, Value>,
) -> Result<Vec<(String, Row)>> {
    check_labels(corpus_name, records, labels)?;
    let mut rows: Vec<(String, Row)> = Vec::new();
    let mut index = HashMap::new();
    for r in Record::all(corpus_name, records) {
        if r.get("split")? != "train" {
            continue;
        }
        let name = category(labels_name, labels, r.id()?)?;
        let at = *index.entry(name.to_owned()).or_insert_with(|| {
            rows.push((name.to_owned(), Row::default()));
            rows.len() - 1
        });
        let row = &mut rows[at].1;
        if r.get("kind")? == "question" {
            row.questions += 1;
            match r.str("outcome")? {
                "took_rec" => row.took_rec += 1,
                "other_option" => row.other_option += 1,
                "own_words" => row.own_words += 1,
                "no_rec" => row.no_rec += 1,
                _ => return Err(r.shape("outcome")),
            }
        } else {
            row.messages += 1;
        }
        let date: String = r.str("timestamp")?.chars().take(10).collect();
        if date > row.last_seen {
            row.last_seen = date;
        }
    }
    for (_, row) in &mut rows {
        row.rec_share = share(row.took_rec, row.questions - row.no_rec);
    }
    rows.sort_by(|(a, ra), (b, rb)| {
        (rb.questions + rb.messages)
            .cmp(&(ra.questions + ra.messages))
            .then_with(|| a.cmp(b))
    });
    Ok(rows)
}

pub fn tally_markdown(rows: &[(String, Row)]) -> String {
    let mut lines = vec![
        "| Category | Questions | Took rec | Other option | Own words | No rec | Rec share | Messages | Last seen |".to_owned(),
        "|---|---|---|---|---|---|---|---|---|".to_owned(),
    ];
    for (name, c) in rows {
        lines.push(format!(
            "| {name} | {} | {} | {} | {} | {} | {} | {} | {} |",
            c.questions,
            c.took_rec,
            c.other_option,
            c.own_words,
            c.no_rec,
            percent(c.rec_share),
            c.messages,
            c.last_seen
        ));
    }
    lines.join("\n") + "\n"
}

pub fn tally_json(rows: &[(String, Row)]) -> String {
    let object: Map<String, Value> = rows
        .iter()
        .map(|(name, c)| {
            let row = Map::from_iter([
                ("questions".to_owned(), Value::from(c.questions)),
                ("took_rec".to_owned(), Value::from(c.took_rec)),
                ("other_option".to_owned(), Value::from(c.other_option)),
                ("own_words".to_owned(), Value::from(c.own_words)),
                ("no_rec".to_owned(), Value::from(c.no_rec)),
                ("messages".to_owned(), Value::from(c.messages)),
                ("rec_share".to_owned(), Value::from(c.rec_share)),
                ("last_seen".to_owned(), Value::from(c.last_seen.clone())),
            ]);
            (name.clone(), Value::Object(row))
        })
        .collect();
    pretty(&Value::Object(object))
}
