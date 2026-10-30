//! Check posts for private details before sharing them.
use persona_builder_ingest::filters::{Filters, Kind};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};
use voice_panels::Block;

const INVISIBLE: &[(u32, u32)] = &[
    (0x00ad, 0x00ad),
    (0x034f, 0x034f),
    (0x061c, 0x061c),
    (0x115f, 0x1160),
    (0x17b4, 0x17b5),
    (0x180b, 0x180f),
    (0x200b, 0x200f),
    (0x202a, 0x202e),
    (0x2060, 0x206f),
    (0x3164, 0x3164),
    (0xfe00, 0xfe0f),
    (0xfeff, 0xfeff),
    (0xffa0, 0xffa0),
    (0x1d173, 0x1d17a),
    (0xe0000, 0xe007f),
    (0xe0100, 0xe01ef),
];
static PATHS: LazyLock<[Result<Regex, regex::Error>; 4]> = LazyLock::new(|| {
    [
        Regex::new(r"(?i)\bfile:/"),
        Regex::new(r"(?:^|[^\w])[A-Za-z]:[\\/]"),
        Regex::new(r"\\\\[\w.$-]+\\"),
        Regex::new(r#"(?:^|[\s(\["'`=])~[\w.-]+/"#),
    ]
});
static EMAILS: LazyLock<[Result<Regex, regex::Error>; 2]> = LazyLock::new(|| {
    [
        Regex::new(r"[\w.+-]+@[\w-]+"),
        Regex::new(
            r"(?i)[\w.+-]+\s*(?:@|＠|\(at\)|\[at\])\s*[\w-]+(?:\s*(?:\.|\(dot\)|\[dot\])\s*[a-z]{2,})+",
        ),
    ]
});

/// Remove invisible characters that can hide a private detail.
pub fn scrub(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !INVISIBLE
                .iter()
                .any(|&(lo, hi)| (lo..=hi).contains(&(*c as u32)))
        })
        .collect()
}

/// Sorted categories of private details found in text.
pub fn text_findings(text: &str) -> Vec<&'static str> {
    let scrubbed = scrub(text);
    let mut found = BTreeSet::new();
    for value in [text, scrubbed.as_str()] {
        found.extend(voice_panels::leak_findings(value, &[]));
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
        for filter in filters {
            for checked in [value, scrubbed.as_str()] {
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
    let found = text_findings(text);
    if found.is_empty() {
        Ok(())
    } else {
        Err(crate::text::input_findings(&found))
    }
}
