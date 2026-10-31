use super::{Mapping, Rules};
use regex::{Regex, RegexBuilder};
use std::ops::Range;
pub(super) struct Term {
    pub text: String,
    pub kind: String,
    pub exact: Regex,
    pub wrapped: Regex,
    pub substring: Option<Regex>,
}
pub(super) fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
impl Term {
    pub fn new(text: &str, kind: &str) -> Self {
        let words = text
            .split_whitespace()
            .map(regex::escape)
            .collect::<Vec<_>>();
        let compile = |pattern: &str| {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .expect("escaped term regex")
        };
        let exact = compile(&words.join(r"[^\p{L}\p{N}\n]{1,8}"));
        let wrapped = compile(&words.join(r"[^\p{L}\p{N}]{1,8}"));
        let substring = if text.chars().count() >= 4 && text.chars().all(char::is_alphanumeric) {
            Some(compile(&regex::escape(text)))
        } else {
            None
        };
        Self {
            text: text.into(),
            kind: kind.into(),
            exact,
            wrapped,
            substring,
        }
    }
}
pub(super) fn matches(text: &str, re: &Regex) -> Vec<(usize, usize)> {
    let mut found = vec![];
    let mut offset = 0;
    while offset < text.len() {
        let Some(m) = re.find_at(text, offset) else {
            break;
        };
        let bounded = !text[..m.start()]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
            && !text[m.end()..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        if bounded {
            found.push((m.start(), m.end()));
            offset = m.end();
        } else {
            offset = m.start()
                + text[m.start()..]
                    .chars()
                    .next()
                    .expect("match start")
                    .len_utf8();
        }
    }
    found
}
pub(super) fn replace(
    text: &str,
    rules: &Rules,
    mapping: &mut Mapping,
) -> (String, Vec<Range<usize>>) {
    let mut found = vec![];
    for (index, term) in rules.terms.iter().enumerate() {
        for (start, end) in matches(text, &term.exact) {
            found.push((start, end, index));
        }
    }
    found.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    let mut out = String::new();
    let mut inserted = Vec::new();
    let mut offset = 0;
    for (start, end, index) in found {
        if start < offset {
            continue;
        }
        out.push_str(&text[offset..start]);
        let term = &rules.terms[index];
        let placeholder_start = out.len();
        out.push_str(&mapping.placeholder(&term.kind, &term.text));
        inserted.push(placeholder_start..out.len());
        offset = end;
    }
    out.push_str(&text[offset..]);
    (out, inserted)
}
