//! `shingle`: the runs of `n` words a draft shares with the corpus, so a distilled file stays
//! paraphrase only.
use crate::input::{Record, Records};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};
use voice_core::error::Result;

const SHOWN: usize = 60;
const FIELDS: [&str; 5] = ["text", "own_words", "notes", "question", "context"];
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new("[a-z0-9']+").expect("token pattern"));

fn shingles(text: &str, n: usize, into: &mut BTreeSet<String>) {
    into.extend(runs(text, n));
}

/// Every `n`-word run, with the same tokens used by [`overlaps`].
pub fn runs(text: &str, n: usize) -> BTreeSet<String> {
    let mut into = BTreeSet::new();
    for run in tokens(text).windows(n) {
        into.insert(run.join(" "));
    }
    into
}

/// The lowercase word tokens shared by every shingle check.
pub fn tokens(text: &str) -> Vec<String> {
    TOKEN
        .find_iter(&text.to_lowercase())
        .map(|m| m.as_str().to_owned())
        .collect()
}

/// Every `n`-word run in the corpora's own words, sorted.
pub fn corpus_runs(corpora: &[(&str, &Records)], n: usize) -> Result<BTreeSet<String>> {
    let mut raw = BTreeSet::new();
    for (name, records) in corpora {
        for r in Record::all(name, records) {
            for key in FIELDS {
                if let Some(value) = r.map.get(key).filter(|v| voice_ingest::python::truthy(v)) {
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
    Ok(raw)
}

/// Every `n`-word run the doc shares with the corpora's own words, sorted.
pub fn overlaps(doc: &str, corpora: &[(&str, &Records)], n: usize) -> Result<Vec<String>> {
    let raw = corpus_runs(corpora, n)?;
    Ok(runs(doc, n).intersection(&raw).cloned().collect())
}

/// The count, then the first 60 overlaps.
pub fn render(hits: &[String]) -> String {
    let mut out = format!("{} overlaps\n", hits.len());
    for h in hits.iter().take(SHOWN) {
        out.push_str(&format!("   {h}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_runs_matches_overlaps_fields_options_and_errors() {
        let records = vec![(
            1,
            serde_json::from_value(serde_json::json!({
                "text":"ONE two three", "own_words":"four five six", "notes":"seven eight nine",
                "question":"ten eleven twelve", "context":"thirteen fourteen fifteen",
                "options":[{"label":"A", "description":"sixteen seventeen"}]
            }))
            .unwrap(),
        )];
        let corpus: [(&str, &Records); 1] = [("fixture", &records)];
        let all = corpus_runs(&corpus, 3).unwrap();
        assert_eq!(
            all,
            runs("one two three", 3)
                .union(&runs("four five six", 3))
                .cloned()
                .chain(
                    [
                        "seven eight nine",
                        "ten eleven twelve",
                        "thirteen fourteen fifteen",
                        "a sixteen seventeen"
                    ]
                    .map(str::to_owned)
                )
                .collect()
        );
        assert_eq!(tokens("Sam's PLAN: 2!"), vec!["sam's", "plan", "2"]);
        assert_eq!(
            overlaps("ONE two three", &corpus, 3).unwrap(),
            vec!["one two three"]
        );
        let bad = vec![(
            1,
            serde_json::from_value(serde_json::json!({"text":42})).unwrap(),
        )];
        assert!(corpus_runs(&[("fixture", &bad)], 3).is_err());
        assert!(overlaps("anything", &[("fixture", &bad)], 3).is_err());
    }

    #[test]
    fn runs_use_the_same_case_apostrophe_and_punctuation_tokens_as_overlaps() {
        let text = "Sam's PLAN: keep 2, small steps!";
        let expected = BTreeSet::from([
            "sam's plan keep".into(),
            "plan keep 2".into(),
            "keep 2 small".into(),
            "2 small steps".into(),
        ]);
        assert_eq!(runs(text, 3), expected);
        let records = vec![(
            1,
            serde_json::from_value(serde_json::json!({"kind":"message", "text":text})).unwrap(),
        )];
        assert_eq!(
            overlaps(text, &[("fixture", &records)], 3).unwrap(),
            expected.into_iter().collect::<Vec<_>>()
        );
        assert!(runs("short", 6).is_empty());
    }
}
