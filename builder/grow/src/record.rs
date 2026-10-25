use crate::text;
use persona_builder::error::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::BTreeSet, sync::LazyLock};
pub const TIMESTAMP: &str =
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})$";
pub const PERSONA: &str = r"^[a-z0-9]+(-[a-z0-9]+)*$";
pub const FILE: &str = r"^[A-Za-z0-9_-][A-Za-z0-9._-]*(/[A-Za-z0-9_-][A-Za-z0-9._-]*)*$";
pub const SIGNAL_V1: &str = "growth-loop/signal/v1";
pub const CASE_V1: &str = "growth-loop/case/v1";
pub const PROPOSAL_V1: &str = "growth-loop/proposal/v1";
pub const APPROVAL_V1: &str = "growth-loop/approval/v1";
pub const MEASUREMENT_V1: &str = "growth-loop/measurement/v1";
pub fn id_pattern(prefix: &str) -> String {
    format!("^{prefix}-[a-z0-9]+(-[a-z0-9]+)*$")
}
static TIMESTAMP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(TIMESTAMP).expect("schema pattern"));
static PERSONA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(PERSONA).expect("schema pattern"));
static FILE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(FILE).expect("schema pattern"));
static SIGNAL_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&id_pattern("sig")).expect("id pattern"));
static CASE_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&id_pattern("case")).expect("id pattern"));
static PROPOSAL_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&id_pattern("prop")).expect("id pattern"));
static APPROVAL_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&id_pattern("appr")).expect("id pattern"));
static MEASUREMENT_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&id_pattern("meas")).expect("id pattern"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GivenBy {
    Subject,
    Staff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignalKind {
    Pick,
    Correction,
    Verdict,
    Instruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Agree,
    Disagree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Match {
    Choice,
    Rubric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Active,
    Retired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonaKind {
    Person,
    Product,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grade {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeepOrRevert {
    Keep,
    Revert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub schema: String,
    pub id: String,
    pub persona: String,
    pub at: String,
    pub given_by: GivenBy,
    pub kind: SignalKind,
    pub area: String,
    pub situation: String,
    pub wanted: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub persona_said: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub verdict: Option<Verdict>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub schema: String,
    pub id: String,
    pub persona: String,
    pub created: String,
    pub from_signals: Vec<String>,
    pub area: String,
    pub situation: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub options: Option<Vec<String>>,
    pub expected: String,
    pub r#match: Match,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub schema: String,
    pub id: String,
    pub persona: String,
    pub persona_kind: PersonaKind,
    pub created: String,
    pub base: String,
    pub summary: String,
    pub changes: Vec<Change>,
    pub cases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub schema: String,
    pub id: String,
    pub proposal: String,
    pub at: String,
    pub role: GivenBy,
    pub decision: Decision,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present"
    )]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub schema: String,
    pub id: String,
    pub proposal: String,
    pub approval: String,
    pub at: String,
    pub base: String,
    pub candidate: String,
    pub results: Vec<CaseResult>,
    pub outcome: KeepOrRevert,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub file: String,
    pub diff: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseResult {
    pub case: String,
    pub new: bool,
    pub before: Grade,
    pub after: Grade,
}

pub trait Record: DeserializeOwned {
    const KIND: &'static str;
    fn check(&self) -> Result<()>;
}

pub fn parse<T: Record>(bytes: &[u8]) -> Result<T> {
    let record: T = serde_json::from_slice(bytes)
        .map_err(|error| Error::Invalid(text::shape(T::KIND, error.line(), error.column())))?;
    record.check()?;
    Ok(record)
}

pub fn to_line<T: Serialize>(record: &T) -> String {
    serde_json::to_string(record).expect("growth loop records serialize") + "\n"
}

pub fn to_pretty<T: Serialize>(record: &T) -> String {
    serde_json::to_string_pretty(record).expect("growth loop records serialize") + "\n"
}
impl Record for Signal {
    const KIND: &'static str = "signal";
    fn check(&self) -> Result<()> {
        require(
            self.schema == SIGNAL_V1,
            Self::KIND,
            "schema",
            text::VERSION,
        )?;
        require(SIGNAL_ID.is_match(&self.id), Self::KIND, "id", text::FORMAT)?;
        require(
            PERSONA_RE.is_match(&self.persona),
            Self::KIND,
            "persona",
            text::FORMAT,
        )?;
        require(
            TIMESTAMP_RE.is_match(&self.at),
            Self::KIND,
            "at",
            text::FORMAT,
        )?;
        require(!self.area.is_empty(), Self::KIND, "area", text::NONEMPTY)?;
        require(
            !self.situation.is_empty(),
            Self::KIND,
            "situation",
            text::NONEMPTY,
        )?;
        require(
            !self.wanted.is_empty(),
            Self::KIND,
            "wanted",
            text::NONEMPTY,
        )?;
        require(
            self.persona_said
                .as_ref()
                .is_none_or(|value| !value.is_empty()),
            Self::KIND,
            "persona_said",
            text::NONEMPTY,
        )?;
        require(
            self.source.as_ref().is_none_or(|value| !value.is_empty()),
            Self::KIND,
            "source",
            text::NONEMPTY,
        )?;
        require(
            (self.kind == SignalKind::Verdict) == self.verdict.is_some(),
            Self::KIND,
            "verdict",
            text::VERDICT,
        )?;
        Ok(())
    }
}
impl Record for Case {
    const KIND: &'static str = "case";
    fn check(&self) -> Result<()> {
        require(self.schema == CASE_V1, Self::KIND, "schema", text::VERSION)?;
        require(CASE_ID.is_match(&self.id), Self::KIND, "id", text::FORMAT)?;
        require(
            PERSONA_RE.is_match(&self.persona),
            Self::KIND,
            "persona",
            text::FORMAT,
        )?;
        require(
            TIMESTAMP_RE.is_match(&self.created),
            Self::KIND,
            "created",
            text::FORMAT,
        )?;
        require(
            !self.from_signals.is_empty(),
            Self::KIND,
            "from_signals",
            text::ONE_ENTRY,
        )?;
        require(
            unique(&self.from_signals),
            Self::KIND,
            "from_signals",
            text::UNIQUE,
        )?;
        require(
            self.from_signals
                .iter()
                .all(|value| SIGNAL_ID.is_match(value)),
            Self::KIND,
            "from_signals",
            text::IDS,
        )?;
        require(!self.area.is_empty(), Self::KIND, "area", text::NONEMPTY)?;
        require(
            !self.situation.is_empty(),
            Self::KIND,
            "situation",
            text::NONEMPTY,
        )?;
        require(
            self.options.as_ref().is_none_or(|value| value.len() >= 2),
            Self::KIND,
            "options",
            text::TWO_CHOICES,
        )?;
        require(
            self.options.as_ref().is_none_or(|value| unique(value)),
            Self::KIND,
            "options",
            text::UNIQUE,
        )?;
        require(
            self.options
                .as_ref()
                .is_none_or(|values| values.iter().all(|value| !value.is_empty())),
            Self::KIND,
            "options",
            text::NONEMPTY_CHOICE,
        )?;
        require(
            !self.expected.is_empty(),
            Self::KIND,
            "expected",
            text::NONEMPTY,
        )?;
        require(
            self.r#match != Match::Choice || self.options.is_some(),
            Self::KIND,
            "options",
            text::CHOICE_OPTIONS,
        )?;
        require(
            self.r#match != Match::Choice
                || self
                    .options
                    .as_ref()
                    .is_some_and(|options| options.contains(&self.expected)),
            Self::KIND,
            "expected",
            text::EXPECTED_CHOICE,
        )?;
        Ok(())
    }
}
impl Record for Proposal {
    const KIND: &'static str = "proposal";
    fn check(&self) -> Result<()> {
        require(
            self.schema == PROPOSAL_V1,
            Self::KIND,
            "schema",
            text::VERSION,
        )?;
        require(
            PROPOSAL_ID.is_match(&self.id),
            Self::KIND,
            "id",
            text::FORMAT,
        )?;
        require(
            PERSONA_RE.is_match(&self.persona),
            Self::KIND,
            "persona",
            text::FORMAT,
        )?;
        require(
            TIMESTAMP_RE.is_match(&self.created),
            Self::KIND,
            "created",
            text::FORMAT,
        )?;
        require(!self.base.is_empty(), Self::KIND, "base", text::NONEMPTY)?;
        require(
            !self.summary.is_empty(),
            Self::KIND,
            "summary",
            text::NONEMPTY,
        )?;
        require(
            !self.changes.is_empty(),
            Self::KIND,
            "changes",
            text::ONE_ENTRY,
        )?;
        for entry in &self.changes {
            require(
                FILE_RE.is_match(&entry.file),
                Self::KIND,
                "changes[].file",
                text::FORMAT,
            )?;
            require(
                !entry.diff.is_empty(),
                Self::KIND,
                "changes[].diff",
                text::NONEMPTY,
            )?;
            require(
                !entry.why.is_empty(),
                Self::KIND,
                "changes[].why",
                text::NONEMPTY,
            )?;
        }
        require(!self.cases.is_empty(), Self::KIND, "cases", text::ONE_ENTRY)?;
        require(unique(&self.cases), Self::KIND, "cases", text::UNIQUE)?;
        require(
            self.cases.iter().all(|value| CASE_ID.is_match(value)),
            Self::KIND,
            "cases",
            text::IDS,
        )?;
        Ok(())
    }
}
impl Record for Approval {
    const KIND: &'static str = "approval";
    fn check(&self) -> Result<()> {
        require(
            self.schema == APPROVAL_V1,
            Self::KIND,
            "schema",
            text::VERSION,
        )?;
        require(
            APPROVAL_ID.is_match(&self.id),
            Self::KIND,
            "id",
            text::FORMAT,
        )?;
        require(
            PROPOSAL_ID.is_match(&self.proposal),
            Self::KIND,
            "proposal",
            text::FORMAT,
        )?;
        require(
            TIMESTAMP_RE.is_match(&self.at),
            Self::KIND,
            "at",
            text::FORMAT,
        )?;
        require(
            self.note.as_ref().is_none_or(|value| !value.is_empty()),
            Self::KIND,
            "note",
            text::NONEMPTY,
        )?;
        Ok(())
    }
}
impl Record for Measurement {
    const KIND: &'static str = "measurement";
    fn check(&self) -> Result<()> {
        require(
            self.schema == MEASUREMENT_V1,
            Self::KIND,
            "schema",
            text::VERSION,
        )?;
        require(
            MEASUREMENT_ID.is_match(&self.id),
            Self::KIND,
            "id",
            text::FORMAT,
        )?;
        require(
            PROPOSAL_ID.is_match(&self.proposal),
            Self::KIND,
            "proposal",
            text::FORMAT,
        )?;
        require(
            APPROVAL_ID.is_match(&self.approval),
            Self::KIND,
            "approval",
            text::FORMAT,
        )?;
        require(
            TIMESTAMP_RE.is_match(&self.at),
            Self::KIND,
            "at",
            text::FORMAT,
        )?;
        require(!self.base.is_empty(), Self::KIND, "base", text::NONEMPTY)?;
        require(
            !self.candidate.is_empty(),
            Self::KIND,
            "candidate",
            text::NONEMPTY,
        )?;
        require(
            !self.results.is_empty(),
            Self::KIND,
            "results",
            text::ONE_ENTRY,
        )?;
        for entry in &self.results {
            require(
                CASE_ID.is_match(&entry.case),
                Self::KIND,
                "results[].case",
                text::FORMAT,
            )?;
        }
        require(
            !self.reason.is_empty(),
            Self::KIND,
            "reason",
            text::NONEMPTY,
        )?;
        require(
            self.results
                .iter()
                .map(|entry| &entry.case)
                .collect::<BTreeSet<_>>()
                .len()
                == self.results.len(),
            Self::KIND,
            "results",
            text::CASE_ONCE,
        )?;
        Ok(())
    }
}

fn require(ok: bool, kind: &str, field: &str, rule: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(text::field(kind, field, rule)))
    }
}
fn unique(items: &[String]) -> bool {
    items.iter().collect::<BTreeSet<_>>().len() == items.len()
}

// Missing optional fields default to None; an explicit null is not a schema value.
fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
