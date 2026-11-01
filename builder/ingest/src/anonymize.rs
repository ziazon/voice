//! Deterministic removal and refusal of private skill details.
mod detect;
mod rules;
mod scan;
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
    OddName,
    Unreadable,
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
    Regex::new(r#"(?i)/Users/[^/\s"'`<>()\[\]{}]+|/home/[^/\s"'`<>()\[\]{}]+|(?i:[a-z]:\\Users\\[^\\/\s"'`<>()\[\]{}]+)"#).expect("fixed regex")
});

pub(crate) fn exclusion(relative: &Path, size: usize, rules: &Rules) -> Option<LeftOut> {
    if relative.to_str().is_none() {
        return Some(LeftOut::OddName);
    }
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
    let original = text;
    let scrubbed = scrub(text);
    let (text, lines, mut found) = sections(&scrubbed);
    // U+034F is scrubbed as invisible; detect combining marks on retained source lines too.
    if rules
        .terms
        .iter()
        .any(|t| t.text.chars().any(|c| c.is_alphabetic() && !c.is_ascii()))
    {
        for (index, line) in original.split_inclusive('\n').enumerate() {
            if lines.binary_search(&(index + 1)).is_ok()
                && line.chars().any(|c| ('\u{0300}'..='\u{036f}').contains(&c))
            {
                found.insert((index + 1, "accent mark"));
            }
        }
    }
    let text = HOMES
        .replace_all(&text, |cap: &regex::Captures<'_>| {
            if cap[0].eq_ignore_ascii_case("/Users/Shared") {
                cap[0].to_owned()
            } else {
                "~".into()
            }
        })
        .into_owned();
    let source_eof_line = scrubbed.bytes().filter(|b| *b == b'\n').count() + 1;
    scan::scan(
        &scan::mask(&text, rules, &[]),
        rules,
        &lines,
        source_eof_line,
        false,
        &mut found,
    );
    let (text, inserted) = terms::replace(&text, rules, mapping);
    scan::scan(
        &scan::mask(&text, rules, &inserted),
        rules,
        &lines,
        source_eof_line,
        true,
        &mut found,
    );
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
    let mut names = BTreeSet::new();
    for (path, source) in files {
        if path.to_str().is_some()
            && path
                .components()
                .any(|c| name_holds_private_detail(&c.as_os_str().to_string_lossy(), rules))
        {
            result.refused.push(Finding {
                file: report_path(path, rules),
                line: 1,
                kind: "file name",
            });
        }
        let outcome = if path.to_str().is_none() {
            FileResult::LeftOut(LeftOut::OddName)
        } else {
            match source {
                crate::skills::Source::Bytes(bytes) => file(path, bytes, rules, mapping),
                crate::skills::Source::Link => FileResult::LeftOut(LeftOut::Link),
                crate::skills::Source::TooLarge => FileResult::LeftOut(LeftOut::TooLarge),
                crate::skills::Source::Unreadable => FileResult::LeftOut(LeftOut::Unreadable),
            }
        };
        match outcome {
            FileResult::Kept(text) => {
                if path == Path::new("SKILL.md") {
                    skill_kept = true;
                }
                if !names.insert(path.to_string_lossy().to_lowercase()) {
                    result.refused.push(Finding {
                        file: report_path(path, rules),
                        line: 1,
                        kind: "same name in another case",
                    });
                }
                result.kept.push((path.clone(), text));
            }
            FileResult::Refused(mut found) => {
                for finding in &mut found {
                    finding.file = report_path(&finding.file, rules);
                }
                result.refused.extend(found);
            }
            FileResult::LeftOut(reason) => result.left_out.push((report_path(path, rules), reason)),
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
        LeftOut::OddName => "odd name",
        LeftOut::Unreadable => "unreadable",
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
        if f.kind == "file name" {
            text.push_str(&format!("  {name}/{} (file name)\n", f.file.display()));
            continue;
        }
        text.push_str(&format!(
            "  {name}/{}:{} ({})\n",
            f.file.display(),
            f.line,
            f.kind
        ));
        if f.kind == "accent mark" {
            text.push_str("    Rewrite this line with single accented letters.\n");
        }
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

/// Names cannot be transformed: any private detail refuses the skill.
pub fn name_holds_private_detail(name: &str, rules: &Rules) -> bool {
    scan::private_name(name, rules)
}
pub fn report_path(path: &Path, rules: &Rules) -> PathBuf {
    path.components()
        .map(|c| match c.as_os_str().to_str() {
            Some(text) if !name_holds_private_detail(text, rules) => text,
            _ => "*",
        })
        .collect()
}

/// Skill IDs can also hold a private project or plugin name.
pub fn report_id(id: &str, rules: &Rules) -> String {
    id.split(':')
        .map(|part| {
            if name_holds_private_detail(part, rules) {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(":")
}
