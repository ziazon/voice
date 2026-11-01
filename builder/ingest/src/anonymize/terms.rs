use super::{Mapping, Rules};
use regex::{Regex, RegexBuilder};
use std::ops::Range;
pub(super) struct Term {
    pub text: String,
    pub kind: String,
    pub exact: Regex,
    pub rescans: Vec<(Regex, bool)>,
}
pub(super) fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
impl Term {
    pub fn new(text: &str, kind: &str) -> Result<Self, regex::Error> {
        let compile = |pattern: &str| {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .size_limit(1 << 20)
                .build()
        };
        let words = |value: &str| {
            value
                .split_whitespace()
                .filter(|word| word.chars().any(char::is_alphanumeric))
                .map(regex::escape)
                .collect::<Vec<_>>()
        };
        let exact = compile(&words(text).join(r"[^\p{L}\p{N}\n]+"))?;
        let mut rescans = Vec::new();
        let lowercase = text.to_lowercase();
        let folded = lowercase.replace('ß', "ss");
        let mut variants = vec![(text, false)];
        if lowercase.contains("ss") || lowercase != folded {
            variants = vec![(folded.as_str(), true)];
            if text != folded {
                variants.push((text, false));
            }
        }
        for (variant, fold_sharp_s) in variants {
            let escape = |piece: &str| {
                let escaped = regex::escape(piece);
                if fold_sharp_s {
                    escaped.replace("ss", "(?:ss|ß)")
                } else {
                    escaped
                }
            };
            let pattern = variant
                .split_whitespace()
                .filter(|word| word.chars().any(char::is_alphanumeric))
                .map(escape)
                .collect::<Vec<_>>()
                .join(r"[^\p{L}\p{N}]+");
            rescans.push((compile(&pattern)?, true));
            let runs: Vec<_> = variant
                .split(|c: char| !c.is_alphanumeric())
                .filter(|run| !run.is_empty())
                .collect();
            if runs.iter().map(|run| run.chars().count()).sum::<usize>() >= 4 {
                let pattern = runs
                    .iter()
                    .map(|run| escape(run))
                    .collect::<Vec<_>>()
                    .join(r"[^\p{L}\p{N}]*");
                rescans.push((compile(&pattern)?, false));
            }
        }
        Ok(Self {
            text: text.into(),
            kind: kind.into(),
            exact,
            rescans,
        })
    }
}
pub(super) fn matches<'a>(
    text: &'a str,
    re: &'a Regex,
) -> impl Iterator<Item = (usize, usize)> + 'a {
    let mut offset = 0;
    std::iter::from_fn(move || {
        while offset < text.len() {
            let m = re.find_at(text, offset)?;
            let bounded = !text[..m.start()]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
                && !text[m.end()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
            // Advance even after an empty match; literals and terms are nonempty by validation.
            offset = m.start() + text[m.start()..].chars().next()?.len_utf8();
            if bounded {
                return Some((m.start(), m.end()));
            }
        }
        None
    })
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
