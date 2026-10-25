//! `similar`: the past answers closest to a new question, by shared rare words, from the training
//! split only, so a backtest never sees the answers it is scored on.
use crate::input::{Record, Records, first, pretty};
use persona_builder::{error::Result, member::Member};
use persona_builder_ingest::{privacy::searchable, python::py_space};
use regex::Regex;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeSet, HashMap},
    sync::LazyLock,
};

/// One shared word matched "review that PR" to any question about reviews.
pub const MIN_SHARED_WORDS: usize = 2;
/// A one-line status check or go-ahead is no precedent for a decision.
pub const MIN_MESSAGE_WORDS: usize = 8;
const KEPT_CHARS: usize = 400;
const STOPWORDS: &str = "the and for that this with what should which from into when where does would there their \
    they them then than have has had will can are was were you your our its it's not but all any \
    now one two how who why use";

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("[a-z][a-z0-9-]{2,}").expect("word pattern"));
static STOP: LazyLock<BTreeSet<&str>> = LazyLock::new(|| STOPWORDS.split_whitespace().collect());

/// The Python's `words`: lowercased words of three or more characters, without the stop words.
pub fn words(text: &str) -> BTreeSet<String> {
    let lower = text.to_lowercase();
    WORD.find_iter(&lower)
        .map(|m| m.as_str())
        .filter(|w| !STOP.contains(w))
        .map(str::to_owned)
        .collect()
}

fn precedent(member: &Member, r: Record) -> Result<Map<String, Value>> {
    let date = first(r.str("timestamp")?, 10);
    let project = r.str("project")?;
    let mut p = Map::new();
    if r.get("kind")? == "message" {
        p.insert("kind".into(), Value::from("message"));
        p.insert("date".into(), Value::from(date));
        p.insert("project".into(), Value::from(project));
        p.insert(
            "text".into(),
            Value::from(first(r.str("text")?, KEPT_CHARS)),
        );
        return Ok(p);
    }
    p.insert("kind".into(), Value::from("question"));
    p.insert("date".into(), Value::from(date));
    p.insert("project".into(), Value::from(project));
    p.insert("header".into(), Value::from(r.str("header")?));
    p.insert("question".into(), Value::from(r.str("question")?));
    let options: Vec<Value> = r
        .options()?
        .into_iter()
        .map(|(l, _)| Value::from(l))
        .collect();
    p.insert("options".into(), Value::from(options));
    p.insert("recommended".into(), r.get("recommended")?.clone());
    let pick_field = member.pick_field();
    p.insert(
        pick_field.clone(),
        r.map.get(&pick_field).cloned().unwrap_or(Value::Null),
    );
    let picks: Vec<Value> = r.strings("picks")?.into_iter().map(Value::from).collect();
    p.insert("picks".into(), Value::from(picks));
    p.insert(
        "own_words".into(),
        Value::from(first(r.str("own_words")?, KEPT_CHARS)),
    );
    p.insert(
        "notes".into(),
        Value::from(first(r.str("notes")?, KEPT_CHARS)),
    );
    p.insert("outcome".into(), Value::from(r.str("outcome")?));
    Ok(p)
}

/// CPython 3.12's `sum()` of floats: Neumaier's compensated sum.
fn python_sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut sum, mut compensation) = (0.0_f64, 0.0_f64);
    for x in values {
        let t = sum + x;
        compensation += if sum.abs() >= x.abs() {
            (sum - t) + x
        } else {
            (x - t) + sum
        };
        sum = t;
    }
    if compensation != 0.0 && compensation.is_finite() {
        sum += compensation;
    }
    sum
}

/// The `k` training records closest to the query, as precedents. Weights sum the shared words in
/// byte order; the Python sums in set order, which can only change the last bit.
pub fn similar(
    member: &Member,
    corpus_name: &str,
    records: &Records,
    query: &str,
    k: usize,
) -> Result<Vec<Map<String, Value>>> {
    let mut docs = Vec::new();
    for r in Record::all(corpus_name, records) {
        if r.get("split")? != "train" {
            continue;
        }
        if r.get("kind")? != "question"
            && r.str("text")?
                .split(py_space)
                .filter(|w| !w.is_empty())
                .count()
                < MIN_MESSAGE_WORDS
        {
            continue;
        }
        let header = match r.map.get("header") {
            None => "",
            Some(_) => r.str("header")?,
        };
        let text = searchable(r.map).map_err(|e| r.at(e))?;
        docs.push((r, words(&format!("{text} {header}"))));
    }
    let mut df: HashMap<&str, usize> = HashMap::new();
    for (_, ws) in &docs {
        for w in ws {
            *df.entry(w.as_str()).or_default() += 1;
        }
    }
    let wanted = words(query);
    let total = docs.len() as f64;
    let mut scored = Vec::new();
    for (r, ws) in &docs {
        let shared: Vec<&String> = wanted.intersection(ws).collect();
        if shared.len() >= MIN_SHARED_WORDS {
            let sum = python_sum(
                shared
                    .iter()
                    .map(|w| (total / df[w.as_str()] as f64).ln() + 1.0),
            );
            scored.push((sum / (ws.len() as f64).sqrt(), r.str("timestamp")?, *r));
        }
    }
    scored.sort_by(|(wa, ta, _), (wb, tb, _)| wb.total_cmp(wa).then_with(|| ta.cmp(tb)));
    scored
        .into_iter()
        .take(k)
        .map(|(_, _, r)| precedent(member, r))
        .collect()
}

pub fn similar_json(hits: &[Map<String, Value>]) -> String {
    pretty(&Value::Array(
        hits.iter().cloned().map(Value::Object).collect(),
    ))
}

fn text<'a>(hit: &'a Map<String, Value>, key: &str) -> &'a str {
    hit.get(key).and_then(Value::as_str).unwrap_or_default()
}

pub fn similar_text(member: &Member, hits: &[Map<String, Value>]) -> String {
    let subject = member.principal_subject();
    let mut out = String::new();
    for h in hits {
        let (date, project) = (text(h, "date"), text(h, "project"));
        if text(h, "kind") == "message" {
            out.push_str(&format!(
                "- {date} {project} · {subject} said: {}\n",
                text(h, "text")
            ));
            continue;
        }
        let list = |key: &str| -> Vec<&str> {
            h[key]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default()
        };
        let own = text(h, "own_words");
        let mut answer = list("picks").join(", ");
        if !own.is_empty() {
            answer.push_str(&format!(" + own words: {own}"));
        }
        out.push_str(&format!(
            "- {date} {project} · {}\n    options: {}\n    {subject} chose: {answer} ({})",
            text(h, "question"),
            list("options").join(" / "),
            text(h, "outcome")
        ));
        let notes = text(h, "notes");
        if !notes.is_empty() {
            out.push_str(&format!("\n    notes: {notes}"));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::python_sum;

    #[test]
    fn sums_floats_as_cpython_3_12_does() {
        // CPython 3.12 prints 1.0 and 1.0; a plain running sum gives 0.9999999999999999 and 0.0.
        assert_eq!(python_sum(std::iter::repeat_n(0.1, 10)), 1.0);
        assert_eq!(python_sum([1e16, 1.0, -1e16].into_iter()), 1.0);
    }
}
