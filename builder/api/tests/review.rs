//! Review against invented assistants and isolated Claude Code stubs.
use persona_builder_api::{dto::*, *};
use persona_builder_build::report;
use persona_builder_grow::{apply, diff, record::*, store::Store, text};
use persona_builder_ingest::private::*;
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
static CASES: Mutex<()> = Mutex::new(());
const PERMISSION: &str =
    "Persona Builder needs your permission before it reads anything. Give it in the first step.";
const FIND: &str = "Persona Builder couldn't find that assistant. Close it and open it again.";
const BUSY: &str =
    "Persona Builder is already working on this assistant. Wait for it to finish, then try again.";
const GONE: &str =
    "That change isn't waiting for your answer any more. Close it and open it again.";
const PRIVATE: &str = "That looked personal or secret, so it wasn't kept. Leave out names and private details, then try again.";
struct Home {
    path: PathBuf,
    bin: PathBuf,
}
impl Home {
    fn new(agree: bool) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-review-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let path = path.canonicalize().unwrap();
        new_assistant_in(
            &path,
            NewAssistantRequest {
                name: "Wren".into(),
                pronouns: "she/her".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        if agree {
            agree_consent_in(&path, slug()).unwrap();
        }
        fs::remove_file(path.join(".claude/crew/wren/decisions.md")).unwrap();
        let bin = path.join("bin");
        fs::create_dir(&bin).unwrap();
        Self { path, bin }
    }
    fn dir(&self) -> PathBuf {
        self.path.join(".claude/crew/wren")
    }
    fn state(&self, name: &str) -> PathBuf {
        self.path.join(".claude/state/wren").join(name)
    }
    fn store(&self) -> Store {
        Store::open(
            &self.path,
            &persona_builder::member::read_member(&self.dir().join("member.json")).unwrap(),
        )
        .unwrap()
    }
    fn built(&self) {
        fs::write(self.dir().join("decisions.md"),"# Decisions\n\n## By kind of question\n\n### scope\n\n- Keep each step reversible.\n\n### sequencing\n\n- Start with a small task.\n").unwrap();
    }
    fn stub(&self, apply: bool, transform: impl FnOnce(String) -> String) {
        let out = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let source = if apply {
            include_str!("../../grow/tests/fixtures/apply_stub.py")
        } else {
            include_str!("../../grow/tests/fixtures/refresh_stub.py")
        };
        let p = self.bin.join("claude");
        fs::write(
            &p,
            format!(
                "#!{} -I\n{}",
                String::from_utf8(out.stdout).unwrap().trim(),
                transform(source.into())
            ),
        )
        .unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn calls(&self) -> usize {
        (0..)
            .take_while(|n| self.bin.join(format!("stdin{n}")).exists())
            .count()
    }
    fn keep(&self) -> ReviewStatus {
        keep_answer_in(&self.path, answer()).unwrap()
    }
    fn draft(&self) -> Result<RefreshDrafted, Failure> {
        draft_refresh_in(&self.path, Some(self.bin.as_os_str()), &[], slug())
    }
    fn approve(&self, id: &str, approve: bool) -> Result<ReviewStatus, Failure> {
        answer_refresh_in(
            &self.path,
            RefreshAnswerRequest {
                slug: "wren".into(),
                id: id.into(),
                approve,
            },
        )
    }
    fn apply(
        &self,
        id: &str,
        progress: &mut dyn FnMut(BuildProgress),
    ) -> Result<RefreshApplied, Failure> {
        apply_refresh_in(
            &self.path,
            Some(self.bin.as_os_str()),
            &[],
            RefreshApplyRequest {
                slug: "wren".into(),
                id: id.into(),
            },
            progress,
        )
    }
    fn prepared(&self) -> String {
        self.built();
        self.keep();
        self.stub(false, |s| s);
        self.draft().unwrap().change.unwrap().id
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn slug() -> SlugRequest {
    SlugRequest {
        slug: "wren".into(),
    }
}
fn answer() -> KeepAnswerRequest {
    KeepAnswerRequest {
        slug: "wren".into(),
        area: "scope".into(),
        situation: "Choose the next classroom activity.".into(),
        wanted: "Begin quietly before the group work.".into(),
        assistant_said: "".into(),
    }
}
fn safe<T: std::fmt::Debug>(h: &Home, r: Result<T, Failure>, expected: &str) {
    let m = r.unwrap_err().message;
    assert!(!m.contains(h.path.to_str().unwrap()), "home leaked: {m}");
    assert_eq!(m, expected);
}
#[test]
fn every_command_requires_permission_and_valid_slug_without_writes() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(false);
    for (s, e) in [("wren", PERMISSION), ("missing", FIND), ("../x", FIND)] {
        let req = || SlugRequest { slug: s.into() };
        let before = fs::read_dir(h.state("")).into_iter().flatten().count();
        safe(&h, show_report_in(&h.path, req()), e);
        safe(&h, review_status_in(&h.path, req()), e);
        let mut a = answer();
        a.slug = s.into();
        safe(&h, keep_answer_in(&h.path, a), e);
        safe(&h, draft_refresh_in(&h.path, None, &[], req()), e);
        safe(
            &h,
            answer_refresh_in(
                &h.path,
                RefreshAnswerRequest {
                    slug: s.into(),
                    id: "prop-no".into(),
                    approve: true,
                },
            ),
            e,
        );
        safe(
            &h,
            apply_refresh_in(
                &h.path,
                None,
                &[],
                RefreshApplyRequest {
                    slug: s.into(),
                    id: "prop-no".into(),
                },
                &mut |_| panic!("gate progress"),
            ),
            e,
        );
        assert_eq!(
            fs::read_dir(h.state("")).into_iter().flatten().count(),
            before
        );
    }
}
#[test]
fn report_maps_every_field_and_damage() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    assert_eq!(
        show_report_in(&h.path, slug()).unwrap(),
        ReportView {
            report: None,
            unreadable: false,
            growth: None
        }
    );
    write_private(&h.path, "wren", report::REPORT, b"broken").unwrap();
    assert!(show_report_in(&h.path, slug()).unwrap().unreadable);
    let r = report::Report {
        schema: report::SCHEMA.into(),
        built: "2026-10-24".into(),
        overall: report::Level::GettingThere,
        answered: 170,
        held: 31,
        test: Some(report::Test { right: 29, of: 31 }),
        areas: vec![report::Area {
            area: "scope".into(),
            level: report::Level::Thin,
            examples: 4,
            strengthen: vec![
                report::Strengthen::Interview,
                report::Strengthen::Claudeai,
                report::Strengthen::Time,
            ],
        }],
    };
    report::write(&h.path, "wren", &r).unwrap();
    let v = show_report_in(&h.path, slug()).unwrap();
    assert!(!v.unreadable);
    assert_eq!(
        v.report,
        Some(BuildReport {
            built: r.built,
            overall: ReportLevel::GettingThere,
            answered: 170,
            thin_below: 150,
            held_out: 31,
            test: Some(ReportTest { right: 29, of: 31 }),
            areas: vec![ReportArea {
                area: "scope".into(),
                level: ReportLevel::Thin,
                examples: 4,
                help: vec![
                    ReportHelp::Interview,
                    ReportHelp::Claudeai,
                    ReportHelp::Time
                ]
            }]
        })
    );
}
#[test]
fn keep_validates_and_saves_instruction_and_correction_under_one_lock() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let mut a = answer();
    a.wanted = "  ".into();
    safe(
        &h,
        keep_answer_in(&h.path, a),
        "Say what was being decided and what you'd want, then keep it.",
    );
    for change in 0..3 {
        let mut a = answer();
        match change {
            0 => a.situation = "é".repeat(2001),
            1 => a.area = "unknown".into(),
            _ => a.area = "x".repeat(101),
        }
        safe(
            &h,
            keep_answer_in(&h.path, a),
            "Persona Builder couldn't keep that answer. Check it and try again.",
        );
    }
    let mut a = answer();
    a.wanted = "My therapist asked about this.".into();
    safe(&h, keep_answer_in(&h.path, a), PRIVATE);
    assert_eq!(h.store().signals().unwrap().len(), 0);
    let status = h.keep();
    assert!(!status.built);
    assert_eq!(status.tests.waiting, 1);
    let mut a = answer();
    a.assistant_said = " Make it complicated. ".into();
    keep_answer_in(&h.path, a).unwrap();
    let signals = h.store().signals().unwrap();
    assert_eq!(signals[0].kind, SignalKind::Instruction);
    assert_eq!(signals[1].kind, SignalKind::Correction);
    assert_eq!(
        signals[1].persona_said.as_deref(),
        Some("Make it complicated.")
    );
    assert!(
        signals
            .iter()
            .all(|s| s.source.as_deref() == Some("review"))
    );
    let _lock = lock_existing_state(&h.path, "wren").unwrap();
    safe(&h, keep_answer_in(&h.path, answer()), BUSY);
}
#[test]
fn status_counts_latest_status_in_category_order() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    h.keep();
    let mut a = answer();
    a.area = "sequencing".into();
    keep_answer_in(&h.path, a).unwrap();
    h.keep();
    let store = h.store();
    let cases: Vec<_> = store.cases().unwrap().iter().cloned().collect();
    let mut c = cases[0].clone();
    c.status = Status::Active;
    store.append_case(&c).unwrap();
    let mut c = cases[1].clone();
    c.status = Status::Retired;
    store.append_case(&c).unwrap();
    let s = review_status_in(&h.path, slug()).unwrap();
    assert!(!s.built);
    assert_eq!(
        s.tests,
        TestCounts {
            waiting: 1,
            in_use: 1,
            retired: 1
        }
    );
    assert_eq!(
        s.by_area
            .iter()
            .map(|a| a.area.as_str())
            .collect::<Vec<_>>(),
        vec!["scope", "sequencing"]
    );
    assert_eq!(
        s.by_area[0].tests,
        TestCounts {
            waiting: 1,
            in_use: 1,
            retired: 0
        }
    );
    assert_eq!(s.by_area[1].tests.retired, 1);
}
#[test]
fn draft_maps_diff_and_refuses_second_draft_without_call() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    h.stub(false, |s| s);
    safe(&h, h.draft(), text::NOT_BUILT);
    h.built();
    assert!(h.draft().unwrap().change.is_none());
    assert_eq!(h.calls(), 0);
    h.keep();
    let d = h.draft().unwrap();
    let change = d.change.unwrap();
    assert!(!change.approved);
    assert_eq!(change.learned_from, 1);
    assert_eq!(change.drafted.len(), 10);
    let p = h.store().proposal(&change.id).unwrap().unwrap();
    assert_eq!(change.summary, p.summary);
    assert_eq!(change.points.len(), p.changes.len());
    for (point, c) in change.points.iter().zip(p.changes) {
        assert_eq!(point.why, c.why);
        assert_eq!(point.before, diff::removed_lines(&c.diff));
        assert_eq!(point.after, diff::added_lines(&c.diff));
    }
    safe(
        &h,
        h.draft(),
        "A change is already waiting for your answer. Approve or reject it first.",
    );
    assert_eq!(h.calls(), 1);
}
#[test]
fn answer_only_accepts_the_unanswered_waiting_change() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    safe(&h, h.approve("prop-other", true), GONE);
    assert!(h.store().approvals().unwrap().is_empty());
    assert!(h.approve(&id, true).unwrap().change.unwrap().approved);
    safe(&h, h.approve(&id, true), GONE);
    let h = Home::new(true);
    let id = h.prepared();
    assert!(h.approve(&id, false).unwrap().change.is_none());
    let h = Home::new(true);
    let id = h.prepared();
    fs::write(h.dir().join("decisions.md"), "# changed\n").unwrap();
    safe(&h, h.approve(&id, true), GONE);
    assert!(h.store().approvals().unwrap().is_empty());
}
#[test]
fn apply_requires_approval_then_keeps_with_reason_progress_and_spend() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    let calls = h.calls();
    safe(&h, h.apply(&id, &mut |_| {}), GONE);
    assert_eq!(h.calls(), calls);
    h.approve(&id, true).unwrap();
    let before = fs::read(h.dir().join("decisions.md")).unwrap();
    h.stub(true, |s| s);
    let mut lines = vec![];
    let done = h.apply(&id, &mut |p| lines.push(p)).unwrap();
    assert!(done.kept);
    assert_eq!(done.reason, text::KEEP_REASON);
    assert_ne!(fs::read(h.dir().join("decisions.md")).unwrap(), before);
    assert_eq!(
        lines.iter().map(|p| p.line.as_str()).collect::<Vec<_>>(),
        vec![
            "Testing your assistant as it is now on 1 test.",
            "Testing it with the change on 1 test.",
            "Checking the written answers against what you wanted."
        ]
    );
    assert!(
        lines
            .iter()
            .all(|p| p.spend.daily_cap_usd == 50.0 && p.spend.spent_usd >= 0.0)
    );
    assert!(done.spend.spent_usd > 0.0);
    assert!(review_status_in(&h.path, slug()).unwrap().change.is_none());
}
#[test]
fn undone_preserves_decisions_and_report_growth_matches_counts() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    h.approve(&id, true).unwrap();
    h.stub(true, |s| {
        s.replace("Offer a smaller first step.", "Offer a huge first step.")
    });
    let before = fs::read(h.dir().join("decisions.md")).unwrap();
    let done = h.apply(&id, &mut |_| {}).unwrap();
    assert!(!done.kept);
    assert_eq!(fs::read(h.dir().join("decisions.md")).unwrap(), before);
    let id = {
        h.stub(false, |s| s.replace("reversible step", "flexible step"));
        h.draft().unwrap().change.unwrap().id
    };
    h.approve(&id, true).unwrap();
    h.stub(true, |s| s);
    assert!(h.apply(&id, &mut |_| {}).unwrap().kept);
    let r = report::Report {
        schema: report::SCHEMA.into(),
        built: "2026-01-01".into(),
        overall: report::Level::Thin,
        answered: 1,
        held: 0,
        test: None,
        areas: vec![],
    };
    report::write(&h.path, "wren", &r).unwrap();
    let g = apply::growth_counts(&h.store(), Some(&r.built))
        .unwrap()
        .unwrap();
    assert_eq!(
        show_report_in(&h.path, slug()).unwrap().growth,
        Some(GrowthCounts {
            kept: g.kept as u32,
            undone: g.undone as u32,
            passing: g.passing as u32,
            of: g.of as u32
        })
    );
    assert_eq!((g.kept, g.undone), (1, 1));
}
#[test]
fn apply_rechecks_permission_after_open_and_before_calls() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    h.approve(&id, true).unwrap();
    let state = h.state("");
    h.stub(true, |s| {
        s.replace(
            "print(\"2.1.292 (Claude Code)\")",
            &format!(
                "os.remove({:?})\n        print(\"2.1.292 (Claude Code)\")",
                state.join("consent.json").to_str().unwrap()
            ),
        )
    });
    let before = fs::read(h.dir().join("decisions.md")).unwrap();
    let calls = h.calls();
    safe(&h, h.apply(&id, &mut |_| {}), PERMISSION);
    assert_eq!(h.calls(), calls);
    assert_eq!(fs::read(h.dir().join("decisions.md")).unwrap(), before);
}
#[test]
fn apply_cap_is_a_safe_sentence() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    h.approve(&id, true).unwrap();
    h.stub(true, |s| s);
    let today = persona_builder_agent::ledger::local_day(std::time::SystemTime::now());
    let row = json!({"at":format!("{today}T12:00:00Z"),"id":"seed","day":today,"agent":"claude","step":"label","model":"sonnet","outcome":"success","seconds":1,"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"cost_usd":49.0,"cost_from":"reported"});
    write_private(&h.path, "wren", "runs.jsonl", format!("{row}\n").as_bytes()).unwrap();
    safe(
        &h,
        h.apply(&id, &mut |_| {}),
        "Today's spending limit for this assistant is reached: about $49.00 of $50.00, as Claude Code estimates it. Run the build again tomorrow to carry on.",
    );
}
#[test]
fn dtos_use_camel_case_and_requests_reject_unknown_fields() {
    assert_eq!(
        serde_json::to_value(ReportLevel::GettingThere).unwrap(),
        "gettingThere"
    );
    assert_eq!(
        serde_json::to_value(TestCounts {
            waiting: 1,
            in_use: 2,
            retired: 3
        })
        .unwrap()["inUse"],
        2
    );
    let a = serde_json::to_value(answer()).unwrap();
    assert_eq!(a["assistantSaid"], "");
    let mut a = a;
    a["extra"] = json!(true);
    assert!(serde_json::from_value::<KeepAnswerRequest>(a).is_err());
    assert!(
        serde_json::from_value::<RefreshAnswerRequest>(
            json!({"slug":"wren","id":"prop-a","approve":true,"extra":1})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<RefreshApplyRequest>(
            json!({"slug":"wren","id":"prop-a","extra":1})
        )
        .is_err()
    );
    let view = serde_json::to_value(BuildReport {
        built: "2026-10-24".into(),
        overall: ReportLevel::Thin,
        answered: 1,
        thin_below: 150,
        held_out: 2,
        test: None,
        areas: vec![],
    })
    .unwrap();
    assert_eq!(view["heldOut"], 2);
    assert_eq!(view["thinBelow"], 150);
    let status = serde_json::to_value(ReviewStatus {
        built: true,
        areas: vec![],
        tests: TestCounts {
            waiting: 0,
            in_use: 0,
            retired: 0,
        },
        by_area: vec![],
        change: None,
    })
    .unwrap();
    assert_eq!(status["byArea"], json!([]));
    let change = serde_json::to_value(RefreshChange {
        id: "prop-a".into(),
        drafted: "2026-10-24".into(),
        summary: "A smaller step.".into(),
        points: vec![],
        learned_from: 3,
        approved: false,
    })
    .unwrap();
    assert_eq!(change["learnedFrom"], 3);
    assert!(serde_json::from_value::<SlugRequest>(json!({"slug":"wren","extra":true})).is_err());
}

#[test]
fn missing_decisions_means_no_waiting_change_for_answer_and_apply() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    fs::remove_file(h.dir().join("decisions.md")).unwrap();
    safe(&h, h.approve(&id, true), GONE);
    assert!(h.store().approvals().unwrap().is_empty());
}
#[test]
fn missing_decisions_means_no_waiting_change_for_apply() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    h.approve(&id, true).unwrap();
    fs::remove_file(h.dir().join("decisions.md")).unwrap();
    let calls = h.calls();
    safe(&h, h.apply(&id, &mut |_| {}), GONE);
    assert_eq!(h.calls(), calls);
}

#[test]
fn apply_refuses_forgotten_state_between_check_and_open_without_calls() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    h.approve(&id, true).unwrap();
    let state = h.state("");
    h.stub(true,|s|s.replace("print(\"2.1.292 (Claude Code)\")",&format!("import shutil\n        shutil.rmtree({:?})\n        print(\"2.1.292 (Claude Code)\")",state.to_str().unwrap())));
    let before = fs::read(h.dir().join("decisions.md")).unwrap();
    let calls = h.calls();
    safe(&h, h.apply(&id, &mut |_| {}), PERMISSION);
    assert_eq!(h.calls(), calls);
    assert_eq!(fs::read(h.dir().join("decisions.md")).unwrap(), before);
    assert!(!h.path.join(".claude/state/wren").exists());
}

#[test]
fn draft_refuses_forgotten_state_between_check_and_open_without_calls() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    h.built();
    h.keep();
    h.stub(false,|s|s.replace("print(\"2.1.292 (Claude Code)\")",&format!("import shutil\n        shutil.rmtree({:?})\n        print(\"2.1.292 (Claude Code)\")",h.state("").to_str().unwrap())));
    safe(&h, h.draft(), PERMISSION);
    assert_eq!(h.calls(), 0);
    assert!(!h.path.join(".claude/state/wren").exists());
}

const FOLDER_KEPT_HOME: &str = "Nothing was sent. This step uses your assistant's own folder, which you keep on your own machines, and the provider for this step, Claude, through your own Claude Code, sends what it's given to Anthropic. To let it go there, run: persona-builder ceilings set <assistant folder> crew-folder anthropic";

fn keep_folder_local(member: &std::path::Path) {
    let path = member.join("member.json");
    let mut settings: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    settings["ceilings"] = json!({"crew-folder": "local-only"});
    fs::write(path, serde_json::to_vec(&settings).unwrap()).unwrap();
}

#[test]
fn draft_refresh_honors_the_members_local_folder_ceiling() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    h.built();
    h.keep();
    h.stub(false, |s| s);
    keep_folder_local(&h.dir());
    safe(&h, h.draft(), FOLDER_KEPT_HOME);
    assert_eq!(h.calls(), 0);
    assert!(!h.state("runs.jsonl").exists());
}

#[test]
fn apply_refresh_honors_the_members_local_folder_ceiling() {
    let _serial = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let id = h.prepared();
    h.approve(&id, true).unwrap();
    h.stub(true, |s| s);
    keep_folder_local(&h.dir());
    let calls = h.calls();
    let ledger = fs::read(h.state("runs.jsonl")).unwrap();
    safe(&h, h.apply(&id, &mut |_| {}), FOLDER_KEPT_HOME);
    assert_eq!(h.calls(), calls);
    assert_eq!(fs::read(h.state("runs.jsonl")).unwrap(), ledger);
}
