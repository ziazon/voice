//! Files that must never be read into an assistant's history.
use crate::{
    error::{Error, Result},
    member::Member,
};
use std::path::{Path, PathBuf};

const NAMES: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".env*",
    ".envrc",
    ".npmrc",
    ".netrc",
    ".pgpass",
    "*.pem",
    "*.key",
    "*.p12",
    "*.keychain",
    "*.keychain-db",
    ".credentials.json",
];
const SEQUENCES: &[&[&str]] = &[
    &[".config", "gh"],
    &[".docker", "config.json"],
    &["library", "keychains"],
];
pub const TOO_MANY: &str = "member config: filters.denied_paths holds more than 200 entries.";
pub fn bad_entry(entry: &str) -> String {
    format!(
        "member config: filters.denied_paths: {entry:?} isn't a name or a path Persona Builder can use. Write a name like diary*, or a path starting with ~/ or / below a folder, in plain letters (use * for an accented letter), with no . or .. parts, at most 255 characters."
    )
}
pub fn refusal(shown: &str) -> String {
    format!(
        "{shown} is a kind of file that holds passwords or keys, so Persona Builder never opens it. Choose another file."
    )
}

#[derive(Debug)]
pub struct DenyList {
    home: Option<PathBuf>,
    names: Vec<String>,
    paths: Vec<Vec<String>>,
}
impl DenyList {
    pub fn builtin() -> Self {
        Self {
            home: None,
            names: vec![],
            paths: vec![],
        }
    }
    pub fn new(home: &Path, extra: &[String]) -> Result<Self> {
        if extra.len() > 200 {
            return Err(Error::Invalid(TOO_MANY.into()));
        }
        let mut deny = Self {
            home: Some(home.into()),
            ..Self::builtin()
        };
        for entry in extra {
            if entry.trim().is_empty()
                || !entry.is_ascii()
                || matches!(entry.as_str(), "/" | "~/" | "~")
                || entry.contains("//")
                || entry.len() > 255
                || entry.chars().any(char::is_control)
                || entry.split('/').any(|part| matches!(part, "." | ".."))
                || (entry.contains('/') && !entry.starts_with("~/") && !entry.starts_with('/'))
            {
                return Err(Error::Invalid(bad_entry(entry)));
            }
            if !entry.contains('/') {
                deny.names.push(entry.to_lowercase());
            } else {
                let path = match entry.strip_prefix("~/") {
                    Some(rest) => home.join(rest),
                    None => PathBuf::from(entry),
                };
                deny.paths.push(parts(&path));
                if let Ok(target) = path.canonicalize() {
                    deny.paths.push(parts(&target));
                }
            }
        }
        Ok(deny)
    }
    pub fn for_member(member: &Member, home: &Path) -> Result<Self> {
        Self::new(home, &member.filters.denied_paths)
    }
    pub fn denied(&self, path: &Path) -> bool {
        let absolute = if path.is_absolute() {
            path.to_owned()
        } else {
            match std::env::current_dir() {
                Ok(cwd) => cwd.join(path),
                Err(_) => path.to_owned(),
            }
        };
        self.matches(&parts(&absolute))
            || path.canonicalize().is_ok_and(|p| self.matches(&parts(&p)))
    }
    pub fn denied_relative(&self, relative: &Path) -> bool {
        builtin_matches(&parts(relative))
    }
    pub fn check(&self, path: &Path) -> Result<()> {
        if !self.denied(path) {
            return Ok(());
        }
        let shown = self
            .home
            .as_ref()
            .and_then(|home| path.strip_prefix(home).ok())
            .map(|rest| Path::new("~").join(rest))
            .unwrap_or_else(|| path.to_owned());
        Err(Error::Invalid(refusal(&shown.to_string_lossy())))
    }
    fn matches(&self, parts: &[String]) -> bool {
        builtin_matches(parts)
            || parts
                .iter()
                .any(|part| self.names.iter().any(|pattern| wildcard(pattern, part)))
            || self.paths.iter().any(|prefix| {
                parts.len() >= prefix.len()
                    && prefix
                        .iter()
                        .zip(parts)
                        .all(|(pattern, part)| wildcard(pattern, part))
            })
    }
}
// Unlike Path::components, retain written dots so sequences cannot match across them.
fn parts(path: &Path) -> Vec<String> {
    path.as_os_str()
        .to_string_lossy()
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect()
}
fn builtin_matches(parts: &[String]) -> bool {
    parts
        .iter()
        .any(|part| NAMES.iter().any(|pattern| wildcard(pattern, part)))
        || SEQUENCES.iter().any(|sequence| {
            parts.windows(sequence.len()).any(|window| {
                sequence
                    .iter()
                    .zip(window)
                    .all(|(pattern, part)| wildcard(pattern, part))
            })
        })
}
// Only '*' is special. A byte match is sufficient after Unicode lowercasing because
// every literal chunk must match whole UTF-8 bytes and '*' accepts any intervening run.
fn wildcard(pattern: &str, value: &str) -> bool {
    let (pattern, value) = (pattern.as_bytes(), value.as_bytes());
    let (mut p, mut v, mut star) = (0, 0, None);
    while v < value.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some((p, v));
            p += 1;
        } else if p < pattern.len() && pattern[p] == value[v] {
            p += 1;
            v += 1;
        } else if let Some((s, matched)) = star {
            let next = matched + 1;
            star = Some((s, next));
            p = s + 1;
            v = next;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_needs_no_home_and_reports_full_paths() {
        let deny = DenyList::builtin();
        assert!(deny.home.is_none());
        let path = Path::new("/fictional-home/.aws/export.zip");
        assert_eq!(
            deny.check(path).unwrap_err().to_string(),
            refusal("/fictional-home/.aws/export.zip")
        );
    }
}
