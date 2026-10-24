use persona_builder::error::{Error, Result};
use regex::{Regex, RegexBuilder};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Personal,
    Secret,
}
impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
            Self::Secret => "secret",
        }
    }
}

pub struct Filters {
    excluded: Regex,
    personal: Regex,
    secret: Regex,
    pasted: Vec<String>,
}
impl Filters {
    pub fn compile(filters: &persona_builder::member::Filters) -> Result<Self> {
        fn compile(key: &str, pattern: &str) -> Result<Regex> {
            RegexBuilder::new(pattern).case_insensitive(true).build().map_err(|_| {
                Error::Invalid(format!("member config: filters.{key} is not a valid regex, or it uses look-around or a backreference, which Persona Builder can't read"))
            })
        }
        Ok(Self {
            excluded: compile("excluded_folders", &filters.excluded_folders)?,
            personal: compile("personal", &filters.personal)?,
            secret: compile("secret", &filters.secret)?,
            pasted: filters.pasted_prefixes.clone(),
        })
    }
    pub fn excluded(&self, folder_name: &str) -> bool {
        self.excluded.is_match(folder_name)
    }
    pub fn pasted_prefixes(&self) -> &[String] {
        &self.pasted
    }
    pub fn kind(&self, text: &str) -> Option<Kind> {
        if self.secret.is_match(text) {
            Some(Kind::Secret)
        } else if self.personal.is_match(text) {
            Some(Kind::Personal)
        } else {
            None
        }
    }
    pub fn lines(&self, text: &str) -> Vec<(usize, Kind)> {
        let mut found = BTreeMap::new();
        for (kind, pattern) in [
            (Kind::Personal, &self.personal),
            (Kind::Secret, &self.secret),
        ] {
            let mut offset = 0;
            let mut line = 1;
            for hit in pattern.find_iter(text) {
                line += text.as_bytes()[offset..hit.start()]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count();
                offset = hit.start();
                found.insert(line, kind);
            }
        }
        found.into_iter().collect()
    }
}
