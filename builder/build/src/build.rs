//! The build (`bootstrap/PROMPT.md` steps 8 to 10) without sub-agents or tools: label, distil and
//! the blind backtest, one call at a time through the person's own Claude Code, every reply checked
//! before it is used and kept in `build/` so a rerun pays only for what was not finished.
use crate::text;
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_agent::{
    claude::{Call, Claude},
    ledger::Session,
};
use persona_builder_ingest::{
    corpus::{read_records, split},
    filters::Filters,
    mine::to_jsonl,
    private::{private_folder, read_private_at, write_private_at},
    python::truthy,
};
use persona_builder_steps::{
    book::Book,
    chunk::chunk,
    distil::{Group, distil_in},
    input::{RecordList, Records, read_json},
    labels::{Mode, Row, label_problems, tally},
    score::{Score, TIERS, score},
    shingle::overlaps,
    similar::{similar, similar_text},
};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

/// The most records in one labelling call, as `PROMPT.md` chunks them.
pub const PART_RECORDS: usize = 300;
/// The most JSONL bytes in one labelling call, so a part of long records never overruns a model's
/// context; a single larger record goes alone.
pub const PART_BYTES: usize = 400_000;
/// Below this many held-out questions there is too little to measure.
pub const MIN_HELD: usize = 30;
/// About this many questions per backtest call.
pub const BATCH: usize = 40;
/// The past decisions sent with each backtest question.
pub const PRECEDENTS: usize = 6;
/// A run of this many of the person's words in the draft is a quote.
pub const SHINGLE: usize = 6;
/// A category is thin below this many records.
pub const THIN: usize = 5;
/// The folder in the state folder that keeps every checked reply.
pub const CACHE: &str = "build";
const MAX_POINT: usize = 400;
const MAX_WORDS: usize = 900;
const SHOWN_PROBLEMS: usize = 40;

/// Fixed text only: the system prompts go in the argv, which other local users can read.
pub const LABEL_SYSTEM: &str = include_str!("prompts/label.md");
pub const DISTIL_SYSTEM: &str = include_str!("prompts/distil.md");
pub const BACKTEST_SYSTEM: &str = include_str!("prompts/backtest.md");

/// What a reply got wrong: `told` goes back to Claude with the retry (it may name ids and the
/// person's own words, which that prompt already holds); `shown` is for the person (counts and
/// places only, never words).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Misfit {
    pub told: String,
    pub shown: String,
}
impl Misfit {
    fn new(told: impl Into<String>, shown: impl Into<String>) -> Self {
        Misfit {
            told: told.into(),
            shown: shown.into(),
        }
    }
    /// A problem whose words are all the code's own, so it can be shown as it is.
    fn plain(problem: impl Into<String>) -> Self {
        let problem = problem.into();
        Misfit::new(problem.clone(), problem)
    }
}

fn nullable(kind: &str) -> Value {
    json!({"type": [kind, "null"]})
}
fn strings() -> Value {
    json!({"type": "array", "items": {"type": "string"}})
}

pub fn label_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["labels"],
        "properties": {"labels": {"type": "array", "items": {
            "type": "object", "additionalProperties": false,
            "required": ["id", "category", "domain", "escalates", "signal", "reason"],
            "properties": {
                "id": {"type": "string"}, "category": {"type": "string"}, "domain": nullable("string"),
                "escalates": strings(), "signal": nullable("string"), "reason": nullable("string")
            }
        }}}
    })
}

pub fn distil_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["sections"],
        "properties": {"sections": {"type": "array", "items": {
            "type": "object", "additionalProperties": false,
            "required": ["category", "lean", "flips", "own_answer", "rules", "precedents"],
            "properties": {
                "category": {"type": "string"}, "lean": strings(), "flips": strings(),
                "own_answer": strings(), "rules": strings(), "precedents": strings()
            }
        }}}
    })
}

pub fn backtest_schema() -> Value {
    json!({
        "type": "object", "additionalProperties": false, "required": ["predictions"],
        "properties": {"predictions": {"type": "array", "items": {
            "type": "object", "additionalProperties": false,
            "required": ["id", "pick", "confidence"],
            "properties": {
                "id": {"type": "string"},
                "pick": {"anyOf": [{"type": "string"}, strings()]},
                "confidence": {"enum": TIERS}
            }
        }}}
    })
}

/// What the build reads: the member, its codebook (the lists and the person's own
/// `briefs/codebook.md`), the merged corpus and its filters, and the model to call.
pub struct Inputs<'a> {
    pub member: &'a Member,
    pub book: &'a Book,
    pub codebook: &'a str,
    pub corpus_name: &'a str,
    pub corpus: &'a [u8],
    pub filters: &'a Filters,
    pub model: &'a str,
}

/// One labelling call's records.
pub struct Part {
    pub name: String,
    pub mode: Mode,
    pub records: RecordList,
}

/// The training records, then the held-out questions without their answers, packed in order into
/// parts of at most [`PART_RECORDS`] records and [`PART_BYTES`] bytes of JSONL.
pub fn parts(corpus_name: &str, records: &Records) -> Result<Vec<Part>> {
    let all = chunk(corpus_name, records, usize::MAX)?;
    let mut parts = Vec::new();
    for (mode, kind, list) in [
        (Mode::Train, "train", all.train.concat()),
        (Mode::Heldout, "heldout", all.heldout.concat()),
    ] {
        let mut packed: Vec<Vec<Map<String, Value>>> = Vec::new();
        let mut bytes = 0;
        for record in list {
            let size = to_jsonl(std::slice::from_ref(&record)).len();
            match packed.last_mut() {
                Some(part) if part.len() < PART_RECORDS && bytes + size <= PART_BYTES => {
                    part.push(record);
                    bytes += size;
                }
                _ => {
                    packed.push(vec![record]);
                    bytes = size;
                }
            }
        }
        for (at, part) in packed.into_iter().enumerate() {
            parts.push(Part {
                name: format!("{kind}-{:02}", at + 1),
                mode,
                records: part
                    .into_iter()
                    .enumerate()
                    .map(|(n, r)| (n + 1, r))
                    .collect(),
            });
        }
    }
    Ok(parts)
}

fn listed(list: &[String]) -> String {
    if list.is_empty() {
        "none named".to_owned()
    } else {
        list.join(", ")
    }
}

/// A labelling call's prompt: fixed text first, then the codebook, the exact lists and the records.
pub fn label_prompt(book: &Book, codebook: &str, part: &Part) -> String {
    let mode = match part.mode {
        Mode::Train => "train",
        Mode::Heldout => "heldout",
    };
    let records: Vec<_> = part.records.iter().map(|(_, r)| r.clone()).collect();
    format!(
        "Label every record below for the person's assistant.\n\nMode: {mode}\n\nThe person's codebook:\n\n{codebook}\n\nThe lists every label must use, exactly as written:\n- category: {}\n- domain: {}\n- escalates: {}\n- signal: {}\n\nThe records, one JSON object per line:\n\n{}",
        listed(&book.categories),
        listed(&book.domains),
        listed(&book.escalates),
        listed(&book.signals),
        to_jsonl(&records)
    )
}

fn present(value: Option<&Value>) -> Option<&Value> {
    value.filter(|v| !v.is_null())
}

/// A labelling reply rebuilt as `check-labels` reads labels (each kind and mode's own keys, never a
/// held-out reason), then checked by `check-labels`' own rules.
pub fn check_label_reply(
    book: &Book,
    part: &Part,
    reply: &Value,
) -> std::result::Result<Map<String, Value>, Misfit> {
    let items = reply
        .get("labels")
        .and_then(Value::as_array)
        .ok_or_else(|| Misfit::plain("the answer holds no list of labels"))?;
    let kinds: HashMap<&str, &str> = part
        .records
        .iter()
        .filter_map(|(_, r)| Some((r.get("id")?.as_str()?, r.get("kind")?.as_str()?)))
        .collect();
    let mut labels = Map::new();
    let mut repeated = 0;
    for item in items {
        let (Some(item), Some(id)) = (item.as_object(), item.get("id").and_then(Value::as_str))
        else {
            return Err(Misfit::plain("a label has no record id"));
        };
        let kind = kinds.get(id).copied();
        let mut label = Map::new();
        for key in ["category", "domain"] {
            if let Some(value) = present(item.get(key)) {
                label.insert(key.to_owned(), value.clone());
            }
        }
        match kind {
            Some("question") => {
                let escalates = present(item.get("escalates")).cloned();
                label.insert(
                    "escalates".to_owned(),
                    escalates.unwrap_or_else(|| json!([])),
                );
            }
            Some("message") => {
                if let Some(signal) = present(item.get("signal")) {
                    label.insert("signal".to_owned(), signal.clone());
                }
            }
            _ => {}
        }
        if part.mode == Mode::Train
            && kind.is_some()
            && let Some(reason) = present(item.get("reason"))
        {
            label.insert("reason".to_owned(), reason.clone());
        }
        if labels.insert(id.to_owned(), Value::Object(label)).is_some() {
            repeated += 1;
        }
    }
    if repeated > 0 {
        return Err(Misfit::plain(format!(
            "{repeated} records are labelled more than once"
        )));
    }
    let problems = label_problems(book, &part.name, &part.records, &labels, part.mode)
        .map_err(|error| Misfit::plain(error.to_string()))?;
    if !problems.is_empty() {
        let told: Vec<_> = problems.iter().take(SHOWN_PROBLEMS).cloned().collect();
        return Err(Misfit::new(
            format!(
                "these labels are wrong (the record id, then the field): {}",
                told.join("; ")
            ),
            format!("{} of its labels didn't fit the codebook", problems.len()),
        ));
    }
    Ok(labels)
}

/// The categories a group writes about, in the codebook's order, each with its rows.
pub fn group_categories(book: &Book, group: &Group) -> Vec<(String, usize)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for row in &group.rows {
        if let Some(category) = row.get("category").and_then(Value::as_str) {
            *counts.entry(category).or_default() += 1;
        }
    }
    let mut ordered: Vec<(String, usize)> = Vec::new();
    for category in &book.categories {
        if let Some(n) = counts.remove(category.as_str())
            && !ordered.iter().any(|(c, _)| c == category)
        {
            ordered.push((category.clone(), n));
        }
    }
    // Every category of a row is in the codebook (`check-labels` checked each part); none is left.
    ordered
}

/// The most characters of a record's own words (`msg`, `own`, `notes`) a distil call reads: the
/// gist of a long message is enough to learn how they decide.
pub const DISTIL_TEXT_CHARS: usize = 600;
/// The most bytes a distil call's prompt may hold, so no call is paid for that a model's context
/// can't take; a larger group stops the build before it is sent.
pub const DISTIL_BYTES: usize = 700_000;

pub fn distil_prompt(group: &Group, categories: &[(String, usize)]) -> String {
    let named: String = categories
        .iter()
        .map(|(category, n)| format!("- {category}: {n} records\n"))
        .collect();
    let rows: Vec<Map<String, Value>> = group
        .rows
        .iter()
        .map(|row| {
            let mut row = row.clone();
            for key in ["msg", "own", "notes"] {
                if let Some(Value::String(words)) = row.get_mut(key)
                    && words.chars().count() > DISTIL_TEXT_CHARS
                {
                    *words = words.chars().take(DISTIL_TEXT_CHARS).collect();
                }
            }
            row
        })
        .collect();
    format!(
        "Write how the person decides, one section for each kind of decision below.\n\nThe kinds of decision:\n{named}\nThe records, one JSON object per line:\n\n{}",
        to_jsonl(&rows)
    )
}

const LISTS: [(&str, &str, usize); 5] = [
    ("lean", "The lean", 6),
    ("flips", "What flips it", 5),
    ("own_answer", "When they write their own answer", 3),
    ("rules", "Standing rules", 6),
    ("precedents", "Precedents", 8),
];

/// A character a point never holds: control characters, line and paragraph separators, the
/// invisible ones that could split a word past the shingle check or the filters, and `<`, which
/// would start hidden markup in the Markdown the assistant reads.
fn hidden(c: char) -> bool {
    c.is_control()
        || c == '<'
        || matches!(c, '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{FEFF}')
        || ('\u{200B}'..='\u{200F}').contains(&c)
        || ('\u{2028}'..='\u{202E}').contains(&c)
        || ('\u{2060}'..='\u{206F}').contains(&c)
        || ('\u{FFF9}'..='\u{FFFB}').contains(&c)
        || matches!(
            c,
            '\u{034F}' | '\u{115F}' | '\u{1160}' | '\u{3164}' | '\u{FFA0}'
        )
        || ('\u{FE00}'..='\u{FE0F}').contains(&c)
        || ('\u{13430}'..='\u{1343F}').contains(&c)
        || ('\u{1BCA0}'..='\u{1BCA3}').contains(&c)
        || ('\u{1D173}'..='\u{1D17A}').contains(&c)
        || ('\u{E0000}'..='\u{E007F}').contains(&c)
        || ('\u{E0100}'..='\u{E01EF}').contains(&c)
}

fn dated(point: &str) -> bool {
    let bytes = point.as_bytes();
    bytes.len() > 13
        && bytes[..10].iter().enumerate().all(|(at, b)| {
            if at == 4 || at == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
        && point[10..].starts_with(" · ")
}

/// A distilling reply checked whole and rendered in the distil brief's Markdown: one section for
/// each category asked for, the brief's limits, and no point that quotes the person or trips a
/// filter.
pub fn check_distil_reply(
    categories: &[(String, usize)],
    corpus_name: &str,
    records: &Records,
    filters: &Filters,
    reply: &Value,
) -> std::result::Result<Vec<(String, String)>, Misfit> {
    let sections = reply
        .get("sections")
        .and_then(Value::as_array)
        .ok_or_else(|| Misfit::plain("the answer holds no list of sections"))?;
    let mut found: HashMap<&str, &Map<String, Value>> = HashMap::new();
    for section in sections {
        let section = section
            .as_object()
            .ok_or_else(|| Misfit::plain("a section is not an object"))?;
        let category = section
            .get("category")
            .and_then(Value::as_str)
            .filter(|c| categories.iter().any(|(asked, _)| asked == c))
            .ok_or_else(|| {
                Misfit::plain("a section is about a kind of decision that wasn't asked for")
            })?;
        if found.insert(category, section).is_some() {
            return Err(Misfit::plain(format!(
                "there are two sections for {category}"
            )));
        }
    }
    let mut rendered = Vec::new();
    let mut points: Vec<(String, &str, usize, String)> = Vec::new();
    for (category, rows) in categories {
        let section = found
            .get(category.as_str())
            .ok_or_else(|| Misfit::plain(format!("there is no section for {category}")))?;
        let mut out = format!("### {category}\n");
        let mut words = 0;
        for (key, title, most) in LISTS {
            let list: Vec<&str> = section
                .get(key)
                .and_then(Value::as_array)
                .and_then(|list| list.iter().map(Value::as_str).collect::<Option<_>>())
                .ok_or_else(|| Misfit::plain(format!("{category}: {key} is not a list of text")))?;
            let fewest = usize::from(key == "lean");
            if list.len() < fewest || list.len() > most {
                return Err(Misfit::plain(format!(
                    "{category}: {key} needs {fewest} to {most} points"
                )));
            }
            out.push_str(&format!("\n**{title}:**\n"));
            if list.is_empty() {
                out.push_str("- None yet.\n");
            }
            for (at, point) in list.iter().enumerate() {
                let point = point.trim();
                if point.is_empty() || point.contains(['\n', '\r']) {
                    return Err(Misfit::plain(format!(
                        "{category}: {key} point {} must be one line of text",
                        at + 1
                    )));
                }
                if point.chars().any(hidden) {
                    return Err(Misfit::plain(format!(
                        "{category}: {key} point {} holds a character a point never has (a control or invisible character, or <)",
                        at + 1
                    )));
                }
                if point.chars().count() > MAX_POINT {
                    return Err(Misfit::plain(format!(
                        "{category}: {key} point {} is longer than {MAX_POINT} characters",
                        at + 1
                    )));
                }
                if key == "precedents" && !dated(point) {
                    return Err(Misfit::plain(format!(
                        "{category}: precedent {} must start with its date, YYYY-MM-DD · ",
                        at + 1
                    )));
                }
                words += point.split_whitespace().count();
                out.push_str(&format!("- {point}\n"));
                points.push((category.clone(), key, at + 1, point.to_owned()));
            }
        }
        if words > MAX_WORDS {
            return Err(Misfit::plain(format!(
                "{category}: the section is longer than {MAX_WORDS} words"
            )));
        }
        if *rows < THIN {
            out.push_str(&format!(
                "\n_Thin: {rows} records; treat as Low until more history exists._\n"
            ));
        }
        out.push('\n');
        rendered.push((category.clone(), out));
    }
    let marked: Vec<String> = points
        .iter()
        .filter(|(_, _, _, point)| filters.kind(point).is_some())
        .map(|(category, key, at, _)| format!("{category} {key} point {at}"))
        .collect();
    if !marked.is_empty() {
        return Err(Misfit::new(
            format!(
                "these points look personal or secret, so leave that detail out: {}",
                marked.join("; ")
            ),
            format!("{} points looked personal or secret", marked.len()),
        ));
    }
    let text: Vec<&str> = points
        .iter()
        .map(|(_, _, _, point)| point.as_str())
        .collect();
    let quoted = overlaps(&text.join("\n"), &[(corpus_name, records)], SHINGLE)
        .map_err(|error| Misfit::plain(error.to_string()))?;
    if !quoted.is_empty() {
        let runs: Vec<String> = quoted
            .iter()
            .take(SHOWN_PROBLEMS)
            .map(|run| format!("\"{run}\""))
            .collect();
        return Err(Misfit::new(
            format!(
                "these runs of words are the person's own, so rewrite every point that holds one in your own words: {}",
                runs.join(", ")
            ),
            format!(
                "{} runs of {SHINGLE} or more of their own words",
                quoted.len()
            ),
        ));
    }
    Ok(rendered)
}

/// The held-out questions in batches of near-equal size, about [`BATCH`] each.
pub fn batches(questions: &[Map<String, Value>]) -> Vec<&[Map<String, Value>]> {
    if questions.is_empty() {
        return Vec::new();
    }
    let count = questions.len().div_ceil(BATCH);
    let (base, extra) = (questions.len() / count, questions.len() % count);
    let mut out = Vec::with_capacity(count);
    let mut start = 0;
    for at in 0..count {
        let size = base + usize::from(at < extra);
        out.push(&questions[start..start + size]);
        start += size;
    }
    out
}

fn option_labels(question: &Map<String, Value>) -> Vec<&str> {
    question
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|option| option.get("label").and_then(Value::as_str))
        .collect()
}

/// The text `similar` searches for: the question and its option labels.
pub fn query(question: &Map<String, Value>) -> String {
    let mut query = question
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    for label in option_labels(question) {
        query.push(' ');
        query.push_str(label);
    }
    query
}

/// A backtest call's prompt: the model of how they decide, then each question (never its answer)
/// with its closest training precedents.
pub fn backtest_prompt(
    member: &Member,
    corpus_name: &str,
    records: &Records,
    model: &str,
    batch: &[Map<String, Value>],
) -> Result<String> {
    let mut prompt = format!(
        "Predict the person's answer to each question below.\n\nHow the person decides, per kind of question:\n\n{model}\nThe questions, each with the closest past decisions from their history:\n"
    );
    for (at, question) in batch.iter().enumerate() {
        let found = similar(member, corpus_name, records, &query(question), PRECEDENTS)?;
        let precedents = if found.is_empty() {
            "- none found\n".to_owned()
        } else {
            similar_text(member, &found)
        };
        prompt.push_str(&format!(
            "\n## Question {} of {}\n\n{}\nClosest past decisions:\n{precedents}",
            at + 1,
            batch.len(),
            to_jsonl(std::slice::from_ref(question))
        ));
    }
    Ok(prompt)
}

/// A backtest reply checked whole: one prediction for every question in the batch, each pick
/// `"own"` or exact option labels, each confidence a tier.
pub fn check_backtest_reply(
    batch: &[Map<String, Value>],
    reply: &Value,
) -> std::result::Result<Map<String, Value>, Misfit> {
    let items = reply
        .get("predictions")
        .and_then(Value::as_array)
        .ok_or_else(|| Misfit::plain("the answer holds no list of predictions"))?;
    let asked: HashMap<&str, &Map<String, Value>> = batch
        .iter()
        .filter_map(|q| Some((q.get("id")?.as_str()?, q)))
        .collect();
    let mut problems = Vec::new();
    let mut predictions = Map::new();
    let mut extra = 0;
    for item in items {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            return Err(Misfit::plain("a prediction has no question id"));
        };
        let Some(question) = asked.get(id) else {
            extra += 1;
            continue;
        };
        if predictions.contains_key(id) {
            problems.push(format!("{id} predicted twice"));
            continue;
        }
        let labels = option_labels(question);
        let multi = question.get("multiSelect").is_some_and(truthy);
        let pick = item.get("pick").cloned().unwrap_or(Value::Null);
        let fits = match &pick {
            Value::String(p) if p == "own" => true,
            Value::String(p) => !multi && labels.contains(&p.as_str()),
            Value::Array(list) => {
                let mut seen = HashSet::new();
                multi
                    && !list.is_empty()
                    && list.iter().all(|p| {
                        p.as_str()
                            .is_some_and(|p| labels.contains(&p) && seen.insert(p))
                    })
            }
            _ => false,
        };
        if !fits {
            problems.push(format!("{id} pick"));
        }
        let confidence = item.get("confidence").cloned().unwrap_or(Value::Null);
        if !confidence.as_str().is_some_and(|c| TIERS.contains(&c)) {
            problems.push(format!("{id} confidence"));
        }
        predictions.insert(
            id.to_owned(),
            json!({"pick": pick, "confidence": confidence}),
        );
    }
    let missing = asked
        .keys()
        .filter(|id| !predictions.contains_key(**id))
        .count();
    if missing > 0 || extra > 0 {
        problems.insert(0, format!("ids differ: missing {missing}, extra {extra}"));
    }
    if !problems.is_empty() {
        let told: Vec<_> = problems.iter().take(SHOWN_PROBLEMS).cloned().collect();
        return Err(Misfit::new(
            format!(
                "these predictions are wrong (the question id, then the field): {}",
                told.join("; ")
            ),
            format!("{} of its predictions didn't fit", problems.len()),
        ));
    }
    Ok(predictions)
}

/// True at 5 or more High picks with at least 90% right.
pub fn live(high: Option<&persona_builder_steps::score::Cell>) -> bool {
    high.is_some_and(|cell| cell.n >= THIN && cell.hits * 10 >= cell.n * 9)
}

/// The record counts behind the draft's `sources` line.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    pub questions: usize,
    pub messages: usize,
    pub claudeai: usize,
    pub interview: usize,
    pub rounds: usize,
    pub first: String,
    pub last: String,
    pub mined_through: String,
}

/// `YYYY-MM-DD` digits and dashes, so a crafted timestamp never adds a line to the front matter.
fn is_date(date: &str) -> bool {
    date.len() == 10
        && date.bytes().enumerate().all(|(at, b)| {
            if at == 4 || at == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

pub fn counts(records: &Records) -> Counts {
    let mut counts = Counts::default();
    for (_, record) in records {
        let text = |key: &str| record.get(key).and_then(Value::as_str).unwrap_or_default();
        let project = text("project");
        let question = text("kind") == "question";
        if project == "interview" {
            counts.interview += 1;
        } else if project.starts_with("round-") {
            counts.rounds += 1;
        } else if project == "claude.ai" || project.starts_with("claude.ai/") {
            counts.claudeai += 1;
        } else if question {
            counts.questions += 1;
        } else {
            counts.messages += 1;
        }
        let date: String = text("timestamp").chars().take(10).collect();
        if !is_date(&date) {
            continue;
        }
        if counts.first.is_empty() || date < counts.first {
            counts.first = date.clone();
        }
        if date > counts.last {
            counts.last = date.clone();
        }
        if text("split") == "train" && date > counts.mined_through {
            counts.mined_through = date;
        }
    }
    counts
}

/// The blind backtest's result.
pub struct Backtest {
    pub score: Score,
    pub first: String,
    pub last: String,
}

/// Everything the draft is written from.
pub struct Built {
    /// Every group's sections, in the codebook's category order.
    pub sections: String,
    pub tally: Vec<(String, Row)>,
    /// `None` when there were fewer than [`MIN_HELD`] held-out questions.
    pub backtest: Option<Backtest>,
    pub held: usize,
    pub counts: Counts,
    pub calls: usize,
    pub reused: usize,
}

/// The cache key: the step, the model, the fixed instructions and shape, and the first prompt.
pub fn cache_name(step: &str, model: &str, system: &str, schema: &Value, prompt: &str) -> String {
    let mut hash = sha1_smol::Sha1::new();
    for part in [step, model, system, &schema.to_string(), prompt] {
        hash.update(part.as_bytes());
        hash.update(&[0]);
    }
    format!("{step}-{}.json", hash.digest())
}

struct Asker<'a> {
    claude: &'a Claude,
    session: &'a Session,
    home: &'a Path,
    slug: &'a str,
    model: &'a str,
    calls: usize,
    reused: usize,
}

impl Asker<'_> {
    /// A reply kept by an earlier run, or `None`; a damaged one is asked for again.
    fn kept(&self, name: &str) -> Result<Option<Value>> {
        let folder = private_folder(self.home, self.slug, CACHE)?;
        match std::fs::symlink_metadata(folder.join(name)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::Io {
                path: folder.join(name),
                source,
            }),
            Ok(_) => {
                let bytes = read_private_at(self.home, self.slug, Some(CACHE), name)?;
                Ok(read_json(name, &bytes).ok())
            }
        }
    }

    /// One step's call, reused from the cache when an earlier run paid for it, else asked once
    /// more when its reply does not fit. Answers the checked reply and whether it was reused.
    fn ask<T>(
        &mut self,
        step: &str,
        system: &str,
        schema: &Value,
        prompt: &str,
        what: &str,
        check: impl Fn(&Value) -> std::result::Result<T, Misfit>,
    ) -> Result<(T, bool)> {
        let name = cache_name(step, self.model, system, schema, prompt);
        if let Some(kept) = self.kept(&name)?
            && let Ok(checked) = check(&kept)
        {
            self.reused += 1;
            return Ok((checked, true));
        }
        let mut asked = prompt.to_owned();
        let mut last = Misfit::plain("");
        for _ in 0..2 {
            let reply = self.claude.call(
                self.session,
                &Call {
                    step,
                    system,
                    prompt: &asked,
                    model: self.model,
                    schema,
                },
            )?;
            self.calls += 1;
            match check(&reply.output) {
                Ok(checked) => {
                    write_private_at(
                        self.home,
                        self.slug,
                        Some(CACHE),
                        &name,
                        reply.output.to_string().as_bytes(),
                    )?;
                    return Ok((checked, false));
                }
                Err(misfit) => {
                    asked = format!(
                        "{prompt}\nYour last answer didn't fit: {}. Answer again in full, following every rule above.\n",
                        misfit.told
                    );
                    last = misfit;
                }
            }
        }
        Err(Error::Invalid(text::misfit(what, &last.shown)))
    }
}

/// Label, distil and backtest. Writes nothing in the member's folder: only checked replies in
/// the state folder's `build/`. `progress` gets each plain line the app shows.
pub fn build(
    claude: &Claude,
    session: &Session,
    home: &Path,
    inputs: &Inputs,
    progress: &mut dyn FnMut(&str),
) -> Result<Built> {
    let member = inputs.member;
    if let Some(category) = inputs.book.categories.iter().find(|c| {
        !inputs
            .book
            .groups
            .iter()
            .any(|(_, group)| group.contains(c))
    }) {
        return Err(Error::Invalid(text::no_group(category)));
    }
    let records = read_records(inputs.corpus_name, inputs.corpus)?;
    let counts = counts(&records);
    let heldout = split(member, inputs.corpus_name, inputs.corpus)?;
    let tested = heldout.questions.len() >= MIN_HELD;
    // Held-out labels only place each test question in a kind: without a test, none is paid for.
    let parts: Vec<Part> = parts(inputs.corpus_name, &records)?
        .into_iter()
        .filter(|part| tested || part.mode == Mode::Train)
        .collect();
    let trained = parts.iter().any(|part| part.mode == Mode::Train);
    if !trained {
        return Err(Error::Invalid(text::NOTHING_TO_BUILD.into()));
    }
    progress(&text::reading(
        crate::interview::answered(&records),
        heldout.questions.len(),
    ));
    let mut asker = Asker {
        claude,
        session,
        home,
        slug: &member.slug,
        model: inputs.model,
        calls: 0,
        reused: 0,
    };

    let schema = label_schema();
    let mut labels = Map::new();
    for (at, part) in parts.iter().enumerate() {
        let what = text::labelling(at + 1, parts.len());
        let (found, reused) = asker.ask(
            "label",
            LABEL_SYSTEM,
            &schema,
            &label_prompt(inputs.book, inputs.codebook, part),
            &what,
            |reply| check_label_reply(inputs.book, part, reply),
        )?;
        labels.extend(found);
        progress(&text::step_line(&what, reused));
    }
    let rows = tally(inputs.corpus_name, &records, "labels", &labels)?;

    let groups = distil_in(inputs.book, inputs.corpus_name, &records, "labels", &labels)?;
    let groups: Vec<_> = groups.iter().filter(|g| !g.rows.is_empty()).collect();
    let schema = distil_schema();
    let mut rendered: HashMap<String, String> = HashMap::new();
    for (at, group) in groups.iter().enumerate() {
        let what = text::distilling(at + 1, groups.len());
        let categories = group_categories(inputs.book, group);
        let prompt = distil_prompt(group, &categories);
        if prompt.len() > DISTIL_BYTES {
            return Err(Error::Invalid(text::too_big(&what, group.rows.len())));
        }
        let (written, reused) =
            asker.ask("distil", DISTIL_SYSTEM, &schema, &prompt, &what, |reply| {
                check_distil_reply(
                    &categories,
                    inputs.corpus_name,
                    &records,
                    inputs.filters,
                    reply,
                )
            })?;
        rendered.extend(written);
        progress(&text::step_line(&what, reused));
    }
    let sections: String = inputs
        .book
        .categories
        .iter()
        .filter_map(|category| rendered.remove(category))
        .collect();

    let held = heldout.questions.len();
    let backtest = if !tested {
        progress(&text::too_few(held));
        None
    } else {
        let model = format!(
            "{sections}{}",
            persona_builder_steps::labels::tally_markdown(&rows)
        );
        let schema = backtest_schema();
        let mut predictions = Map::new();
        let batches = batches(&heldout.questions);
        for (at, batch) in batches.iter().enumerate() {
            let what = text::testing(at + 1, batches.len(), held);
            let prompt = backtest_prompt(member, inputs.corpus_name, &records, &model, batch)?;
            let (found, reused) = asker.ask(
                "backtest",
                BACKTEST_SYSTEM,
                &schema,
                &prompt,
                &what,
                |reply| check_backtest_reply(batch, reply),
            )?;
            predictions.extend(found);
            progress(&text::step_line(&what, reused));
        }
        let answers: RecordList = heldout
            .answers
            .iter()
            .cloned()
            .enumerate()
            .map(|(n, a)| (n + 1, a))
            .collect();
        let score = score(
            member,
            "heldout-answers",
            &answers,
            "labels",
            &labels,
            &predictions,
        )?;
        let dates: Vec<String> = heldout
            .questions
            .iter()
            .filter_map(|q| q.get("timestamp").and_then(Value::as_str))
            .map(|t| t.chars().take(10).collect::<String>())
            .filter(|date| is_date(date))
            .collect();
        Some(Backtest {
            score,
            first: dates.iter().min().cloned().unwrap_or_default(),
            last: dates.iter().max().cloned().unwrap_or_default(),
        })
    };
    Ok(Built {
        sections,
        tally: rows,
        backtest,
        held,
        counts,
        calls: asker.calls,
        reused: asker.reused,
    })
}
