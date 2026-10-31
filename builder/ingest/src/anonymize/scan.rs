//! Shared masks and scans, with one line index per scanned text.
use super::{Rules, detect, terms};
use regex::Regex;
use std::{collections::BTreeSet, sync::LazyLock};

static BUILTIN_MASKS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    ["git@github.com", "git@gitlab.com", "git@bitbucket.org"]
        .iter()
        .map(|s| Regex::new(&format!("(?i){}", regex::escape(s))).expect("fixed mask"))
        .collect()
});

pub(super) fn mask(text: &str, rules: &Rules, inserted: &[std::ops::Range<usize>]) -> String {
    let mut masked = text.as_bytes().to_vec();
    for range in inserted {
        masked[range.clone()].fill(b' ');
    }
    let mut allowed = false;
    for pattern in rules.allow.iter().chain(BUILTIN_MASKS.iter()) {
        for (start, end) in terms::matches(text, pattern) {
            masked[start..end].fill(b' ');
            allowed = true;
        }
    }
    if allowed {
        // A suffix such as `com` has word boundaries but must not hide part of an email.
        // Only a mask covering the entire detected identifier can exempt it.
        for (range, _) in detect::find(text) {
            if masked[range.clone()].iter().any(|b| *b != b' ') {
                masked[range.clone()].copy_from_slice(&text.as_bytes()[range]);
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
    // Fullwidth ASCII has different byte offsets, so index this copy independently.
    let narrowed: String = text
        .chars()
        .map(|c| {
            if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
                char::from_u32(c as u32 - 0xfee0).expect("ASCII")
            } else {
                c
            }
        })
        .collect();
    let narrowed_starts = line_starts(&narrowed);
    for (copy, starts) in [
        (text, starts),
        (narrowed.as_str(), narrowed_starts.as_slice()),
    ] {
        for term in &rules.terms {
            for (pattern, bounded) in &term.rescans {
                let mut add = |start| {
                    let index = starts.partition_point(|offset| *offset <= start) - 1;
                    found.insert((lines.get(index).copied().unwrap_or(eof), "your term"));
                };
                if *bounded {
                    for (start, _) in terms::matches(copy, pattern) {
                        add(start);
                    }
                } else {
                    for m in pattern.find_iter(copy) {
                        add(m.start());
                    }
                }
            }
        }
    }
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
