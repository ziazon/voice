//! The claude.ai export: check its shape, list its chats for review, mark them, and mine the ones
//! marked as the person's own. A port of `crew_decisions.py`'s `claudeai` commands.
use crate::{
    filters::{Filters, Kind},
    mine::{self, Split, message_record, own_message, text_blocks},
    privacy::privacy,
    python::{line_break, py_space, splitlines, truthy},
};
use persona_builder::error::{Error, Result};
use regex::Regex;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

const CAP: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;
const CONVERSATIONS: &str = "conversations.json";
const MINE_MARKS: [&str; 3] = ["y", "yes", "mine"];
const OPENING_WORDS: usize = 8;
const LONGEST_WORD: usize = 24;
pub const REVIEW_HEADER: &str = "mine\tcreated\tupdated\tmessages\tproject\ttitle\tuuid";

pub struct Export {
    pub files: usize,
    pub conversations: Value,
}
pub struct Inspection {
    pub text: String,
    pub missing: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewRow {
    pub mine: String,
    pub created: String,
    pub updated: String,
    pub messages: String,
    pub project: String,
    pub title: String,
    pub uuid: String,
}
#[derive(Debug, Default)]
pub struct Titles {
    pub rows: Vec<ReviewRow>,
    pub untitled: usize,
    pub opens: usize,
    pub filtered: usize,
    pub empty: usize,
}
#[derive(Default)]
pub struct Selector {
    pub uuids: Vec<String>,
    pub project: Option<String>,
    pub title: Option<Regex>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub unmarked: bool,
    pub all: bool,
}
#[derive(Debug, Default)]
pub struct MinedExport {
    pub records: Vec<Map<String, Value>>,
    pub read: usize,
    pub not_mine: usize,
    pub unmarked: usize,
    pub personal: usize,
    pub secret: usize,
    pub cutoff: Option<String>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
fn shape(n: usize, key: &str) -> Error {
    invalid(format!(
        "conversations.json: conversation {n} has an unexpected shape ({key})"
    ))
}
fn message_shape(n: usize, k: usize, key: &str) -> Error {
    invalid(format!(
        "conversations.json: conversation {n}, message {k} has an unexpected shape ({key})"
    ))
}
fn too_big() -> Error {
    invalid("the export's conversations.json is bigger than the 2 GB cap")
}

fn open(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|e| {
            if e.raw_os_error() == Some(libc::ELOOP) {
                invalid(format!(
                    "{} is a link: Persona Builder doesn't follow links. Use the export file itself",
                    path.display()
                ))
            } else if e.kind() == std::io::ErrorKind::NotFound {
                invalid(format!("{} doesn't exist", path.display()))
            } else {
                Error::Io {
                    path: path.to_owned(),
                    source: e,
                }
            }
        })
}
fn read_capped(path: &Path, file: File, declared: u64) -> Result<Vec<u8>> {
    if declared > CAP {
        return Err(too_big());
    }
    let mut bytes = Vec::new();
    file.take(CAP + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })?;
    if bytes.len() as u64 > CAP {
        return Err(too_big());
    }
    Ok(bytes)
}
fn component_is_conversations(name: &[u8]) -> bool {
    name.split(|b| *b == b'/')
        .rfind(|part| !part.is_empty() && *part != b".")
        == Some(CONVERSATIONS.as_bytes())
}
fn from_zip(file: File) -> Result<(usize, Vec<u8>)> {
    let unreadable =
        |e: &dyn std::fmt::Display| invalid(format!("the export zip can't be read ({e})"));
    let mut archive = zip::ZipArchive::new(file).map_err(|e| unreadable(&e))?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid(
            "the export zip holds more than 100,000 files, so Persona Builder won't open it",
        ));
    }
    let mut files = 0;
    let mut found = vec![];
    for index in 0..archive.len() {
        let entry = archive.by_index_raw(index).map_err(|e| unreadable(&e))?;
        let name = entry.name_raw();
        if name.ends_with(b"/") {
            continue;
        }
        files += 1;
        if component_is_conversations(name) {
            found.push((index, entry.size(), entry.compressed_size()));
        }
    }
    let [(index, size, compressed)] = found[..] else {
        return Err(invalid(format!(
            "the export zip should hold one conversations.json, and it holds {}",
            found.len()
        )));
    };
    if size > CAP {
        return Err(too_big());
    }
    if u128::from(size) > 100 * u128::from(compressed) {
        return Err(invalid(
            "the export's conversations.json is compressed more than 100:1, so Persona Builder won't open it",
        ));
    }
    // The zip crate keeps one entry per name, so a repeated name would hide an entry; this also
    // refuses local headers that disagree with the central directory.
    archive
        .validate_header_consistency()
        .map_err(|e| unreadable(&e))?;
    let entry = archive.by_index(index).map_err(|e| unreadable(&e))?;
    let mut bytes = Vec::new();
    entry
        .take(size + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| unreadable(&e))?;
    if bytes.len() as u64 > size {
        return Err(invalid(
            "the export's conversations.json is bigger than the zip says",
        ));
    }
    Ok((files, bytes))
}

/// The export's file count and its parsed conversations.json, from a zip, a folder or the file
/// itself. Read in memory: nothing from the export is unpacked to disk.
pub fn read_export(path: &Path) -> Result<Export> {
    let io = |source| Error::Io {
        path: path.to_owned(),
        source,
    };
    let mut file = open(path)?;
    let metadata = file.metadata().map_err(io)?;
    let (files, bytes) = if metadata.is_dir() {
        let mut files = 0;
        for entry in fs::read_dir(path).map_err(io)? {
            if fs::metadata(entry.map_err(io)?.path()).is_ok_and(|m| m.is_file()) {
                files += 1;
            }
        }
        let inner = path.join(CONVERSATIONS);
        let file = open(&inner).map_err(|e| match e {
            Error::Invalid(m) if m.ends_with("doesn't exist") => {
                invalid(format!("{} has no conversations.json", path.display()))
            }
            other => other,
        })?;
        let metadata = file.metadata().map_err(io)?;
        if !metadata.is_file() {
            return Err(invalid(format!("{} is not a file", inner.display())));
        }
        (files, read_capped(&inner, file, metadata.len())?)
    } else if metadata.is_file() {
        let mut magic = Vec::with_capacity(4);
        (&mut file).take(4).read_to_end(&mut magic).map_err(io)?;
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        if magic == b"PK\x03\x04" || magic == b"PK\x05\x06" {
            if metadata.len() > CAP {
                return Err(invalid("the export zip is bigger than the 2 GB cap"));
            }
            from_zip(file)?
        } else {
            (1, read_capped(path, file, metadata.len())?)
        }
    } else {
        return Err(invalid(format!(
            "{} is not a file or a folder",
            path.display()
        )));
    };
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("the export's conversations.json is not UTF-8 text"))?;
    let conversations = serde_json::from_str(text)
        .map_err(|_| invalid("the export's conversations.json is not valid JSON"))?;
    Ok(Export {
        files,
        conversations,
    })
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "list",
        Value::String(_) => "str",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_i64() || n.is_u64() => "int",
        Value::Number(_) => "float",
        Value::Null => "null",
    }
}
fn list(export: &Export) -> Result<&[Value]> {
    export
        .conversations
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("the export's conversations.json isn't a list of conversations"))
}
/// Control characters escaped, so a crafted key can't send terminal escapes.
fn shown(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}
#[derive(Default)]
struct Entry {
    types: BTreeSet<&'static str>,
    count: usize,
    of: usize,
    values: BTreeSet<String>,
}
#[derive(Default)]
struct Tree {
    order: Vec<(String, Entry)>,
    index: HashMap<String, usize>,
}
impl Tree {
    fn entry(&mut self, path: String) -> &mut Entry {
        let next = self.order.len();
        let at = *self.index.entry(path.clone()).or_insert(next);
        if at == next {
            self.order.push((path, Entry::default()));
        }
        &mut self.order[at].1
    }
}
/// Every key under prefix: the types seen and how many of the objects carry it. `messages` says
/// the items are the elements of a conversation's `chat_messages` list.
fn key_tree(items: &[&Value], prefix: &str, messages: bool, tree: &mut Tree) {
    let objects: Vec<&Map<String, Value>> = items.iter().filter_map(|v| v.as_object()).collect();
    let keys: BTreeSet<&str> = objects
        .iter()
        .flat_map(|o| o.keys().map(String::as_str))
        .collect();
    for key in keys {
        let values: Vec<&Value> = objects.iter().filter_map(|o| o.get(key)).collect();
        let path = format!("{prefix}.{key}");
        let entry = tree.entry(path.clone());
        entry.types.extend(values.iter().map(|v| type_name(v)));
        entry.count += values.len();
        entry.of += objects.len();
        // Only the message senders' values are shown: a `sender` elsewhere (a tool's input) may
        // hold a person's name. Checked by place, not by path, since a key may hold `.` or `[]`.
        if messages && key == "sender" {
            entry
                .values
                .extend(values.iter().filter_map(|v| v.as_str()).map(str::to_owned));
        }
        let children: Vec<&Value> = values
            .iter()
            .filter_map(|v| v.as_array())
            .flatten()
            .collect();
        if !children.is_empty() {
            let messages = prefix.is_empty() && key == "chat_messages";
            key_tree(&children, &format!("{path}[]"), messages, tree);
        }
        let nested: Vec<&Value> = values.iter().copied().filter(|v| v.is_object()).collect();
        if !nested.is_empty() {
            key_tree(&nested, &path, false, tree);
        }
    }
}

/// The export's field names, types and counts, never its values (bar the message senders), and
/// the fields the miner reads that it lacks.
pub fn inspect(export: &Export) -> Result<Inspection> {
    let conversations = list(export)?;
    let mut tree = Tree::default();
    key_tree(
        &conversations.iter().collect::<Vec<_>>(),
        "",
        false,
        &mut tree,
    );
    let mut text = format!(
        "files: {}\nconversations.json: list of {}\n",
        export.files,
        conversations.len()
    );
    for (path, e) in &tree.order {
        let types = e.types.iter().copied().collect::<Vec<_>>().join("|");
        text += &format!("  {}: {types} ({}/{})", shown(path), e.count, e.of);
        if !e.values.is_empty() {
            let values = e
                .values
                .iter()
                .map(|v| shown(v))
                .collect::<Vec<_>>()
                .join(", ");
            text += &format!(" values: {values}");
        }
        text.push('\n');
    }
    let wanted = [
        ".uuid",
        ".name",
        ".created_at",
        ".chat_messages",
        ".chat_messages[].sender",
        ".chat_messages[].created_at",
    ];
    let mut missing: Vec<String> = wanted
        .iter()
        .filter(|w| !tree.index.contains_key(**w))
        .map(|w| w.to_string())
        .collect();
    if !tree.index.contains_key(".chat_messages[].text")
        && !tree.index.contains_key(".chat_messages[].content[].text")
    {
        missing.push(".chat_messages[].text or .chat_messages[].content[].text".into());
    }
    Ok(Inspection { text, missing })
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// An export timestamp as UTC with milliseconds and a Z, the shape Claude Code transcripts use.
/// Reads `YYYY-MM-DD[(T| )HH:MM[:SS[.digits]][Z|±HH:MM]]`; `None` for anything else.
pub fn utc(when: &str) -> Option<String> {
    fn number(bytes: &[u8]) -> Option<i64> {
        bytes
            .iter()
            .all(u8::is_ascii_digit)
            .then(|| bytes.iter().fold(0, |n, d| n * 10 + i64::from(d - b'0')))
    }
    let b = when.as_bytes();
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (year, month, day) = (number(&b[..4])?, number(&b[5..7])?, number(&b[8..10])?);
    if year < 1 || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    let (mut hour, mut minute, mut second, mut millis, mut offset) = (0, 0, 0, 0, 0);
    let mut rest = &b[10..];
    if let Some((&separator, time)) = rest.split_first() {
        if separator != b'T' && separator != b' ' || time.len() < 5 || time[2] != b':' {
            return None;
        }
        hour = number(&time[..2])?;
        minute = number(&time[3..5])?;
        rest = &time[5..];
        if let Some(seconds) = rest.strip_prefix(b":") {
            second = number(seconds.get(..2)?)?;
            rest = &seconds[2..];
            if let Some(fraction) = rest.strip_prefix(b".") {
                let digits = fraction.iter().take_while(|d| d.is_ascii_digit()).count();
                if digits == 0 {
                    return None;
                }
                millis = fraction[..digits]
                    .iter()
                    .chain(b"000")
                    .take(3)
                    .fold(0, |n, d| n * 10 + i64::from(d - b'0'));
                rest = &fraction[digits..];
            }
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        match rest {
            [] | [b'Z'] => {}
            [sign @ (b'+' | b'-'), h1, h2, b':', m1, m2] => {
                let (h, m) = (number(&[*h1, *h2])?, number(&[*m1, *m2])?);
                if h > 23 || m > 59 {
                    return None;
                }
                offset = (h * 60 + m) * 60 * if *sign == b'+' { 1 } else { -1 };
            }
            _ => return None,
        }
    }
    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset;
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    if !(1..=9999).contains(&year) {
        return None;
    }
    let time = seconds.rem_euclid(86_400);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        time / 3600,
        time % 3600 / 60,
        time % 60
    ))
}
fn timestamp(value: &Value) -> Result<String> {
    value.as_str().and_then(utc).ok_or_else(|| {
        invalid("a date in the export isn't one Persona Builder can read: the export's shape changed, run persona-builder claudeai inspect")
    })
}

/// Python's `re.sub(r"\s+", " ", text).strip()`.
fn flat(text: &str) -> String {
    text.split(py_space)
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn chat_text(message: &Map<String, Value>) -> std::result::Result<String, &'static str> {
    if let Some(Value::String(text)) = message.get("text")
        && !text.trim_matches(py_space).is_empty()
    {
        return Ok(text.clone());
    }
    Ok(text_blocks(message.get("content").unwrap_or(&Value::Null))?.join("\n"))
}
fn dotted<'a>(mut value: &'a Value, path: &str) -> &'a str {
    for part in path.split('.') {
        value = value.get(part).unwrap_or(&Value::Null);
    }
    value.as_str().unwrap_or("")
}

/// The review file's rows, as `read_review` reads them.
pub fn read_review(name: &str, bytes: &[u8]) -> Result<Vec<ReviewRow>> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| invalid(format!("{name} is not UTF-8 text")))?;
    let lines = splitlines(text);
    if lines.first() != Some(&REVIEW_HEADER) {
        return Err(invalid(format!(
            "{name}: not a review file (its first line must be mine created updated messages project title uuid)"
        )));
    }
    let mut rows = vec![];
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.chars().all(py_space) {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [mine, created, updated, messages, project, title, uuid, ..] = fields[..] else {
            return Err(invalid(format!("{name}:{}: a row needs 7 columns", i + 1)));
        };
        rows.push(ReviewRow {
            mine: mine.into(),
            created: created.into(),
            updated: updated.into(),
            messages: messages.into(),
            project: project.into(),
            title: title.into(),
            uuid: uuid.into(),
        });
    }
    Ok(rows)
}
pub fn review_text(rows: &[ReviewRow]) -> String {
    let mut text = format!("{REVIEW_HEADER}\n");
    for r in rows {
        let fields = [
            &r.mine,
            &r.created,
            &r.updated,
            &r.messages,
            &r.project,
            &r.title,
            &r.uuid,
        ];
        text += &fields.map(String::as_str).join("\t");
        text.push('\n');
    }
    text
}

fn conversation(value: &Value, n: usize) -> Result<&Map<String, Value>> {
    value.as_object().ok_or_else(|| shape(n, "conversation"))
}
fn uuid(c: &Map<String, Value>, n: usize) -> Result<&str> {
    c.get("uuid")
        .and_then(Value::as_str)
        .ok_or_else(|| shape(n, "uuid"))
}
fn messages(c: &Map<String, Value>, n: usize) -> Result<Vec<(usize, &Map<String, Value>)>> {
    c.get("chat_messages")
        .and_then(Value::as_array)
        .ok_or_else(|| shape(n, "chat_messages"))?
        .iter()
        .enumerate()
        .map(|(i, m)| {
            m.as_object()
                .map(|m| (i + 1, m))
                .ok_or_else(|| message_shape(n, i + 1, "message"))
        })
        .collect()
}
fn is_sender(message: &Map<String, Value>, sender: &str) -> bool {
    message.get("sender").and_then(Value::as_str) == Some(sender)
}

/// An untitled conversation's stand-in title: its first human words, or [filtered] when that whole
/// message trips a privacy filter. A long word (a link, an ID) shows as ….
fn opening_title(
    filters: &Filters,
    humans: &[(usize, &Map<String, Value>)],
    n: usize,
) -> Result<String> {
    let mut text = String::new();
    for (k, m) in humans {
        let t = chat_text(m).map_err(|key| message_shape(n, *k, key))?;
        if !flat(&t).is_empty() {
            text = t;
            break;
        }
    }
    if text.is_empty() {
        return Ok(String::new());
    }
    if filters.kind(&text).is_some() {
        return Ok("[filtered]".into());
    }
    let flattened = flat(&text);
    let words: Vec<&str> = flattened.split(' ').collect();
    let shown: Vec<&str> = words
        .iter()
        .take(OPENING_WORDS)
        .map(|w| {
            if w.chars().count() <= LONGEST_WORD {
                w
            } else {
                "…"
            }
        })
        .collect();
    let more = if words.len() > OPENING_WORDS {
        "…"
    } else {
        ""
    };
    Ok(format!("opens: {}{more}", shown.join(" ")))
}

/// The review rows for every conversation, oldest first, keeping the marks already made.
pub fn titles(
    filters: &Filters,
    export: &Export,
    existing: &[ReviewRow],
    project_field: Option<&str>,
) -> Result<Titles> {
    let marks: HashMap<&str, &str> = existing
        .iter()
        .map(|r| (r.uuid.as_str(), r.mine.as_str()))
        .collect();
    let mut keyed = vec![];
    for (i, value) in list(export)?.iter().enumerate() {
        let n = i + 1;
        let c = conversation(value, n)?;
        let created = timestamp(c.get("created_at").ok_or_else(|| shape(n, "created_at"))?)?;
        let id = uuid(c, n)?;
        if id.chars().any(|ch| ch == '\t' || line_break(ch)) {
            return Err(invalid(format!(
                "conversations.json: conversation {n} has a uuid Persona Builder can't list"
            )));
        }
        keyed.push((created, id, n, value, c));
    }
    keyed.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    let mut t = Titles::default();
    for (created, id, n, value, c) in keyed {
        let humans: Vec<_> = messages(c, n)?
            .into_iter()
            .filter(|(_, m)| is_sender(m, "human"))
            .collect();
        let name = match c.get("name") {
            Some(v) if truthy(v) => v.as_str().ok_or_else(|| shape(n, "name"))?,
            _ => "",
        };
        let mut title = flat(name);
        if title.is_empty() {
            title = opening_title(filters, &humans, n)?;
            t.untitled += 1;
            match title.as_str() {
                "[filtered]" => t.filtered += 1,
                "" => t.empty += 1,
                _ => t.opens += 1,
            }
        }
        let updated = match c.get("updated_at") {
            Some(v) if truthy(v) => timestamp(v)?,
            _ => created.clone(),
        };
        let project = match project_field {
            Some(field) if !field.is_empty() => flat(dotted(value, field)),
            _ => String::new(),
        };
        t.rows.push(ReviewRow {
            mine: marks.get(id).copied().unwrap_or("").into(),
            created: created[..10].into(),
            updated: updated[..10].into(),
            messages: humans.len().to_string(),
            project,
            title,
            uuid: id.into(),
        });
    }
    Ok(t)
}
pub fn titles_summary(t: &Titles, shown: &str) -> String {
    let kept = t.rows.iter().filter(|r| !r.mine.is_empty()).count();
    let more = if t.untitled > 0 {
        format!(
            "; {} untitled: {} by opening words, {} filtered",
            t.untitled, t.opens, t.filtered
        )
    } else {
        String::new()
    };
    format!(
        "listed {} conversations in {shown} ({kept} already marked{more})\n",
        t.rows.len()
    )
}

/// A `--title` pattern, compiled by the `regex` crate.
pub fn title_pattern(pattern: &str) -> Result<Regex> {
    Regex::new(pattern).map_err(|_| {
        invalid("--title is not a valid pattern, or it uses look-around or a backreference, which Persona Builder can't read")
    })
}
impl Selector {
    /// Whether any selector was given; `mark` needs one.
    pub fn is_given(&self) -> bool {
        !self.uuids.is_empty()
            || self.project.is_some()
            || self.title.is_some()
            || self.before.is_some()
            || self.after.is_some()
            || self.unmarked
            || self.all
    }
    fn matches(&self, r: &ReviewRow) -> bool {
        (self.uuids.is_empty() || self.uuids.contains(&r.uuid))
            && self.project.as_ref().is_none_or(|p| *p == r.project)
            && self.title.as_ref().is_none_or(|t| t.is_match(&r.title))
            && self.before.as_ref().is_none_or(|b| r.created <= *b)
            && self.after.as_ref().is_none_or(|a| r.created >= *a)
            && (!self.unmarked || r.mine.is_empty())
    }
}
/// Mark every row the selector matches; the number of rows whose mark changed.
pub fn mark(rows: &mut [ReviewRow], selector: &Selector, mark: &str) -> usize {
    let mut changed = 0;
    for r in rows.iter_mut().filter(|r| selector.matches(r)) {
        if r.mine != mark {
            changed += 1;
        }
        r.mine = mark.into();
    }
    changed
}

/// The messages of every conversation marked as the person's own, filtered and split.
pub fn mine(
    filters: &Filters,
    export: &Export,
    review: &[ReviewRow],
    split: &Split,
) -> Result<MinedExport> {
    if matches!(split, Split::Holdout(h) if !h.is_finite() || !(0.0..=1.0).contains(h)) {
        return Err(invalid("--holdout must be a number from 0 to 1"));
    }
    let rows: HashMap<&str, &ReviewRow> = review.iter().map(|r| (r.uuid.as_str(), r)).collect();
    let mut m = MinedExport::default();
    for (i, value) in list(export)?.iter().enumerate() {
        let n = i + 1;
        let c = conversation(value, n)?;
        let id = uuid(c, n)?;
        let row = rows.get(id);
        let mark = row
            .map(|r| r.mine.trim_matches(py_space).to_lowercase())
            .unwrap_or_default();
        let Some(row) = row.filter(|_| MINE_MARKS.contains(&mark.as_str())) else {
            if mark.is_empty() {
                m.unmarked += 1;
            } else {
                m.not_mine += 1;
            }
            continue;
        };
        m.read += 1;
        let project = if row.project.is_empty() {
            "claude.ai".to_owned()
        } else {
            format!("claude.ai/{}", row.project)
        };
        let mut context = String::new();
        for (k, x) in messages(c, n)? {
            if is_sender(x, "assistant") {
                let reply = chat_text(x).map_err(|key| message_shape(n, k, key))?;
                let reply = reply.trim_matches(py_space);
                if !reply.is_empty() {
                    context = reply.to_owned();
                }
                continue;
            }
            if !is_sender(x, "human") {
                continue;
            }
            let text = chat_text(x).map_err(|key| message_shape(n, k, key))?;
            let Some(text) = own_message(filters, text.trim_matches(py_space).to_owned()) else {
                continue;
            };
            let when = timestamp(
                x.get("created_at")
                    .ok_or_else(|| message_shape(n, k, "created_at"))?,
            )?;
            let record = Value::Object(message_record(id, &when, &project, &text, &context));
            match privacy(filters, &record)? {
                Some(Kind::Personal) => m.personal += 1,
                Some(Kind::Secret) => m.secret += 1,
                None => match record {
                    Value::Object(record) => m.records.push(record),
                    _ => unreachable!("built as an object"),
                },
            }
        }
    }
    m.records.sort_by(|a, b| {
        (a["timestamp"].as_str(), a["id"].as_str())
            .cmp(&(b["timestamp"].as_str(), b["id"].as_str()))
    });
    m.cutoff = mine::mark_split(&mut m.records, split);
    Ok(m)
}
pub fn mine_summary(m: &MinedExport) -> String {
    format!(
        "read {} conversations marked mine; skipped {} not mine and {} unmarked\nmined {} messages; dropped {} personal records, {} secret records\n",
        m.read,
        m.not_mine,
        m.unmarked,
        m.records.len(),
        m.personal,
        m.secret
    )
}
