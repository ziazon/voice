//! Deterministic removal and refusal of private skill details.
mod detect;
mod rules;
mod terms;
use persona_builder::{
    deny::DenyList,
    error::{Error, Result},
};
use regex::Regex;
pub use rules::Rules;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::LazyLock,
};

const INVISIBLE: &[(u32, u32)] = &[
    (0x00ad, 0x00ad),
    (0x034f, 0x034f),
    (0x061c, 0x061c),
    (0x115f, 0x1160),
    (0x17b4, 0x17b5),
    (0x180b, 0x180f),
    (0x200b, 0x200f),
    (0x202a, 0x202e),
    (0x2060, 0x206f),
    (0x3164, 0x3164),
    (0xfe00, 0xfe0f),
    (0xfeff, 0xfeff),
    (0xffa0, 0xffa0),
    (0x1d173, 0x1d17a),
    (0xe0000, 0xe007f),
    (0xe0100, 0xe01ef),
];
/// Remove invisible characters that can hide a private detail.
pub fn scrub(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !INVISIBLE
                .iter()
                .any(|&(lo, hi)| (lo..=hi).contains(&(*c as u32)))
        })
        .collect()
}

const BAD_MAPPING: &str =
    "the anonymizer's placeholder file can't be read. Move it away and run anonymize again.";
#[derive(Default, Debug)]
pub struct Mapping {
    kinds: BTreeMap<String, Vec<String>>,
}
impl Mapping {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let bad = || Error::Invalid(BAD_MAPPING.into());
        let value: Value = serde_json::from_slice(bytes).map_err(|_| bad())?;
        let object = value.as_object().ok_or_else(bad)?;
        if object.len() != 2 || object.get("version").and_then(Value::as_u64) != Some(1) {
            return Err(bad());
        }
        let mut kinds = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for (kind, values) in object
            .get("kinds")
            .and_then(Value::as_object)
            .ok_or_else(bad)?
        {
            if !rules::valid_kind(kind) {
                return Err(bad());
            }
            let mut terms = Vec::new();
            for value in values.as_array().ok_or_else(bad)? {
                let term = value.as_str().ok_or_else(bad)?;
                if term.is_empty()
                    || term.chars().any(char::is_control)
                    || terms::normalize(term) != term
                    || !seen.insert(term.to_owned())
                {
                    return Err(bad());
                }
                terms.push(term.to_owned());
            }
            kinds.insert(kind.clone(), terms);
        }
        Ok(Self { kinds })
    }
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(&json!({"version":1,"kinds":self.kinds}))
            .expect("mapping serializes")
    }
    pub fn placeholder(&mut self, kind: &str, term: &str) -> String {
        let term = terms::normalize(term);
        for (known_kind, terms) in &self.kinds {
            if let Some(index) = terms.iter().position(|known| *known == term) {
                return format!("<{known_kind}-{}>", index + 1);
            }
        }
        let terms = self.kinds.entry(kind.to_owned()).or_default();
        terms.push(term);
        format!("<{kind}-{}>", terms.len())
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LeftOut {
    PrivateFile,
    Hidden,
    Denied,
    Link,
    NotText,
    TooLarge,
}
#[derive(Debug)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub kind: &'static str,
}
#[derive(Debug)]
pub enum FileResult {
    Kept(String),
    Refused(Vec<Finding>),
    LeftOut(LeftOut),
}
#[derive(Debug, Default)]
pub struct SkillResult {
    pub kept: Vec<(PathBuf, String)>,
    pub refused: Vec<Finding>,
    pub left_out: Vec<(PathBuf, LeftOut)>,
}
static HOMES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/Users/[^/\s"'`<>()\[\]{}]+|/home/[^/\s"'`<>()\[\]{}]+|(?i:[a-z]:\\Users\\[^\\/\s"'`<>()\[\]{}]+)"#).expect("fixed regex")
});

pub(crate) fn exclusion(relative: &Path, size: usize, rules: &Rules) -> Option<LeftOut> {
    let name = relative.file_name().unwrap_or_default().to_string_lossy();
    if ["LOCAL.md", "LEARNINGS.md"]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
        || rules
            .drop_files
            .iter()
            .any(|p| rules::wildcard(p, &name.to_lowercase()))
    {
        Some(LeftOut::PrivateFile)
    } else if relative
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        Some(LeftOut::Hidden)
    } else if DenyList::builtin().denied_relative(relative) {
        Some(LeftOut::Denied)
    } else if size > 1 << 20 {
        Some(LeftOut::TooLarge)
    } else {
        None
    }
}

pub fn file(relative: &Path, bytes: &[u8], rules: &Rules, mapping: &mut Mapping) -> FileResult {
    if let Some(reason) = exclusion(relative, bytes.len(), rules) {
        return FileResult::LeftOut(reason);
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return FileResult::LeftOut(LeftOut::NotText);
    };
    let scrubbed = scrub(text);
    let (text, lines, mut found) = sections(&scrubbed);
    let text = HOMES
        .replace_all(&text, |cap: &regex::Captures<'_>| {
            if &cap[0] == "/Users/Shared" {
                cap[0].to_owned()
            } else {
                "~".into()
            }
        })
        .into_owned();
    let (text, inserted) = terms::replace(&text, rules, mapping);
    let mut masked = text.as_bytes().to_vec();
    for range in inserted {
        masked[range].fill(b' ');
    }
    for allow in rules.allow.iter().map(String::as_str).chain([
        "git@github.com",
        "git@gitlab.com",
        "git@bitbucket.org",
    ]) {
        let re = regex::RegexBuilder::new(&regex::escape(allow))
            .case_insensitive(true)
            .build()
            .expect("literal regex");
        let mut offset = 0;
        while let Some(m) = re.find_at(&text, offset) {
            masked[m.range()].fill(b' ');
            offset = m.start()
                + text[m.start()..]
                    .chars()
                    .next()
                    .expect("nonempty literal")
                    .len_utf8();
        }
    }
    let masked = String::from_utf8(masked).expect("space masking preserves utf8");
    let source_eof_line = scrubbed.bytes().filter(|b| *b == b'\n').count() + 1;
    let source_line = |offset| {
        let index = masked[..offset].bytes().filter(|b| *b == b'\n').count();
        lines.get(index).copied().unwrap_or(source_eof_line)
    };
    for (offset, kind) in detect::find(&masked) {
        found.insert((source_line(offset), kind));
    }
    for pattern in &rules.patterns {
        for m in pattern.find_iter(&masked) {
            found.insert((source_line(m.start()), "your pattern"));
        }
    }
    for term in &rules.terms {
        for (start, _) in terms::matches(&masked, &term.wrapped) {
            found.insert((source_line(start), "your term"));
        }
        if let Some(pattern) = &term.substring {
            for m in pattern.find_iter(&masked) {
                found.insert((source_line(m.start()), "your term"));
            }
        }
    }
    if found.is_empty() {
        FileResult::Kept(text)
    } else {
        FileResult::Refused(
            found
                .into_iter()
                .map(|(line, kind)| Finding {
                    file: relative.to_owned(),
                    line,
                    kind,
                })
                .collect(),
        )
    }
}

fn sections(text: &str) -> (String, Vec<usize>, BTreeSet<(usize, &'static str)>) {
    let mut output = String::new();
    let mut lines = Vec::new();
    let mut found = BTreeSet::new();
    let mut open = None;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let n = index + 1;
        if line.trim().eq_ignore_ascii_case("<!-- private -->") {
            if open.is_some() {
                found.insert((n, "private marker"));
            } else {
                open = Some(n);
            }
        } else if line.trim().eq_ignore_ascii_case("<!-- /private -->") {
            if open.take().is_none() {
                found.insert((n, "private marker"));
            }
        } else if open.is_none() {
            lines.push(n);
            output.push_str(line);
        }
    }
    if let Some(line) = open {
        found.insert((line, "private marker"));
    }
    (output, lines, found)
}
pub fn skill(
    files: &[(PathBuf, crate::skills::Source)],
    rules: &Rules,
    mapping: &mut Mapping,
) -> SkillResult {
    let mut result = SkillResult::default();
    let mut skill_kept = false;
    for (path, source) in files {
        let outcome = match source {
            crate::skills::Source::Bytes(bytes) => file(path, bytes, rules, mapping),
            crate::skills::Source::Link => FileResult::LeftOut(LeftOut::Link),
            crate::skills::Source::TooLarge => FileResult::LeftOut(LeftOut::TooLarge),
        };
        match outcome {
            FileResult::Kept(text) => {
                if path == Path::new("SKILL.md") {
                    skill_kept = true;
                }
                result.kept.push((path.clone(), text));
            }
            FileResult::Refused(found) => result.refused.extend(found),
            FileResult::LeftOut(reason) => result.left_out.push((path.clone(), reason)),
        }
    }
    if !skill_kept {
        result.refused.push(Finding {
            file: PathBuf::from("SKILL.md"),
            line: 1,
            kind: "skill file",
        });
    }
    result
        .refused
        .sort_by(|a, b| (&a.file, a.line, a.kind).cmp(&(&b.file, b.line, b.kind)));
    result
}
pub fn left_out_reason(reason: &LeftOut) -> &'static str {
    match reason {
        LeftOut::PrivateFile => "private file",
        LeftOut::Hidden => "hidden",
        LeftOut::Denied => "password or key file",
        LeftOut::Link => "a link",
        LeftOut::NotText => "not text",
        LeftOut::TooLarge => "too large",
    }
}
pub fn report(name: &str, result: &SkillResult) -> String {
    let mut text = if result.refused.is_empty() {
        format!("{name}: copied, {} files", result.kept.len())
    } else {
        format!("{name}: not copied, {} things to fix", result.refused.len())
    };
    if result.refused.is_empty() && !result.left_out.is_empty() {
        text.push_str(&format!(", {} left out", result.left_out.len()));
    }
    text.push('\n');
    for f in &result.refused {
        text.push_str(&format!(
            "  {name}/{}:{} ({})\n",
            f.file.display(),
            f.line,
            f.kind
        ));
    }
    for (path, reason) in &result.left_out {
        text.push_str(&format!(
            "  {name}/{} (left out: {})\n",
            path.display(),
            left_out_reason(reason)
        ));
    }
    text
}
