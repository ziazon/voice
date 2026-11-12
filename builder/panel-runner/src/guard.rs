//! Check posts for private details before sharing them.
use radio_panels::Block;
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};
use voice_ingest::filters::{Filters, Kind};

static PATHS: LazyLock<[Result<Regex, regex::Error>; 5]> = LazyLock::new(|| {
    [
        Regex::new(r"(?i)\bfile:/"),
        Regex::new(r"(?:^|[^\w])[A-Za-z]:[\\/]"),
        Regex::new(r"\\\\[\w.$-]+\\"),
        Regex::new(r"(?:^|[^\w])~+/|\w~/"),
        Regex::new(r"(?:^|[^\w])~+[^\W\d][\w.-]*/"),
    ]
});
static EMAILS: LazyLock<[Result<Regex, regex::Error>; 2]> = LazyLock::new(|| {
    [
        Regex::new(r"[\w.+-]+[@＠﹫][\w-]+"),
        Regex::new(
            r"(?i)[\w.+-]+\s*(?:@|＠|﹫|\(at\)|\[at\])\s*[\w-]+(?:\s*(?:\.|\(dot\)|\[dot\])\s*[a-z]{2,})+",
        ),
    ]
});

pub use voice_ingest::anonymize::scrub;

/// The exact text shared as a previous post in the next round's brief.
pub fn forwarded(text: &str) -> String {
    radio_panels::clean(&scrub(text), 8000)
}

/// Sorted categories of private details found in text.
pub fn text_findings(text: &str) -> Vec<&'static str> {
    let scrubbed = scrub(text);
    let forwarded = forwarded(text);
    let mut found = BTreeSet::new();
    for value in [text, scrubbed.as_str(), forwarded.as_str()] {
        found.extend(radio_panels::leak_findings(value, &[]));
        for (category, patterns) in [("path", PATHS.as_slice()), ("email", EMAILS.as_slice())] {
            if patterns.iter().any(|pattern| {
                pattern
                    .as_ref()
                    .map_or(true, |pattern| pattern.is_match(value))
            }) {
                found.insert(category);
            }
        }
    }
    found.into_iter().collect()
}

/// Check the text and decoded block against all assistants' filters.
pub fn post_findings(text: &str, block: Option<&Block>, filters: &[Filters]) -> Vec<&'static str> {
    post_findings_refs(text, block, &filters.iter().collect::<Vec<_>>())
}

/// Check against borrowed filters held by the running panel.
pub fn post_findings_refs(
    text: &str,
    block: Option<&Block>,
    filters: &[&Filters],
) -> Vec<&'static str> {
    let mut decoded = Vec::new();
    if let Some(block) = block {
        decoded.extend(block.items.iter().map(String::as_str));
        decoded.extend(block.draft.as_deref());
        decoded.extend(block.agree_with.as_deref());
        decoded.push(&block.objections);
    }
    let decoded = decoded.join("\n");
    let mut found = BTreeSet::new();
    for value in [text, decoded.as_str()] {
        found.extend(text_findings(value));
        let scrubbed = scrub(value);
        let forwarded = forwarded(value);
        for filter in filters {
            for checked in [value, scrubbed.as_str(), forwarded.as_str()] {
                match filter.kind(checked) {
                    Some(Kind::Personal) => {
                        found.insert("private");
                    }
                    Some(Kind::Secret) => {
                        found.insert("secret");
                    }
                    None => {}
                }
            }
        }
    }
    found.into_iter().collect()
}

/// Refuse private details in a question, background or steer.
pub fn guard_input(text: &str) -> Result<(), String> {
    let mut found = text_findings(text);
    found.extend(text_findings(&radio_panels::clean(text, usize::MAX)));
    found.sort_unstable();
    found.dedup();
    if found.is_empty() {
        Ok(())
    } else {
        Err(crate::text::input_findings(&found))
    }
}
