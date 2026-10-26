mod common;
use common::*;
use persona_builder::error::Result;
use persona_builder_agent::{
    claude::Claude,
    ledger::{Cap, Session},
};
use persona_builder_grow::{
    apply::{self, Left, Measured},
    diff,
    keep::keep,
    record::*,
    refresh, rules,
    store::Store,
    version::version,
};
use persona_builder_ingest::{filters::Filters, private::write_private_at};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, MutexGuard},
};
const BASE: &str = "# Decisions\n\n## By kind of question\n\n### communication\n\n**The lean:**\n- Keep the first step small.\n\n**Standing rules:**\n- None yet.\n\n**Precedents:**\n- None yet.\n";
const CANDIDATE: &str = "# Decisions\n\n## By kind of question\n\n### communication\n\n**The lean:**\n- Keep the first step small.\n\n**Standing rules:**\n- ADJUST the first step.\n\n**Precedents:**\n- None yet.\n";
static SUBPROCESS_TEST: Mutex<()> = Mutex::new(());
struct Setup {
    f: Fixture,
    dir: PathBuf,
    stub: PathBuf,
    _serial: MutexGuard<'static, ()>,
}
impl Setup {
    fn new() -> Self {
        let serial = SUBPROCESS_TEST.lock().unwrap_or_else(|p| p.into_inner());
        let f = Fixture::new();
        let dir = f.home.join("member");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("decisions.md"), BASE).unwrap();
        let stub = f.home.join("stub");
        fs::create_dir(&stub).unwrap();
        let python = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        fs::write(
            stub.join("claude"),
            format!(
                "#!{} -I\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                include_str!("fixtures/apply_stub.py")
            ),
        )
        .unwrap();
        fs::set_permissions(stub.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
        let s = Self {
            f,
            dir,
            stub,
            _serial: serial,
        };
        keep(
            &s.store(),
            &s.f.member,
            &s.filters(),
            &serde_json::to_vec(&input()).unwrap(),
            NOW,
        )
        .unwrap();
        let mut c = s.cases().remove(0);
        c.options = Some(vec![
            "STAY with the original choice.".into(),
            "ADJUST the approach.".into(),
        ]);
        c.expected = "ADJUST the approach.".into();
        c.r#match = Match::Choice;
        s.replace_cases(&[c]);
        s
    }
    fn store(&self) -> Store {
        Store::open(&self.f.home, &self.f.member).unwrap()
    }
    fn filters(&self) -> Filters {
        Filters::compile(&self.f.member.filters).unwrap()
    }
    fn cases(&self) -> Vec<Case> {
        self.store().cases().unwrap().iter().cloned().collect()
    }
    fn replace_cases(&self, cases: &[Case]) {
        let text: String = cases.iter().map(to_line).collect();
        write_private_at(
            &self.f.home,
            &self.f.member.slug,
            Some("growth"),
            "cases.jsonl",
            text.as_bytes(),
        )
        .unwrap();
    }
    fn proposal(&self, candidate: &str) -> Proposal {
        self.proposal_from(BASE, candidate)
    }
    fn proposal_from(&self, base: &str, candidate: &str) -> Proposal {
        let p = Proposal {
            schema: PROPOSAL_V1.into(),
            id: "prop-2026-03-04-001".into(),
            persona: self.f.member.slug.clone(),
            persona_kind: PersonaKind::Person,
            created: NOW.into(),
            base: version(base.as_bytes()),
            summary: "Start with a flexible first step.".into(),
            changes: vec![Change {
                file: "decisions.md".into(),
                diff: diff::unified("decisions.md", base, candidate).unwrap(),
                why: "Leave room to reconsider.".into(),
            }],
            cases: vec![self.cases()[0].id.clone()],
        };
        self.store().save_proposal(&p).unwrap();
        p
    }
    fn approved(&self, candidate: &str) -> Proposal {
        let p = self.proposal(candidate);
        refresh::answer(&self.store(), &self.dir, &p.id, Decision::Approved, NOW).unwrap();
        p
    }
    fn rewrite(&self, p: &Proposal) {
        write_private_at(
            &self.f.home,
            &self.f.member.slug,
            Some("growth"),
            &format!("{}.json", p.id),
            &serde_json::to_vec(p).unwrap(),
        )
        .unwrap();
    }
    fn measure(&self, id: &str) -> Result<Measured> {
        self.measure_progress(id, &mut |_| {})
    }
    fn measure_progress(&self, id: &str, progress: &mut dyn FnMut(&str)) -> Result<Measured> {
        let session = Session::open(&self.f.home, &self.f.member.slug, Cap::default()).unwrap();
        apply::measure(
            &Claude::at(self.stub.join("claude")),
            &session,
            &self.f.home,
            &self.store(),
            &self.f.member,
            &self.dir,
            &self.filters(),
            "sonnet",
            id,
            &|| NOW.into(),
            progress,
        )
    }
    fn measured(&self, id: &str) -> Measurement {
        let measured = self.measure(id).expect("approved change must be measured");
        assert!(!measured.already);
        measured.measurement
    }
    fn finish(&self, p: &Proposal, m: &Measurement) -> Result<apply::Finished> {
        let _session = Session::open(&self.f.home, &self.f.member.slug, Cap::default()).unwrap();
        apply::finish(
            &self.store(),
            &self.f.member,
            &self.dir,
            &self.filters(),
            p,
            m,
        )
    }
    fn calls(&self) -> usize {
        fs::read_dir(&self.stub)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("stdin")
            })
            .count()
    }
    fn plan(&self, v: Value) {
        fs::write(self.stub.join("plan.json"), v.to_string()).unwrap();
    }
    fn prompt(&self, n: usize) -> String {
        fs::read_to_string(self.stub.join(format!("stdin{n}"))).unwrap()
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(p: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            if !p.exists() {
                return;
            }
            for e in fs::read_dir(p).unwrap() {
                let path = e.unwrap().path();
                if path.is_dir() {
                    walk(&path, out)
                } else {
                    out.insert(path.clone(), fs::read(path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.dir, &mut out);
        walk(&self.f.growth(), &mut out);
        out
    }
}
fn done(s: &Setup, p: &Proposal) -> Measurement {
    let m = s.measured(&p.id);
    s.finish(p, &m).unwrap();
    m
}
#[test]
fn keep_saves_valid_measurement_before_files_and_activates_named_tests() {
    let s = Setup::new();
    let mut cases = s.cases();
    let mut active = cases[0].clone();
    active.id = "case-2026-03-04-002".into();
    active.status = Status::Active;
    active.expected = "STAY with the original choice.".into();
    active.options = Some(vec![active.expected.clone(), "OTHER response.".into()]);
    cases.push(active);
    s.replace_cases(&cases);
    fs::write(
        s.dir.join("measurements.md"),
        "# Measurements\n\n## Reports\n\nPrior report.\n",
    )
    .unwrap();
    let p = s.approved(CANDIDATE);
    let before = s.snapshot();
    let measured = s.measure(&p.id).unwrap();
    assert!(!measured.already);
    assert_eq!(measured.proposal, p);
    assert_eq!(
        measured.proposal,
        s.store().proposal(&p.id).unwrap().unwrap()
    );
    let m = measured.measurement;
    assert_eq!(m.outcome, KeepOrRevert::Keep);
    assert_eq!(
        m.reason,
        "Every new test now passes, and none of the others got worse."
    );
    assert_eq!(m.results.len(), 2);
    assert_eq!(m.results[0].before, Grade::Fail);
    assert_eq!(m.results[0].after, Grade::Pass);
    assert_eq!(m.results[1].before, Grade::Pass);
    assert_eq!(m.results[1].after, Grade::Pass);
    assert_eq!(
        fs::read(s.dir.join("decisions.md")).unwrap(),
        BASE.as_bytes()
    );
    assert_eq!(
        fs::read(s.dir.join("measurements.md")).unwrap(),
        before[&s.dir.join("measurements.md")]
    );
    assert_eq!(s.store().measurements().unwrap(), vec![m.clone()]);
    s.store().check_all().unwrap();
    rules::check_measurement(
        &m,
        &p,
        &s.store().approvals().unwrap()[0],
        &s.store().cases().unwrap(),
    )
    .unwrap();
    assert_eq!(
        s.finish(&p, &m).unwrap().left,
        vec![Left::Decisions, Left::Entry, Left::Cases]
    );
    assert_eq!(
        fs::read(s.dir.join("decisions.md")).unwrap(),
        CANDIDATE.as_bytes()
    );
    let text = fs::read_to_string(s.dir.join("measurements.md")).unwrap();
    let heading = format!("## 2026-03-04: refresh ({})", p.id);
    assert_eq!(text.matches(&heading).count(), 1);
    assert!(text.find(&heading) < text.find("## Reports"));
    assert!(text.contains("1 of 1 new tests pass; 1 of 1 earlier tests held."));
    assert!(s.cases().iter().all(|c| c.status == Status::Active));
    assert_eq!(s.calls(), 2, "choice answers require no grading call");
}
#[test]
fn revert_for_new_failure_never_rewrites_decisions_and_unblocks_a_new_draft() {
    let s = Setup::new();
    let p = s.approved(&CANDIDATE.replace("ADJUST", "RECONSIDER"));
    let modified = fs::metadata(s.dir.join("decisions.md"))
        .unwrap()
        .modified()
        .unwrap();
    let m = done(&s, &p);
    assert_eq!(m.outcome, KeepOrRevert::Revert);
    assert_eq!(
        m.reason,
        "1 of the 1 new test still didn't pass, so the change was undone."
    );
    assert_eq!(
        fs::read(s.dir.join("decisions.md")).unwrap(),
        BASE.as_bytes()
    );
    assert_eq!(
        fs::metadata(s.dir.join("decisions.md"))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert_eq!(s.cases()[0].status, Status::New);
    assert!(
        fs::read_to_string(s.dir.join("measurements.md"))
            .unwrap()
            .contains("**Outcome:** undone.")
    );
    let before = s.snapshot();
    let calls = s.calls();
    let saved = s.measure(&p.id).unwrap();
    assert!(saved.already);
    assert_eq!(saved.measurement, m);
    assert_eq!(saved.proposal, p);
    assert!(
        s.finish(&saved.proposal, &saved.measurement)
            .unwrap()
            .left
            .is_empty()
    );
    assert_eq!(s.calls(), calls);
    assert_eq!(s.snapshot(), before);
    assert!(refresh::open(&s.store(), &s.dir).unwrap().is_none());
    let python = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .unwrap();
    fs::write(
        s.stub.join("claude"),
        format!(
            "#!{} -I\n{}",
            String::from_utf8(python.stdout).unwrap().trim(),
            include_str!("fixtures/refresh_stub.py")
        ),
    )
    .unwrap();
    let session = Session::open(&s.f.home, &s.f.member.slug, Cap::default()).unwrap();
    refresh::propose(
        &Claude::at(s.stub.join("claude")),
        &session,
        &s.f.home,
        &s.store(),
        &s.f.member,
        &s.dir,
        &s.filters(),
        "sonnet",
        NOW,
    )
    .unwrap();
    let prompt = s.prompt(2);
    assert!(prompt.contains("The person said no to these earlier drafts, or they were tested and undone; propose something else"));
    assert!(prompt.contains(&p.summary));
}
#[test]
fn regression_reverts_even_when_every_new_test_passes() {
    let s = Setup::new();
    let base = BASE.replace("Keep the first step small.", "ADJUST the first step.");
    fs::write(s.dir.join("decisions.md"), &base).unwrap();
    let mut cases = s.cases();
    cases[0].expected = "STAY with the original choice.".into();
    let mut earlier = cases[0].clone();
    earlier.id = "case-2026-03-04-002".into();
    earlier.expected = "ADJUST the approach.".into();
    earlier.status = Status::Active;
    cases.push(earlier);
    s.replace_cases(&cases);
    let mut p = s.proposal(CANDIDATE);
    p.base = version(base.as_bytes());
    p.changes[0].diff = diff::unified("decisions.md", &base, BASE).unwrap();
    s.rewrite(&p);
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).unwrap();
    let m = done(&s, &p);
    assert_eq!(m.outcome, KeepOrRevert::Revert);
    assert_eq!(
        m.reason,
        "1 earlier test that passed before failed with the change, so it was undone."
    );
    assert_eq!(
        fs::read_to_string(s.dir.join("decisions.md")).unwrap(),
        base
    );
}
#[test]
fn rubric_grades_are_blind_deduplicated_and_answers_hide_expected_and_signals() {
    let s = Setup::new();
    let mut cases = s.cases();
    let mut rubric = cases[0].clone();
    rubric.id = "case-2026-03-04-002".into();
    rubric.options = None;
    rubric.r#match = Match::Rubric;
    rubric.expected = "RUBRIC_SENTINEL accept a smaller opening.".into();
    rubric.situation = "SITUATION_SENTINEL consider alternatives.".into();
    rubric.status = Status::Active;
    cases.push(rubric);
    s.replace_cases(&cases);
    let p = s.approved(CANDIDATE);
    let m = done(&s, &p);
    assert_eq!(m.outcome, KeepOrRevert::Keep);
    assert_eq!(s.calls(), 3);
    for n in 0..2 {
        let prompt = s.prompt(n);
        assert!(!prompt.contains("RUBRIC_SENTINEL"));
        assert!(!prompt.contains("\"expected\""));
        assert!(!prompt.contains("\"match\""));
        let rows: Vec<Value> = prompt
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        assert_eq!(rows[0]["options"], json!(cases[0].options));
        for signal in s.store().signals().unwrap() {
            assert!(!prompt.contains(&signal.wanted));
        }
    }
    let prompt = s.prompt(2);
    assert!(prompt.starts_with("Grade each answer"));
    assert!(prompt.contains("RUBRIC_SENTINEL"));
    assert!(prompt.contains("Offer a smaller first step."));
    for forbidden in [
        &cases[1].id,
        "SITUATION_SENTINEL",
        "Keep the first step small.",
        "ADJUST the first step.",
        "base",
        "candidate",
    ] {
        assert!(!prompt.contains(forbidden), "{forbidden}: {prompt}");
    }
    let rows: Vec<Value> = prompt
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "item-1");
}
fn refusal(mode: &str) {
    let s = Setup::new();
    let mut p = s.proposal(CANDIDATE);
    if mode != "unanswered" && mode != "unknown" {
        refresh::answer(
            &s.store(),
            &s.dir,
            &p.id,
            if mode == "rejected" {
                Decision::Rejected
            } else {
                Decision::Approved
            },
            NOW,
        )
        .unwrap();
    }
    match mode {
        "role" => {
            let mut a = s.store().approvals().unwrap().remove(0);
            a.role = GivenBy::Staff;
            write_private_at(
                &s.f.home,
                &s.f.member.slug,
                Some("growth"),
                &format!("{}.json", a.id),
                &serde_json::to_vec(&a).unwrap(),
            )
            .unwrap();
        }
        "moved" => fs::write(s.dir.join("decisions.md"), format!("{BASE}\n")).unwrap(),
        "active" => {
            let mut c = s.cases().remove(0);
            c.status = Status::Active;
            s.store().append_case(&c).unwrap();
        }
        "diff" => p.changes[0].diff = diff::unified("decisions.md", "absent\n", "other\n").unwrap(),
        "filter" => {
            p.changes[0].diff = diff::unified(
                "decisions.md",
                BASE,
                &format!("{BASE}zebracorn@example.invalid\n"),
            )
            .unwrap()
        }
        "quote" | "plusplus" => {
            let mut c = s.cases().remove(0);
            c.options = None;
            c.r#match = Match::Rubric;
            c.expected = "amber birch cedar dogwood elm fir".into();
            s.replace_cases(&[c]);
            p.changes[0].diff = diff::unified(
                "decisions.md",
                BASE,
                &format!(
                    "{BASE}{}amber birch cedar dogwood elm fir\n",
                    if mode == "plusplus" { "++ " } else { "" }
                ),
            )
            .unwrap();
        }
        "hidden" => {
            p.changes[0].diff =
                diff::unified("decisions.md", BASE, &format!("{BASE}hidden\u{202e}\n")).unwrap()
        }
        "file" => p.changes[0].file = "other.md".into(),
        "unchanged" => p.changes[0].diff = diff::unified("decisions.md", BASE, BASE).unwrap(),
        "oversize" => {
            p.changes[0].diff = diff::unified(
                "decisions.md",
                BASE,
                &format!("{BASE}{}", format!("{}\n", "x".repeat(500)).repeat(600)),
            )
            .unwrap()
        }
        "long" => p.summary = "x".repeat(513),
        "damaged" => p.summary.push('\u{200b}'),
        "preflight" => fs::create_dir(s.dir.join("measurements.md")).unwrap(),
        _ => {}
    }
    s.rewrite(&p);
    let before = s.snapshot();
    let err = s
        .measure(if mode == "unknown" {
            "prop-missing"
        } else {
            &p.id
        })
        .expect_err(mode)
        .to_string();
    let expected = match mode {
        "unknown" => "No saved change",
        "unanswered" => "still waiting for your answer",
        "rejected" => "You rejected that change",
        "moved" => "Draft a new change",
        "damaged" | "hidden" => persona_builder_grow::text::DAMAGED_PROPOSAL,
        "active" => "Some of the tests this change was meant to pass have changed",
        _ => "",
    };
    assert!(!err.contains("not implemented"), "{mode}: {err}");
    assert!(err.contains(expected), "{mode}: {err}");
    assert!(!err.contains("zebracorn"));
    assert_eq!(s.calls(), 0, "{mode}");
    assert_eq!(s.snapshot(), before, "{mode}");
}
macro_rules! refusal_tests {
    ($($name:ident => $mode:literal),+ $(,)?) => {$(#[test] fn $name() { refusal($mode); })+};
}
refusal_tests! {
    refusal_unknown => "unknown",
    refusal_unanswered => "unanswered",
    refusal_rejected => "rejected",
    refusal_role => "role",
    refusal_moved => "moved",
    refusal_active => "active",
    refusal_diff => "diff",
    refusal_filter => "filter",
    refusal_quote => "quote",
    refusal_plusplus => "plusplus",
    refusal_hidden => "hidden",
    refusal_file => "file",
    refusal_unchanged => "unchanged",
    refusal_oversize => "oversize",
    refusal_long => "long",
    refusal_damaged => "damaged",
    refusal_preflight => "preflight",
}
#[test]
fn changed_decisions_during_answers_refuse_before_saving_measurement() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    s.plan(json!({"0":"move","decisions_path":s.dir.join("decisions.md")}));
    let error = s
        .measure(&p.id)
        .expect_err("moved while testing")
        .to_string();
    assert_eq!(
        error,
        "Your assistant's decisions changed while the change was being tested, so nothing was saved. Draft a new one with `persona-builder refresh propose`."
    );
    assert!(s.store().measurements().unwrap().is_empty());
    assert!(!s.dir.join("measurements.md").exists());
    assert_eq!(s.cases()[0].status, Status::New);
}
#[test]
fn each_answer_misfit_is_told_back_once_and_good_retry_is_kept() {
    for mode in ["bad", "missing", "hidden"] {
        let s = Setup::new();
        let mut cases = s.cases();
        let mut open = cases[0].clone();
        open.id = "case-2026-03-04-002".into();
        open.r#match = Match::Rubric;
        open.options = None;
        open.expected = "Use a smaller step.".into();
        open.status = Status::Active;
        cases.push(open);
        s.replace_cases(&cases);
        let p = s.approved(CANDIDATE);
        s.plan(json!({"0":mode}));
        let m = done(&s, &p);
        assert_eq!(m.outcome, KeepOrRevert::Keep, "{mode}");
        assert_eq!(s.calls(), 4, "{mode}");
        let retry = s.prompt(1);
        assert!(retry.contains("Your last answer didn't fit"));
        assert!(!retry.contains("private sentinel"));
    }
}
#[test]
fn two_misfits_refuse_without_echoing_open_answer_or_saving() {
    for mode in ["bad", "missing", "hidden"] {
        let s = Setup::new();
        if mode == "hidden" {
            let mut c = s.cases().remove(0);
            c.r#match = Match::Rubric;
            c.options = None;
            s.replace_cases(&[c]);
        }
        let p = s.approved(CANDIDATE);
        s.plan(json!({"0":mode,"1":mode}));
        let error = s.measure(&p.id).expect_err("two misfits").to_string();
        assert!(
            error.contains("Claude's answers didn't fit twice, so nothing was saved ("),
            "{mode}: {error}"
        );
        assert!(error.contains("Run `persona-builder refresh apply` again."));
        assert!(!error.contains("private sentinel"));
        assert!(!error.contains("not an offered option"));
        assert_eq!(s.calls(), 2);
        assert!(s.store().measurements().unwrap().is_empty());
        assert_eq!(
            fs::read(s.dir.join("decisions.md")).unwrap(),
            BASE.as_bytes()
        );
    }
}
#[test]
fn failed_call_saves_nothing_and_retry_reuses_successful_cached_answers() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    s.plan(json!({"1":"error"}));
    assert!(s.measure(&p.id).is_err());
    assert_eq!(s.calls(), 2);
    assert!(s.store().measurements().unwrap().is_empty());
    s.plan(json!({}));
    let m = s.measured(&p.id);
    assert_eq!(m.outcome, KeepOrRevert::Keep);
    assert_eq!(s.calls(), 3, "successful first answer must be cached");
}
#[test]
fn finish_resumes_candidate_without_entry_and_does_not_rewrite_decisions() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let m = s.measured(&p.id);
    fs::write(s.dir.join("decisions.md"), CANDIDATE).unwrap();
    let modified = fs::metadata(s.dir.join("decisions.md"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(
        s.finish(&p, &m).unwrap().left,
        vec![Left::Entry, Left::Cases]
    );
    assert_eq!(
        fs::metadata(s.dir.join("decisions.md"))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert_eq!(s.cases()[0].status, Status::Active);
    let entry = fs::read_to_string(s.dir.join("measurements.md")).unwrap();
    assert_eq!(
        entry
            .matches(&format!("## 2026-03-04: refresh ({})", p.id))
            .count(),
        1
    );
}
#[test]
fn full_apply_is_idempotent_even_after_named_tests_became_active() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let m = done(&s, &p);
    let before = s.snapshot();
    let modified = fs::metadata(s.dir.join("decisions.md"))
        .unwrap()
        .modified()
        .unwrap();
    let saved = s.measure(&p.id).unwrap();
    assert!(saved.already);
    assert_eq!(saved.measurement, m);
    assert!(s.finish(&p, &m).unwrap().left.is_empty());
    assert_eq!(s.calls(), 2);
    assert_eq!(s.snapshot(), before);
    assert_eq!(
        fs::metadata(s.dir.join("decisions.md"))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
}
#[test]
fn finish_refuses_a_third_version_without_writing_entry_or_cases() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let m = s.measured(&p.id);
    fs::write(s.dir.join("decisions.md"), "# Changed afterwards\n").unwrap();
    let before = s.snapshot();
    let error = s.finish(&p, &m).unwrap_err().to_string();
    assert_eq!(
        error,
        "Your assistant's decisions.md changed after this change was tested, so it wasn't applied. Draft a new one with `persona-builder refresh propose`."
    );
    assert_eq!(s.snapshot(), before);
}
#[test]
fn finish_resumes_base_with_existing_entry_and_rejects_wrong_candidate_hash() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let m = s.measured(&p.id);
    fs::write(
        s.dir.join("measurements.md"),
        format!(
            "# Measurements\n\n## 2026-03-04: refresh ({})\n\nAlready recorded.\n",
            p.id
        ),
    )
    .unwrap();
    let previous = fs::read(s.dir.join("measurements.md")).unwrap();
    assert_eq!(
        s.finish(&p, &m).unwrap().left,
        vec![Left::Decisions, Left::Cases]
    );
    assert_eq!(fs::read(s.dir.join("measurements.md")).unwrap(), previous);
    assert_eq!(
        fs::read(s.dir.join("decisions.md")).unwrap(),
        CANDIDATE.as_bytes()
    );
    fs::write(s.dir.join("decisions.md"), BASE).unwrap();
    let mut damaged = p.clone();
    damaged.changes[0].diff =
        diff::unified("decisions.md", BASE, &format!("{BASE}Wrong result.\n")).unwrap();
    let before = s.snapshot();
    assert!(s.finish(&damaged, &m).is_err());
    assert_eq!(s.snapshot(), before);
}
#[test]
fn failed_file_write_keeps_measurement_and_resume_needs_no_more_calls() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let m = s.measured(&p.id);
    fs::create_dir(s.dir.join("measurements.md")).unwrap();
    assert!(s.finish(&p, &m).is_err());
    assert_eq!(s.store().measurements().unwrap(), vec![m.clone()]);
    fs::remove_dir(s.dir.join("measurements.md")).unwrap();
    let saved = s.measure(&p.id).unwrap();
    assert!(saved.already);
    assert_eq!(saved.measurement, m);
    assert_eq!(s.calls(), 2);
    s.finish(&p, &m).unwrap();
    assert_eq!(s.cases()[0].status, Status::Active);
}
#[test]
fn growth_counts_both_outcomes_and_uses_newest_current_version_grades() {
    let s = Setup::new();
    assert_eq!(apply::growth(&s.store(), None).unwrap(), None);
    let p = s.approved(CANDIDATE);
    let first = done(&s, &p);
    assert_eq!(
        apply::growth(&s.store(), None).unwrap(),
        Some("Since the build: 1 change kept and 0 undone. Your assistant passes 1 of 1 tests from your answers.".into())
    );
    let mut second = first.clone();
    second.id = "meas-2026-03-05-001".into();
    second.at = "2026-03-05T00:00:00Z".into();
    second.outcome = KeepOrRevert::Revert;
    second.results[0].before = Grade::Fail;
    second.results[0].after = Grade::Pass;
    s.store().save_measurement(&second).unwrap();
    assert_eq!(apply::growth(&s.store(),None).unwrap(),Some("Since the build: 1 change kept and 1 undone. Your assistant passes 0 of 1 tests from your answers.".into()));
    assert_eq!(apply::growth(&s.store(), Some("2026-03-06")).unwrap(), None);
    assert_eq!(
        apply::growth_counts(&s.store(), None).unwrap(),
        Some(apply::Growth {
            kept: 1,
            undone: 1,
            passing: 0,
            of: 1
        })
    );
    assert_eq!(
        apply::growth_counts(&s.store(), Some("2026-03-05")).unwrap(),
        Some(apply::Growth {
            kept: 0,
            undone: 1,
            passing: 0,
            of: 1
        })
    );
    assert_eq!(
        apply::growth_counts(&s.store(), Some("2026-03-06")).unwrap(),
        None
    );
}

#[test]
fn resume_rechecks_saved_measurement_and_proposal() {
    for mode in ["failed", "approval", "base", "quote"] {
        let s = Setup::new();
        let mut cases = s.cases();
        cases[0].options = None;
        cases[0].r#match = Match::Rubric;
        cases[0].expected = "amber birch cedar dogwood elm fir".into();
        s.replace_cases(&cases);
        let mut p = s.approved(CANDIDATE);
        let mut m = s.measured(&p.id);
        assert_eq!(m.outcome, KeepOrRevert::Keep);
        match mode {
            "failed" => m.results[0].after = Grade::Fail,
            "approval" => m.approval = "appr-2026-03-04-999".into(),
            "base" => m.base = version(b"another base"),
            "quote" => {
                p.changes[0].diff = diff::unified(
                    "decisions.md",
                    BASE,
                    &format!("{CANDIDATE}++ amber birch cedar dogwood elm fir\n"),
                )
                .unwrap();
                s.rewrite(&p);
            }
            _ => unreachable!(),
        }
        write_private_at(
            &s.f.home,
            &s.f.member.slug,
            Some("growth"),
            &format!("{}.json", m.id),
            &serde_json::to_vec(&m).unwrap(),
        )
        .unwrap();
        let before = s.snapshot();
        let calls = s.calls();
        assert_eq!(
            s.measure(&p.id).expect_err(mode).to_string(),
            persona_builder_grow::text::CHANGE_DOES_NOT_FIT,
            "{mode}"
        );
        assert_eq!(s.calls(), calls, "{mode}");
        assert_eq!(s.snapshot(), before, "{mode}");
    }
}

#[test]
fn finish_recomputes_reason_from_results() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let mut m = s.measured(&p.id);
    m.reason = "Untrusted stored sentence.".into();
    write_private_at(
        &s.f.home,
        &s.f.member.slug,
        Some("growth"),
        &format!("{}.json", m.id),
        &serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    let saved = s.measure(&p.id).unwrap();
    assert!(saved.already);
    assert_eq!(saved.measurement, m);
    s.finish(&saved.proposal, &saved.measurement).unwrap();
    let entry = fs::read_to_string(s.dir.join("measurements.md")).unwrap();
    assert!(entry.contains(persona_builder_grow::text::KEEP_REASON));
    assert!(!entry.contains(&m.reason));
}

#[test]
fn finish_refuses_missing_named_case_before_any_write() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let m = s.measured(&p.id);
    s.replace_cases(&[]);
    let before = s.snapshot();
    let error = s
        .finish(&p, &m)
        .expect_err("missing named test")
        .to_string();
    assert_eq!(
        error,
        persona_builder_grow::text::rule(
            &p.cases[0],
            persona_builder_grow::text::MISSING_REFERENCE
        )
    );
    assert_eq!(s.snapshot(), before);
}

#[test]
fn duplicate_approvals_refuse_before_any_call() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let mut a = s.store().approvals().unwrap().remove(0);
    a.id = "appr-2026-03-04-002".into();
    write_private_at(
        &s.f.home,
        &s.f.member.slug,
        Some("growth"),
        &format!("{}.json", a.id),
        &serde_json::to_vec(&a).unwrap(),
    )
    .unwrap();
    let before = s.snapshot();
    assert_eq!(
        s.measure(&p.id)
            .expect_err("duplicate approval")
            .to_string(),
        persona_builder_grow::text::rule(&p.id, persona_builder_grow::text::ALREADY_APPROVED)
    );
    assert_eq!(s.calls(), 0);
    assert_eq!(s.snapshot(), before);
}

#[test]
fn earlier_drafts_include_plus_content_without_file_headers() {
    let s = Setup::new();
    let p = s.proposal(&format!("{CANDIDATE}++ reconsider this approach\n"));
    let prompt = refresh::refresh_prompt(&[], BASE, &s.cases(), &[], &[p]);
    assert!(prompt.contains("\n+++ reconsider this approach\n"));
    assert!(!prompt.contains("+++ b/decisions.md"));
}

#[test]
fn stray_carriage_return_on_an_added_line_refuses_before_any_call() {
    let s = Setup::new();
    let mut p = s.approved(CANDIDATE);
    p.changes[0].diff = p.changes[0].diff.replace(
        "+- ADJUST the first step.\n",
        "+- ADJUST the first step.\r\n",
    );
    s.rewrite(&p);
    let before = s.snapshot();
    assert_eq!(
        s.measure(&p.id)
            .expect_err("hidden added character")
            .to_string(),
        persona_builder_grow::text::CHANGE_DOES_NOT_FIT
    );
    assert_eq!(s.calls(), 0);
    assert_eq!(s.snapshot(), before);
}

#[test]
fn crlf_decisions_keep_their_line_endings() {
    let s = Setup::new();
    let base = BASE.replace('\n', "\r\n");
    let candidate = CANDIDATE.replace('\n', "\r\n");
    fs::write(s.dir.join("decisions.md"), &base).unwrap();
    let p = s.proposal_from(&base, &candidate);
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).unwrap();
    let m = s.measured(&p.id);
    assert_eq!(m.outcome, KeepOrRevert::Keep);
    s.finish(&p, &m).unwrap();
    assert_eq!(
        fs::read(s.dir.join("decisions.md")).unwrap(),
        candidate.as_bytes()
    );
}

#[test]
fn resume_rechecks_an_undone_result_by_the_keep_rule() {
    let s = Setup::new();
    let p = s.approved(CANDIDATE);
    let mut m = s.measured(&p.id);
    assert_eq!(m.outcome, KeepOrRevert::Keep);
    m.outcome = KeepOrRevert::Revert;
    write_private_at(
        &s.f.home,
        &s.f.member.slug,
        Some("growth"),
        &format!("{}.json", m.id),
        &serde_json::to_vec(&m).unwrap(),
    )
    .unwrap();
    let before = s.snapshot();
    let calls = s.calls();
    assert_eq!(
        s.measure(&p.id)
            .expect_err("forged undone result")
            .to_string(),
        persona_builder_grow::text::CHANGE_DOES_NOT_FIT
    );
    assert_eq!(s.calls(), calls);
    assert_eq!(s.snapshot(), before);
}

#[test]
fn progress_describes_each_test_phase_and_is_silent_when_already_measured() {
    let s = Setup::new();
    let mut cases = s.cases();
    cases[0].r#match = Match::Rubric;
    cases[0].options = None;
    cases[0].expected = "Use a smaller first step.".into();
    s.replace_cases(&cases);
    let p = s.approved(CANDIDATE);
    let mut lines = Vec::new();
    s.measure_progress(&p.id, &mut |line| lines.push(line.to_owned()))
        .unwrap();
    assert_eq!(
        lines,
        vec![
            "Testing your assistant as it is now on 1 test.",
            "Testing it with the change on 1 test.",
            "Checking the written answers against what you wanted.",
        ]
    );
    lines.clear();
    assert!(
        s.measure_progress(&p.id, &mut |line| lines.push(line.to_owned()))
            .unwrap()
            .already
    );
    assert!(lines.is_empty());
}
#[test]
fn waiting_tracks_unanswered_approved_and_measured_changes() {
    let s = Setup::new();
    let p = s.proposal(CANDIDATE);
    assert_eq!(
        refresh::waiting(&s.store(), &s.dir).unwrap(),
        Some((p.clone(), false))
    );
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).unwrap();
    assert_eq!(
        refresh::waiting(&s.store(), &s.dir).unwrap(),
        Some((p.clone(), true))
    );
    s.measured(&p.id);
    assert_eq!(refresh::waiting(&s.store(), &s.dir).unwrap(), None);
}
#[test]
fn locked_answer_matches_answer_without_taking_another_lock() {
    let expected = {
        let s = Setup::new();
        let p = s.proposal(CANDIDATE);
        refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).unwrap()
    };
    let s = Setup::new();
    let p = s.proposal(CANDIDATE);
    let store = s.store();
    let _lock = persona_builder_ingest::private::lock_state(&s.f.home, &s.f.member.slug).unwrap();
    let actual = refresh::answer_locked(&store, &s.dir, &p.id, Decision::Approved, NOW);
    assert!(
        actual.is_ok(),
        "locked answer must reuse the caller's lock: {actual:?}"
    );
    assert_eq!(actual.unwrap(), expected);
}
