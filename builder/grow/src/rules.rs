use crate::{record::*, text};
use persona_builder::error::{Error, Result};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct CaseBook(BTreeMap<String, Case>);

impl CaseBook {
    pub fn new(lines: &[Case]) -> Result<Self> {
        let mut cases: BTreeMap<String, Case> = BTreeMap::new();
        for case in lines {
            if let Some(previous) = cases.get(&case.id) {
                let mut same_status = case.clone();
                same_status.status = previous.status;
                if &same_status != previous {
                    return Err(Error::Invalid(text::case_changed(&case.id)));
                }
                let allowed = previous.status == case.status
                    || matches!(
                        (previous.status, case.status),
                        (Status::New, Status::Active | Status::Retired)
                            | (Status::Active, Status::Retired)
                    );
                if !allowed {
                    return Err(Error::Invalid(text::status_move(
                        &case.id,
                        status_name(previous.status),
                        status_name(case.status),
                    )));
                }
            }
            cases.insert(case.id.clone(), case.clone());
        }
        Ok(Self(cases))
    }

    pub fn get(&self, id: &str) -> Option<&Case> {
        self.0.get(id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Case> {
        self.0.values()
    }
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::New => "new",
        Status::Active => "active",
        Status::Retired => "retired",
    }
}

pub fn approver(kind: PersonaKind) -> GivenBy {
    match kind {
        PersonaKind::Person => GivenBy::Subject,
        PersonaKind::Product => GivenBy::Staff,
    }
}
pub fn may_apply(proposal: &Proposal, approval: &Approval) -> Result<()> {
    if approval.proposal != proposal.id
        || approval.decision != Decision::Approved
        || approval.role != approver(proposal.persona_kind)
    {
        return Err(Error::Invalid(text::rule(&proposal.id, text::NOT_APPROVED)));
    }
    Ok(())
}
pub fn to_measure(proposal: &Proposal, cases: &CaseBook) -> Result<Vec<(String, bool)>> {
    for id in &proposal.cases {
        if !cases.get(id).is_some_and(|case| case.status == Status::New) {
            return Err(Error::Invalid(text::rule(id, text::NOT_NEW)));
        }
    }
    let mut list: Vec<_> = proposal.cases.iter().map(|id| (id.clone(), true)).collect();
    list.extend(
        cases
            .iter()
            .filter(|case| case.status == Status::Active)
            .map(|case| (case.id.clone(), false)),
    );
    Ok(list)
}
pub fn outcome(
    measurement: &Measurement,
    proposal: &Proposal,
    approval: &Approval,
    cases_at_start: &CaseBook,
) -> KeepOrRevert {
    if may_apply(proposal, approval).is_err() {
        return KeepOrRevert::Revert;
    }
    let Ok(expected) = to_measure(proposal, cases_at_start) else {
        return KeepOrRevert::Revert;
    };
    let expected: BTreeMap<_, _> = expected.into_iter().collect();
    let actual: BTreeMap<_, _> = measurement
        .results
        .iter()
        .map(|result| (result.case.clone(), result.new))
        .collect();
    if actual != expected || measurement.results.len() != expected.len() {
        return KeepOrRevert::Revert;
    }
    for result in &measurement.results {
        if (result.new && result.after != Grade::Pass)
            || (!result.new && result.before == Grade::Pass && result.after == Grade::Fail)
        {
            return KeepOrRevert::Revert;
        }
    }
    KeepOrRevert::Keep
}
pub fn check_measurement(
    measurement: &Measurement,
    proposal: &Proposal,
    approval: &Approval,
    cases_at_start: &CaseBook,
) -> Result<()> {
    if measurement.proposal != proposal.id
        || measurement.approval != approval.id
        || measurement.base != proposal.base
        || measurement.outcome != outcome(measurement, proposal, approval, cases_at_start)
    {
        return Err(Error::Invalid(text::rule(
            &measurement.id,
            text::WRONG_MEASUREMENT,
        )));
    }
    Ok(())
}
pub fn check_references(
    persona: &str,
    signals: &[Signal],
    cases: &CaseBook,
    proposals: &[Proposal],
    approvals: &[Approval],
    measurements: &[Measurement],
) -> Result<()> {
    let missing = |id: &str| Error::Invalid(text::rule(id, text::MISSING_REFERENCE));
    let signal_ids: std::collections::BTreeSet<_> =
        signals.iter().map(|signal| &signal.id).collect();
    for signal in signals {
        if signal.persona != persona {
            return Err(missing(&signal.id));
        }
    }
    for case in cases.iter() {
        if case.persona != persona || case.from_signals.iter().any(|id| !signal_ids.contains(id)) {
            return Err(missing(&case.id));
        }
    }
    let proposal_ids: BTreeMap<_, _> = proposals
        .iter()
        .map(|proposal| (&proposal.id, proposal))
        .collect();
    for proposal in proposals {
        if proposal.persona != persona || proposal.cases.iter().any(|id| cases.get(id).is_none()) {
            return Err(missing(&proposal.id));
        }
    }
    for approval in approvals {
        if !proposal_ids.contains_key(&approval.proposal) {
            return Err(missing(&approval.id));
        }
    }
    let approval_ids: BTreeMap<_, _> = approvals
        .iter()
        .map(|approval| (&approval.id, approval))
        .collect();
    for measurement in measurements {
        let proposal = proposal_ids
            .get(&measurement.proposal)
            .ok_or_else(|| missing(&measurement.id))?;
        let approval = approval_ids
            .get(&measurement.approval)
            .ok_or_else(|| missing(&measurement.id))?;
        if may_apply(proposal, approval).is_err() {
            return Err(missing(&measurement.id));
        }
    }
    Ok(())
}
pub fn one_approval(approvals: &[Approval]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    for approval in approvals {
        if !seen.insert(&approval.proposal) {
            return Err(Error::Invalid(text::rule(
                &approval.proposal,
                text::ALREADY_APPROVED,
            )));
        }
    }
    Ok(())
}
pub fn no_measurement_when_rejected(
    approvals: &[Approval],
    measurements: &[Measurement],
) -> Result<()> {
    for approval in approvals
        .iter()
        .filter(|approval| approval.decision == Decision::Rejected)
    {
        if measurements.iter().any(|measurement| {
            measurement.proposal == approval.proposal || measurement.approval == approval.id
        }) {
            return Err(Error::Invalid(text::rule(
                &approval.proposal,
                text::REJECTED,
            )));
        }
    }
    Ok(())
}
pub fn base_is_current(proposal: &Proposal, current: &str) -> Result<()> {
    if proposal.base != current {
        return Err(Error::Invalid(text::base_moved(&proposal.id)));
    }
    Ok(())
}
