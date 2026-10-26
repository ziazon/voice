//! Build and interview commands against invented assistants, isolated homes and Claude stubs.
use persona_builder_agent::{ledger::local_day, text as agent_text};
use persona_builder_api::{dto::*, *};
use persona_builder_build::{interview, text as build_text};
use persona_builder_ingest::{filters::Filters, mine, private::*};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

// A parallel test can spawn a child which retains another test's flock until exec.
// Keep cases separate; scoped threads inside stop tests still exercise concurrent commands.
static CASES: Mutex<()> = Mutex::new(());
static NEXT: AtomicUsize = AtomicUsize::new(0);
const PERMISSION: &str =
    "Persona Builder needs your permission before it reads anything. Give it in the first step.";
const FIND: &str = "Persona Builder couldn't find that assistant. Close it and open it again.";
const BUSY: &str =
    "Persona Builder is already working on this assistant. Wait for it to finish, then try again.";
const STOPPED: &str = "Stopped. Your assistant is as it was before this build. What was already finished is kept, so building again carries on from there.";
const NOTHING: &str = "There's nothing to build from yet. Read your history and put it together first, in Your history.";
const ANSWER_BAD: &str = "Persona Builder couldn't keep that answer. Close it and open it again.";
const NO_ANSWERS: &str = "Answer at least one question first.";
const NO_QUESTIONS: &str = "Draft the questions first.";
const ENOUGH: &str =
    "Your history already holds enough answers to build from, and no kind of decision is thin.";
const ALREADY: &str = "Your questions are already drafted. Answer them, or draft different ones.";
struct Home {
    path: PathBuf,
    bin: PathBuf,
}
impl Home {
    fn new(agree: bool) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-building-{}-{}",
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
        let bin = path.join("bin");
        fs::create_dir(&bin).unwrap();
        Self { path, bin }
    }
    fn member(&self) -> PathBuf {
        self.path.join(".claude/crew/wren")
    }
    fn state(&self, name: &str) -> PathBuf {
        self.path.join(".claude/state/wren").join(name)
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        write_private(&self.path, "wren", name, bytes).unwrap();
    }
    fn stub(&self, transform: impl FnOnce(String) -> String) {
        let output = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(output.status.success(), "python3 is required");
        let source = include_str!("../../build/tests/fixtures/claude_stub.py").to_owned();
        self.program(&format!(
            "#!{} -I\n{}",
            String::from_utf8(output.stdout).unwrap().trim(),
            transform(source)
        ));
    }
    fn program(&self, source: &str) {
        let p = self.bin.join("claude");
        fs::write(&p, source).unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn build(
        &self,
        builds: &Builds,
        progress: &mut dyn FnMut(BuildProgress),
    ) -> Result<BuildDone, Failure> {
        build_assistant_in(
            builds,
            &self.path,
            Some(self.bin.as_os_str()),
            &[],
            slug(),
            progress,
        )
    }
    fn draft(&self, replace: bool) -> Result<InterviewStatus, Failure> {
        draft_interview_in(
            &self.path,
            Some(self.bin.as_os_str()),
            &[],
            DraftInterviewRequest {
                slug: "wren".into(),
                replace,
            },
        )
    }
    fn answers(&self, answers: Vec<InterviewAnswer>) -> Result<InterviewStatus, Failure> {
        save_interview_answers_in(
            &self.path,
            InterviewAnswersRequest {
                slug: "wren".into(),
                answers,
            },
        )
    }
    fn snapshots(&self) -> Vec<Vec<u8>> {
        ["decisions.md", "measurements.md"]
            .iter()
            .map(|n| fs::read(self.member().join(n)).unwrap())
            .collect()
    }
    fn calls(&self) -> usize {
        (0..)
            .take_while(|n| self.bin.join(format!("stdin{n}")).exists())
            .count()
    }
    fn corpus(&self, train: usize, held: usize) {
        let records: Vec<_> = (0..train)
            .map(|n| question(n, "train"))
            .chain((0..held).map(|n| question(n, "heldout")))
            .collect();
        self.write(
            "decisions.jsonl",
            records
                .iter()
                .map(|v| format!("{v}\n"))
                .collect::<String>()
                .as_bytes(),
        );
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
fn answer(n: u32, picks: &[&str], words: &str) -> InterviewAnswer {
    InterviewAnswer {
        n,
        picks: picks.iter().map(|v| (*v).into()).collect(),
        own_words: words.into(),
    }
}
fn safe<T: std::fmt::Debug>(h: &Home, result: Result<T, Failure>, expected: &str) {
    let message = result.unwrap_err().message;
    assert!(
        !message.contains(h.path.to_str().unwrap()),
        "home leaked: {message}"
    );
    assert_eq!(message, expected);
}
fn question(n: usize, split: &str) -> Value {
    json!({"id":format!("question:{split}{n:03}"),"kind":"question","timestamp":format!("2026-09-{:02}T10:00:00.000Z",1+n%28),"project":"kit","header":(["Scope","Methods","Tooling"][n%3]),"question":format!("Should the reading list for week {n} stay short?"),"multiSelect":false,"options":[{"label":"Keep it short","description":"Less to read"},{"label":"Make it longer","description":"More detail"}],"recommended":null,"picks":["Keep it short"],"own_words":"","notes":"","outcome":"no_rec","split":split,"wren_pick":null})
}
fn questions() -> Value {
    json!({"questions":(1..=20).map(|n|json!({"n":n,"header":(["scope","methods","tooling"][(n-1)%3]),"question":format!("Which useful change should we make in week {n}?"),"multiSelect":n==2,"recommended":"A","options":[{"label":"Small","description":"Keep changes small"},{"label":"Broad","description":"Cover more work"},{"label":"Wait","description":"Learn more"},{"label":"Ask","description":"Ask the team"}]})).collect::<Vec<_>>()})
}
fn install_questions(h: &Home) {
    h.write(
        interview::QUESTIONS_FILE,
        &serde_json::to_vec(&questions()).unwrap(),
    );
}
fn interview_stub(h: &Home) {
    let q = questions().to_string();
    h.stub(|source|source.replace("first = prompt.split", &format!("if prompt.startswith('Draft a calibration interview'):\n        print(json.dumps({{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":False,\"total_cost_usd\":0.02,\"structured_output\":json.loads({q:?})}}))\n        return 0\n    first = prompt.split")));
}
fn revoke_consent_stub(h: &Home) {
    interview_stub(h);
    let source = fs::read_to_string(h.bin.join("claude")).unwrap();
    h.program(&source.replace(
        "if ARGS[:1] == [\"--version\"]:",
        "if ARGS[:1] == [\"--version\"]:\n        os.remove(os.path.join(os.getcwd(), '.claude/state/wren/consent.json'))",
    ));
}
fn forget_stub(h: &Home) {
    interview_stub(h);
    let source = fs::read_to_string(h.bin.join("claude")).unwrap();
    h.program(&source.replace(
        "if ARGS[:1] == [\"--version\"]:",
        "if ARGS[:1] == [\"--version\"]:\n        import shutil; shutil.rmtree(os.path.join(os.getcwd(), '.claude/state/wren'))",
    ));
}
fn wait_for(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "stub never reached {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn every_command_requires_permission_and_valid_assistant_without_writing() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(false);
    let builds = Builds::new();
    for (s, expected) in [("wren", PERMISSION), ("missing", FIND), ("../x", FIND)] {
        let request = || SlugRequest { slug: s.into() };
        let before = fs::read_dir(h.state("")).into_iter().flatten().count();
        safe(
            &h,
            build_assistant_in(&builds, &h.path, None, &[], request(), &mut |_| {
                panic!("gate sent progress")
            }),
            expected,
        );
        safe(&h, spend_today_in(&h.path, request()), expected);
        safe(&h, interview_status_in(&h.path, request()), expected);
        safe(
            &h,
            draft_interview_in(
                &h.path,
                None,
                &[],
                DraftInterviewRequest {
                    slug: s.into(),
                    replace: false,
                },
            ),
            expected,
        );
        safe(
            &h,
            save_interview_answers_in(
                &h.path,
                InterviewAnswersRequest {
                    slug: s.into(),
                    answers: vec![],
                },
            ),
            expected,
        );
        safe(&h, keep_interview_answers_in(&h.path, request()), expected);
        // Stop intentionally works without consent or an existing assistant and writes nothing.
        stop_build_in(&builds, request()).unwrap();
        assert_eq!(
            fs::read_dir(h.state("")).into_iter().flatten().count(),
            before
        );
    }
}
#[test]
fn build_rechecks_permission_after_claude_check() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    revoke_consent_stub(&h);
    let before = h.snapshots();
    let result = h.build(&Builds::new(), &mut |_| {});
    assert!(!h.state("consent.json").exists());
    assert_eq!(h.calls(), 0, "revoked history reached a model call");
    assert_eq!(h.snapshots(), before);
    safe(&h, result, PERMISSION);
}
#[test]
fn draft_rechecks_permission_after_claude_check() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    revoke_consent_stub(&h);
    let result = h.draft(false);
    assert!(!h.state("consent.json").exists());
    assert_eq!(h.calls(), 0, "revoked permission allowed a model call");
    assert!(!h.state(interview::QUESTIONS_FILE).exists());
    safe(&h, result, PERMISSION);
}
#[test]
fn build_forgotten_after_claude_check_leaves_no_state_folder() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    forget_stub(&h);
    let before = h.snapshots();
    let result = h.build(&Builds::new(), &mut |_| {});
    assert!(!h.path.join(".claude/state/wren").exists());
    assert_eq!(h.calls(), 0, "forgotten history reached a model call");
    assert_eq!(h.snapshots(), before);
    safe(&h, result, PERMISSION);
}
#[test]
fn draft_forgotten_after_claude_check_leaves_no_state_folder() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    forget_stub(&h);
    let result = h.draft(false);
    assert!(!h.path.join(".claude/state/wren").exists());
    assert_eq!(h.calls(), 0, "forgotten permission allowed a model call");
    safe(&h, result, PERMISSION);
}
#[test]
fn build_needs_history_and_refuses_the_state_lock() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    let builds = Builds::new();
    safe(&h, h.build(&builds, &mut |_| {}), NOTHING);
    h.corpus(12, 32);
    h.stub(|s| s);
    let _lock = lock_state(&h.path, "wren").unwrap();
    safe(&h, h.build(&builds, &mut |_| {}), BUSY);
    safe(&h, spend_today_in(&h.path, slug()), BUSY);
}
#[test]
fn successful_build_streams_cli_lines_counts_and_spend() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s| s);
    let mut progress = vec![];
    let done = h.build(&Builds::new(), &mut |p| progress.push(p)).unwrap();
    assert_eq!((done.answered, done.held_out, done.tested), (44, 32, true));
    assert_eq!(
        progress.iter().map(|p| p.line.as_str()).collect::<Vec<_>>(),
        vec![
            "Reading your history: 44 answered questions, 32 of them kept back to test your assistant.",
            "Sorting your decisions by kind, part 1 of 2: done.",
            "Sorting your decisions by kind, part 2 of 2: done.",
            "Learning how you decide, group 1 of 3: done.",
            "Learning how you decide, group 2 of 3: done.",
            "Learning how you decide, group 3 of 3: done.",
            "Testing your assistant on 32 questions it hasn't seen, batch 1 of 1: done.",
            build_text::CHECKING
        ]
    );
    assert!(
        progress
            .iter()
            .all(|p| p.spend.daily_cap_usd == 50.0 && p.spend.spent_usd >= 0.0)
    );
    assert!(done.spend.spent_usd > 0.0);
    assert!(
        fs::read_to_string(h.member().join("decisions.md"))
            .unwrap()
            .contains("### scope")
    );
    assert!(
        fs::read_to_string(h.member().join("measurements.md"))
            .unwrap()
            .contains("32 held-out questions")
    );
}
#[test]
fn saved_build_keeps_last_progress_spend_when_final_read_fails() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s| s);
    let mut last_spend = None;
    let done = h
        .build(&Builds::new(), &mut |p| {
            if p.line == build_text::CHECKING {
                last_spend = Some(p.spend);
                h.write("runs.jsonl", b"broken ledger\n");
            }
        })
        .unwrap();
    assert_eq!(Some(done.spend), last_spend);
    assert!(
        fs::read_to_string(h.member().join("decisions.md"))
            .unwrap()
            .contains("### scope")
    );
    assert!(
        fs::read_to_string(h.member().join("measurements.md"))
            .unwrap()
            .contains("32 held-out questions")
    );
    assert!(h.state("report.json").exists());
}

#[test]
fn stop_after_last_call_saves_nothing_and_releases_registry() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s| s);
    let before = h.snapshots();
    let builds = Builds::new();
    safe(
        &h,
        h.build(&builds, &mut |p| {
            if p.line == build_text::CHECKING {
                stop_build_in(&builds, slug()).unwrap();
            }
        }),
        STOPPED,
    );
    assert_eq!(h.snapshots(), before);
    assert!(!h.state("report.json").exists());
    assert!(
        h.build(&builds, &mut |_| {}).is_ok(),
        "registry still holds the finished slug"
    );
}
#[test]
fn stop_early_reaches_build_while_version_check_sleeps() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s|s.replace("print(\"2.1.292 (Claude Code)\")","open(os.path.join(HERE, 'version-started'), 'w').close()\n        import time\n        time.sleep(1)\n        print(\"2.1.292 (Claude Code)\")"));
    let before = h.snapshots();
    let builds = Builds::new();
    thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        let h = &h;
        let builds = &builds;
        scope.spawn(move || tx.send(h.build(builds, &mut |_| {})).unwrap());
        wait_for(&h.bin.join("version-started"));
        stop_build_in(builds, slug()).unwrap();
        safe(
            h,
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            STOPPED,
        );
    });
    assert_eq!(h.calls(), 0);
    assert_eq!(h.snapshots(), before);
    assert!(!h.state("report.json").exists());
}
#[test]
fn stop_second_call_is_busy_then_reuses_first_kept_reply() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s|s.replace("how = plan.get(str(n), \"good\")","if n == 1:\n        import time\n        time.sleep(30)\n    how = plan.get(str(n), \"good\")"));
    let before = h.snapshots();
    let builds = Builds::new();
    thread::scope(|scope| {
        let (tx, rx) = mpsc::channel();
        let h = &h;
        let builds = &builds;
        scope.spawn(move || tx.send(h.build(builds, &mut |_| {})).unwrap());
        wait_for(&h.bin.join("stdin1"));
        safe(h, h.build(builds, &mut |_| {}), BUSY);
        let start = Instant::now();
        stop_build_in(builds, slug()).unwrap();
        safe(
            h,
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            STOPPED,
        );
        assert!(start.elapsed() < Duration::from_secs(10));
    });
    assert_eq!(h.snapshots(), before);
    assert!(!h.state("report.json").exists());
    assert_eq!(h.calls(), 2);
    h.stub(|s| s);
    let mut progress = vec![];
    h.build(&builds, &mut |p| progress.push(p.line))
        .expect("registry still holds the finished slug");
    assert!(
        progress
            .iter()
            .any(|p| p == "Sorting your decisions by kind, part 1 of 2: already done.")
    );
    assert_eq!(
        h.calls(),
        7,
        "six successful calls plus one stopped call; first call must be reused"
    );
}
#[test]
fn cap_and_signed_out_errors_are_safe_sentences() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s| s);
    let today = local_day(SystemTime::now());
    h.write("runs.jsonl",format!("{}\n",json!({"at":format!("{today}T12:00:00Z"),"id":"seed","day":today,"agent":"claude","step":"label","model":"sonnet","outcome":"success","seconds":1,"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"cost_usd":49.0,"cost_from":"reported"})).as_bytes());
    assert_eq!(
        spend_today_in(&h.path, slug()).unwrap(),
        Spend {
            spent_usd: 49.0,
            daily_cap_usd: 50.0
        }
    );
    safe(
        &h,
        h.build(&Builds::new(), &mut |_| {}),
        "Today's spending limit for this assistant is reached: about $49.00 of $50.00, as Claude Code estimates it. Build again tomorrow to carry on.",
    );
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s| s.replace("\"loggedIn\": True", "\"loggedIn\": False"));
    safe(
        &h,
        h.build(&Builds::new(), &mut |_| {}),
        agent_text::SIGNED_OUT,
    );
}
#[test]
fn interview_drafting_letters_replacement_damage_and_enough() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    interview_stub(&h);
    h.corpus(150, 0);
    safe(&h, h.draft(false), ENOUGH);
    h.corpus(12, 0);
    let status = h.draft(false).unwrap();
    assert_eq!(status.questions.len(), 20);
    assert_eq!(status.thin_below, 150);
    assert!(status.thin_kinds.is_empty());
    assert_eq!(
        status.questions[0]
            .options
            .iter()
            .map(|o| o.letter.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "C", "D"]
    );
    assert_eq!(status.questions[0].recommended.as_deref(), Some("A"));
    assert_eq!(interview_status_in(&h.path, slug()).unwrap(), status);
    safe(&h, h.draft(false), ALREADY);
    h.answers(vec![answer(1, &["A"], "")]).unwrap();
    assert!(h.state(interview::ANSWERS_FILE).exists());
    h.draft(true).unwrap();
    assert!(!h.state(interview::ANSWERS_FILE).exists());
    h.write(interview::QUESTIONS_FILE, b"broken json");
    assert!(
        interview_status_in(&h.path, slug())
            .unwrap()
            .questions
            .is_empty()
    );
    assert_eq!(h.draft(false).unwrap().questions.len(), 20);
}
#[test]
fn answers_validate_every_boundary_and_round_trip() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    safe(&h, h.answers(vec![]), NO_QUESTIONS);
    safe(
        &h,
        h.answers((0..21).map(|_| answer(1, &["A"], "")).collect()),
        ANSWER_BAD,
    );
    safe(
        &h,
        h.answers(vec![answer(1, &["A", "B", "C", "D", "A"], "")]),
        ANSWER_BAD,
    );
    install_questions(&h);
    let cases = vec![
        (0..21).map(|_| answer(1, &["A"], "")).collect(),
        vec![answer(1, &["A"], ""), answer(1, &["B"], "")],
        vec![answer(21, &["A"], "")],
        vec![answer(1, &["E"], "")],
        vec![answer(2, &["A", "A"], "")],
        vec![answer(2, &["A", "B", "C", "D", "A"], "")],
        vec![answer(1, &["A", "C"], "")],
        vec![answer(1, &[], &"é".repeat(2001))],
    ];
    for bad in cases {
        safe(&h, h.answers(bad), ANSWER_BAD);
        assert!(!h.state(interview::ANSWERS_FILE).exists());
    }
    let good = vec![
        answer(1, &["B"], "Keep the work useful."),
        answer(2, &["A", "C"], ""),
        answer(3, &[], "  "),
    ];
    let status = h.answers(good.clone()).unwrap();
    assert_eq!(status.answers, good[..2]);
    assert_eq!(interview_status_in(&h.path, slug()).unwrap(), status);
    let saved: Value =
        serde_json::from_slice(&read_private(&h.path, "wren", interview::ANSWERS_FILE).unwrap())
            .unwrap();
    assert_eq!(saved["answers"]["1"]["pick"], "B");
    assert_eq!(saved["answers"]["2"]["pick"], json!(["A", "C"]));
    assert!(saved["answers"].get("3").is_none());
    h.answers(vec![answer(1, &[], &"é".repeat(2000))]).unwrap();
}
#[test]
fn kept_answers_equal_core_records_and_count_private_ones() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    install_questions(&h);
    safe(&h, keep_interview_answers_in(&h.path, slug()), NO_ANSWERS);
    h.answers(vec![
        answer(1, &["A"], ""),
        answer(2, &[], "My therapist asked about this."),
    ])
    .unwrap();
    let m = persona_builder::member::read_member(&h.member().join("member.json")).unwrap();
    let filters = Filters::compile(&m.filters).unwrap();
    let q: Value =
        serde_json::from_slice(&read_private(&h.path, "wren", interview::QUESTIONS_FILE).unwrap())
            .unwrap();
    let a: Value =
        serde_json::from_slice(&read_private(&h.path, "wren", interview::ANSWERS_FILE).unwrap())
            .unwrap();
    let expected = interview::answer(
        interview::Form::Full,
        &m,
        &filters,
        q.as_object().unwrap(),
        a.as_object().unwrap(),
    )
    .unwrap();
    let kept = keep_interview_answers_in(&h.path, slug()).unwrap();
    assert_eq!(kept.kept, 1);
    assert_eq!(kept.private_left_out, 1);
    assert_eq!(kept.unanswered, 18);
    assert_eq!(
        read_private(&h.path, "wren", interview::OUT).unwrap(),
        mine::to_jsonl(&expected.records).as_bytes()
    );
    assert!(interview_status_in(&h.path, slug()).unwrap().kept);
}
#[test]
fn dtos_are_camel_case_and_all_requests_refuse_unknown_fields() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(
        serde_json::to_value(Spend {
            spent_usd: 1.25,
            daily_cap_usd: 50.0
        })
        .unwrap(),
        json!({"spentUsd":1.25,"dailyCapUsd":50.0})
    );
    let done = serde_json::to_value(BuildDone {
        answered: 1,
        held_out: 2,
        tested: false,
        spend: Spend {
            spent_usd: 0.0,
            daily_cap_usd: 50.0,
        },
    })
    .unwrap();
    assert_eq!(done["heldOut"], 2);
    let q = InterviewQuestion {
        n: 1,
        kind: "scope".into(),
        question: "Which?".into(),
        multi_select: true,
        recommended: None,
        options: vec![],
    };
    assert_eq!(serde_json::to_value(q).unwrap()["multiSelect"], true);
    assert_eq!(
        serde_json::to_value(answer(1, &["A"], "Words")).unwrap(),
        json!({"n":1,"picks":["A"],"ownWords":"Words"})
    );
    assert!(serde_json::from_value::<SlugRequest>(json!({"slug":"wren","extra":true})).is_err());
    assert!(
        serde_json::from_value::<DraftInterviewRequest>(
            json!({"slug":"wren","replace":false,"extra":true})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<InterviewAnswersRequest>(
            json!({"slug":"wren","answers":[],"extra":true})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<InterviewAnswer>(
            json!({"n":1,"picks":[],"ownWords":"","extra":true})
        )
        .is_err()
    );
}

#[test]
fn shorter_option_lists_reject_the_first_letter_past_the_end() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    let mut q = questions();
    q["questions"][0]["options"]
        .as_array_mut()
        .unwrap()
        .truncate(2);
    h.write(interview::QUESTIONS_FILE, &serde_json::to_vec(&q).unwrap());
    safe(&h, h.answers(vec![answer(1, &["C"], "")]), ANSWER_BAD);
    assert!(!h.state(interview::ANSWERS_FILE).exists());
}
#[test]
fn interview_writers_refuse_state_lock_without_changing_answers() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    install_questions(&h);
    interview_stub(&h);
    h.answers(vec![answer(1, &["A"], "")]).unwrap();
    let before = read_private(&h.path, "wren", interview::ANSWERS_FILE).unwrap();
    let _lock = lock_state(&h.path, "wren").unwrap();
    safe(&h, h.answers(vec![answer(1, &["B"], "")]), BUSY);
    safe(&h, keep_interview_answers_in(&h.path, slug()), BUSY);
    safe(&h, h.draft(true), BUSY);
    assert_eq!(
        read_private(&h.path, "wren", interview::ANSWERS_FILE).unwrap(),
        before
    );
    assert!(!h.state(interview::OUT).exists());
}
#[test]
fn unreadable_settings_and_ledger_return_fixed_safe_messages() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.write("decisions.jsonl", b"not json\n");
    let settings = "Something in this assistant's settings stops the build. Use the persona-builder command to see what.";
    safe(&h, interview_status_in(&h.path, slug()), settings);
    safe(&h, h.build(&Builds::new(), &mut |_| {}), settings);
    h.write("runs.jsonl", b"broken ledger\n");
    safe(
        &h,
        spend_today_in(&h.path, slug()),
        "Persona Builder's record of what this assistant has spent can't be read, so it won't spend anything more. Use the persona-builder command to see what's wrong.",
    );
}
#[test]
fn keep_requires_questions_and_refuses_a_wholly_private_set() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    safe(&h, keep_interview_answers_in(&h.path, slug()), NO_QUESTIONS);
    install_questions(&h);
    h.answers(vec![answer(1, &[], "My therapist asked about this.")])
        .unwrap();
    safe(
        &h,
        keep_interview_answers_in(&h.path, slug()),
        "Every answer looked personal or secret, so none was kept. Answer without names or private details, then try again.",
    );
    assert!(!h.state(interview::OUT).exists());
}

fn report_for_long_history(thin: bool) -> Vec<u8> {
    let kinds = [
        "scope",
        "sequencing",
        "methods",
        "framing",
        "writing-voice",
        "communication",
        "collaboration",
        "compliance-ethics",
        "tooling",
        "verification",
        "placement-naming",
        "go-no-go",
    ];
    let report = serde_json::to_vec(&json!({"schema":"persona-builder/report/1","built":"2026-10-24","overall":"thin","answered":150,"held":0,"test":null,
        "areas":kinds.iter().enumerate().map(|(n, kind)|json!({"area":kind,"level":if thin && n < 2 {"thin"} else {"strong"},"examples":if thin && n < 2 {n * 3} else {40},"strengthen":[]})).collect::<Vec<_>>()})).unwrap();
    serde_json::from_slice::<persona_builder_build::report::Report>(&report).unwrap();
    report
}

fn short_interview_stub(h: &Home) {
    let mut q = questions();
    q["questions"].as_array_mut().unwrap().truncate(8);
    for (at, question) in q["questions"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        question["header"] = json!(["scope", "sequencing"][at % 2]);
    }
    let q = q.to_string();
    h.stub(|source|source.replace("first = prompt.split", &format!("print(json.dumps({{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":False,\"total_cost_usd\":0.02,\"structured_output\":json.loads({q:?})}}))\n    return 0\n    first = prompt.split")));
}

#[test]
fn interview_status_names_thin_kinds_only_for_long_histories() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.write("report.json", &report_for_long_history(true));
    h.corpus(149, 0);
    assert!(
        interview_status_in(&h.path, slug())
            .unwrap()
            .thin_kinds
            .is_empty()
    );
    h.corpus(150, 0);
    let status = interview_status_in(&h.path, slug()).unwrap();
    assert_eq!(status.thin_kinds, ["scope", "sequencing"]);
    assert!(status.questions.is_empty());
    assert_eq!(h.calls(), 0);
}

#[test]
fn long_history_drafts_saves_and_keeps_short_answers_without_changing_full_answers() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(150, 0);
    h.write("report.json", &report_for_long_history(true));
    let full = format!("{}\n", question(999, "train"));
    h.write("decisions-interview.jsonl", full.as_bytes());
    short_interview_stub(&h);
    let status = h.draft(false).unwrap();
    assert_eq!(status.questions.len(), 8);
    assert_eq!(status.thin_kinds, ["scope", "sequencing"]);
    assert_eq!(h.calls(), 1);
    assert!(h.state("interview-short-questions.json").exists());
    assert!(!h.state(interview::QUESTIONS_FILE).exists());
    let answers = vec![answer(1, &["A"], ""), answer(2, &[], "Wait a week")];
    assert_eq!(h.answers(answers.clone()).unwrap().answers, answers);
    assert!(h.state("interview-short-answers.json").exists());
    assert!(!h.state(interview::ANSWERS_FILE).exists());
    let kept = keep_interview_answers_in(&h.path, slug()).unwrap();
    assert_eq!(kept.kept, 2);
    assert_eq!(kept.unanswered, 6);
    assert_eq!(
        fs::read(h.state("decisions-interview.jsonl")).unwrap(),
        full.as_bytes()
    );
    let records: Vec<Value> = fs::read_to_string(h.state("decisions-interview-short.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    for (at, record) in records.iter().enumerate() {
        assert_eq!(
            record["id"],
            format!("question:interview-short-{:02}", at + 1)
        );
        assert_eq!(record["project"], "interview");
        assert_eq!(record["split"], "train");
    }
    assert_eq!(
        fs::read(h.state("decisions-interview.jsonl")).unwrap(),
        full.as_bytes()
    );
    assert!(interview_status_in(&h.path, slug()).unwrap().kept);
    let ledger = fs::read_to_string(h.state("runs.jsonl")).unwrap();
    assert!(
        ledger
            .lines()
            .all(|line| serde_json::from_str::<Value>(line).unwrap()["step"] == "interview-short")
    );
}

#[test]
fn long_history_without_thin_kinds_spends_nothing() {
    let _case = CASES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let h = Home::new(true);
    h.corpus(150, 0);
    h.write("report.json", &report_for_long_history(false));
    interview_stub(&h);
    safe(&h, h.draft(false), ENOUGH);
    assert_eq!(h.calls(), 0);
    assert!(!h.state("runs.jsonl").exists());
    assert!(
        interview_status_in(&h.path, slug())
            .unwrap()
            .thin_kinds
            .is_empty()
    );
}

const LIMIT_DAMAGED: &str = "Persona Builder can't read the daily limit saved for this assistant, so it won't spend anything. Set the limit again on Connect Claude.";
const LIMIT_BAD: &str = "The daily limit must be at least $0.01 and at most $1,000. The most one step may use must be at least $0.01 and no more than the daily limit.";
fn limit_request(daily_usd: f64, call_usd: f64) -> SpendingLimitRequest {
    SpendingLimitRequest {
        slug: "wren".into(),
        daily_usd,
        call_usd,
    }
}
#[test]
fn spending_limit_defaults_and_saved_values_reach_todays_spend() {
    let _case = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let before: Vec<_> = fs::read_dir(h.state(""))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        spending_limit_in(&h.path, slug()).unwrap(),
        SpendingLimit {
            daily_usd: 50.0,
            call_usd: 5.0
        }
    );
    assert_eq!(
        fs::read_dir(h.state(""))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>(),
        before
    );
    let saved = set_spending_limit_in(&h.path, limit_request(20.0, 2.5)).unwrap();
    assert_eq!(
        saved,
        SpendingLimit {
            daily_usd: 20.0,
            call_usd: 2.5
        }
    );
    assert_eq!(spending_limit_in(&h.path, slug()).unwrap(), saved);
    assert_eq!(spend_today_in(&h.path, slug()).unwrap().daily_cap_usd, 20.0);
}
#[test]
fn a_build_enforces_the_saved_daily_limit() {
    let _case = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    h.corpus(12, 32);
    h.stub(|s| s);
    set_spending_limit_in(&h.path, limit_request(1.0, 1.0)).unwrap();
    h.write(
        "runs.jsonl",
        format!(
            "{}\n",
            json!({"day":local_day(SystemTime::now()),"cost_usd":0.5})
        )
        .as_bytes(),
    );
    let before = h.snapshots();
    safe(
        &h,
        h.build(&Builds::new(), &mut |_| {}),
        "Today's spending limit for this assistant is reached: about $0.50 of $1.00, as Claude Code estimates it. Build again tomorrow to carry on.",
    );
    assert_eq!(h.calls(), 0);
    assert_eq!(h.snapshots(), before);
}
#[test]
fn invalid_spending_limits_write_nothing_even_while_busy() {
    let _case = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    let _session = persona_builder_agent::ledger::Session::open(
        &h.path,
        "wren",
        persona_builder_agent::ledger::Cap::default(),
    )
    .unwrap();
    for (day, call) in [(0.0, 0.0), (2000.0, 5.0), (10.0, 20.0), (f64::NAN, 5.0)] {
        safe(
            &h,
            set_spending_limit_in(&h.path, limit_request(day, call)),
            LIMIT_BAD,
        );
        assert!(!h.state("limit.json").exists());
    }
}
#[test]
fn a_running_session_prevents_changing_the_limit() {
    let _case = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(true);
    set_spending_limit_in(&h.path, limit_request(20.0, 2.5)).unwrap();
    let before = fs::read(h.state("limit.json")).unwrap();
    let _session = persona_builder_agent::ledger::Session::open(
        &h.path,
        "wren",
        persona_builder_agent::ledger::Cap::default(),
    )
    .unwrap();
    safe(
        &h,
        set_spending_limit_in(&h.path, limit_request(30.0, 3.0)),
        BUSY,
    );
    assert_eq!(fs::read(h.state("limit.json")).unwrap(), before);
    assert_eq!(
        spending_limit_in(&h.path, slug()).unwrap().daily_usd,
        20.0,
        "reading takes no lock"
    );
}
#[test]
fn spending_limits_require_permission_and_damaged_limits_can_be_repaired() {
    let _case = CASES.lock().unwrap_or_else(|p| p.into_inner());
    let h = Home::new(false);
    safe(&h, spending_limit_in(&h.path, slug()), PERMISSION);
    safe(
        &h,
        set_spending_limit_in(&h.path, limit_request(20.0, 2.5)),
        PERMISSION,
    );
    assert!(!h.state("limit.json").exists());
    agree_consent_in(&h.path, slug()).unwrap();
    h.write("limit.json", b"not json");
    safe(&h, spending_limit_in(&h.path, slug()), LIMIT_DAMAGED);
    safe(&h, spend_today_in(&h.path, slug()), LIMIT_DAMAGED);
    set_spending_limit_in(&h.path, limit_request(20.0, 2.5)).unwrap();
    assert_eq!(spending_limit_in(&h.path, slug()).unwrap().daily_usd, 20.0);
}
