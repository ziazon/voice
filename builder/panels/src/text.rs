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

// Python 3.12's re._casefix._EXTRA_CASES.
static EXTRA_CASES: &[(char, &[char])] = &[
    ('\u{0069}', &['\u{0131}']),
    ('\u{0073}', &['\u{017f}']),
    ('\u{00b5}', &['\u{03bc}']),
    ('\u{0131}', &['\u{0069}']),
    ('\u{017f}', &['\u{0073}']),
    ('\u{0345}', &['\u{03b9}', '\u{1fbe}']),
    ('\u{0390}', &['\u{1fd3}']),
    ('\u{03b0}', &['\u{1fe3}']),
    ('\u{03b2}', &['\u{03d0}']),
    ('\u{03b5}', &['\u{03f5}']),
    ('\u{03b8}', &['\u{03d1}']),
    ('\u{03b9}', &['\u{0345}', '\u{1fbe}']),
    ('\u{03ba}', &['\u{03f0}']),
    ('\u{03bc}', &['\u{00b5}']),
    ('\u{03c0}', &['\u{03d6}']),
    ('\u{03c1}', &['\u{03f1}']),
    ('\u{03c2}', &['\u{03c3}']),
    ('\u{03c3}', &['\u{03c2}']),
    ('\u{03c6}', &['\u{03d5}']),
    ('\u{03d0}', &['\u{03b2}']),
    ('\u{03d1}', &['\u{03b8}']),
    ('\u{03d5}', &['\u{03c6}']),
    ('\u{03d6}', &['\u{03c0}']),
    ('\u{03f0}', &['\u{03ba}']),
    ('\u{03f1}', &['\u{03c1}']),
    ('\u{03f5}', &['\u{03b5}']),
    ('\u{0432}', &['\u{1c80}']),
    ('\u{0434}', &['\u{1c81}']),
    ('\u{043e}', &['\u{1c82}']),
    ('\u{0441}', &['\u{1c83}']),
    ('\u{0442}', &['\u{1c84}', '\u{1c85}']),
    ('\u{044a}', &['\u{1c86}']),
    ('\u{0463}', &['\u{1c87}']),
    ('\u{1c80}', &['\u{0432}']),
    ('\u{1c81}', &['\u{0434}']),
    ('\u{1c82}', &['\u{043e}']),
    ('\u{1c83}', &['\u{0441}']),
    ('\u{1c84}', &['\u{0442}', '\u{1c85}']),
    ('\u{1c85}', &['\u{0442}', '\u{1c84}']),
    ('\u{1c86}', &['\u{044a}']),
    ('\u{1c87}', &['\u{0463}']),
    ('\u{1c88}', &['\u{a64b}']),
    ('\u{1e61}', &['\u{1e9b}']),
    ('\u{1e9b}', &['\u{1e61}']),
    ('\u{1fbe}', &['\u{0345}', '\u{03b9}']),
    ('\u{1fd3}', &['\u{0390}']),
    ('\u{1fe3}', &['\u{03b0}']),
    ('\u{a64b}', &['\u{1c88}']),
    ('\u{fb05}', &['\u{fb06}']),
    ('\u{fb06}', &['\u{fb05}']),
];

/// Trim the whitespace recognized by Python's str.strip().
pub(crate) fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{001c}'..='\u{001f}'))
}

static WORD: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"^[\p{L}\p{N}_]$"));

fn simple_lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn same_case(t: char, p: char) -> bool {
    let (t, p) = (simple_lower(t), simple_lower(p));
    t == p
        || EXTRA_CASES
            .binary_search_by_key(&p, |(key, _)| *key)
            .is_ok_and(|i| EXTRA_CASES[i].1.contains(&t))
}

fn term_matches(text: &[char], term: &str) -> bool {
    let term: Vec<char> = term.chars().take(1001).collect();
    if term.len() > 1000 {
        return true;
    }
    if term.is_empty() || term.len() > text.len() {
        return false;
    }
    let Ok(word) = WORD.as_ref() else {
        return true;
    };
    let is_word = |c: char| word.is_match(c.encode_utf8(&mut [0; 4]));
    let first_word = is_word(term[0]);
    let last_word = is_word(term[term.len() - 1]);
    text.windows(term.len()).enumerate().any(|(start, part)| {
        let before_word = start.checked_sub(1).is_some_and(|i| is_word(text[i]));
        let after_word = text.get(start + term.len()).is_some_and(|c| is_word(*c));
        first_word != before_word
            && last_word != after_word
            && part.iter().zip(&term).all(|(&t, &p)| same_case(t, p))
    })
}

static EMAIL: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{N}_.+-]+@[\p{L}\p{N}_-]+(\.[\p{L}\p{N}_-]+)+"));
static PATH: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r#"(?:^|[\s\x1c-\x1f(\["'`=])(?:~/|/)[\p{L}\p{N}_.-]+/"#));
/// Return sorted leak categories without exposing matched text.
pub fn leak_findings(text: &str, terms: &[String]) -> Vec<&'static str> {
    let mut findings = Vec::new();
    if EMAIL.as_ref().map_or(true, |r| r.is_match(text)) {
        findings.push("email");
    }
    if PATH.as_ref().map_or(true, |r| r.is_match(text)) {
        findings.push("path");
    }
    let chars: Vec<char> = text.chars().collect();
    let found = terms.iter().any(|term| term_matches(&chars, term));
    if found {
        findings.push("term");
    }
    findings
}
