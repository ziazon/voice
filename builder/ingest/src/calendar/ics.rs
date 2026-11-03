//! RFC 5545 §3.1 content lines, §3.3 values; discarded properties are never retained.
use super::{BAD, TOO_BIG, invalid};
use crate::dates::{days_from_civil, days_in_month};
use std::collections::BTreeMap;
use voice_core::error::Result;

const MAX_LINE: usize = 64 * 1024;
const MAX_COMPONENTS: usize = 200_000;
const FIELDS: &[&str] = &[
    "UID",
    "DTSTART",
    "DTEND",
    "DURATION",
    "SUMMARY",
    "LOCATION",
    "STATUS",
    "RRULE",
    "EXDATE",
    "RECURRENCE-ID",
    "RDATE",
    "EXRULE",
];
#[derive(Default)]
pub(super) struct Entry {
    pub fields: BTreeMap<String, Property>,
    pub exceptions: Vec<Property>,
    pub bad: bool,
}
#[derive(Clone)]
pub(super) struct Property {
    pub params: BTreeMap<String, String>,
    pub value: String,
}
impl Property {
    pub fn time(&self) -> Option<Time> {
        time(&self.value, &self.params)
    }
}
pub(super) fn time(text: &str, params: &BTreeMap<String, String>) -> Option<Time> {
    let value_type = params.get("VALUE").map(|v| v.to_ascii_uppercase());
    if value_type
        .as_deref()
        .is_some_and(|v| !matches!(v, "DATE" | "DATE-TIME"))
    {
        return None;
    }
    let is_date = value_type.as_deref() == Some("DATE") || text.len() == 8;
    let wall = value(text, is_date)?;
    let frame = if is_date {
        if params.contains_key("TZID") {
            return None;
        }
        Frame::Date
    } else if text.ends_with('Z') {
        if params.contains_key("TZID") {
            return None;
        }
        Frame::Utc
    } else if let Some(name) = params.get("TZID") {
        Frame::Named(name.clone())
    } else {
        Frame::Local
    };
    Some(Time { wall, frame })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Frame {
    Date,
    Utc,
    Local,
    Named(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Time {
    pub wall: i64,
    pub frame: Frame,
}
impl Time {
    pub fn all_day(&self) -> bool {
        self.frame == Frame::Date
    }
}

pub(super) fn number(bytes: &[u8]) -> Option<i64> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    bytes.iter().try_fold(0i64, |n, b| {
        n.checked_mul(10)?.checked_add(i64::from(b - b'0'))
    })
}
// RFC 5545 §3.3.4, §3.3.5: civil dates, full times and a possible leap second.
pub(super) fn value(text: &str, date: bool) -> Option<i64> {
    let b = text.as_bytes();
    if (date && b.len() != 8) || (!date && !(b.len() == 15 || b.len() == 16 && b[15] == b'Z')) {
        return None;
    }
    let y = number(b.get(..4)?)?;
    let m = number(b.get(4..6)?)?;
    let d = number(b.get(6..8)?)?;
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let mut wall = days_from_civil(y, m, d) * 86_400;
    if !date {
        if b[8] != b'T' {
            return None;
        }
        let h = number(&b[9..11])?;
        let m = number(&b[11..13])?;
        let s = number(&b[13..15])?;
        if h > 23 || m > 59 || s > 60 {
            return None;
        }
        wall += h * 3600 + m * 60 + s.min(59);
    }
    Some(wall)
}

pub(super) fn entries(files: &[Vec<u8>]) -> Result<Vec<Entry>> {
    let mut all = Vec::new();
    let mut components = 0;
    for file in files {
        let bytes = file.strip_prefix(b"\xef\xbb\xbf").unwrap_or(file);
        let text = std::str::from_utf8(bytes).map_err(|_| invalid(BAD))?;
        let mut parser = Parser {
            stack: Vec::new(),
            current: None,
            entries: &mut all,
            components: &mut components,
            seen: false,
        };
        let mut line = String::new();
        let mut long = false;
        for physical in text.split('\n') {
            let physical = physical.strip_suffix('\r').unwrap_or(physical);
            if let Some(rest) = physical.strip_prefix([' ', '\t']) {
                if line.is_empty() && !long {
                    return Err(invalid(BAD));
                }
                if line.len().saturating_add(rest.len()) > MAX_LINE {
                    long = true;
                }
                if !long {
                    line.push_str(rest);
                }
            } else {
                parser.line(&line, long)?;
                line.clear();
                long = physical.len() > MAX_LINE;
                if !long {
                    line.push_str(physical);
                }
            }
        }
        parser.line(&line, long)?;
        if !parser.seen || !parser.stack.is_empty() {
            return Err(invalid(BAD));
        }
    }
    if files.is_empty() {
        return Err(invalid(BAD));
    }
    Ok(all)
}
struct Parser<'a> {
    stack: Vec<String>,
    current: Option<Entry>,
    entries: &'a mut Vec<Entry>,
    components: &'a mut usize,
    seen: bool,
}
impl Parser<'_> {
    fn line(&mut self, line: &str, long: bool) -> Result<()> {
        if long {
            if let Some(entry) = &mut self.current {
                entry.bad = true;
                return Ok(());
            }
            return Err(invalid(BAD));
        }
        if line.is_empty() {
            return Ok(());
        }
        let Some(colon) = outside(line, ':').next() else {
            if let Some(entry) = &mut self.current {
                entry.bad = true;
            }
            return Ok(());
        };
        let head = &line[..colon];
        let value = &line[colon + 1..];
        let name = head.split(';').next().unwrap_or("").to_ascii_uppercase();
        if matches!(name.as_str(), "BEGIN" | "END") {
            if head.contains(';') || value.is_empty() {
                return Err(invalid(BAD));
            }
            let component = value.to_ascii_uppercase();
            if name == "BEGIN" {
                *self.components += 1;
                if *self.components > MAX_COMPONENTS {
                    return Err(invalid(TOO_BIG));
                }
                if self.stack.is_empty() {
                    if component != "VCALENDAR" {
                        return Err(invalid(BAD));
                    }
                    self.seen = true;
                }
                if self.stack.len() == 1 && component == "VEVENT" {
                    self.current = Some(Entry::default());
                }
                self.stack.push(component);
            } else {
                if self.stack.pop().as_ref() != Some(&component) {
                    return Err(invalid(BAD));
                }
                if self.stack.len() == 1
                    && component == "VEVENT"
                    && let Some(entry) = self.current.take()
                {
                    self.entries.push(entry);
                }
            }
            return Ok(());
        }
        if self.stack.len() == 2
            && let Some(entry) = &mut self.current
            && outside(head, ';').take(17).count() > 16
        {
            entry.bad = true;
            return Ok(());
        }
        if self.stack.len() != 2 || self.current.is_none() || !FIELDS.contains(&name.as_str()) {
            return Ok(());
        }
        let entry = self.current.as_mut().expect("checked entry");
        let Some(params) = parameters(head) else {
            entry.bad = true;
            return Ok(());
        };
        let property = Property {
            params,
            value: value.into(),
        };
        if name == "EXDATE" {
            entry.exceptions.push(property);
        } else if let std::collections::btree_map::Entry::Vacant(field) = entry.fields.entry(name) {
            field.insert(property);
        } else {
            entry.bad = true;
        }
        Ok(())
    }
}
fn outside(text: &str, separator: char) -> impl Iterator<Item = usize> + '_ {
    let mut quoted = false;
    text.char_indices().filter_map(move |(i, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        (c == separator && !quoted).then_some(i)
    })
}
fn parameters(head: &str) -> Option<BTreeMap<String, String>> {
    let mut params = BTreeMap::new();
    let mut start = outside(head, ';').next().map_or(head.len(), |i| i + 1);
    while start < head.len() {
        let end = outside(&head[start..], ';')
            .next()
            .map_or(head.len(), |i| start + i);
        let (key, value) = head[start..end].split_once('=')?;
        if key.is_empty() || value.is_empty() {
            return None;
        }
        let value = if value.starts_with('"') {
            value.strip_prefix('"')?.strip_suffix('"')?
        } else {
            if value.contains('"') {
                return None;
            }
            value
        };
        if params
            .insert(key.to_ascii_uppercase(), value.into())
            .is_some()
        {
            return None;
        }
        start = end + 1;
    }
    Some(params)
}
// RFC 5545 §3.3.11 TEXT, then the brief's one-line, 200-character display policy.
pub(super) fn clean(text: &str) -> String {
    let mut chars = text.chars();
    let mut unescaped = String::new();
    while let Some(c) = chars.next() {
        let c = if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => '\n',
                Some(c) => c,
                None => '\\',
            }
        } else {
            c
        };
        if c != '\u{00ad}' {
            unescaped.push(if crate::text::hidden(c) { ' ' } else { c });
        }
    }
    unescaped.split_whitespace().collect::<Vec<_>>().join(" ")
}
pub(super) fn cut(text: &str) -> String {
    let mut chars = text.chars();
    let mut shown: String = chars.by_ref().take(200).collect();
    if chars.next().is_some() {
        shown.push('…');
    }
    shown
}
// RFC 5545 §3.3.6, limited to positive week/day/hour/minute/second durations.
pub(super) fn duration(text: &str) -> Option<i64> {
    let mut rest = text.strip_prefix('+').unwrap_or(text).strip_prefix('P')?;
    let mut time = false;
    let mut rank = 0;
    let mut total = 0i64;
    let mut any = false;
    let mut time_value = false;
    while !rest.is_empty() {
        if let Some(next) = rest.strip_prefix('T') {
            if time {
                return None;
            }
            time = true;
            rest = next;
            continue;
        }
        let n = rest.bytes().take_while(u8::is_ascii_digit).count();
        let value = number(rest.as_bytes().get(..n)?)?;
        let unit = *rest.as_bytes().get(n)?;
        let (order, seconds) = match (time, unit) {
            (false, b'W') => (1, 604800),
            (false, b'D') => (2, 86400),
            (true, b'H') => (3, 3600),
            (true, b'M') => (4, 60),
            (true, b'S') => (5, 1),
            _ => return None,
        };
        if order <= rank || rank == 1 || (unit == b'W' && n + 1 != rest.len()) {
            return None;
        }
        rank = order;
        any = true;
        time_value |= time;
        total = total.checked_add(value.checked_mul(seconds)?)?;
        rest = &rest[n + 1..];
    }
    (any && (!time || time_value)).then_some(total)
}
