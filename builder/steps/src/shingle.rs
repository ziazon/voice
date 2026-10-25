//! `shingle`: the runs of `n` words a draft shares with the corpus, so a distilled file stays
//! paraphrase only.
use crate::input::{Record, Records};
use persona_builder::error::Result;
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};

const SHOWN: usize = 60;
const FIELDS: [&str; 5] = ["text", "own_words", "notes", "question", "context"];
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new("[a-z0-9']+").expect("token pattern"));

fn shingles(text: &str, n: usize, into: &mut BTreeSet<String>) {
    let lower = text.to_lowercase();
    let tokens: Vec<&str> = TOKEN.find_iter(&lower).map(|m| m.as_str()).collect();
    for run in tokens.windows(n) {
        into.insert(run.join(" "));
    }
}

/// Every `n`-word run the doc shares with the corpora's own words, sorted.
pub fn overlaps(doc: &str, corpora: &[(&str, &Records)], n: usize) -> Result<Vec<String>> {
    let mut raw = BTreeSet::new();
    for (name, records) in corpora {
        for r in Record::all(name, records) {
            for key in FIELDS {
                if let Some(value) = r
                    .map
                    .get(key)
                    .filter(|v| persona_builder_ingest::python::truthy(v))
                {
                    shingles(value.as_str().ok_or_else(|| r.shape(key))?, n, &mut raw);
                }
            }
            if r.map.contains_key("options") {
                for (label, description) in r.options()? {
                    shingles(&format!("{label} {description}"), n, &mut raw);
                }
            }
        }
    }
    let mut ours = BTreeSet::new();
    shingles(doc, n, &mut ours);
    Ok(ours.intersection(&raw).cloned().collect())
}

/// The count, then the first 60 overlaps.
pub fn render(hits: &[String]) -> String {
    let mut out = format!("{} overlaps\n", hits.len());
    for h in hits.iter().take(SHOWN) {
        out.push_str(&format!("   {h}\n"));
    }
    out
}
