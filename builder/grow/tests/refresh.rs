mod common;
use common::*;
use persona_builder_agent::{
    claude::Claude,
    ledger::{Cap, Session},
};
use persona_builder_grow::{
    diff,
    keep::keep,
    record::*,
    refresh::{self, Proposed},
    store::Store,
    version::version,
};
use persona_builder_ingest::{
    filters::Filters,
    private::{lock_state, write_private_at},
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard},
};

const BASE: &str = "# Decisions\n\n## By kind of question\n\n### communication\n\n**The lean:**\n- Keep the first step small.\n\n**Standing rules:**\n- None yet.\n\n**Precedents:**\n- None yet.\n";
static SUBPROCESS_TEST: Mutex<()> = Mutex::new(());
struct Setup {
    f: Fixture,
    dir: PathBuf,
    stub: PathBuf,
    _serial: MutexGuard<'static, ()>,
}
impl Setup {
    fn new(count: usize) -> Self {
        // Other test threads spawning children can briefly inherit a held flock.
        let serial = SUBPROCESS_TEST
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
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
                include_str!("fixtures/refresh_stub.py")
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
        for _ in 0..count {
            keep(
                &s.store(),
                &s.f.member,
                &s.filters(),
                &serde_json::to_vec(&input()).unwrap(),
                NOW,
            )
            .unwrap();
        }
        s
    }
    fn store(&self) -> Store {
        Store::open(&self.f.home, &self.f.member).unwrap()
    }
    fn filters(&self) -> Filters {
        Filters::compile(&self.f.member.filters).unwrap()
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
    fn plan(&self, plan: Value) {
        fs::write(self.stub.join("plan.json"), plan.to_string()).unwrap();
    }
    fn propose(&self) -> persona_builder::error::Result<Proposed> {
        let session = Session::open(&self.f.home, &self.f.member.slug, Cap::default()).unwrap();
        refresh::propose(
            &Claude::at(self.stub.join("claude")),
            &session,
            &self.f.home,
            &self.store(),
            &self.f.member,
            &self.dir,
            &self.filters(),
            "sonnet",
            NOW,
        )
    }
    fn drafted(&self) -> Proposal {
        match self.propose().unwrap() {
            Proposed::Drafted(p) => p,
            Proposed::Nothing => panic!("expected a change"),
        }
    }
    fn unchanged(&self) {
        assert_eq!(
            fs::read_to_string(self.dir.join("decisions.md")).unwrap(),
            BASE
        );
        assert_eq!(fs::read_dir(&self.dir).unwrap().count(), 1);
    }
    fn asked(&self) -> (Vec<Case>, Vec<Signal>) {
        (
            self.store().cases().unwrap().iter().cloned().collect(),
            self.store().signals().unwrap(),
        )
    }
    fn check(
        &self,
        reply: &Value,
    ) -> Result<Vec<persona_builder_grow::edit::Edit>, persona_builder_build::build::Misfit> {
        let (cases, signals) = self.asked();
        refresh::check_reply(
            &self.f.member.codebook.as_ref().unwrap().categories,
            BASE,
            &cases,
            &signals,
            &self.filters(),
            reply,
        )
    }
}
fn reply() -> Value {
    json!({"summary":"Start small and adjust together.","edits":[{"area":"communication","kind":"add","old":null,"text":"Offer a small reversible first step.","why":"Leave room for later choices.","cases":["case-2026-03-04-001"]}]})
}
#[test]
fn saves_private_change_and_proves_each_diff_without_touching_member() {
    let s = Setup::new(2);
    let p = s.drafted();
    assert_eq!(p.schema, PROPOSAL_V1);
    assert_eq!(p.persona_kind, PersonaKind::Person);
    assert_eq!(p.base, version(BASE.as_bytes()));
    assert_eq!(p.changes.len(), 2);
    assert_eq!(p.cases, vec!["case-2026-03-04-001", "case-2026-03-04-002"]);
    let path = s.f.growth().join(format!("{}.json", p.id));
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut candidate = BASE.to_string();
    for change in &p.changes {
        assert_eq!(change.file, "decisions.md");
        candidate = diff::apply(&candidate, &change.diff, &change.file).unwrap();
    }
    assert!(candidate.contains("lesson 1") && candidate.contains("lesson 2"));
    assert_eq!(s.store().proposal(&p.id).unwrap(), Some(p.clone()));
    let lines = refresh::render(&p, &s.dir).join("\n");
    for expected in [
        &p.id,
        "2026-03-04",
        &p.summary,
        "1. A smaller",
        "2. A smaller",
        "--- a/decisions.md",
        "2",
        "persona-builder refresh approve",
        "persona-builder refresh reject",
    ] {
        assert!(lines.contains(expected), "missing {expected}");
    }
    assert_eq!(refresh::open(&s.store(), &s.dir).unwrap(), Some(p));
    s.unchanged();
}
#[test]
fn no_new_answers_makes_no_call() {
    let s = Setup::new(0);
    assert!(matches!(s.propose().unwrap(), Proposed::Nothing));
    assert_eq!(s.calls(), 0);
    s.unchanged();
}
#[test]
fn waiting_and_approved_changes_block_calls_but_stale_changes_do_not() {
    let s = Setup::new(1);
    let p = s.drafted();
    assert!(s.propose().err().unwrap().to_string().contains("waiting"));
    assert_eq!(s.calls(), 1);
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).unwrap();
    assert!(s.propose().err().unwrap().to_string().contains("approved"));
    assert_eq!(s.calls(), 1);
    s.unchanged();
    fs::write(s.dir.join("decisions.md"), format!("{BASE}\n")).unwrap();
    assert!(refresh::open(&s.store(), &s.dir).unwrap().is_none());
    assert!(matches!(s.propose().unwrap(), Proposed::Drafted(_)));
}
#[test]
fn every_model_misfit_is_told_back_once_and_retry_is_kept() {
    for mode in ["bad", "quote", "private", "hidden"] {
        let s = Setup::new(1);
        s.plan(json!({"0": mode}));
        s.drafted();
        assert_eq!(s.calls(), 2, "{mode}");
        let retry = fs::read_to_string(s.stub.join("stdin1")).unwrap();
        assert!(retry.contains("Your last answer didn't fit"));
        s.unchanged();
    }
}
#[test]
fn two_misfits_save_nothing_and_do_not_echo_reply_words() {
    for mode in ["bad", "quote", "private", "hidden"] {
        let s = Setup::new(1);
        s.plan(json!({"0":mode,"1":mode}));
        let error = s.propose().err().unwrap().to_string();
        assert!(!error.contains("zebracorn"));
        assert!(!error.contains("make-up"));
        assert_eq!(s.calls(), 2);
        assert!(s.store().proposals().unwrap().is_empty());
        s.unchanged();
    }
}
#[test]
fn failed_call_saves_no_change() {
    let s = Setup::new(1);
    s.plan(json!({"0":"error"}));
    assert!(s.propose().is_err());
    assert!(s.store().proposals().unwrap().is_empty());
    s.unchanged();
}
#[test]
fn changed_base_during_call_refuses_to_save() {
    let s = Setup::new(1);
    s.plan(json!({"0":"move","decisions_path":s.dir.join("decisions.md")}));
    assert!(s.propose().is_err());
    assert!(s.store().proposals().unwrap().is_empty());
}
#[test]
fn answers_save_subject_role_and_second_answer_is_refused() {
    for decision in [Decision::Approved, Decision::Rejected] {
        let s = Setup::new(1);
        let p = s.drafted();
        let a = refresh::answer(&s.store(), &s.dir, &p.id, decision, NOW).unwrap();
        assert_eq!(a.schema, APPROVAL_V1);
        assert_eq!(a.role, GivenBy::Subject);
        assert_eq!(a.decision, decision);
        assert_eq!(a.proposal, p.id);
        assert_eq!(a.note, None);
        assert_eq!(s.store().approval(&a.id).unwrap(), Some(a));
        assert!(refresh::answer(&s.store(), &s.dir, &p.id, decision, NOW).is_err());
        s.unchanged();
    }
}
#[test]
fn answer_requires_lock_and_known_change() {
    let s = Setup::new(1);
    let p = s.drafted();
    {
        let _lock = lock_state(&s.f.home, &s.f.member.slug).unwrap();
        assert!(refresh::answer(&s.store(), &s.dir, &p.id, Decision::Rejected, NOW).is_err());
    }
    assert!(s.store().approvals().unwrap().is_empty());
    assert!(refresh::answer(&s.store(), &s.dir, "prop-missing", Decision::Rejected, NOW).is_err());
    s.unchanged();
}
#[test]
fn approve_requires_current_base_and_new_answers_reject_does_not() {
    for stale in [true, false] {
        let s = Setup::new(1);
        let p = s.drafted();
        if stale {
            fs::write(s.dir.join("decisions.md"), format!("{BASE}\n")).unwrap();
        } else {
            let mut case = s.asked().0.remove(0);
            case.status = Status::Active;
            s.store().append_case(&case).unwrap();
        }
        assert!(refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).is_err());
        assert!(refresh::answer(&s.store(), &s.dir, &p.id, Decision::Rejected, NOW).is_ok());
    }
}
#[test]
fn damaged_stored_text_is_never_shown() {
    for place in ["summary", "why", "diff"] {
        let s = Setup::new(1);
        let mut p = s.drafted();
        match place {
            "summary" => p.summary.push('\u{200b}'),
            "why" => p.changes[0].why.push('\u{202e}'),
            _ => p.changes[0].diff.push('\x1b'),
        }
        write_private_at(
            &s.f.home,
            &s.f.member.slug,
            Some("growth"),
            &format!("{}.json", p.id),
            &serde_json::to_vec(&p).unwrap(),
        )
        .unwrap();
        assert!(refresh::show(&s.store(), &s.dir, Some(&p.id)).is_err());
        assert!(refresh::open(&s.store(), &s.dir).is_err());
        s.unchanged();
    }
}
#[test]
fn reply_validation_covers_fields_counts_areas_old_and_case_links() {
    let s = Setup::new(1);
    assert!(s.check(&reply()).is_ok());
    let reject = |v: Value| {
        let bad = s.check(&v).unwrap_err();
        assert!(!bad.shown.contains("zebracorn"), "{}", bad.shown);
    };
    for field in ["summary", "text", "why", "old"] {
        for bad in [
            "".to_owned(),
            "   ".into(),
            "zebracorn\nnext".into(),
            " zebracorn".into(),
            "zebracorn ".into(),
            "zebracorn\u{200b}".into(),
            "x".repeat(401),
        ] {
            let mut v = reply();
            if field == "summary" {
                v[field] = json!(bad);
            } else {
                if field == "old" {
                    v["edits"][0]["kind"] = json!("replace");
                }
                v["edits"][0][field] = json!(bad);
            }
            reject(v);
        }
    }
    let mut v = reply();
    v["edits"] = json!([]);
    reject(v);
    let mut v = reply();
    v["edits"] = Value::Array(
        (0..11)
            .map(|n| {
                let mut e = v["edits"][0].clone();
                e["text"] = json!(format!("Distinct rule number {n}."));
                e
            })
            .collect(),
    );
    reject(v);
    for (field, bad) in [
        ("area", json!("zebracorn")),
        ("kind", json!("unknown")),
        ("old", json!("Keep the first step small.")),
        ("cases", json!([])),
        ("cases", json!(["case-zebracorn"])),
        (
            "cases",
            json!(["case-2026-03-04-001", "case-2026-03-04-001"]),
        ),
    ] {
        let mut v = reply();
        v["edits"][0][field] = bad;
        reject(v);
    }
    let mut v = reply();
    v["edits"][0]["kind"] = json!("replace");
    reject(v);
    let mut v = reply();
    v["edits"][0]["kind"] = json!("replace");
    v["edits"][0]["old"] = json!("Absent zebracorn item.");
    reject(v);
    let mut v = reply();
    v["edits"][0]["kind"] = json!("replace");
    v["edits"][0]["old"] = json!("Keep the first step small.");
    assert!(s.check(&v).is_ok());
    let mut duplicate = v["edits"][0].clone();
    duplicate["old"] = duplicate["text"].clone();
    duplicate["text"] = json!("A third possible rule.");
    v["edits"].as_array_mut().unwrap().push(duplicate);
    reject(v);
}
#[test]
fn all_output_places_pass_privacy_filters() {
    let s = Setup::new(1);
    for field in ["summary", "text", "why"] {
        let mut v = reply();
        if field == "summary" {
            v[field] = json!("zebracorn@example.invalid");
        } else {
            v["edits"][0][field] = json!("zebracorn@example.invalid");
        }
        let misfit = s.check(&v).unwrap_err();
        assert!(!misfit.shown.contains("zebracorn"));
    }
}
#[test]
fn six_word_runs_from_situation_expected_and_signal_answer_are_caught() {
    let s = Setup::new(1);
    for origin in ["situation", "expected", "wanted", "persona_said"] {
        let (mut cases, mut signals) = s.asked();
        let words = "zebracorn amber birch cedar dogwood elm";
        match origin {
            "situation" => cases[0].situation = words.into(),
            "expected" => cases[0].expected = words.into(),
            "wanted" => signals[0].wanted = words.into(),
            _ => signals[0].persona_said = Some(words.into()),
        }
        for field in ["summary", "text", "why"] {
            let mut v = reply();
            if field == "summary" {
                v[field] = json!(words);
            } else {
                v["edits"][0][field] = json!(words);
            }
            let misfit = refresh::check_reply(
                &s.f.member.codebook.as_ref().unwrap().categories,
                BASE,
                &cases,
                &signals,
                &s.filters(),
                &v,
            )
            .unwrap_err();
            assert!(!misfit.shown.contains("zebracorn"));
        }
    }
}
#[test]
fn prompt_has_complete_records_and_rejected_drafts_but_no_approval() {
    let s = Setup::new(1);
    let p = s.drafted();
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Rejected, NOW).unwrap();
    let (cases, signals) = s.asked();
    let prompt = refresh::refresh_prompt(
        &s.f.member.codebook.as_ref().unwrap().categories,
        BASE,
        &cases,
        &signals,
        std::slice::from_ref(&p),
    );
    for expected in [
        BASE,
        &cases[0].id,
        &cases[0].situation,
        &cases[0].expected,
        signals[0].persona_said.as_ref().unwrap(),
        &p.summary,
        "The person said no to these earlier drafts; propose something else",
    ] {
        assert!(prompt.contains(expected), "missing {expected}");
    }
    assert!(!prompt.contains(APPROVAL_V1));
    assert!(!prompt.contains("appr-"));
    assert!(!prompt.contains("\"decision\""));
    s.unchanged();
}
#[test]
fn schema_closes_both_objects_and_requires_every_key() {
    let schema = refresh::refresh_schema();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"].as_array().unwrap().len(), 2);
    let edit = &schema["properties"]["edits"]["items"];
    assert_eq!(edit["additionalProperties"], false);
    assert_eq!(edit["required"].as_array().unwrap().len(), 6);
}
#[test]
fn old_line_is_checked_independently_even_when_it_matches_a_stored_item() {
    let s = Setup::new(1);
    let (cases, signals) = s.asked();
    for old in [
        "zebracorn\u{200b}".to_owned(),
        "zebracorn\r".into(),
        "x".repeat(401),
    ] {
        let decisions = BASE.replace("Keep the first step small.", &old);
        let mut v = reply();
        v["edits"][0]["kind"] = json!("replace");
        v["edits"][0]["old"] = json!(old);
        assert!(
            refresh::check_reply(
                &s.f.member.codebook.as_ref().unwrap().categories,
                &decisions,
                &cases,
                &signals,
                &s.filters(),
                &v
            )
            .is_err()
        );
    }
}
#[test]
fn only_new_cases_are_asked_in_oldest_order_with_a_limit_of_forty() {
    let s = Setup::new(42);
    let mut cases = s.asked().0;
    cases[0].status = Status::Active;
    s.store().append_case(&cases[0]).unwrap();
    // The final id is oldest despite its append order.
    let mut history = String::new();
    for (n, case) in cases.iter_mut().enumerate() {
        if n == 41 {
            case.created = "2026-03-01T00:00:00Z".into();
        }
        history.push_str(&to_line(case));
    }
    write_private_at(
        &s.f.home,
        &s.f.member.slug,
        Some("growth"),
        "cases.jsonl",
        history.as_bytes(),
    )
    .unwrap();
    s.drafted();
    let prompt = fs::read_to_string(s.stub.join("stdin0")).unwrap();
    let rows: Vec<Value> = prompt
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v.get("wanted").is_some())
        .collect();
    assert_eq!(rows.len(), 40);
    assert_eq!(rows[0]["id"], "case-2026-03-04-042");
    assert_eq!(rows[1]["id"], "case-2026-03-04-002");
    assert!(
        !rows
            .iter()
            .any(|r| r["id"] == "case-2026-03-04-001" || r["id"] == "case-2026-03-04-041")
    );
    s.unchanged();
}
#[test]
fn rejected_changes_are_available_to_later_calls_but_approvals_are_not() {
    let s = Setup::new(1);
    let p = s.drafted();
    refresh::answer(&s.store(), &s.dir, &p.id, Decision::Rejected, NOW).unwrap();
    s.drafted();
    let prompt = fs::read_to_string(s.stub.join("stdin1")).unwrap();
    assert!(prompt.contains(&p.summary));
    assert!(prompt.contains("The person said no"));
    assert!(!prompt.contains("appr-"));
    assert!(!prompt.contains(APPROVAL_V1));
    s.unchanged();
}
#[test]
fn missing_and_linked_decisions_refuse_without_calling() {
    let s = Setup::new(1);
    fs::remove_file(s.dir.join("decisions.md")).unwrap();
    assert!(
        s.propose()
            .err()
            .unwrap()
            .to_string()
            .contains("Build your assistant first")
    );
    std::os::unix::fs::symlink(s.stub.join("claude"), s.dir.join("decisions.md")).unwrap();
    assert!(s.propose().is_err());
    assert_eq!(s.calls(), 0);
}
#[test]
fn malformed_reply_objects_and_missing_fields_are_refused() {
    let s = Setup::new(1);
    for field in ["summary", "edits"] {
        let mut v = reply();
        v.as_object_mut().unwrap().remove(field);
        assert!(s.check(&v).is_err());
    }
    for field in ["area", "kind", "old", "text", "why", "cases"] {
        let mut v = reply();
        v["edits"][0].as_object_mut().unwrap().remove(field);
        assert!(s.check(&v).is_err());
    }
    for v in [json!([]), json!(null), json!({"summary":true,"edits":[]})] {
        assert!(s.check(&v).is_err());
    }
    let mut v = reply();
    v["extra"] = json!("zebracorn");
    assert!(s.check(&v).is_err());
    let mut v = reply();
    v["edits"][0]["extra"] = json!("zebracorn");
    assert!(s.check(&v).is_err());
}
#[test]
fn unicode_line_limit_counts_characters_and_not_bytes() {
    let s = Setup::new(1);
    let mut v = reply();
    v["summary"] = json!("é".repeat(400));
    v["edits"][0]["text"] = json!("é".repeat(400));
    assert!(s.check(&v).is_ok());
}
#[test]
fn replacing_the_same_item_twice_is_refused_even_when_first_keeps_its_words() {
    let s = Setup::new(1);
    let mut v = reply();
    v["edits"][0]["kind"] = json!("replace");
    v["edits"][0]["old"] = json!("Keep the first step small.");
    v["edits"][0]["text"] = json!("Keep the first step small.");
    let mut next = v["edits"][0].clone();
    next["text"] = json!("Choose a smaller next step.");
    v["edits"].as_array_mut().unwrap().push(next);
    assert!(s.check(&v).is_err());
}
#[test]
fn showing_existing_less_than_and_noncontrol_unicode_is_allowed() {
    let s = Setup::new(1);
    let mut p = s.drafted();
    p.changes[0].diff="--- a/decisions.md\n+++ b/decisions.md\n@@ -1,1 +1,1 @@\n-old < 3\u{200b}\n+new < 4\u{200b}\n".into();
    write_private_at(
        &s.f.home,
        &s.f.member.slug,
        Some("growth"),
        &format!("{}.json", p.id),
        &serde_json::to_vec(&p).unwrap(),
    )
    .unwrap();
    assert!(refresh::show(&s.store(), &s.dir, Some(&p.id)).is_ok());
    assert!(refresh::render(&p, &s.dir).join("\n").contains("old < 3"));
}
#[test]
fn rendering_damaged_text_returns_only_the_refusal() {
    let s = Setup::new(1);
    let mut p = s.drafted();
    p.summary = "zebracorn\u{200b}".into();
    assert_eq!(
        refresh::render(&p, &s.dir),
        vec![persona_builder_grow::text::DAMAGED_PROPOSAL]
    );
}
#[test]
fn prompt_uses_only_the_five_newest_related_rejections_and_added_lines() {
    let s = Setup::new(1);
    let p = s.drafted();
    let (cases, signals) = s.asked();
    let mut rejected = Vec::new();
    for n in 1..=7 {
        let mut prior = p.clone();
        prior.id = format!("prop-older-{n}");
        prior.created = format!("2026-03-{n:02}T00:00:00Z");
        prior.summary = format!("Earlier draft number {n}.");
        prior.changes[0].diff = format!(
            "--- a/decisions.md\n+++ b/decisions.md\n-Removed marker {n}.\n+Added marker {n}.\n"
        );
        rejected.push(prior);
    }
    let mut unrelated = p;
    unrelated.summary = "Unrelated draft marker.".into();
    unrelated.cases = vec!["case-unrelated".into()];
    unrelated.created = "2026-04-01T00:00:00Z".into();
    rejected.push(unrelated);
    let prompt = refresh::refresh_prompt(
        &s.f.member.codebook.as_ref().unwrap().categories,
        BASE,
        &cases,
        &signals,
        &rejected,
    );
    for n in 3..=7 {
        assert!(prompt.contains(&format!("Earlier draft number {n}.")));
        assert!(prompt.contains(&format!("+Added marker {n}.")));
    }
    for absent in [
        "Earlier draft number 1.",
        "Earlier draft number 2.",
        "Unrelated draft marker.",
        "Removed marker",
        "+++ b/",
    ] {
        assert!(!prompt.contains(absent), "{absent}");
    }
    assert!(prompt.find("Earlier draft number 7.") < prompt.find("Earlier draft number 6."));
}
#[test]
fn an_approved_change_with_a_measurement_no_longer_blocks_refresh() {
    let s = Setup::new(1);
    let p = s.drafted();
    let a = refresh::answer(&s.store(), &s.dir, &p.id, Decision::Approved, NOW).unwrap();
    let measurement = Measurement {
        schema: MEASUREMENT_V1.into(),
        id: "meas-2026-03-04-001".into(),
        proposal: p.id.clone(),
        approval: a.id,
        at: NOW.into(),
        base: p.base.clone(),
        candidate: version(b"candidate"),
        results: vec![CaseResult {
            case: p.cases[0].clone(),
            new: true,
            before: Grade::Fail,
            after: Grade::Pass,
        }],
        outcome: KeepOrRevert::Keep,
        reason: "The checks passed.".into(),
    };
    s.store().save_measurement(&measurement).unwrap();
    assert!(refresh::open(&s.store(), &s.dir).unwrap().is_none());
    assert!(matches!(s.propose().unwrap(), Proposed::Drafted(_)));
    s.unchanged();
}
