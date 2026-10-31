//! Sources of a brief and the limits chosen for their data.
use crate::{
    error::{Error, Result},
    member::Member,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Where a brief's data came from. Ordered as listed; `ALL` is that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    ClaudeHistory,
    CrewFolder,
    Typed,
    SkillMirror,
    Mail,
    Calendar,
    Contacts,
    FamilySkills,
    LegalSkills,
}

impl Source {
    pub const ALL: [Self; 9] = [
        Self::ClaudeHistory,
        Self::CrewFolder,
        Self::Typed,
        Self::SkillMirror,
        Self::Mail,
        Self::Calendar,
        Self::Contacts,
        Self::FamilySkills,
        Self::LegalSkills,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::ClaudeHistory => "claude-history",
            Self::CrewFolder => "crew-folder",
            Self::Typed => "typed",
            Self::SkillMirror => "skill-mirror",
            Self::Mail => "mail",
            Self::Calendar => "calendar",
            Self::Contacts => "contacts",
            Self::FamilySkills => "family-skills",
            Self::LegalSkills => "legal-skills",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeHistory => {
                "your Claude history (your Claude Code sessions and your claude.ai export)"
            }
            Self::CrewFolder => "your assistant's own folder",
            Self::Typed => {
                "what you type into Persona Builder (interview answers, notes on answers, a panel's question)"
            }
            Self::SkillMirror => "the copy of your skills",
            Self::Mail => "your mail",
            Self::Calendar => "your calendar",
            Self::Contacts => "your contacts",
            Self::FamilySkills => "your family skills",
            Self::LegalSkills => "your legal skills",
        }
    }
    pub fn default_ceiling(self) -> Ceiling {
        match self {
            Self::ClaudeHistory | Self::CrewFolder | Self::Typed | Self::SkillMirror => {
                Ceiling::Anthropic
            }
            _ => Ceiling::LocalOnly,
        }
    }
    pub fn read_today(self) -> bool {
        matches!(self, Self::ClaudeHistory | Self::CrewFolder | Self::Typed)
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|source| source.id() == id)
    }
}

/// How far a source's data may go. Ordered from the narrowest to the widest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ceiling {
    LocalOnly,
    Anthropic,
    AnyConsented,
}
impl Ceiling {
    pub fn id(self) -> &'static str {
        match self {
            Self::LocalOnly => "local-only",
            Self::Anthropic => "anthropic",
            Self::AnyConsented => "any-consented",
        }
    }
    pub fn words(self) -> &'static str {
        match self {
            Self::LocalOnly => "on your own machines",
            Self::Anthropic => "on your own machines or with Anthropic",
            Self::AnyConsented => "with any provider you've agreed to",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "local-only" => Some(Self::LocalOnly),
            "anthropic" => Some(Self::Anthropic),
            "any-consented" => Some(Self::AnyConsented),
            _ => None,
        }
    }
}

/// Every source's ceiling: the defaults, with the person's own settings from member.json over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ceilings {
    values: BTreeMap<Source, Ceiling>,
    settings: BTreeSet<Source>,
}
impl Ceilings {
    pub fn defaults() -> Self {
        Self {
            values: Source::ALL
                .into_iter()
                .map(|source| (source, source.default_ceiling()))
                .collect(),
            settings: BTreeSet::new(),
        }
    }
    pub fn get(&self, source: Source) -> Ceiling {
        self.values[&source]
    }
    pub fn is_set(&self, source: Source) -> bool {
        self.settings.contains(&source)
    }
    pub fn for_member(member: &Member) -> Result<Self> {
        let mut ceilings = Self::defaults();
        if let Some(value) = member.extra.get("ceilings") {
            ceilings.apply(value, "member.json")?;
        }
        Ok(ceilings)
    }
    fn apply(&mut self, value: &Value, source: &str) -> Result<()> {
        let object = value.as_object().ok_or_else(|| {
            Error::Invalid(format!(
                "member config: {source}: ceilings must be a JSON object"
            ))
        })?;
        for (key, value) in object {
            let entry = Source::from_id(key).ok_or_else(|| Error::Invalid(format!("member config: {source}: ceilings names {key:?}, which isn't a source. The sources are claude-history, crew-folder, typed, skill-mirror, mail, calendar, contacts, family-skills, legal-skills.")))?;
            let ceiling = value.as_str().and_then(Ceiling::from_id).ok_or_else(|| Error::Invalid(format!("member config: {source}: ceilings.{key} must be local-only, anthropic or any-consented")))?;
            self.values.insert(entry, ceiling);
            self.settings.insert(entry);
        }
        Ok(())
    }
}

pub(crate) fn validate(value: &Value, source: &str) -> Result<()> {
    Ceilings::defaults().apply(value, source)
}

#[cfg(test)]
mod tests {
    use crate::ceilings::{Ceiling, Ceilings, Source};
    use crate::member::Member;
    use serde_json::json;

    #[test]
    fn sources_match_every_row_and_round_trip_only_exact_ids() {
        use Ceiling::{Anthropic, LocalOnly};
        use Source::*;
        let rows = [
            (
                ClaudeHistory,
                "claude-history",
                "your Claude history (your Claude Code sessions and your claude.ai export)",
                Anthropic,
                true,
            ),
            (
                CrewFolder,
                "crew-folder",
                "your assistant's own folder",
                Anthropic,
                true,
            ),
            (
                Typed,
                "typed",
                "what you type into Persona Builder (interview answers, notes on answers, a panel's question)",
                Anthropic,
                true,
            ),
            (
                SkillMirror,
                "skill-mirror",
                "the copy of your skills",
                Anthropic,
                false,
            ),
            (Mail, "mail", "your mail", LocalOnly, false),
            (Calendar, "calendar", "your calendar", LocalOnly, false),
            (Contacts, "contacts", "your contacts", LocalOnly, false),
            (
                FamilySkills,
                "family-skills",
                "your family skills",
                LocalOnly,
                false,
            ),
            (
                LegalSkills,
                "legal-skills",
                "your legal skills",
                LocalOnly,
                false,
            ),
        ];
        assert_eq!(Source::ALL, rows.map(|row| row.0));
        for (source, id, name, default, read_today) in rows {
            assert_eq!(source.id(), id);
            assert_eq!(source.name(), name);
            assert_eq!(source.default_ceiling(), default);
            assert_eq!(source.read_today(), read_today);
            assert_eq!(Source::from_id(id), Some(source));
        }
        for id in ["Mail", "mail ", ""] {
            assert_eq!(Source::from_id(id), None);
        }
    }

    #[test]
    fn ceilings_match_ids_words_and_narrowest_first_order() {
        use Ceiling::*;
        for (ceiling, id, words) in [
            (LocalOnly, "local-only", "on your own machines"),
            (
                Anthropic,
                "anthropic",
                "on your own machines or with Anthropic",
            ),
            (
                AnyConsented,
                "any-consented",
                "with any provider you've agreed to",
            ),
        ] {
            assert_eq!(ceiling.id(), id);
            assert_eq!(ceiling.words(), words);
            assert_eq!(Ceiling::from_id(id), Some(ceiling));
        }
        for id in ["Mail", "mail ", "", "Anthropic", "anthropic "] {
            assert_eq!(Ceiling::from_id(id), None);
        }
        assert!(LocalOnly < Anthropic);
        assert!(Anthropic < AnyConsented);
    }

    fn member() -> Member {
        serde_json::from_str(include_str!("../../../crew/_template/member.json")).unwrap()
    }

    #[test]
    fn an_unset_member_uses_all_defaults_without_personal_settings() {
        let ceilings = Ceilings::for_member(&member()).unwrap();
        assert_eq!(ceilings, Ceilings::defaults());
        for source in Source::ALL {
            assert_eq!(ceilings.get(source), source.default_ceiling());
            assert!(!ceilings.is_set(source));
        }
    }

    #[test]
    fn a_mail_setting_changes_only_mail_and_marks_it_set() {
        let mut member = member();
        member
            .extra
            .insert("ceilings".into(), json!({"mail": "anthropic"}));
        let ceilings = Ceilings::for_member(&member).unwrap();
        for source in Source::ALL {
            assert_eq!(ceilings.is_set(source), source == Source::Mail);
            assert_eq!(
                ceilings.get(source),
                if source == Source::Mail {
                    Ceiling::Anthropic
                } else {
                    source.default_ceiling()
                }
            );
        }
    }
}
