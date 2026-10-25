//! Exact unified changes to a text file.
use crate::text;
use persona_builder::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Line<'a> {
    text: &'a str,
    newline: bool,
}
fn lines(text: &str) -> Vec<Line<'_>> {
    text.split_inclusive('\n')
        .map(|line| Line {
            text: line.strip_suffix('\n').unwrap_or(line),
            newline: line.ends_with('\n'),
        })
        .collect()
}
fn invalid() -> Error {
    Error::Invalid(text::DIFF_DOES_NOT_FIT.into())
}

/// Build a bounded line-LCS change with three lines of context.
pub fn unified(file: &str, old: &str, new: &str) -> Result<String> {
    let (a, b) = (lines(old), lines(new));
    let mut head = 0;
    while head < a.len().min(b.len()) && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < (a.len() - head).min(b.len() - head)
        && a[a.len() - tail - 1] == b[b.len() - tail - 1]
    {
        tail += 1;
    }
    let (left, right) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    if left.len() > 2000 || right.len() > 2000 {
        return Err(Error::Invalid(text::DIFF_TOO_BIG.into()));
    }
    let width = right.len() + 1;
    let mut lcs = vec![0u16; (left.len() + 1) * width];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            lcs[i * width + j] = if left[i] == right[j] {
                1 + lcs[(i + 1) * width + j + 1]
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let mut ops = Vec::new();
    for line in &a[..head] {
        ops.push((' ', *line));
    }
    let (mut i, mut j) = (0, 0);
    while i < left.len() || j < right.len() {
        if i < left.len() && j < right.len() && left[i] == right[j] {
            ops.push((' ', left[i]));
            i += 1;
            j += 1;
        } else if i < left.len()
            && (j == right.len() || lcs[(i + 1) * width + j] >= lcs[i * width + j + 1])
        {
            ops.push(('-', left[i]));
            i += 1;
        } else {
            ops.push(('+', right[j]));
            j += 1;
        }
    }
    for line in &a[a.len() - tail..] {
        ops.push((' ', *line));
    }
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (index, _) in ops.iter().enumerate().filter(|(_, (kind, _))| *kind != ' ') {
        let start = index.saturating_sub(3);
        let end = (index + 4).min(ops.len());
        if let Some(last) = hunks.last_mut().filter(|last| start <= last.1) {
            last.1 = end;
        } else {
            hunks.push((start, end));
        }
    }
    let mut out = format!("--- a/{file}\n+++ b/{file}\n");
    for (start, end) in hunks {
        let old_before = ops[..start].iter().filter(|(k, _)| *k != '+').count();
        let new_before = ops[..start].iter().filter(|(k, _)| *k != '-').count();
        let old_count = ops[start..end].iter().filter(|(k, _)| *k != '+').count();
        let new_count = ops[start..end].iter().filter(|(k, _)| *k != '-').count();
        let old_start = old_before + usize::from(old_count != 0);
        let new_start = new_before + usize::from(new_count != 0);
        out.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));
        for (kind, line) in &ops[start..end] {
            out.push(*kind);
            out.push_str(line.text);
            out.push('\n');
            if !line.newline {
                out.push_str("\\ No newline at end of file\n");
            }
        }
    }
    Ok(out)
}
fn range(value: &str) -> Option<(usize, usize)> {
    let (start, count) = value.split_once(',')?;
    Some((start.parse().ok()?, count.parse().ok()?))
}
fn position(start: usize, count: usize) -> Option<usize> {
    if count == 0 {
        Some(start)
    } else {
        start.checked_sub(1)
    }
}
/// Apply only an exact change: no offset search or softened context matching.
pub fn apply(old: &str, diff: &str, file: &str) -> Result<String> {
    if !diff.ends_with('\n') {
        return Err(invalid());
    }
    let input: Vec<_> = diff.split_terminator('\n').collect();
    if input.first().copied() != Some(format!("--- a/{file}").as_str())
        || input.get(1).copied() != Some(format!("+++ b/{file}").as_str())
    {
        return Err(invalid());
    }
    let source = lines(old);
    let mut result: Vec<Line<'_>> = Vec::new();
    let (mut at, mut cursor) = (2, 0);
    while at < input.len() {
        let header = input[at]
            .strip_prefix("@@ -")
            .and_then(|s| s.strip_suffix(" @@"))
            .ok_or_else(invalid)?;
        let (left, right) = header.split_once(" +").ok_or_else(invalid)?;
        let (old_start, old_count) = range(left).ok_or_else(invalid)?;
        let (new_start, new_count) = range(right).ok_or_else(invalid)?;
        let old_pos = position(old_start, old_count).ok_or_else(invalid)?;
        let new_pos = position(new_start, new_count).ok_or_else(invalid)?;
        if old_pos < cursor || old_pos > source.len() {
            return Err(invalid());
        }
        result.extend_from_slice(&source[cursor..old_pos]);
        if new_pos != result.len() {
            return Err(invalid());
        }
        cursor = old_pos;
        at += 1;
        let (mut removed, mut added) = (0usize, 0usize);
        while at < input.len() && !input[at].starts_with("@@ ") {
            let row = input[at];
            let kind = row.as_bytes().first().copied().ok_or_else(invalid)?;
            if !matches!(kind, b' ' | b'-' | b'+') {
                return Err(invalid());
            }
            let mut line = Line {
                text: &row[1..],
                newline: true,
            };
            at += 1;
            if input.get(at).copied() == Some("\\ No newline at end of file") {
                line.newline = false;
                at += 1;
            }
            if kind != b'+' {
                if source.get(cursor) != Some(&line) {
                    return Err(invalid());
                }
                cursor += 1;
                removed += 1;
            }
            if kind != b'-' {
                result.push(line);
                added += 1;
            }
        }
        if removed != old_count || added != new_count {
            return Err(invalid());
        }
    }
    result.extend_from_slice(&source[cursor..]);
    if result
        .iter()
        .take(result.len().saturating_sub(1))
        .any(|line| !line.newline)
    {
        return Err(invalid());
    }
    let mut out = String::new();
    for line in result {
        out.push_str(line.text);
        if line.newline {
            out.push('\n');
        }
    }
    Ok(out)
}

/// Added content rows, excluding the two file headers.
pub fn added_lines(diff: &str) -> Vec<&str> {
    diff.lines()
        .skip(2)
        .filter_map(|line| line.strip_prefix('+'))
        .collect()
}
