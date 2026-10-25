use persona_builder_grow::{record::*, rules::*};
use serde::de::DeserializeOwned;
use std::{fs, path::PathBuf};

fn example<T: DeserializeOwned>(kind: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../docs/spec/growth-loop/examples/{kind}.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn proposal() -> Proposal {
    example("proposal")
}
fn approval() -> Approval {
    example("approval")
}
fn measurement() -> Measurement {
    example("measurement")
}
fn case(id: &str, status: Status) -> Case {
    Case {
        id: id.into(),
        status,
        ..example("case")
    }
}
fn cases() -> CaseBook {
    CaseBook::new(&[
        case("case-2026-03-04-001", Status::New),
        case("case-2026-02-20-002", Status::Active),
        case("case-2026-02-11-004", Status::Active),
        case("case-other", Status::New),
        case("case-retired", Status::Retired),
    ])
    .unwrap()
}
fn refused(result: persona_builder::error::Result<()>, id: &str) {
    let error = result.expect_err("the rule must refuse this record");
    assert!(matches!(error, persona_builder::error::Error::Invalid(_)));
    assert!(error.to_string().contains(id), "{error}");
    assert!(!error.to_string().contains("zebracorn"), "{error}");
}

#[test]
fn approver_matches_persona_kind() {
    assert_eq!(approver(PersonaKind::Person), GivenBy::Subject);
    assert_eq!(approver(PersonaKind::Product), GivenBy::Staff);
}

#[test]
fn may_apply_needs_this_proposal_approved_by_the_right_role() {
    let mut p = proposal();
    let a = approval();
    assert!(may_apply(&p, &a).is_ok());
    for edit in 0..3 {
        let mut bad = a.clone();
        match edit {
            0 => bad.decision = Decision::Rejected,
            1 => bad.role = GivenBy::Staff,
            _ => bad.proposal = "prop-other".into(),
        }
        refused(may_apply(&p, &bad), &p.id);
    }
    p.persona_kind = PersonaKind::Product;
    refused(may_apply(&p, &a), &p.id);
    let staff = Approval {
        role: GivenBy::Staff,
        ..a
    };
    assert!(may_apply(&p, &staff).is_ok());
}

#[test]
fn to_measure_lists_proposed_then_active_cases() {
    let mut p = proposal();
    p.cases.push("case-z".into());
    let mut lines: Vec<_> = cases().iter().cloned().collect();
    lines.push(case("case-z", Status::New));
    let book = CaseBook::new(&lines).unwrap();
    assert_eq!(
        to_measure(&p, &book).unwrap(),
        vec![
            ("case-2026-03-04-001".into(), true),
            ("case-z".into(), true),
            ("case-2026-02-11-004".into(), false),
            ("case-2026-02-20-002".into(), false)
        ]
    );
}

#[test]
fn to_measure_refuses_missing_or_not_new() {
    for id in ["case-missing", "case-2026-02-11-004", "case-retired"] {
        let mut p = proposal();
        p.cases = vec![id.into()];
        assert!(to_measure(&p, &cases()).is_err(), "{id}");
        assert!(
            to_measure(&p, &cases())
                .unwrap_err()
                .to_string()
                .contains(id)
        );
    }
}

#[test]
fn outcome_keeps_only_complete_passing_nonregressing_approved_results() {
    let p = proposal();
    let a = approval();
    let m = measurement();
    let book = cases();
    assert_eq!(outcome(&m, &p, &a, &book), KeepOrRevert::Keep);
    let mut reordered = m.clone();
    reordered.results.reverse();
    assert_eq!(outcome(&reordered, &p, &a, &book), KeepOrRevert::Keep);
    for edit in 0..9 {
        let mut bad = m.clone();
        let mut answer = a.clone();
        match edit {
            0 => bad.results[1].after = Grade::Fail,
            1 => bad.results[0].after = Grade::Fail,
            2 => {
                bad.results.remove(0);
            }
            3 => bad.results.push(CaseResult {
                case: "case-extra".into(),
                ..bad.results[0].clone()
            }),
            4 => answer.decision = Decision::Rejected,
            5 => bad.results.push(bad.results[2].clone()),
            6 => {
                bad.results.remove(1);
            }
            7 => bad.results[0].new = false,
            _ => answer.role = GivenBy::Staff,
        }
        assert_eq!(
            outcome(&bad, &p, &answer, &book),
            KeepOrRevert::Revert,
            "edit {edit}"
        );
    }
    let mut missing = p.clone();
    missing.cases.push("case-missing".into());
    assert_eq!(outcome(&m, &missing, &a, &book), KeepOrRevert::Revert);
}

#[test]
fn measurement_checks_links_base_and_calculated_outcome() {
    let p = proposal();
    let a = approval();
    let m = measurement();
    let book = cases();
    assert!(check_measurement(&m, &p, &a, &book).is_ok());
    for edit in 0..4 {
        let mut bad = m.clone();
        match edit {
            0 => bad.proposal = "prop-other".into(),
            1 => bad.approval = "appr-other".into(),
            2 => bad.base = "zebracorn".into(),
            _ => bad.outcome = KeepOrRevert::Revert,
        }
        refused(check_measurement(&bad, &p, &a, &book), &bad.id);
    }
    let mut failed = m.clone();
    failed.results[0].after = Grade::Fail;
    failed.outcome = KeepOrRevert::Revert;
    assert!(check_measurement(&failed, &p, &a, &book).is_ok());
}

#[test]
fn case_book_checks_every_status_move_and_keeps_id_order() {
    for before in [Status::New, Status::Active, Status::Retired] {
        for after in [Status::New, Status::Active, Status::Retired] {
            let first = case("case-a", before);
            let last = case("case-a", after);
            let result = CaseBook::new(&[case("case-z", Status::New), first, last]);
            let allowed = before == after
                || matches!(
                    (before, after),
                    (Status::New, Status::Active | Status::Retired)
                        | (Status::Active, Status::Retired)
                );
            assert_eq!(result.is_ok(), allowed, "{before:?} to {after:?}");
            if allowed {
                let book = result.unwrap();
                assert_eq!(book.get("case-a").unwrap().status, after);
                assert_eq!(
                    book.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
                    vec!["case-a", "case-z"]
                );
                assert!(book.get("case-missing").is_none());
            } else {
                let message = result.unwrap_err().to_string();
                for text in [
                    "case-a".to_owned(),
                    serde_json::to_value(before)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                    serde_json::to_value(after)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned(),
                ] {
                    assert!(message.contains(&text), "{message}");
                }
            }
        }
    }
}

#[test]
fn case_book_refuses_changes_other_than_status() {
    let first = case("case-a", Status::New);
    let original = serde_json::to_value(&first).unwrap();
    for field in original
        .as_object()
        .unwrap()
        .keys()
        .filter(|field| *field != "id" && *field != "status")
    {
        let mut changed = original.clone();
        match field.as_str() {
            "from_signals" => changed[field] = serde_json::json!(["sig-other"]),
            "options" => changed[field] = serde_json::json!(["a", "b"]),
            "match" => changed[field] = serde_json::json!("rubric"),
            _ => changed[field] = serde_json::json!("zebracorn"),
        }
        let last: Case = serde_json::from_value(changed).unwrap();
        let error = CaseBook::new(&[first.clone(), last]).expect_err(field);
        assert_eq!(
            error.to_string(),
            "case case-a was appended again with a change other than its status"
        );
    }
    let mut active = first.clone();
    active.status = Status::Active;
    let mut changed = first.clone();
    changed.situation = "zebracorn".into();
    changed.status = Status::Retired;
    assert!(CaseBook::new(&[first, active, changed]).is_err());
}

#[test]
fn one_approval_per_proposal() {
    let a = approval();
    assert!(one_approval(&[]).is_ok());
    assert!(one_approval(std::slice::from_ref(&a)).is_ok());
    let mut other = a.clone();
    other.id = "appr-other".into();
    refused(one_approval(&[a.clone(), other.clone()]), &a.proposal);
    other.proposal = "prop-other".into();
    assert!(one_approval(&[a, other]).is_ok());
}

#[test]
fn no_measurement_for_a_rejected_proposal() {
    let a = approval();
    let m = measurement();
    assert!(
        no_measurement_when_rejected(std::slice::from_ref(&a), std::slice::from_ref(&m)).is_ok()
    );
    let mut rejected = a;
    rejected.decision = Decision::Rejected;
    assert!(no_measurement_when_rejected(std::slice::from_ref(&rejected), &[]).is_ok());
    refused(
        no_measurement_when_rejected(std::slice::from_ref(&rejected), std::slice::from_ref(&m)),
        &rejected.proposal,
    );
    let mut other_approval = m;
    other_approval.approval = "appr-other".into();
    refused(
        no_measurement_when_rejected(&[rejected.clone()], &[other_approval]),
        &rejected.proposal,
    );
}

#[test]
fn base_must_be_current_without_echoing_versions() {
    let p = proposal();
    assert!(base_is_current(&p, &p.base).is_ok());
    refused(base_is_current(&p, "zebracorn"), &p.id);
    let mut stale = p;
    stale.base = "zebracorn".into();
    refused(base_is_current(&stale, "current"), &stale.id);
}

#[test]
fn references_stay_within_the_persona_and_name_existing_records() {
    let s: Signal = example("signal");
    let c: Case = example("case");
    let p = proposal();
    let a = approval();
    let m = measurement();
    let check = |s: &[Signal], c: &[Case], p: &[Proposal], a: &[Approval], m: &[Measurement]| {
        check_references("robin", s, &CaseBook::new(c).unwrap(), p, a, m)
    };
    assert!(
        check(
            std::slice::from_ref(&s),
            std::slice::from_ref(&c),
            std::slice::from_ref(&p),
            std::slice::from_ref(&a),
            std::slice::from_ref(&m)
        )
        .is_ok()
    );
    assert!(check(&[], &[], &[], &[], &[]).is_ok());
    assert!(check(std::slice::from_ref(&s), &[], &[], &[], &[]).is_ok()); // A signal need not have a case.
    for edit in 0..11 {
        let mut sig = s.clone();
        let mut test = c.clone();
        let mut prop = p.clone();
        let mut answer = a.clone();
        let mut meas = m.clone();
        match edit {
            0 => test.from_signals = vec!["sig-missing".into()],
            1 => prop.cases = vec!["case-missing".into()],
            2 => answer.proposal = "prop-missing".into(),
            3 => meas.proposal = "prop-missing".into(),
            4 => meas.approval = "appr-missing".into(),
            5 => answer.decision = Decision::Rejected,
            6 => sig.persona = "zebracorn".into(),
            7 => test.persona = "zebracorn".into(),
            8 => prop.persona = "zebracorn".into(),
            9 => answer.role = GivenBy::Staff,
            _ => answer.proposal = "prop-other".into(),
        }
        let mut proposals = vec![prop];
        let mut other = p.clone();
        other.id = "prop-other".into();
        proposals.push(other);
        let result = check(&[sig], &[test], &proposals, &[answer], &[meas]);
        assert!(result.is_err(), "edit {edit}");
        let message = result.unwrap_err().to_string();
        assert!(!message.contains("zebracorn"), "{message}");
        assert!(
            message.contains(&s.id)
                || message.contains(&c.id)
                || message.contains(&p.id)
                || message.contains(&a.id)
                || message.contains(&m.id),
            "{message}"
        );
    }
    let mut missing = a.clone();
    missing.proposal = "prop-missing".into();
    refused(
        check(
            std::slice::from_ref(&s),
            std::slice::from_ref(&c),
            std::slice::from_ref(&p),
            &[missing],
            &[],
        ),
        &a.id,
    );
    let mut rejected = a.clone();
    rejected.decision = Decision::Rejected;
    assert!(check(&[s], &[c], &[p], &[rejected], &[]).is_ok());
}

#[test]
fn rejected_approval_cannot_be_measured_under_another_proposal() {
    let mut rejected = approval();
    rejected.decision = Decision::Rejected;
    let mut measured = measurement();
    measured.proposal = "prop-other".into();
    refused(
        no_measurement_when_rejected(std::slice::from_ref(&rejected), &[measured]),
        &rejected.proposal,
    );
}
