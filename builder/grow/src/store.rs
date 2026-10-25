//! Private, append-only growth records, shared by the command line and refresh flow.
use crate::{
    record::*,
    rules::{self, CaseBook},
    text,
};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_ingest::private::{
    StateLock, lock_state, read_private_optional_at, write_private_at,
};
use regex::Regex;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};
static PERSONA_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(PERSONA).expect("persona pattern"));

pub struct Store {
    home: PathBuf,
    slug: String,
}
impl Store {
    pub fn open(home: &Path, member: &Member) -> Result<Self> {
        if !PERSONA_PATTERN.is_match(&member.slug) {
            return Err(Error::Invalid(text::SLUG_SHAPE.into()));
        }
        Ok(Self {
            home: home.into(),
            slug: member.slug.clone(),
        })
    }
    pub fn slug(&self) -> &str {
        &self.slug
    }
    pub(crate) fn lock(&self) -> Result<StateLock> {
        lock_state(&self.home, &self.slug)
    }
    fn bytes(&self, name: &str) -> Result<Option<Vec<u8>>> {
        read_private_optional_at(&self.home, &self.slug, Some("growth"), name)
    }
    fn lines<T: Record>(&self, file: &str) -> Result<Vec<T>> {
        let bytes = self.bytes(file)?.unwrap_or_default();
        parse_lines(&bytes, file)
    }
    pub fn signals(&self) -> Result<Vec<Signal>> {
        self.lines("signals.jsonl")
    }
    pub fn case_lines(&self) -> Result<Vec<Case>> {
        self.lines("cases.jsonl")
    }
    pub fn cases(&self) -> Result<CaseBook> {
        CaseBook::new(&self.case_lines()?)
    }
    pub fn append_signal(&self, signal: &Signal) -> Result<()> {
        let _lock = self.lock()?;
        self.append_signal_unlocked(signal)
    }
    pub fn append_case(&self, case: &Case) -> Result<()> {
        let _lock = self.lock()?;
        self.append_case_unlocked(case)
    }
    pub(crate) fn append_signal_unlocked(&self, signal: &Signal) -> Result<()> {
        signal.check()?;
        self.append("signals.jsonl", signal, |lines: &[Signal]| {
            if lines.iter().any(|old| old.id == signal.id) {
                return Err(Error::Invalid(text::already_kept(&signal.id)));
            }
            Ok(())
        })
    }
    pub(crate) fn append_case_unlocked(&self, case: &Case) -> Result<()> {
        case.check()?;
        self.append("cases.jsonl", case, |lines: &[Case]| {
            let mut all = lines.to_vec();
            all.push(case.clone());
            CaseBook::new(&all).map(|_| ())
        })
    }
    fn append<T: Record + Serialize>(
        &self,
        file: &str,
        record: &T,
        check: impl FnOnce(&[T]) -> Result<()>,
    ) -> Result<()> {
        let mut old = self.bytes(file)?.unwrap_or_default();
        let lines: Vec<T> = parse_lines(&old, file)?;
        check(&lines)?;
        if !old.is_empty() && !old.ends_with(b"\n") {
            old.push(b'\n');
        }
        old.extend_from_slice(to_line(record).as_bytes());
        write_private_at(&self.home, &self.slug, Some("growth"), file, &old).map(|_| ())
    }
    pub fn next_id(&self, prefix: &str, day: &str) -> Result<String> {
        let ids: Vec<String> = match prefix {
            "sig" => self.signals()?.into_iter().map(|r| r.id).collect(),
            "case" => self.case_lines()?.into_iter().map(|r| r.id).collect(),
            "prop" => self.proposals()?.into_iter().map(|r| r.id).collect(),
            "appr" => self.approvals()?.into_iter().map(|r| r.id).collect(),
            "meas" => self.measurements()?.into_iter().map(|r| r.id).collect(),
            _ => return Err(Error::Invalid(text::SLUG_SHAPE.into())),
        };
        let start = format!("{prefix}-{day}-");
        let highest = ids
            .iter()
            .filter_map(|id| id.strip_prefix(&start))
            .filter_map(|tail| tail.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        let next = highest
            .checked_add(1)
            .ok_or_else(|| Error::Invalid(text::SLUG_SHAPE.into()))?;
        Ok(format!("{start}{next:03}"))
    }
    fn json<T: JsonRecord>(&self, id: &str, prefix: &str) -> Result<Option<T>> {
        if !Regex::new(&id_pattern(prefix))
            .expect("id pattern")
            .is_match(id)
        {
            return Err(Error::Invalid(text::field(T::KIND, "id", text::FORMAT)));
        }
        self.bytes(&format!("{id}.json"))?
            .map(|bytes| parse_json(&bytes, id))
            .transpose()
    }
    pub fn proposal(&self, id: &str) -> Result<Option<Proposal>> {
        self.json(id, "prop")
    }
    pub fn approval(&self, id: &str) -> Result<Option<Approval>> {
        self.json(id, "appr")
    }
    pub fn measurement(&self, id: &str) -> Result<Option<Measurement>> {
        self.json(id, "meas")
    }
    fn json_files<T: JsonRecord>(&self, prefix: &str) -> Result<Vec<T>> {
        // Validate the descriptor-relative folder before enumerating names; each selected file is
        // independently opened through the private reader, which refuses links even if it moves.
        self.bytes("signals.jsonl")?;
        let folder = self
            .home
            .join(".claude/state")
            .join(&self.slug)
            .join("growth");
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(source) => {
                return Err(Error::Io {
                    path: folder,
                    source,
                });
            }
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                path: folder.clone(),
                source,
            })?;
            if let Some(name) = entry.file_name().to_str()
                && name.starts_with(&format!("{prefix}-"))
                && name.ends_with(".json")
            {
                names.push(name.to_owned());
            }
        }
        names.sort();
        names
            .into_iter()
            .map(|name| {
                self.bytes(&name)?
                    .ok_or_else(|| Error::Invalid(text::damaged(&name, 1)))
                    .and_then(|bytes| parse_json(&bytes, name.trim_end_matches(".json")))
            })
            .collect()
    }
    pub fn proposals(&self) -> Result<Vec<Proposal>> {
        self.json_files("prop")
    }
    pub fn approvals(&self) -> Result<Vec<Approval>> {
        self.json_files("appr")
    }
    pub fn measurements(&self) -> Result<Vec<Measurement>> {
        self.json_files("meas")
    }
    fn save<T: Record + Serialize>(&self, record: &T, id: &str) -> Result<()> {
        let _lock = self.lock()?;
        record.check()?;
        let name = format!("{id}.json");
        if self.bytes(&name)?.is_some() {
            return Err(Error::Invalid(text::already_kept(id)));
        }
        write_private_at(
            &self.home,
            &self.slug,
            Some("growth"),
            &name,
            to_pretty(record).as_bytes(),
        )
        .map(|_| ())
    }
    pub fn save_proposal(&self, p: &Proposal) -> Result<()> {
        self.save(p, &p.id)
    }
    pub fn save_approval(&self, a: &Approval) -> Result<()> {
        self.save(a, &a.id)
    }
    pub fn save_measurement(&self, m: &Measurement) -> Result<()> {
        self.save(m, &m.id)
    }
    /// Validate facts available in the persisted records. Historical active-case
    /// completeness, the exact reason for Revert, and the current member base need
    /// caller-supplied context through `rules::check_measurement` and
    /// `rules::base_is_current`; status append lines do not record when they changed.
    pub fn check_all(&self) -> Result<()> {
        let signals = self.signals()?;
        let cases = self.cases()?;
        let proposals = self.proposals()?;
        let approvals = self.approvals()?;
        let measurements = self.measurements()?;
        rules::check_references(
            &self.slug,
            &signals,
            &cases,
            &proposals,
            &approvals,
            &measurements,
        )?;
        let mut signal_ids = std::collections::BTreeSet::new();
        for signal in &signals {
            if !signal_ids.insert(&signal.id) {
                return Err(Error::Invalid(text::already_kept(&signal.id)));
            }
        }
        for approval in &approvals {
            if let Some(proposal) = proposals
                .iter()
                .find(|proposal| proposal.id == approval.proposal)
                && approval.role != rules::approver(proposal.persona_kind)
            {
                return Err(Error::Invalid(text::rule(&approval.id, text::WRONG_ROLE)));
            }
        }
        for measurement in &measurements {
            if let Some(proposal) = proposals
                .iter()
                .find(|proposal| proposal.id == measurement.proposal)
                && measurement.base != proposal.base
            {
                return Err(Error::Invalid(text::rule(
                    &measurement.id,
                    text::WRONG_MEASUREMENT,
                )));
            }
            if let Some(proposal) = proposals
                .iter()
                .find(|proposal| proposal.id == measurement.proposal)
            {
                check_static_measurement(measurement, proposal, &cases)?;
            }
        }
        rules::one_approval(&approvals)?;
        rules::no_measurement_when_rejected(&approvals, &measurements)
    }
}
// Status append lines carry no timestamp. These checks use only facts that remain
// knowable after a later refresh; historic active-case completeness needs its snapshot.
fn check_static_measurement(
    measurement: &Measurement,
    proposal: &Proposal,
    cases: &CaseBook,
) -> Result<()> {
    let invalid = || Error::Invalid(text::rule(&measurement.id, text::WRONG_MEASUREMENT));
    if measurement
        .results
        .iter()
        .any(|result| cases.get(&result.case).is_none())
    {
        return Err(invalid());
    }
    if measurement.outcome == KeepOrRevert::Keep {
        let new_cases: std::collections::BTreeSet<_> = measurement
            .results
            .iter()
            .filter(|result| result.new)
            .map(|result| &result.case)
            .collect();
        let proposed: std::collections::BTreeSet<_> = proposal.cases.iter().collect();
        if new_cases != proposed {
            return Err(invalid());
        }
        if measurement
            .results
            .iter()
            .any(|result| result.new && result.after != Grade::Pass)
        {
            return Err(invalid());
        }
        if measurement.results.iter().any(|result| {
            !result.new && result.before == Grade::Pass && result.after == Grade::Fail
        }) {
            return Err(invalid());
        }
    }
    Ok(())
}
trait JsonRecord: Record {
    fn id(&self) -> &str;
}
impl JsonRecord for Proposal {
    fn id(&self) -> &str {
        &self.id
    }
}
impl JsonRecord for Approval {
    fn id(&self) -> &str {
        &self.id
    }
}
impl JsonRecord for Measurement {
    fn id(&self) -> &str {
        &self.id
    }
}
fn parse_json<T: JsonRecord>(bytes: &[u8], id: &str) -> Result<T> {
    let record: T = parse(bytes)?;
    if record.id() != id {
        return Err(Error::Invalid(text::damaged(&format!("{id}.json"), 1)));
    }
    Ok(record)
}
fn parse_lines<T: Record>(bytes: &[u8], file: &str) -> Result<Vec<T>> {
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    let mut lines = bytes.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    if lines.last() == Some(&b"".as_slice()) {
        lines.pop();
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            parse(line).map_err(|_| Error::Invalid(text::damaged(file, index + 1)))
        })
        .collect()
}
pub fn utc_now() -> String {
    utc_from_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}
fn utc_from_seconds(seconds: u64) -> String {
    // Gregorian civil date from days since the Unix epoch (400-year eras).
    let days = i128::from(seconds / 86400) + 719468;
    let era = days / 146097;
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = march_month + if march_month < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    let clock = seconds % 86400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        clock / 3600,
        clock / 60 % 60,
        clock % 60
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_fixed_instants() {
        for (seconds, expected) in [
            (0, "1970-01-01T00:00:00Z"),
            (951782400, "2000-02-29T00:00:00Z"),
            (1772637600, "2026-03-04T15:20:00Z"),
        ] {
            assert_eq!(utc_from_seconds(seconds), expected);
        }
    }
}
