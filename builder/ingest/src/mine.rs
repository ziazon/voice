use crate::{
    filters::{Filters, Kind},
    folder::Folder,
    privacy::privacy,
    python::{PythonFormatter, decode, py_space, splitlines, truthy},
};
use regex::Regex;
use serde_json::{Map, Value};
use std::{
    ffi::{CString, OsStr},
    fs::File,
    io::{BufRead, BufReader},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
    sync::LazyLock,
};
use voice_core::{
    deny::DenyList,
    error::{Error, Result},
    member::Member,
};

pub const DEFAULT_HOLDOUT: f64 = 0.15;
const UNEXPECTED_SHAPE: &str = "a transcript record has an unexpected shape";

pub enum Split {
    Holdout(f64),
    Cutoff(String),
}
#[derive(Debug, Default)]
pub struct Mined {
    pub records: Vec<Map<String, Value>>,
    pub questions: usize,
    pub held: usize,
    pub messages: usize,
    pub cutoff: Option<String>,
    pub excluded: usize,
    pub personal: usize,
    pub secret: usize,
    /// Records of a shape Voice can't read, left out whole.
    pub unreadable: usize,
    /// Files on the never-read list, left out unopened.
    pub denied: usize,
}
#[derive(Debug)]
pub struct FolderRow {
    pub excluded: bool,
    pub sessions: usize,
    pub project: String,
    pub folder: String,
}
fn io<T>(path: &Path, result: std::io::Result<T>) -> Result<T> {
    result.map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}
fn link(path: &Path) -> Error {
    Error::Invalid(format!(
        "{} is a link: Voice doesn't follow links in your Claude Code history. Move it away and try again",
        path.display()
    ))
}
// Only shape errors may be skipped; other failures must stop mining.
fn unreadable(error: &Error) -> bool {
    matches!(error, Error::Invalid(m) if m.contains(UNEXPECTED_SHAPE))
}
fn name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_owned)
        .ok_or_else(|| Error::Invalid(format!("{} is not UTF-8", path.display())))
}
struct Entry {
    path: PathBuf,
    name: CString,
}
fn open_projects(projects: &Path) -> Result<Folder> {
    match Folder::open(projects, true) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Invalid(format!(
            "{} doesn't exist: there is no Claude Code history there",
            projects.display()
        ))),
        result => io(projects, result),
    }
}
fn project_folders(projects: &Path, root: &Folder) -> Result<Vec<Entry>> {
    let mut folders = vec![];
    for entry_name in io(projects, root.names())? {
        let path = projects.join(OsStr::from_bytes(entry_name.to_bytes()));
        let kind = io(&path, root.stat(&entry_name, false))?.st_mode & libc::S_IFMT;
        if kind == libc::S_IFLNK {
            return Err(link(&path));
        }
        if kind == libc::S_IFDIR {
            name(&path)?;
            folders.push(Entry {
                path,
                name: entry_name,
            });
        }
    }
    folders.sort_by_key(|entry| name(&entry.path).expect("validated folder name"));
    Ok(folders)
}
fn open_project(root: &Folder, entry: &Entry) -> Result<Folder> {
    root.folder(&entry.name).map_err(|e| {
        if e.raw_os_error() == Some(libc::ELOOP) {
            link(&entry.path)
        } else {
            Error::Io {
                path: entry.path.clone(),
                source: e,
            }
        }
    })
}
fn session_files(
    folder_path: &Path,
    folder: &Folder,
    deny: &DenyList,
) -> Result<(Vec<Entry>, usize)> {
    let mut files = vec![];
    let mut denied = 0;
    for entry_name in io(folder_path, folder.names())? {
        let path = folder_path.join(OsStr::from_bytes(entry_name.to_bytes()));
        if !entry_name.to_bytes().ends_with(b".jsonl") {
            continue;
        }
        if deny.denied(&path) {
            denied += 1;
            continue;
        }
        name(&path)?;
        let metadata = io(&path, folder.stat(&entry_name, false))?;
        let kind = metadata.st_mode & libc::S_IFMT;
        if kind == libc::S_IFLNK {
            return Err(link(&path));
        }
        if kind != libc::S_IFREG {
            return Err(Error::Invalid(format!("{} is not a file", path.display())));
        }
        if metadata.st_nlink > 1 {
            denied += 1;
            continue;
        }
        files.push(Entry {
            path,
            name: entry_name,
        });
    }
    files.sort_by_key(|entry| entry.path.file_name().map(|name| name.to_owned()));
    Ok((files, denied))
}
fn open_session(folder: &Folder, entry: &Entry) -> Result<Option<File>> {
    let path = &entry.path;
    let file = folder.file(&entry.name, libc::O_NONBLOCK).map_err(|e| {
        if e.raw_os_error() == Some(libc::ELOOP) {
            link(path)
        } else {
            Error::Io {
                path: path.to_owned(),
                source: e,
            }
        }
    })?;
    let metadata = io(path, file.metadata())?;
    if !metadata.is_file() {
        return Err(Error::Invalid(format!("{} is not a file", path.display())));
    }
    if metadata.nlink() > 1 {
        return Ok(None);
    }
    Ok(Some(file))
}
pub fn project_name(folder: &str, home: &Path) -> String {
    static WORKTREE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("--claude-worktrees-.*").unwrap());
    static HOME: LazyLock<Regex> =
        LazyLock::new(|| Regex::new("^-(Users|home)-[A-Za-z0-9]+-").unwrap());
    let name = WORKTREE.replace_all(folder, "").into_owned();
    let prefix: String = home
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .chain(std::iter::once('-'))
        .collect();
    let name = match name.strip_prefix(&prefix) {
        Some(n) => n.to_owned(),
        None => HOME.replace(&name, "").into_owned(),
    };
    let name = name.strip_prefix("projects-").unwrap_or(&name);
    match name.strip_prefix('-') {
        Some(n) => format!(".{n}"),
        None => name.to_owned(),
    }
}
fn record_id(kind: &str, session: &str, when: &str, text: &str) -> String {
    // SHA-1 is only a stable record id, never a security control.
    let digest = sha1_smol::Sha1::from(format!("{session}\n{when}\n{text}"))
        .digest()
        .to_string();
    format!("{kind}:{}", &digest[..12])
}
struct Builder<'a> {
    member: &'a Member,
    filters: &'a Filters,
    mark: Regex,
    reminder: Regex,
    commas: Regex,
    path: &'a Path,
    line: usize,
}
impl Builder<'_> {
    fn shape(&self, key: &str) -> Error {
        Error::Invalid(format!(
            "{}:{}: {UNEXPECTED_SHAPE} ({key})",
            self.path.display(),
            self.line
        ))
    }
    fn object<'a>(&self, v: &'a Value, key: &str) -> Result<&'a Map<String, Value>> {
        v.as_object().ok_or_else(|| self.shape(key))
    }
    fn string<'a>(&self, v: &'a Value, key: &str) -> Result<&'a str> {
        v.as_str().ok_or_else(|| self.shape(key))
    }
    fn field<'a>(&self, obj: &'a Map<String, Value>, key: &str, error: &str) -> Result<&'a str> {
        match obj.get(key) {
            None => Ok(""),
            Some(v) => self.string(v, error),
        }
    }
    fn marks(&self, label: &str) -> Vec<String> {
        self.mark
            .captures_iter(label)
            .flat_map(|c| {
                [c.get(1), c.get(2)]
                    .into_iter()
                    .flatten()
                    .filter(|m| !m.as_str().is_empty())
                    .map(|m| m.as_str().to_lowercase())
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    fn recommended_label(&self, labels: &[String]) -> Option<String> {
        labels
            .iter()
            .find(|l| self.marks(l).iter().any(|m| m == "recommended"))
            .cloned()
    }
    fn member_label(&self, labels: &[String]) -> Option<String> {
        labels
            .iter()
            .find(|l| {
                self.marks(l)
                    .contains(&self.member.mark_word.to_lowercase())
            })
            .cloned()
    }
    fn parse_answer(&self, answer: &str, labels: &[String], multi: bool) -> (Vec<String>, String) {
        if !multi {
            return if labels.iter().any(|l| l == answer) {
                (vec![answer.into()], String::new())
            } else {
                (vec![], answer.trim_matches(py_space).into())
            };
        }
        let mut labels = labels.to_vec();
        labels.sort_by_key(|l| std::cmp::Reverse(l.chars().count()));
        let mut rest = answer.to_owned();
        let mut picks = vec![];
        for label in labels {
            if rest.contains(&label) {
                rest = rest.replacen(&label, "", 1);
                picks.push(label)
            }
        }
        (
            picks,
            self.commas
                .replace_all(&rest, ", ")
                .trim_matches([' ', ','])
                .to_owned(),
        )
    }
    fn list_answer(&self, answer: &[Value], labels: &[String]) -> Result<(Vec<String>, String)> {
        let mut picks = vec![];
        let mut own_words = vec![];
        for item in answer {
            let item = self.string(item, "answer")?;
            if labels.iter().any(|label| label == item) {
                if !picks.iter().any(|pick| pick == item) {
                    picks.push(item.to_owned());
                }
            } else {
                let text = item.trim_matches(py_space);
                if !text.is_empty() {
                    own_words.push(text);
                }
            }
        }
        Ok((picks, own_words.join(", ")))
    }
    fn questions_in(
        &self,
        r: &Map<String, Value>,
        session: &str,
        project: &str,
        when: &str,
    ) -> Result<Vec<Map<String, Value>>> {
        let Some(result) = r.get("toolUseResult").and_then(Value::as_object) else {
            return Ok(vec![]);
        };
        let (Some(questions), Some(answers)) = (result.get("questions"), result.get("answers"))
        else {
            return Ok(vec![]);
        };
        let questions = questions
            .as_array()
            .ok_or_else(|| self.shape("questions"))?;
        let mut out = vec![];
        for q in questions {
            let answers = self.object(answers, "answers")?;
            let q = self.object(q, "question")?;
            let question = match q.get("question") {
                Some(Value::String(s)) => s.as_str(),
                Some(Value::Array(_) | Value::Object(_)) => return Err(self.shape("question")),
                _ => continue,
            };
            let Some(answer) = answers.get(question).filter(|v| !v.is_null()) else {
                continue;
            };
            let options = match q.get("options") {
                None => vec![],
                Some(v) => v.as_array().ok_or_else(|| self.shape("options"))?.clone(),
            };
            let mut labels = vec![];
            let mut normalized = vec![];
            for o in options {
                let o = self.object(&o, "options")?;
                let label = self.field(o, "label", "options")?;
                let description = self.field(o, "description", "options")?;
                labels.push(label.to_owned());
                normalized.push(Value::Object(Map::from_iter([
                    ("label".into(), Value::from(label)),
                    ("description".into(), Value::from(description)),
                ])))
            }
            let multi = q.get("multiSelect").is_some_and(truthy);
            let (picks, own) = match answer {
                Value::Array(items) => self.list_answer(items, &labels)?,
                _ => self.parse_answer(self.string(answer, "answer")?, &labels, multi),
            };
            let recommended = self.recommended_label(&labels);
            let mut notes = "";
            if let Some(annotations) = result.get("annotations").filter(|v| truthy(v)) {
                let a = self.object(annotations, "annotations")?;
                if let Some(entry) = a.get(question).filter(|v| truthy(v)) {
                    let e = self.object(entry, "annotations")?;
                    if let Some(n) = e.get("notes").filter(|v| truthy(v)) {
                        notes = self.string(n, "notes")?
                    }
                }
            }
            let header = self.field(q, "header", "header")?;
            let outcome = outcome(&picks, &own, recommended.as_deref());
            let mut record = Map::new();
            for (k, v) in [
                (
                    "id",
                    Value::from(record_id("question", session, when, question)),
                ),
                ("kind", Value::from("question")),
                ("timestamp", Value::from(when)),
                ("project", Value::from(project)),
                ("header", Value::from(header)),
                ("question", Value::from(question)),
                ("multiSelect", Value::from(multi)),
                ("options", Value::Array(normalized)),
                (
                    "recommended",
                    recommended.map(Value::from).unwrap_or(Value::Null),
                ),
            ] {
                record.insert(k.into(), v);
            }
            record.insert(
                self.member.pick_field(),
                self.member_label(&labels)
                    .map(Value::from)
                    .unwrap_or(Value::Null),
            );
            for (k, v) in [
                (
                    "picks",
                    Value::Array(picks.into_iter().map(Value::from).collect()),
                ),
                ("own_words", Value::from(own)),
                ("notes", Value::from(notes)),
                ("outcome", Value::from(outcome)),
            ] {
                record.insert(k.into(), v);
            }
            out.push(record);
        }
        Ok(out)
    }
    fn text_blocks<'a>(&self, content: &'a Value) -> Result<Vec<&'a str>> {
        text_blocks(content).map_err(|key| self.shape(key))
    }
    fn content<'a>(&self, r: &'a Map<String, Value>) -> Result<&'a Value> {
        match r.get("message") {
            None => Ok(&Value::Null),
            Some(m) => Ok(self
                .object(m, "message")?
                .get("content")
                .unwrap_or(&Value::Null)),
        }
    }
    fn typed_text(&self, r: &Map<String, Value>) -> Result<Option<String>> {
        if r.get("isMeta").is_some_and(truthy) || r.get("isCompactSummary").is_some_and(truthy) {
            return Ok(None);
        }
        let Some(origin) = r.get("origin").filter(|v| truthy(v)) else {
            return Ok(None);
        };
        if self
            .object(origin, "origin")?
            .get("kind")
            .and_then(Value::as_str)
            != Some("human")
        {
            return Ok(None);
        }
        let blocks = self.text_blocks(self.content(r)?)?;
        let text = blocks
            .into_iter()
            .map(|b| {
                self.reminder
                    .replace_all(b, "")
                    .trim_matches(py_space)
                    .to_owned()
            })
            .filter(|b| !b.is_empty() && !b.starts_with('<'))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(self.own_message(text))
    }
    fn own_message(&self, text: String) -> Option<String> {
        own_message(self.filters, text)
    }
    fn assistant_text(&self, r: &Map<String, Value>) -> Result<String> {
        Ok(self.text_blocks(self.content(r)?)?.join("\n"))
    }
    fn harvest(
        &self,
        r: &Value,
        session: &str,
        project: &str,
        context: &mut String,
    ) -> Result<Vec<Map<String, Value>>> {
        let r = self.object(r, "record")?;
        let kind = r.get("type").and_then(Value::as_str);
        if kind == Some("assistant") {
            let text = self.assistant_text(r)?;
            let text = text.trim_matches(py_space);
            if !text.is_empty() {
                *context = text.into()
            }
        }
        if kind != Some("user") {
            return Ok(vec![]);
        }
        let Some(when) = r.get("timestamp").filter(|v| truthy(v)) else {
            return Ok(vec![]);
        };
        let when = self.string(when, "timestamp")?;
        let mut out = self.questions_in(r, session, project, when)?;
        if let Some(text) = self.typed_text(r)? {
            out.push(message_record(session, when, project, &text, context))
        }
        Ok(out)
    }
}
/// Text strings from a message's string content or typed block list; the error is the key whose
/// shape is wrong.
pub(crate) fn text_blocks(content: &Value) -> std::result::Result<Vec<&str>, &'static str> {
    if let Value::String(s) = content {
        return Ok(vec![s]);
    }
    if !truthy(content) {
        return Ok(vec![]);
    }
    match content {
        Value::Object(_) => Ok(vec![]),
        Value::Array(a) => a
            .iter()
            .filter_map(|v| v.as_object())
            .filter(|o| o.get("type").and_then(Value::as_str) == Some("text"))
            .map(|o| match o.get("text") {
                None => Ok(""),
                Some(Value::String(s)) => Ok(s.as_str()),
                Some(_) => Err("text"),
            })
            .collect(),
        _ => Err("content"),
    }
}
/// The text, unless it is empty, pasted, a one-word nudge or too long to be typed.
pub(crate) fn own_message(filters: &Filters, text: String) -> Option<String> {
    if text.is_empty()
        || filters
            .pasted_prefixes()
            .iter()
            .any(|p| text.starts_with(p))
        || ["try again", "continue", "retry"].contains(&text.to_lowercase().as_str())
        || text.chars().count() > 4000
    {
        None
    } else {
        Some(text)
    }
}
/// The Python's `outcome`: own words win, then whether the recommendation was picked.
pub fn outcome(picks: &[String], own: &str, recommended: Option<&str>) -> &'static str {
    if !own.is_empty() {
        "own_words"
    } else {
        match recommended {
            None => "no_rec",
            Some(r) if picks.iter().any(|p| p == r) => "took_rec",
            Some(_) => "other_option",
        }
    }
}
pub(crate) fn message_record(
    session: &str,
    when: &str,
    project: &str,
    text: &str,
    context: &str,
) -> Map<String, Value> {
    let context: String = context
        .chars()
        .skip(context.chars().count().saturating_sub(300))
        .collect();
    Map::from_iter([
        (
            "id".into(),
            Value::from(record_id("message", session, when, text)),
        ),
        ("kind".into(), Value::from("message")),
        ("timestamp".into(), Value::from(when)),
        ("project".into(), Value::from(project)),
        ("text".into(), Value::from(text)),
        ("context".into(), Value::from(context)),
    ])
}
pub(crate) fn mark_split(records: &mut [Map<String, Value>], split: &Split) -> Option<String> {
    let mut questions: Vec<_> = records
        .iter()
        .filter(|r| r["kind"] == "question")
        .map(|r| r["timestamp"].as_str().unwrap().to_owned())
        .collect();
    questions.sort();
    let cutoff = match split {
        Split::Cutoff(s) => Some(s.clone()),
        Split::Holdout(h) if !questions.is_empty() => {
            let held = ((questions.len() as f64 * h).round_ties_even() as usize).max(1);
            Some(questions[questions.len() - held].clone())
        }
        _ => None,
    };
    for r in records {
        let held = cutoff
            .as_ref()
            .is_some_and(|c| r["timestamp"].as_str().unwrap() >= c.as_str());
        r.insert(
            "split".into(),
            Value::from(if held { "heldout" } else { "train" }),
        );
    }
    cutoff
}
/// The gate marks a label carries: `(Recommended)`, `(Recommended · <Name>)` or `(<Name>)`.
pub struct Mark(Regex);
impl Mark {
    pub fn new(mark_word: &str) -> Result<Self> {
        mark_pattern(mark_word).map(Self)
    }
    /// The Python's `bare`: the label without its marks, stripped as `str.strip()` strips.
    pub fn bare(&self, label: &str) -> String {
        self.0
            .replace_all(label, "")
            .trim_matches(py_space)
            .to_owned()
    }
}
fn mark_pattern(mark_word: &str) -> Result<Regex> {
    const SPACE: &str = r"[\s\x1c-\x1f]";
    let word = regex::escape(mark_word);
    Regex::new(&format!(
        r"(?i){SPACE}*\({SPACE}*(recommended|{word})(?:{SPACE}*[·•/+,&-]{SPACE}*(recommended|{word}))?{SPACE}*\)"
    ))
    .map_err(|_| Error::Invalid("member config: mark_word is too large".into()))
}

pub fn mine(
    member: &Member,
    filters: &Filters,
    projects: &Path,
    home: &Path,
    split: &Split,
) -> Result<Mined> {
    mine_with_progress(member, filters, projects, home, split, &mut |_, _| {})
}

pub fn mine_with_progress(
    member: &Member,
    filters: &Filters,
    projects: &Path,
    home: &Path,
    split: &Split,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Mined> {
    let deny = DenyList::for_member(member, home)?;
    deny.check(projects)?;
    if matches!(split, Split::Holdout(h) if !h.is_finite() || !(0.0..=1.0).contains(h)) {
        return Err(Error::Invalid(
            "--holdout must be a number from 0 to 1".into(),
        ));
    }
    let mark = mark_pattern(&member.mark_word)?;
    let reminder = Regex::new(r"(?s)<system-reminder>.*?</system-reminder>").unwrap();
    let commas = Regex::new(r"[\s\x1c-\x1f]*,([\s\x1c-\x1f]*,)+[\s\x1c-\x1f]*").unwrap();
    let mut mined = Mined::default();
    let root = open_projects(projects)?;
    let folders = project_folders(projects, &root)?;
    let total = folders.len();
    progress(0, total);
    for (index, entry) in folders.into_iter().enumerate() {
        let folder_path = &entry.path;
        if deny.denied(folder_path) {
            mined.excluded += 1;
            progress(index + 1, total);
            continue;
        }
        let folder_name = name(folder_path)?;
        if filters.excluded(&folder_name) {
            mined.excluded += 1;
            progress(index + 1, total);
            continue;
        }
        let project = project_name(&folder_name, home);
        let folder = open_project(&root, &entry)?;
        let (files, denied) = session_files(folder_path, &folder, &deny)?;
        mined.denied += denied;
        for entry in files {
            let path = &entry.path;
            let session = path
                .file_stem()
                .and_then(|n| n.to_str())
                .expect("validated UTF-8 name");
            let Some(file) = open_session(&folder, &entry)? else {
                mined.denied += 1;
                continue;
            };
            let mut reader = BufReader::new(file);
            let mut bytes = vec![];
            let mut context = String::new();
            let mut b = Builder {
                member,
                filters,
                mark: mark.clone(),
                reminder: reminder.clone(),
                commas: commas.clone(),
                path: &entry.path,
                line: 0,
            };
            loop {
                bytes.clear();
                if io(path, reader.read_until(b'\n', &mut bytes))? == 0 {
                    break;
                }
                b.line += 1;
                let text = decode(&bytes);
                for line in splitlines(&text) {
                    if line.is_empty() {
                        continue;
                    }
                    let Ok(r) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    let records = match b.harvest(&r, session, &project, &mut context) {
                        Ok(records) => records,
                        Err(e) if unreadable(&e) => {
                            mined.unreadable += 1;
                            continue;
                        }
                        Err(e) => return Err(e),
                    };
                    for record in records {
                        match privacy(filters, &Value::Object(record.clone()))? {
                            Some(Kind::Personal) => mined.personal += 1,
                            Some(Kind::Secret) => mined.secret += 1,
                            None => mined.records.push(record),
                        }
                    }
                }
            }
        }
        progress(index + 1, total);
    }
    mined.records.sort_by(|a, b| {
        (a["timestamp"].as_str().unwrap(), a["id"].as_str().unwrap())
            .cmp(&(b["timestamp"].as_str().unwrap(), b["id"].as_str().unwrap()))
    });
    mined.cutoff = mark_split(&mut mined.records, split);
    mined.questions = mined
        .records
        .iter()
        .filter(|r| r["kind"] == "question")
        .count();
    mined.held = mined
        .records
        .iter()
        .filter(|r| r["kind"] == "question" && r["split"] == "heldout")
        .count();
    mined.messages = mined.records.len() - mined.questions;
    Ok(mined)
}
pub fn summary(m: &Mined) -> String {
    let date = |r: &Map<String, Value>| {
        r["timestamp"]
            .as_str()
            .unwrap()
            .chars()
            .take(10)
            .collect::<String>()
    };
    let span = match (m.records.first(), m.records.last()) {
        (Some(a), Some(b)) => format!("{} → {}", date(a), date(b)),
        _ => "none".into(),
    };
    let skipped = if m.unreadable == 0 {
        String::new()
    } else {
        format!("; skipped {} records of an unexpected shape", m.unreadable)
    };
    let denied = if m.denied == 0 {
        String::new()
    } else {
        format!("; left out {} files on the never-read list", m.denied)
    };
    format!(
        "mined {} questions ({} held out) and {} messages, {}; held out from {}\ndropped {} excluded folders, {} personal records, {} secret records{skipped}{denied}\n",
        m.questions,
        m.held,
        m.messages,
        span,
        m.cutoff
            .as_deref()
            .filter(|c| !c.is_empty())
            .unwrap_or("nothing"),
        m.excluded,
        m.personal,
        m.secret
    )
}
pub fn to_jsonl(records: &[Map<String, Value>]) -> String {
    use serde::Serialize;
    let mut bytes = vec![];
    for r in records {
        r.serialize(&mut serde_json::Serializer::with_formatter(
            &mut bytes,
            PythonFormatter,
        ))
        .expect("JSON records serialize");
        bytes.push(b'\n')
    }
    String::from_utf8(bytes).expect("JSON is UTF-8")
}
pub fn folders(
    filters: &Filters,
    projects: &Path,
    home: &Path,
    deny: &DenyList,
) -> Result<Vec<FolderRow>> {
    deny.check(projects)?;
    let mut rows = vec![];
    let root = open_projects(projects)?;
    for entry in project_folders(projects, &root)? {
        let p = &entry.path;
        if deny.denied(p) {
            continue;
        }
        let folder = name(p)?;
        let opened = open_project(&root, &entry)?;
        rows.push(FolderRow {
            excluded: filters.excluded(&folder),
            sessions: session_files(p, &opened, deny)?.0.len(),
            project: project_name(&folder, home),
            folder,
        })
    }
    rows.sort_by(|a, b| (&a.project, &a.folder).cmp(&(&b.project, &b.folder)));
    Ok(rows)
}
pub fn render_folders(rows: &[FolderRow]) -> String {
    let mut out = String::new();
    let mut totals = [[0, 0], [0, 0]];
    for r in rows {
        let state = if r.excluded { "excluded" } else { "mined" };
        out.push_str(&format!(
            "{state}\t{}\t{}\t{}\n",
            r.sessions, r.project, r.folder
        ));
        let t = &mut totals[usize::from(r.excluded)];
        t[0] += 1;
        t[1] += r.sessions
    }
    out.push_str(&format!(
        "{} mined folders ({} sessions), {} excluded ({} sessions)\n",
        totals[0][0], totals[0][1], totals[1][0], totals[1][1]
    ));
    out
}

#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod test_common;

#[cfg(test)]
mod tests {
    use super::test_common::{self as common, repo};
    use super::*;
    use std::{
        fs::{self, OpenOptions},
        io::Read,
        os::unix::fs::{OpenOptionsExt, symlink},
    };

    #[test]
    fn a_project_folder_swapped_for_a_link_after_listing_is_refused() {
        let scratch = common::Scratch::new("project-swap");
        let projects = scratch.0.join("projects");
        let project = projects.join("-p-app");
        let elsewhere = scratch.0.join("elsewhere");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(&elsewhere).unwrap();
        fs::write(project.join("s.jsonl"), "first").unwrap();
        fs::write(elsewhere.join("s.jsonl"), "second").unwrap();
        let root = open_projects(&projects).unwrap();
        let entries = project_folders(&projects, &root).unwrap();
        fs::rename(&project, projects.join("moved")).unwrap();
        symlink(&elsewhere, &project).unwrap();
        let error = open_project(&root, &entries[0])
            .err()
            .expect("refuse swapped folder");
        assert_eq!(error.to_string(), link(&project).to_string());
    }

    #[test]
    fn a_session_folder_swapped_after_listing_reads_the_listed_folder() {
        let scratch = common::Scratch::new("session-folder-swap");
        let projects = scratch.0.join("projects");
        let project = projects.join("-p-app");
        let elsewhere = scratch.0.join("elsewhere");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(&elsewhere).unwrap();
        fs::write(project.join("s.jsonl"), "first").unwrap();
        fs::write(elsewhere.join("s.jsonl"), "second").unwrap();
        let root = open_projects(&projects).unwrap();
        let entries = project_folders(&projects, &root).unwrap();
        let folder = open_project(&root, &entries[0]).unwrap();
        let deny = DenyList::builtin();
        let (files, _) = session_files(&project, &folder, &deny).unwrap();
        fs::rename(&project, projects.join("moved")).unwrap();
        symlink(&elsewhere, &project).unwrap();
        let mut text = String::new();
        open_session(&folder, &files[0])
            .unwrap()
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "first");
    }

    #[test]
    fn a_session_file_swapped_for_a_link_after_listing_is_refused() {
        let scratch = common::Scratch::new("session-file-swap");
        let projects = scratch.0.join("projects");
        let project = projects.join("-p-app");
        let elsewhere = scratch.0.join("elsewhere");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir(&elsewhere).unwrap();
        let path = project.join("s.jsonl");
        fs::write(&path, "first").unwrap();
        fs::write(elsewhere.join("s.jsonl"), "second").unwrap();
        let root = open_projects(&projects).unwrap();
        let entries = project_folders(&projects, &root).unwrap();
        let folder = open_project(&root, &entries[0]).unwrap();
        let (files, _) = session_files(&project, &folder, &DenyList::builtin()).unwrap();
        fs::remove_file(&path).unwrap();
        symlink(elsewhere.join("s.jsonl"), &path).unwrap();
        assert_eq!(
            open_session(&folder, &files[0]).unwrap_err().to_string(),
            link(&path).to_string()
        );
    }

    #[test]
    fn a_hard_link_made_after_listing_is_left_out() {
        let scratch = common::Scratch::new("session-hard-link");
        let projects = scratch.0.join("projects");
        let project = projects.join("-p-app");
        fs::create_dir_all(&project).unwrap();
        let path = project.join("s.jsonl");
        fs::write(&path, "first").unwrap();
        let root = open_projects(&projects).unwrap();
        let entries = project_folders(&projects, &root).unwrap();
        let folder = open_project(&root, &entries[0]).unwrap();
        let (files, _) = session_files(&project, &folder, &DenyList::builtin()).unwrap();
        fs::hard_link(&path, scratch.0.join("outside.jsonl")).unwrap();
        assert!(open_session(&folder, &files[0]).unwrap().is_none());
    }

    #[test]
    fn open_session_leaves_out_hard_links() {
        let source = common::Scratch::new("session-source");
        let outside = common::Scratch::new("session-outside");
        let canary = outside.0.join("canary");
        fs::write(&canary, "canary-outside").unwrap();
        let linked = source.0.join("linked.jsonl");
        fs::hard_link(canary, &linked).unwrap();
        let folder = Folder::open(&source.0, false).unwrap();
        let linked = Entry {
            path: linked,
            name: CString::new("linked.jsonl").unwrap(),
        };
        assert!(open_session(&folder, &linked).unwrap().is_none());
        let plain = source.0.join("plain.jsonl");
        fs::write(&plain, "plain").unwrap();
        let plain = Entry {
            path: plain,
            name: CString::new("plain.jsonl").unwrap(),
        };
        assert!(open_session(&folder, &plain).unwrap().is_some());
    }

    #[test]
    fn only_a_shape_error_counts_as_an_unreadable_record() {
        assert!(unreadable(&Error::Invalid(format!(
            "x:1: {UNEXPECTED_SHAPE} (message)"
        ))));
        assert!(!unreadable(&Error::Invalid("x is not a file".into())));
        assert!(!unreadable(&Error::Io {
            path: "x".into(),
            source: std::io::Error::other("boom"),
        }));
        assert!(!unreadable(&Error::Exists("x".into())));
    }

    #[test]
    fn opening_a_fifo_returns_without_waiting_for_a_writer() {
        let _serial = crate::serial_test();
        let scratch = common::Scratch::new("fifo");
        let path = scratch.0.join("fifo");
        let folder = Folder::open(&scratch.0, false).unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        let (send, receive) = std::sync::mpsc::channel();
        let fifo = path.clone();
        let worker = std::thread::spawn(move || {
            let entry = Entry {
                path: fifo,
                name: CString::new("fifo").unwrap(),
            };
            send.send(
                open_session(&folder, &entry)
                    .map(|_| ())
                    .map_err(|e| e.to_string()),
            )
            .unwrap();
        });
        let result = receive.recv_timeout(std::time::Duration::from_secs(1));
        if result.is_err() {
            // Release the old blocking opener before reporting the timeout.
            let _writer = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&path)
                .unwrap();
            worker.join().unwrap();
        } else {
            worker.join().unwrap();
        }
        fs::remove_file(&path).unwrap();
        assert_eq!(
            result
                .expect("opening a FIFO waited for a writer")
                .unwrap_err(),
            format!("{} is not a file", path.display())
        );
    }
    #[test]
    fn every_worktree_suffix_matches_python() {
        let _serial = crate::serial_test();
        if !common::parity_tools_available() {
            return;
        }
        let folder = "-Users-p-projects-app--claude-worktrees-first\nnext--claude-worktrees-second";
        let py = common::checked(std::process::Command::new("python3")
        .env("HOME", "/Users/p").arg("-I").arg("-c")
        .arg("import sys; sys.path.insert(0, sys.argv[1]); import crew_decisions; print(crew_decisions.project_name(sys.argv[2]), end='')")
        .arg(repo().join("scripts")).arg(folder));
        assert_eq!(
            project_name(folder, std::path::Path::new("/Users/p")).as_bytes(),
            py.stdout
        );
    }
}
