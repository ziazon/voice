use super::terms::{self, Term};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use regex::{Regex, RegexBuilder};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct Rules {
    pub(super) terms: Vec<Term>,
    pub(super) patterns: Vec<Regex>,
    pub(super) allow: Vec<String>,
    never_copy: Vec<String>,
    pub(super) drop_files: Vec<String>,
    projects: Vec<PathBuf>,
}
fn bad(suffix: impl AsRef<str>) -> Error {
    Error::Invalid(format!(
        "member config: private.json: anonymizer{}",
        suffix.as_ref()
    ))
}
pub(super) fn valid_literal(s: &str, min: usize) -> bool {
    (min..=200).contains(&s.chars().count()) && !s.chars().any(char::is_control)
}
pub(super) fn valid_kind(s: &str) -> bool {
    let bytes = s.as_bytes();
    (1..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|b| b.is_ascii_lowercase() || *b == b'-')
}
impl Rules {
    pub fn builtin() -> Self {
        Self {
            terms: vec![],
            patterns: vec![],
            allow: vec![],
            never_copy: vec![],
            drop_files: vec![],
            projects: vec![],
        }
    }
    pub fn for_member(member: &Member, home: &Path) -> Result<Self> {
        Self::from_value(member.extra.get("anonymizer"), home)
    }
    pub fn from_value(value: Option<&Value>, home: &Path) -> Result<Self> {
        let mut rules = Self::builtin();
        let Some(value) = value else { return Ok(rules) };
        let object = value.as_object().ok_or_else(|| bad(" must be an object"))?;
        let mut normalized = BTreeMap::new();
        for (key, value) in object {
            let limit = match key.as_str() {
                "terms" => 2000,
                "patterns" | "allow" | "never_copy" => 500,
                "drop_files" | "projects" => 100,
                _ => return Err(bad(format!(".{key} is not a known key"))),
            };
            let array = value
                .as_array()
                .ok_or_else(|| bad(format!(".{key} must be a list")))?;
            if array.len() > limit {
                return Err(bad(format!(".{key} must have at most {limit} entries")));
            }
            for (i, value) in array.iter().enumerate() {
                let at = format!(".{key}[{i}]");
                if key == "terms" {
                    let (term, kind) = if let Some(term) = value.as_str() {
                        (term, "term")
                    } else {
                        let obj = value.as_object().ok_or_else(|| {
                            bad(format!(
                                "{at} must be a string or an object with term and kind"
                            ))
                        })?;
                        if obj.keys().any(|k| k != "term" && k != "kind") {
                            return Err(bad(format!("{at} must contain only term and kind")));
                        }
                        (
                            obj.get("term")
                                .and_then(Value::as_str)
                                .ok_or_else(|| bad(format!("{at}.term must be a string")))?,
                            obj.get("kind").and_then(Value::as_str).ok_or_else(|| {
                                bad(format!("{at}.kind must match ^[a-z][a-z-]{{0,31}}$"))
                            })?,
                        )
                    };
                    let term = term.trim();
                    if !valid_literal(term, 2) {
                        return Err(bad(format!("{at} must be 2 to 200 characters on one line")));
                    }
                    if !valid_kind(kind) {
                        return Err(bad(format!("{at}.kind must match ^[a-z][a-z-]{{0,31}}$")));
                    }
                    let norm = terms::normalize(term);
                    if let Some(previous) = normalized.insert(norm, i) {
                        return Err(bad(format!("{at} repeats anonymizer.terms[{previous}]")));
                    }
                    rules.terms.push(Term::new(term, kind));
                    continue;
                }
                let s = value
                    .as_str()
                    .ok_or_else(|| bad(format!("{at} must be a string")))?;
                match key.as_str() {
                    "patterns" => {
                        let error = || {
                            bad(format!(
                                "{at} is not a valid regex, or it uses look-around or a backreference, which Persona Builder can't read"
                            ))
                        };
                        if s.len() > 1000 {
                            return Err(bad(format!("{at} must be at most 1000 bytes")));
                        }
                        rules.patterns.push(
                            RegexBuilder::new(s)
                                .case_insensitive(true)
                                .size_limit(1 << 20)
                                .build()
                                .map_err(|_| error())?,
                        );
                    }
                    "allow" => {
                        if !valid_literal(s, 3) {
                            return Err(bad(format!(
                                "{at} must be 3 to 200 characters on one line"
                            )));
                        }
                        rules.allow.push(s.to_owned());
                    }
                    "never_copy" => {
                        if s.is_empty()
                            || s.len() > 100
                            || !s
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"._*-".contains(&b))
                        {
                            return Err(bad(format!("{at} must match ^[A-Za-z0-9._*-]{{1,100}}$")));
                        }
                        rules.never_copy.push(s.to_lowercase());
                    }
                    "drop_files" => {
                        if s.is_empty()
                            || s.len() > 255
                            || s.contains('/')
                            || s.chars().any(char::is_control)
                        {
                            return Err(bad(format!(
                                "{at} must be a file name of 1 to 255 bytes with no slash or control character"
                            )));
                        }
                        rules.drop_files.push(s.to_lowercase());
                    }
                    "projects" => {
                        if s.len() > 1024
                            || (!s.starts_with("~/") && !s.starts_with('/'))
                            || s.split('/').any(|part| part == "." || part == "..")
                            || s.chars().any(char::is_control)
                        {
                            return Err(bad(format!(
                                "{at} must start with ~/ or /, have no . or .. component, and be at most 1024 bytes"
                            )));
                        }
                        rules
                            .projects
                            .push(if let Some(rest) = s.strip_prefix("~/") {
                                home.join(rest)
                            } else {
                                PathBuf::from(s)
                            });
                    }
                    _ => unreachable!(),
                }
            }
        }
        Ok(rules)
    }
    pub fn never_copy(&self, name: &str) -> bool {
        self.never_copy
            .iter()
            .any(|p| wildcard(p, &name.to_lowercase()))
    }
    pub fn projects(&self) -> &[PathBuf] {
        &self.projects
    }
}
pub(super) fn wildcard(pattern: &str, value: &str) -> bool {
    let (p, v) = (pattern.as_bytes(), value.as_bytes());
    let (mut pi, mut vi, mut star) = (0, 0, None);
    while vi < v.len() {
        if pi < p.len() && p[pi] == b'*' {
            star = Some((pi, vi));
            pi += 1;
        } else if pi < p.len() && p[pi] == v[vi] {
            pi += 1;
            vi += 1;
        } else if let Some((s, n)) = star {
            star = Some((s, n + 1));
            pi = s + 1;
            vi = n + 1;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}
