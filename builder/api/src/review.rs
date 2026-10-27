//! Reports and the approved, measured growth loop at the app boundary.
use crate::{
    assistants::{claude_message, home},
    building::{call_failure, open, spend},
    dto::*,
    history::{FIND, PERMISSION, count, failure, locked, readable},
};
use persona_builder::{assistants, error::Error, member::Member};
use persona_builder_agent::claude;
use persona_builder_build::{interview, report};
use persona_builder_grow::{
    apply, diff, keep,
    record::{Decision, KeepOrRevert, Proposal, Status},
    refresh,
    store::{self, Store},
    text,
};
use persona_builder_ingest::filters::Filters;
use persona_builder_steps::book::book;
use serde_json::json;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Failure>;
const REVIEW_SETTINGS: &str = "Something in this assistant's settings stops the review. Use the persona-builder command to see what.";
const GROWTH_DAMAGED: &str = "Your assistant's kept answers can't be read, so nothing was changed. Use the persona-builder command to see what's wrong.";
const KEEP_EMPTY: &str = "Say what was being decided and what you'd want, then keep it.";
const KEEP_BAD: &str = "Persona Builder couldn't keep that answer. Check it and try again.";
const KEEP_PRIVATE: &str = "That looked personal or secret, so it wasn't kept. Leave out names and private details, then try again.";
const NOT_BUILT_YET: &str = "Build your assistant first: there's nothing to refresh yet.";
const ALREADY_OPEN: &str =
    "A change is already waiting for your answer. Approve or reject it first.";
const REFRESH_FAILED: &str = "Claude's draft didn't fit, twice, so nothing was saved. Try again.";
const CHANGE_GONE: &str =
    "That change isn't waiting for your answer any more. Close it and open it again.";
const CHANGE_OLD: &str = "Your assistant or your answers changed since this was drafted, so it can't be approved. Reject it, then draft a new one.";
const ANSWER_SAVE: &str = "Persona Builder couldn't save your answer. Check there's space on this computer, then try again.";
const TEST_OLD: &str = "Your assistant or your answers changed since you approved this change, so it wasn't tested and nothing changed. Use the persona-builder command to see what to do.";
const TEST_FAILED: &str = "The test stopped before it finished, so your assistant is as it was. What was already finished is kept: test again to carry on.";
const APPLY_SAVE: &str = "The change was tested, but the result couldn't be saved. Use the persona-builder command to finish it.";
fn level(value: report::Level) -> ReportLevel {
    match value {
        report::Level::Strong => ReportLevel::Strong,
        report::Level::GettingThere => ReportLevel::GettingThere,
        report::Level::Thin => ReportLevel::Thin,
    }
}
fn change(p: Proposal, approved: bool) -> RefreshChange {
    RefreshChange {
        id: p.id,
        drafted: p.created.chars().take(10).collect(),
        summary: p.summary,
        points: p
            .changes
            .into_iter()
            .map(|c| RefreshPoint {
                why: c.why,
                before: diff::removed_lines(&c.diff)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                after: diff::added_lines(&c.diff)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            })
            .collect(),
        learned_from: count(p.cases.len()),
        approved,
    }
}
fn growth_error(e: Error) -> Failure {
    match e {
        Error::Invalid(message) if message == text::NOT_BUILT => failure(NOT_BUILT_YET),
        _ => failure(GROWTH_DAMAGED),
    }
}
fn status(home: &Path, member: &Member) -> Result<ReviewStatus> {
    let areas = book(member)
        .map_err(|_| failure(REVIEW_SETTINGS))?
        .categories;
    let store = Store::open(home, member).map_err(|_| failure(REVIEW_SETTINGS))?;
    let cases = store.cases().map_err(|_| failure(GROWTH_DAMAGED))?;
    let counts = |area: Option<&str>| {
        let mut counts = [0usize; 3];
        for c in cases.iter().filter(|c| area.is_none_or(|a| c.area == a)) {
            counts[match c.status {
                Status::New => 0,
                Status::Active => 1,
                Status::Retired => 2,
            }] += 1;
        }
        TestCounts {
            waiting: count(counts[0]),
            in_use: count(counts[1]),
            retired: count(counts[2]),
        }
    };
    let tests = counts(None);
    let by_area = areas
        .iter()
        .filter_map(|area| {
            let tests = counts(Some(area));
            if tests.waiting == 0 && tests.in_use == 0 && tests.retired == 0 {
                None
            } else {
                Some(AreaTests {
                    area: area.clone(),
                    tests,
                })
            }
        })
        .collect();
    let dir = assistants::assistant_dir(home, &member.slug).map_err(|_| failure(FIND))?;
    let (built, change) = match refresh::waiting(&store, &dir) {
        Ok(p) => (true, p.map(|(p, a)| change(p, a))),
        Err(Error::Invalid(message)) if message == text::NOT_BUILT => (false, None),
        Err(_) => return Err(failure(GROWTH_DAMAGED)),
    };
    Ok(ReviewStatus {
        built,
        areas,
        tests,
        by_area,
        change,
    })
}
pub fn show_report_in(home: &Path, request: SlugRequest) -> Result<ReportView> {
    let member = readable(home, &request.slug)?;
    let (report, unreadable) = match report::read(home, &request.slug) {
        Ok(r) => (r, false),
        Err(_) => (None, true),
    };
    let growth = Store::open(home, &member)
        .and_then(|s| apply::growth_counts(&s, report.as_ref().map(|r| r.built.as_str())))
        .ok()
        .flatten()
        .map(|g| GrowthCounts {
            kept: count(g.kept),
            undone: count(g.undone),
            passing: count(g.passing),
            of: count(g.of),
        });
    let report = report.map(|r| BuildReport {
        built: r.built,
        overall: level(r.overall),
        answered: count(r.answered),
        thin_below: count(interview::THIN_BELOW),
        held_out: count(r.held),
        test: r.test.map(|t| ReportTest {
            right: count(t.right),
            of: count(t.of),
        }),
        areas: r
            .areas
            .into_iter()
            .map(|a| ReportArea {
                area: a.area,
                level: level(a.level),
                examples: count(a.examples),
                help: a
                    .strengthen
                    .into_iter()
                    .map(|h| match h {
                        report::Strengthen::Interview => ReportHelp::Interview,
                        report::Strengthen::Claudeai => ReportHelp::Claudeai,
                        report::Strengthen::Time => ReportHelp::Time,
                    })
                    .collect(),
            })
            .collect(),
    });
    Ok(ReportView {
        report,
        unreadable,
        growth,
    })
}
pub fn review_status_in(home: &Path, request: SlugRequest) -> Result<ReviewStatus> {
    status(home, &readable(home, &request.slug)?)
}
pub fn keep_answer_in(home: &Path, request: KeepAnswerRequest) -> Result<ReviewStatus> {
    keep_answer_after(home, request, &mut || {})
}
fn keep_answer_after(
    home: &Path,
    mut request: KeepAnswerRequest,
    between: &mut dyn FnMut(),
) -> Result<ReviewStatus> {
    request.area = request.area.trim().into();
    request.situation = request.situation.trim().into();
    request.wanted = request.wanted.trim().into();
    request.assistant_said = request.assistant_said.trim().into();
    if request.situation.is_empty() || request.wanted.is_empty() {
        return Err(failure(KEEP_EMPTY));
    }
    if request.area.chars().count() > 100
        || [&request.situation, &request.wanted, &request.assistant_said]
            .iter()
            .any(|s| s.chars().count() > 2000)
    {
        return Err(failure(KEEP_BAD));
    }
    let (member, _lock) = crate::history::locked_after(home, &request.slug, between)?;
    let areas = book(&member)
        .map_err(|_| failure(REVIEW_SETTINGS))?
        .categories;
    if !areas.contains(&request.area) {
        return Err(failure(KEEP_BAD));
    }
    let filters = Filters::compile(&member.filters).map_err(|_| failure(REVIEW_SETTINGS))?;
    if [&request.situation, &request.wanted, &request.assistant_said]
        .iter()
        .any(|s| !s.is_empty() && filters.kind(s).is_some())
    {
        return Err(failure(KEEP_PRIVATE));
    }
    let store = Store::open(home, &member).map_err(|_| failure(REVIEW_SETTINGS))?;
    let kind = if request.assistant_said.is_empty() {
        "instruction"
    } else {
        "correction"
    };
    let mut input = json!({
        "kind": kind,
        "area": request.area,
        "situation": request.situation,
        "wanted": request.wanted,
        "source": "review",
    });
    if !request.assistant_said.is_empty() {
        input["persona_said"] = json!(request.assistant_said);
    }
    keep::keep_locked(
        &store,
        &member,
        &filters,
        input.to_string().as_bytes(),
        &store::utc_now(),
    )
    .map_err(|_| failure(KEEP_BAD))?;
    status(home, &member)
}
pub fn draft_refresh_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: SlugRequest,
) -> Result<RefreshDrafted> {
    let member = readable(home, &request.slug)?;
    let filters = Filters::compile(&member.filters).map_err(|_| failure(REVIEW_SETTINGS))?;
    let claude = claude::find_in(home, path, fallbacks).map_err(|e| failure(&claude_message(e)))?;
    claude
        .check(home)
        .map_err(|e| failure(&claude_message(e)))?;
    let session = open(home, &request.slug)?;
    if !crate::read_consent(home, &request.slug).agreed {
        return Err(failure(PERMISSION));
    }
    let store = Store::open(home, &member).map_err(|_| failure(REVIEW_SETTINGS))?;
    let dir = assistants::assistant_dir(home, &request.slug).map_err(|_| failure(FIND))?;
    if refresh::waiting(&store, &dir)
        .map_err(growth_error)?
        .is_some()
    {
        return Err(failure(ALREADY_OPEN));
    }
    let proposed = refresh::propose(
        &claude,
        &session,
        home,
        &store,
        &member,
        &dir,
        &filters,
        claude::DEFAULT_MODEL,
        &store::utc_now(),
    )
    .map_err(|e| call_failure(e, &session, REFRESH_FAILED))?;
    Ok(RefreshDrafted {
        change: match proposed {
            refresh::Proposed::Nothing => None,
            refresh::Proposed::Drafted(p) => Some(change(p, false)),
        },
        spend: spend(&session)?,
    })
}
pub fn answer_refresh_in(home: &Path, request: RefreshAnswerRequest) -> Result<ReviewStatus> {
    let (member, _lock) = locked(home, &request.slug)?;
    let store = Store::open(home, &member).map_err(|_| failure(REVIEW_SETTINGS))?;
    let dir = assistants::assistant_dir(home, &request.slug).map_err(|_| failure(FIND))?;
    let waiting = refresh::waiting(&store, &dir).map_err(|e| match e {
        Error::Invalid(message) if message == text::NOT_BUILT => failure(CHANGE_GONE),
        _ => failure(GROWTH_DAMAGED),
    })?;
    if !waiting.is_some_and(|(p, approved)| p.id == request.id && !approved) {
        return Err(failure(CHANGE_GONE));
    }
    refresh::answer_locked(
        &store,
        &dir,
        &request.id,
        if request.approve {
            Decision::Approved
        } else {
            Decision::Rejected
        },
        &store::utc_now(),
    )
    .map_err(|_| {
        failure(if request.approve {
            CHANGE_OLD
        } else {
            ANSWER_SAVE
        })
    })?;
    status(home, &member)
}
fn stale_measure(e: &Error, id: &str) -> bool {
    matches!(e, Error::Invalid(message) if message == text::CHANGE_DOES_NOT_FIT
        || message == text::DECISIONS_CHANGED_TESTING || message == &text::cases_moved(id)
        || message == &text::base_moved(id))
}
pub fn apply_refresh_in(
    home: &Path,
    path: Option<&OsStr>,
    fallbacks: &[PathBuf],
    request: RefreshApplyRequest,
    progress: &mut dyn FnMut(BuildProgress),
) -> Result<RefreshApplied> {
    let member = readable(home, &request.slug)?;
    let filters = Filters::compile(&member.filters).map_err(|_| failure(REVIEW_SETTINGS))?;
    let claude = claude::find_in(home, path, fallbacks).map_err(|e| failure(&claude_message(e)))?;
    claude
        .check(home)
        .map_err(|e| failure(&claude_message(e)))?;
    let session = open(home, &request.slug)?;
    if !crate::read_consent(home, &request.slug).agreed {
        return Err(failure(PERMISSION));
    }
    let store = Store::open(home, &member).map_err(|_| failure(REVIEW_SETTINGS))?;
    let dir = assistants::assistant_dir(home, &request.slug).map_err(|_| failure(FIND))?;
    let waiting = refresh::waiting(&store, &dir).map_err(|e| match e {
        Error::Invalid(message) if message == text::NOT_BUILT => failure(CHANGE_GONE),
        _ => failure(GROWTH_DAMAGED),
    })?;
    if !waiting.is_some_and(|(p, approved)| p.id == request.id && approved) {
        return Err(failure(CHANGE_GONE));
    }
    let mut last_spend = Spend {
        spent_usd: 0.0,
        daily_cap_usd: session.cap().daily_usd(),
    };
    let measured = apply::measure(
        &claude,
        &session,
        home,
        &store,
        &member,
        &dir,
        &filters,
        claude::DEFAULT_MODEL,
        &request.id,
        &store::utc_now,
        &mut |line| {
            if let Ok(now) = spend(&session) {
                last_spend = now;
            }
            progress(BuildProgress {
                line: line.into(),
                spend: last_spend.clone(),
            });
        },
    )
    .map_err(|e| {
        if stale_measure(&e, &request.id) {
            failure(TEST_OLD)
        } else {
            call_failure(e, &session, TEST_FAILED)
        }
    })?;
    let finished = apply::finish(
        &store,
        &member,
        &dir,
        &filters,
        &measured.proposal,
        &measured.measurement,
    )
    .map_err(|_| failure(APPLY_SAVE))?;
    Ok(RefreshApplied {
        kept: finished.outcome == KeepOrRevert::Keep,
        reason: apply::reason(&measured.measurement),
        spend: spend(&session).unwrap_or(last_spend),
    })
}
pub fn show_report(request: SlugRequest) -> Result<ReportView> {
    show_report_in(&home(FIND)?, request)
}
pub fn review_status(request: SlugRequest) -> Result<ReviewStatus> {
    review_status_in(&home(FIND)?, request)
}
pub fn keep_answer(request: KeepAnswerRequest) -> Result<ReviewStatus> {
    keep_answer_in(&home(FIND)?, request)
}
pub fn answer_refresh(request: RefreshAnswerRequest) -> Result<ReviewStatus> {
    answer_refresh_in(&home(FIND)?, request)
}
pub fn draft_refresh(request: SlugRequest) -> Result<RefreshDrafted> {
    let home = home(FIND)?;
    draft_refresh_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        request,
    )
}
pub fn apply_refresh(
    request: RefreshApplyRequest,
    progress: &mut dyn FnMut(BuildProgress),
) -> Result<RefreshApplied> {
    let home = home(FIND)?;
    apply_refresh_in(
        &home,
        std::env::var_os("PATH").as_deref(),
        &claude::fallbacks(&home),
        request,
        progress,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measure_recognizes_a_moved_base_as_stale() {
        assert!(stale_measure(
            &Error::Invalid(text::base_moved("change")),
            "change"
        ));
    }
    #[test]
    fn keep_refuses_forgetting_between_check_and_lock_without_recreating_state() {
        let home = std::env::temp_dir().join(format!("pb-review-forget-{}", std::process::id()));
        std::fs::create_dir(&home).unwrap();
        crate::new_assistant_in(
            &home,
            NewAssistantRequest {
                name: "Wren".into(),
                pronouns: "she/her".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        crate::agree_consent_in(
            &home,
            SlugRequest {
                slug: "wren".into(),
            },
        )
        .unwrap();
        let result = keep_answer_after(
            &home,
            KeepAnswerRequest {
                slug: "wren".into(),
                area: "scope".into(),
                situation: "Choose an exercise.".into(),
                wanted: "Begin quietly.".into(),
                assistant_said: String::new(),
            },
            &mut || {
                let preview = crate::forget_preview_in(
                    &home,
                    SlugRequest {
                        slug: "wren".into(),
                    },
                )
                .unwrap();
                crate::forget_in(
                    &home,
                    ForgetRequest {
                        slug: "wren".into(),
                        files: preview.files,
                    },
                )
                .unwrap();
            },
        );
        let state_exists = home.join(".claude/state/wren").exists();
        std::fs::remove_dir_all(&home).unwrap();
        assert_eq!(result.unwrap_err().message, PERMISSION);
        assert!(!state_exists);
    }
}
