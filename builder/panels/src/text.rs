use crate::{ProtocolError, casefold_table::CASEFOLD};
use regex::Regex;
use std::sync::LazyLock;

/// Strip hidden characters and bound text by Unicode code points.
pub fn clean(text: &str, max_chars: usize) -> String {
    let text: String = text.chars().map(|c| {
        if (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}') { ' ' } else { c }
    }).collect();
    let text = text.trim();
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        text.chars().take(max_chars).chain(['…']).collect()
    }
}

fn casefold(text: &str) -> String {
    let mut result = String::new();
    for c in text.chars() {
        // Per-character lowercase avoids the string lowercase final-sigma rule.
        match CASEFOLD.binary_search_by_key(&c, |(key, _)| *key) {
            Ok(i) => result.push_str(CASEFOLD[i].1),
            Err(_) => result.extend(c.to_lowercase()),
        }
    }
    result
}

/// Normalize whitespace and full Unicode case for exact item matching.
pub fn match_key(item: &str) -> String {
    casefold(
        &clean(item, 10_000)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Fence untrusted text with a validated label and a longer backtick run.
pub fn fence(label: &str, text: &str) -> Result<String, ProtocolError> {
    if label.is_empty()
        || label.len() > 80
        || !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " ._:/()-".contains(c))
    {
        return Err(ProtocolError::InvalidFenceLabel);
    }
    let mut run = 0;
    let mut length = 3;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        length = length.max(run + 1);
    }
    let marker = "`".repeat(length);
    Ok(format!("{marker}text {label}\n{text}\n{marker}"))
}

static EMAIL: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"[\w.+-]+@[\w-]+(\.[\w-]+)+"));
static PATH: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r#"(?:^|[\s(\["'`=])(?:~/|/)[\w.-]+/"#));
/// Return sorted leak categories without exposing matched text.
pub fn leak_findings(text: &str, terms: &[String]) -> Vec<&'static str> {
    let mut findings = Vec::new();
    if EMAIL.as_ref().map_or(true, |r| r.is_match(text)) {
        findings.push("email");
    }
    if PATH.as_ref().map_or(true, |r| r.is_match(text)) {
        findings.push("path");
    }
    let found = terms.iter().filter(|term| !term.is_empty()).any(|term| {
        Regex::new(&format!(r"(?i)\b{}\b", regex::escape(term)))
            .as_ref()
            .map_or(true, |r| r.is_match(text))
    });
    if found {
        findings.push("term");
    }
    findings
}
