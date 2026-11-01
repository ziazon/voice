//! Shared masks and scans, with one line index per scanned text.
use super::{Rules, detect, terms};
use regex::Regex;
use std::{collections::BTreeSet, ops::Range, sync::LazyLock};

static BUILTIN_MASKS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    ["git@github.com", "git@gitlab.com", "git@bitbucket.org"]
        .iter()
        .map(|s| Regex::new(&format!("(?i){}", regex::escape(s))).expect("fixed mask"))
        .collect()
});

pub(super) fn mask(text: &str, rules: &Rules, inserted: &[Range<usize>]) -> String {
    let mut masked = text.as_bytes().to_vec();
    for range in inserted {
        masked[range.clone()].fill(b' ');
    }
    let before = masked.clone();
    let mut allowed = Vec::new();
    for pattern in rules.allow.iter().chain(BUILTIN_MASKS.iter()) {
        for (start, end) in terms::matches(text, pattern) {
            masked[start..end].fill(b' ');
            allowed.push(start..end);
        }
    }
    if !allowed.is_empty() {
        let mut hits: Vec<_> = detect::find(text).map(|(range, _)| range).collect();
        for pattern in &rules.patterns {
            hits.extend(pattern.find_iter(text).map(|m| m.range()));
        }
        hits.extend(term_hits(text, rules));
        for hit in hits {
            if !allowed
                .iter()
                .any(|allow| allow.start <= hit.start && hit.end <= allow.end)
            {
                masked[hit.clone()].copy_from_slice(&before[hit]);
            }
        }
    }
    String::from_utf8(masked).expect("space masking preserves utf8")
}

fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            text.bytes()
                .enumerate()
                .filter_map(|(i, b)| (b == b'\n').then_some(i + 1)),
        )
        .collect()
}

pub(super) fn scan(
    text: &str,
    rules: &Rules,
    lines: &[usize],
    eof: usize,
    scan_terms: bool,
    found: &mut BTreeSet<(usize, &'static str)>,
) {
    let starts = line_starts(text);
    let mut add = |offset, kind| {
        let index = starts.partition_point(|start| *start <= offset) - 1;
        let line = lines.get(index).copied().unwrap_or(eof);
        found.insert((line, kind));
    };
    for (range, kind) in detect::find(text) {
        add(range.start, kind);
    }
    for pattern in &rules.patterns {
        for m in pattern.find_iter(text) {
            add(m.start(), "your pattern");
        }
    }
    if rules
        .terms
        .iter()
        .any(|t| t.text.chars().any(|c| c.is_alphabetic() && !c.is_ascii()))
    {
        for (offset, c) in text.char_indices() {
            if ('\u{0300}'..='\u{036f}').contains(&c) {
                add(offset, "accent mark");
            }
        }
    }
    if scan_terms {
        term_findings(text, &starts, rules, lines, eof, found);
    }
}

fn term_findings(
    text: &str,
    starts: &[usize],
    rules: &Rules,
    lines: &[usize],
    eof: usize,
    found: &mut BTreeSet<(usize, &'static str)>,
) {
    for hit in term_hits(text, rules) {
        let index = starts.partition_point(|offset| *offset <= hit.start) - 1;
        found.insert((lines.get(index).copied().unwrap_or(eof), "your term"));
    }
}

fn narrow(text: &str) -> (String, Vec<usize>) {
    let mut narrowed = String::with_capacity(text.len());
    let mut offsets = Vec::with_capacity(text.len() + 1);
    for (offset, c) in text.char_indices() {
        if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
            narrowed.push(char::from_u32(c as u32 - 0xfee0).expect("ASCII"));
            offsets.push(offset);
        } else {
            narrowed.push(c);
            offsets.extend(offset..offset + c.len_utf8());
        }
    }
    offsets.push(text.len());
    (narrowed, offsets)
}

fn term_hits(text: &str, rules: &Rules) -> Vec<Range<usize>> {
    let (narrowed, offsets) = narrow(text);
    let mut hits = Vec::new();
    for (copy, narrowed_copy) in [(text, false), (narrowed.as_str(), true)] {
        for term in &rules.terms {
            for (pattern, bounded) in &term.rescans {
                let mut add = |start: usize, end: usize| {
                    hits.push(if narrowed_copy {
                        offsets[start]..offsets[end]
                    } else {
                        start..end
                    });
                };
                if *bounded {
                    for (start, end) in terms::matches(copy, pattern) {
                        add(start, end);
                    }
                } else {
                    for m in pattern.find_iter(copy) {
                        add(m.start(), m.end());
                    }
                }
            }
        }
    }
    hits
}

pub(super) fn private_name(text: &str, rules: &Rules) -> bool {
    if rules
        .terms
        .iter()
        .any(|t| t.text.chars().any(|c| c.is_alphabetic() && !c.is_ascii()))
        && text.chars().any(|c| ('\u{0300}'..='\u{036f}').contains(&c))
    {
        return true;
    }
    let text = super::scrub(text);
    let masked = mask(&text, rules, &[]);
    let mut found = BTreeSet::new();
    scan(&masked, rules, &[], 1, true, &mut found);
    !found.is_empty()
}
