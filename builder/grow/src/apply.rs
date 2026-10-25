//! Test an approved change before touching the assistant, then finish it resumably.
pub use crate::apply_reply::*;
use crate::{
    diff,
    record::*,
    refresh, rules,
    store::{Prefix, Store},
    text,
    version::{DECISIONS_MOST, version},
};
use persona_builder::{
    error::{Error, Result},
    member::Member,
};
use persona_builder_agent::{claude::Claude, ledger::Session};
use persona_builder_build::{
    build::{Asker, batches},
    draft,
};
use persona_builder_ingest::filters::Filters;
use persona_builder_steps::text_check::hidden;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug)]
pub enum Measured {
    Done(Measurement),
    Already(Measurement),
}
#[derive(Debug, PartialEq, Eq)]
pub enum Left {
    Decisions,
    Entry,
    Cases,
}
#[derive(Debug)]
pub struct Finished {
    pub outcome: KeepOrRevert,
    pub left: Vec<Left>,
}

fn does_not_fit() -> Error {
    Error::Invalid(text::CHANGE_DOES_NOT_FIT.into())
}

fn candidate(before: &str, proposal: &Proposal) -> Result<String> {
    let mut current = before.to_owned();
    for change in &proposal.changes {
        if change.file != "decisions.md" {
            return Err(does_not_fit());
        }
        current =
            diff::apply(&current, &change.diff, "decisions.md").map_err(|_| does_not_fit())?;
    }
    if current == before || current.len() as u64 > DECISIONS_MOST {
        return Err(does_not_fit());
    }
    Ok(current)
}

fn check_candidate(
    proposal: &Proposal,
    cases: &[Case],
    signals: &[Signal],
    filters: &Filters,
) -> Result<()> {
    let mut words = vec![proposal.summary.clone()];
    for change in &proposal.changes {
        words.push(change.why.clone());
        words.extend(
            change
                .diff
                .lines()
                .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
                .map(|line| line[1..].trim_end_matches('\r').trim())
                .filter(|line| !line.is_empty())
                .map(str::to_owned),
        );
    }
    if words
        .iter()
        .any(|line| line.chars().count() > 512 || line.chars().any(hidden))
    {
        return Err(does_not_fit());
    }
    refresh::check_words(&words, cases, signals, filters).map_err(|_| does_not_fit())
}

#[allow(clippy::too_many_arguments)]
pub fn measure(
    claude: &Claude,
    session: &Session,
    home: &Path,
    store: &Store,
    member: &Member,
    member_dir: &Path,
    filters: &Filters,
    model: &str,
    id: &str,
    now: &dyn Fn() -> String,
) -> Result<Measured> {
    // Session holds the state lock for its whole life. All writes reuse it through
    // unlocked store methods; neither this flow nor finish takes another lock.
    let proposal = store
        .proposal(id)?
        .ok_or_else(|| Error::Invalid(text::no_proposal(id)))?;
    refresh::check_stored(&proposal)?;
    let approval = store
        .approvals()?
        .into_iter()
        .find(|a| a.proposal == id)
        .ok_or_else(|| Error::Invalid(text::not_answered(id)))?;
    if approval.decision == Decision::Rejected {
        return Err(Error::Invalid(text::was_rejected(id)));
    }
    rules::may_apply(&proposal, &approval)?;
    if let Some(measurement) = store.measurements()?.into_iter().find(|m| m.proposal == id) {
        return Ok(Measured::Already(measurement));
    }
    draft::preflight(member_dir)?;
    let before = refresh::decisions(member_dir)?;
    rules::base_is_current(&proposal, &version(before.as_bytes()))?;
    let changed = candidate(&before, &proposal)?;
    let cases = store.cases()?;
    let named: Vec<_> = proposal
        .cases
        .iter()
        .filter_map(|id| cases.get(id).cloned())
        .collect();
    check_candidate(&proposal, &named, &store.signals()?, filters)?;
    draft::check_private(filters, &changed, "")?;
    let run =
        rules::to_measure(&proposal, &cases).map_err(|_| Error::Invalid(text::cases_moved(id)))?;
    let tests: Vec<_> = run
        .iter()
        .map(|(id, _)| cases.get(id).expect("checked test"))
        .collect();
    let mut asker =
        Asker::new(claude, session, home, &member.slug, model).with_misfit(text::apply_misfit);
    let mut answers = Vec::new();
    for decisions in [&before, &changed] {
        let mut found = BTreeMap::new();
        for batch in batches(&tests) {
            let (reply, _) = asker.ask(
                "answer",
                ANSWER_SYSTEM,
                &answer_schema(),
                &answer_prompt(decisions, batch),
                text::TESTING,
                |reply| check_answers(batch, reply),
            )?;
            found.extend(reply);
        }
        answers.push(found);
    }
    let grades = grade_answers(&mut asker, &tests, &answers)?;
    if version(refresh::decisions(member_dir)?.as_bytes()) != proposal.base {
        return Err(Error::Invalid(text::DECISIONS_CHANGED_TESTING.into()));
    }
    let at = now();
    let mut measurement = Measurement {
        schema: MEASUREMENT_V1.into(),
        id: store.next_id(Prefix::Measurement, &at[..10])?,
        proposal: proposal.id.clone(),
        approval: approval.id.clone(),
        at,
        base: proposal.base.clone(),
        candidate: version(changed.as_bytes()),
        results: run
            .iter()
            .map(|(id, new)| CaseResult {
                case: id.clone(),
                new: *new,
                before: grades[0][id],
                after: grades[1][id],
            })
            .collect(),
        outcome: KeepOrRevert::Revert,
        reason: String::new(),
    };
    measurement.outcome = rules::outcome(&measurement, &proposal, &approval, &cases);
    measurement.reason = reason(&measurement);
    rules::check_measurement(&measurement, &proposal, &approval, &cases)?;
    store.save_measurement_unlocked(&measurement)?;
    Ok(Measured::Done(measurement))
}

fn grade_answers(
    asker: &mut Asker<'_>,
    tests: &[&Case],
    answers: &[BTreeMap<String, String>],
) -> Result<Vec<BTreeMap<String, Grade>>> {
    let mut pairs = BTreeMap::new();
    for version in answers {
        for case in tests.iter().filter(|c| c.r#match == Match::Rubric) {
            let answer = &version[&case.id];
            pairs.insert((case.id.clone(), answer.clone()), case.expected.as_str());
        }
    }
    let mut ordered: Vec<_> = pairs.into_iter().collect();
    ordered.sort_by_key(|((id, answer), _)| {
        sha1_smol::Sha1::from(format!("{id}\n{answer}"))
            .digest()
            .to_string()
    });
    let items: Vec<_> = ordered
        .iter()
        .enumerate()
        .map(|(n, ((_, answer), expected))| (format!("item-{}", n + 1), *expected, answer.as_str()))
        .collect();
    let mut graded = BTreeMap::new();
    for batch in batches(&items) {
        let (reply, _) = asker.ask(
            "grade",
            GRADE_SYSTEM,
            &grade_schema(),
            &grade_prompt(batch),
            text::GRADING,
            |reply| check_grades(batch, reply),
        )?;
        graded.extend(reply);
    }
    let grades: BTreeMap<_, _> = ordered
        .iter()
        .zip(&items)
        .map(|((pair, _), (label, _, _))| (pair, graded[label]))
        .collect();
    Ok(answers
        .iter()
        .map(|version| {
            tests
                .iter()
                .map(|case| {
                    let answer = &version[&case.id];
                    let grade = match case.r#match {
                        Match::Choice => {
                            if answer == &case.expected {
                                Grade::Pass
                            } else {
                                Grade::Fail
                            }
                        }
                        Match::Rubric => grades[&(case.id.clone(), answer.clone())],
                    };
                    (case.id.clone(), grade)
                })
                .collect()
        })
        .collect())
}

fn reason(measurement: &Measurement) -> String {
    if measurement.outcome == KeepOrRevert::Keep {
        return text::KEEP_REASON.into();
    }
    let new: Vec<_> = measurement.results.iter().filter(|r| r.new).collect();
    let failed = new.iter().filter(|r| r.after == Grade::Fail).count();
    if failed > 0 {
        return text::new_failed(failed, new.len());
    }
    text::regressed(
        measurement
            .results
            .iter()
            .filter(|r| !r.new && r.before == Grade::Pass && r.after == Grade::Fail)
            .count(),
    )
}

pub fn finish(
    store: &Store,
    member: &Member,
    member_dir: &Path,
    filters: &Filters,
    proposal: &Proposal,
    measurement: &Measurement,
) -> Result<Finished> {
    // The caller's Session still holds the state lock, including during status appends.
    let kept = measurement.outcome == KeepOrRevert::Keep;
    let heading = text::refresh_entry_heading(&measurement.at[..10], &proposal.id);
    let new: Vec<_> = measurement.results.iter().filter(|r| r.new).collect();
    let old: Vec<_> = measurement.results.iter().filter(|r| !r.new).collect();
    let entry = text::refresh_entry(
        &heading,
        &proposal.summary,
        new.iter().filter(|r| r.after == Grade::Pass).count(),
        new.len(),
        old.iter()
            .filter(|r| !(r.before == Grade::Pass && r.after == Grade::Fail))
            .count(),
        old.len(),
        kept,
        &measurement.reason,
    );
    let existing = draft::read_text(
        &member_dir.join(draft::MEASUREMENTS),
        draft::MEASUREMENTS_MOST,
    )?;
    let missing = !existing
        .as_deref()
        .is_some_and(|text| text.lines().any(|line| line == heading));
    let measurements = draft::measurements(existing.as_deref(), member, &entry);
    let mut left = Vec::new();
    if kept {
        let current = refresh::decisions(member_dir)?;
        let current_version = version(current.as_bytes());
        if current_version == measurement.base {
            let changed = candidate(&current, proposal)?;
            if version(changed.as_bytes()) != measurement.candidate {
                return Err(does_not_fit());
            }
            draft::check_private(filters, &changed, &entry)?;
            if missing {
                draft::write(member_dir, &changed, &measurements)?;
            } else {
                draft::write_decisions(member_dir, &changed)?;
            }
            left.push(Left::Decisions);
        } else if current_version != measurement.candidate {
            return Err(Error::Invalid(text::DECISIONS_MOVED_AFTER.into()));
        } else if missing {
            draft::check_private(filters, "", &entry)?;
            draft::write_measurements(member_dir, &measurements)?;
        }
    } else if missing {
        draft::check_private(filters, "", &entry)?;
        draft::write_measurements(member_dir, &measurements)?;
    }
    if missing {
        left.push(Left::Entry);
    }
    if kept {
        let cases = store.cases()?;
        for id in &proposal.cases {
            let mut case = cases.get(id).expect("saved test").clone();
            if case.status != Status::Active {
                case.status = Status::Active;
                store.append_case_unlocked(&case)?;
                if !left.contains(&Left::Cases) {
                    left.push(Left::Cases);
                }
            }
        }
    }
    Ok(Finished {
        outcome: measurement.outcome,
        left,
    })
}

pub fn growth(store: &Store, since: Option<&str>) -> Result<Option<String>> {
    let measurements: Vec<_> = store
        .measurements()?
        .into_iter()
        .filter(|m| since.is_none_or(|since| &m.at[..10] >= since))
        .collect();
    let Some(newest) = measurements
        .iter()
        .max_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)))
    else {
        return Ok(None);
    };
    let kept = measurements
        .iter()
        .filter(|m| m.outcome == KeepOrRevert::Keep)
        .count();
    let passing = newest
        .results
        .iter()
        .filter(|r| {
            if newest.outcome == KeepOrRevert::Keep {
                r.after == Grade::Pass
            } else {
                r.before == Grade::Pass
            }
        })
        .count();
    Ok(Some(text::growth(
        kept,
        measurements.len() - kept,
        passing,
        newest.results.len(),
    )))
}
