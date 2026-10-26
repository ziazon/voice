//! The interview when the history is thin: counting, weighting, the prompt, the check of a drafted
//! file, the call with one retry against a stub `claude`, and the answers as training records.
use persona_builder::member::Member;
use persona_builder_agent::{
    claude::Claude,
    ledger::{Cap, Session},
};
use persona_builder_build::{interview, text};
use persona_builder_ingest::{corpus::read_records, filters::Filters};
use persona_builder_steps::{book::book, labels::tally};
use serde_json::{Map, Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, UNIX_EPOCH},
};

/// A word the model writes into a header: it must never come back in an error.
const SENTINEL: &str = "zebracorn";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-interview-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Scratch(path.canonicalize().unwrap())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn member() -> Member {
    Member::from_value(
        json!({"name": "Ada", "slug": "ada", "principal": "Sam", "principal_slug": "sam",
               "principal_pronouns": "they/them", "state_dir": "~/.claude/state/ada", "mark_word": "Ada",
               "filters": {"excluded_folders": "^-private-", "personal": "\\bmy therapist\\b",
                           "secret": "\\b\\d{9,}\\b", "pasted_prefixes": ["You are "]},
               "codebook": {"categories": ["scope", "methods", "tooling", "other"],
                            "domains": ["teaching", "garden", "other"],
                            "escalates": ["submit"], "signals": ["ruling"],
                            "groups": {"g1": ["scope", "methods"], "g2": ["tooling", "other"]}}}),
        "member.json",
    )
    .unwrap()
}

fn records(lines: &[Value]) -> Vec<(usize, Map<String, Value>)> {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    read_records("c.jsonl", text.as_bytes()).unwrap()
}
fn question(id: &str) -> Value {
    json!({"id": id, "kind": "question", "timestamp": "2026-09-01T10:00:00.000Z", "project": "kit",
           "header": "Scope", "question": "Keep the queue?", "multiSelect": false,
           "options": [{"label": "Yes", "description": ""}, {"label": "No", "description": ""}],
           "recommended": null, "picks": ["Yes"], "own_words": "", "notes": "",
           "outcome": "no_rec", "split": "train", "ada_pick": null})
}
fn message(id: &str) -> Value {
    json!({"id": id, "kind": "message", "timestamp": "2026-09-01T10:00:00.000Z", "project": "kit",
           "text": "ship it", "split": "train"})
}

fn weights() -> Vec<(String, Option<usize>)> {
    interview::weights(&book(&member()).unwrap(), None).unwrap()
}

/// Twenty good questions, their headers taken in turn from the kinds of decision.
fn good_draft() -> Value {
    let kinds = ["scope", "methods", "tooling"];
    let questions: Vec<Value> = (1..=20)
        .map(|n| {
            json!({"n": n, "header": kinds[(n - 1) % 3], "question": format!("Situation {n}: what first?"),
                   "multiSelect": false, "recommended": if n % 2 == 0 { json!("A") } else { Value::Null },
                   "options": [{"label": "Cut it", "description": "Less work"},
                               {"label": "Keep it", "description": "More work"}]})
        })
        .collect();
    json!({"questions": questions})
}
fn with_question(n: usize, edit: impl FnOnce(&mut Value)) -> Value {
    let mut draft = good_draft();
    edit(&mut draft["questions"][n - 1]);
    draft
}

#[test]
fn answered_counts_every_question_record() {
    let corpus = records(&[
        question("question:1"),
        message("message:1"),
        question("question:2"),
    ]);
    assert_eq!(interview::answered(&corpus), 2);
    assert_eq!(interview::THIN_BELOW, 150);
}

#[test]
fn weights_leave_out_other_and_put_the_thinnest_first() {
    let book = book(&member()).unwrap();
    assert_eq!(
        interview::weights(&book, None).unwrap(),
        vec![
            ("scope".to_owned(), None),
            ("methods".to_owned(), None),
            ("tooling".to_owned(), None)
        ]
    );
    let corpus = records(&[
        question("question:1"),
        question("question:2"),
        message("message:1"),
        question("question:3"),
    ]);
    let labels = json!({"question:1": {"category": "scope"}, "question:2": {"category": "scope"},
                        "message:1": {"category": "other"}, "question:3": {"category": "methods"}});
    let rows = tally(
        "c.jsonl",
        &corpus,
        "labels.json",
        labels.as_object().unwrap(),
    )
    .unwrap();
    assert_eq!(
        interview::weights(&book, Some(&rows)).unwrap(),
        vec![
            ("tooling".to_owned(), Some(0)),
            ("methods".to_owned(), Some(1)),
            ("scope".to_owned(), Some(2))
        ]
    );
}

#[test]
fn weights_refuse_a_codebook_with_nothing_to_ask_about() {
    let mut value = serde_json::to_value(member()).unwrap();
    value["codebook"]["categories"] = json!(["other"]);
    let member = Member::from_value(value, "member.json").unwrap();
    let error = interview::weights(&book(&member).unwrap(), None).unwrap_err();
    assert_eq!(error.to_string(), text::NOTHING_TO_ASK);
}

#[test]
fn the_prompt_names_the_areas_and_the_kinds_of_decision_only() {
    let book = book(&member()).unwrap();
    let plain = interview::prompt(interview::Form::Full, &book, &weights());
    assert!(plain.starts_with("Draft "), "{plain}");
    assert!(
        plain.contains("Their areas of work: teaching, garden.\n"),
        "{plain}"
    );
    assert!(plain.contains("- scope\n- methods\n- tooling\n"), "{plain}");
    assert!(
        plain.contains("at least one question on each kind of decision"),
        "{plain}"
    );
    assert!(!plain.contains("other"), "{plain}");
    let counted = vec![
        ("tooling".to_owned(), Some(0)),
        ("scope".to_owned(), Some(7)),
    ];
    let weighted = interview::prompt(interview::Form::Full, &book, &counted);
    assert!(
        weighted.contains("- tooling: 0 past decisions\n- scope: 7 past decisions\n"),
        "{weighted}"
    );
    assert!(
        weighted.contains("the rest to the kinds listed first"),
        "{weighted}"
    );
}

#[test]
fn check_keeps_a_good_draft_and_only_the_keys_round_reads() {
    let draft = with_question(1, |q| {
        q["extra"] = json!("dropped");
        q["options"][0]["extra"] = json!("dropped");
        q["options"][1]
            .as_object_mut()
            .unwrap()
            .remove("description");
    });
    let file = interview::check(interview::Form::Full, &weights(), &draft).unwrap();
    let first = &file["questions"][0];
    assert_eq!(
        first,
        &json!({"n": 1, "header": "scope", "question": "Situation 1: what first?", "multiSelect": false,
                "recommended": null, "options": [{"label": "Cut it", "description": "Less work"},
                                                 {"label": "Keep it", "description": ""}]})
    );
    assert_eq!(file.len(), 1);
    assert_eq!(file["questions"].as_array().unwrap().len(), 20);
}

#[test]
fn check_refuses_a_draft_that_does_not_fit_the_form() {
    let weights = weights();
    let refused = |draft: Value| {
        interview::check(interview::Form::Full, &weights, &draft)
            .unwrap_err()
            .to_string()
    };
    let mut nineteen = good_draft();
    nineteen["questions"].as_array_mut().unwrap().pop();
    assert_eq!(
        refused(nineteen),
        "the interview needs exactly 20 questions"
    );
    assert_eq!(
        refused(json!([])),
        "the interview needs exactly 20 questions"
    );
    assert_eq!(
        refused(with_question(3, |q| q["n"] = json!(4))),
        "round: question 4: n must be a distinct positive integer"
    );
    let mut swapped = good_draft();
    swapped["questions"].as_array_mut().unwrap().swap(0, 1);
    assert_eq!(
        refused(swapped),
        "the questions must be numbered 1 to 20 in order"
    );
    assert_eq!(
        refused(with_question(2, |q| q["header"] = json!(SENTINEL))),
        "question 2: the header must be one of the kinds of decision"
    );
    assert_eq!(
        refused(with_question(2, |q| q["header"] = json!("other"))),
        "question 2: the header must be one of the kinds of decision"
    );
    assert_eq!(
        refused(with_question(5, |q| {
            q["options"] = json!([{"label": "a"}, {"label": "b"}, {"label": "c"}, {"label": "d"}, {"label": "e"}])
        })),
        "round: question 5: options must contain 2 to 4 options"
    );
    assert_eq!(
        refused(with_question(6, |q| q["recommended"] = json!("C"))),
        "round: question 6: recommended must be an option letter"
    );
    assert_eq!(
        refused(with_question(7, |q| q["question"] = json!("x".repeat(401)))),
        "question 7: the question is longer than 400 characters"
    );
    assert_eq!(
        refused(with_question(8, |q| q["options"][1]["label"] = json!("x".repeat(101)))),
        "question 8: an option's label is longer than 100 characters"
    );
    assert_eq!(
        refused(with_question(9, |q| q["options"][0]["description"] =
            json!("x".repeat(301)))),
        "question 9: an option's description is longer than 300 characters"
    );
}

/// A stub `claude` that answers the `n`th call with `replies[n]`, keeping each call's stdin.
struct Stub {
    folder: Scratch,
}
impl Stub {
    fn new(replies: &[String]) -> Self {
        let folder = Scratch::new();
        for (n, reply) in replies.iter().enumerate() {
            fs::write(folder.0.join(format!("reply{n}")), reply).unwrap();
        }
        let program = folder.0.join("claude");
        fs::write(
            &program,
            "#!/bin/sh\nhere=\"$(cd \"$(dirname \"$0\")\" && pwd -P)\"\nn=$(ls \"$here\" | grep -c '^stdin')\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > \"$here/argv$n\"\ncat > \"$here/stdin$n\"\ncat \"$here/reply$n\"\n",
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        Stub { folder }
    }
    fn claude(&self) -> Claude {
        Claude::at(self.folder.0.join("claude"))
    }
    fn calls(&self) -> usize {
        (0..)
            .take_while(|n| self.folder.0.join(format!("stdin{n}")).exists())
            .count()
    }
    fn stdin(&self, n: usize) -> String {
        fs::read_to_string(self.folder.0.join(format!("stdin{n}"))).unwrap()
    }
    fn argv(&self, n: usize) -> Vec<String> {
        fs::read_to_string(self.folder.0.join(format!("argv{n}")))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn ok_reply(output: &Value) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": "",
           "total_cost_usd": 0.02, "usage": {"input_tokens": 900, "output_tokens": 2000},
           "structured_output": output})
    .to_string()
}

fn session(home: &Path, cap: Cap) -> Session {
    Session::open_with_clock(
        home,
        "ada",
        cap,
        Box::new(|| UNIX_EPOCH + Duration::from_secs(1_793_000_000)),
    )
    .unwrap()
}

fn after(argv: &[String], flag: &str) -> String {
    let at = argv.iter().position(|a| a == flag).unwrap();
    argv[at + 1].clone()
}

#[test]
fn draft_makes_one_tool_less_call_with_fixed_instructions_and_shape() {
    let home = Scratch::new();
    let stub = Stub::new(&[ok_reply(&good_draft())]);
    let book = book(&member()).unwrap();
    let file = interview::draft(
        interview::Form::Full,
        &stub.claude(),
        &session(&home.0, Cap::default()),
        "sonnet",
        &book,
        &weights(),
    )
    .unwrap();
    assert_eq!(
        file,
        interview::check(interview::Form::Full, &weights(), &good_draft()).unwrap()
    );
    assert_eq!(stub.calls(), 1);
    assert_eq!(
        stub.stdin(0),
        interview::prompt(interview::Form::Full, &book, &weights())
    );
    let argv = stub.argv(0);
    assert_eq!(after(&argv, "--system-prompt"), interview::SYSTEM);
    assert_eq!(
        after(&argv, "--json-schema"),
        interview::schema(interview::COUNT).to_string()
    );
    assert_eq!(after(&argv, "--model"), "sonnet");
    let ledger = fs::read_to_string(home.0.join(".claude/state/ada/runs.jsonl")).unwrap();
    assert!(ledger.contains("\"step\":\"interview\""), "{ledger}");
}

#[test]
fn draft_asks_once_more_when_the_first_draft_does_not_fit() {
    let home = Scratch::new();
    let bad = with_question(2, |q| q["header"] = json!(SENTINEL));
    let stub = Stub::new(&[ok_reply(&bad), ok_reply(&good_draft())]);
    let book = book(&member()).unwrap();
    let file = interview::draft(
        interview::Form::Full,
        &stub.claude(),
        &session(&home.0, Cap::default()),
        "sonnet",
        &book,
        &weights(),
    )
    .unwrap();
    assert_eq!(file["questions"].as_array().unwrap().len(), 20);
    assert_eq!(stub.calls(), 2);
    let again = stub.stdin(1);
    assert!(again.starts_with(&interview::prompt(interview::Form::Full, &book, &weights())));
    assert!(
        again.ends_with("Your last draft didn't fit the form: question 2: the header must be one of the kinds of decision. Draft all 20 questions again, following every rule above.\n"),
        "{again}"
    );
    assert!(!again.contains(SENTINEL));
}

#[test]
fn draft_stops_after_two_drafts_that_do_not_fit() {
    let home = Scratch::new();
    let bad = with_question(2, |q| q["header"] = json!(SENTINEL));
    let stub = Stub::new(&[ok_reply(&bad), ok_reply(&bad), ok_reply(&good_draft())]);
    let error = interview::draft(
        interview::Form::Full,
        &stub.claude(),
        &session(&home.0, Cap::default()),
        "sonnet",
        &book(&member()).unwrap(),
        &weights(),
    )
    .unwrap_err();
    assert_eq!(stub.calls(), 2);
    assert_eq!(
        error.to_string(),
        text::draft_failed("question 2: the header must be one of the kinds of decision")
    );
    assert!(!error.to_string().contains(SENTINEL));
}

#[test]
fn draft_never_retries_a_call_that_failed() {
    let home = Scratch::new();
    let failed =
        json!({"type": "result", "is_error": true, "result": "boom", "total_cost_usd": 0.01})
            .to_string();
    let stub = Stub::new(&[failed, ok_reply(&good_draft())]);
    let error = interview::draft(
        interview::Form::Full,
        &stub.claude(),
        &session(&home.0, Cap::default()),
        "sonnet",
        &book(&member()).unwrap(),
        &weights(),
    )
    .unwrap_err();
    assert_eq!(stub.calls(), 1);
    assert_eq!(error.to_string(), persona_builder_agent::text::FAILED);
}

#[test]
fn draft_spends_nothing_past_the_daily_cap() {
    let home = Scratch::new();
    let stub = Stub::new(&[ok_reply(&good_draft())]);
    let session = session(&home.0, Cap::new(1.0, 1.0).unwrap());
    let first = interview::draft(
        interview::Form::Full,
        &stub.claude(),
        &session,
        "sonnet",
        &book(&member()).unwrap(),
        &weights(),
    );
    assert!(first.is_ok());
    let error = interview::draft(
        interview::Form::Full,
        &stub.claude(),
        &session,
        "sonnet",
        &book(&member()).unwrap(),
        &weights(),
    )
    .unwrap_err();
    assert_eq!(stub.calls(), 1);
    assert!(
        error.to_string().starts_with("Today's spending limit"),
        "{error}"
    );
}

fn answers(answers: Value) -> Map<String, Value> {
    json!({"answered": "2026-10-24", "answers": answers})
        .as_object()
        .unwrap()
        .clone()
}

#[test]
fn answers_become_training_records_in_the_interview_shape() {
    let member = member();
    let filters = Filters::compile(&member.filters).unwrap();
    let questions = interview::check(interview::Form::Full, &weights(), &good_draft()).unwrap();
    let given = answers(json!({
        "1": {"pick": "B"},
        "2": {"pick": "A"},
        "3": {"own_words": "Ask my therapist first"},
        "4": {"own_words": "Neither: wait a week", "notes": "busy month"}
    }));
    let kept =
        interview::answer(interview::Form::Full, &member, &filters, &questions, &given).unwrap();
    assert_eq!(kept.records.len(), 3);
    assert_eq!((kept.unanswered, kept.personal, kept.secret), (16, 1, 0));
    let first = Value::Object(kept.records[0].clone());
    assert_eq!(first["id"], "question:interview-01");
    assert_eq!(first["project"], "interview");
    assert_eq!(first["split"], "train");
    assert_eq!(first["picks"], json!(["Keep it"]));
    assert_eq!(first["outcome"], "no_rec");
    assert_eq!(first["ada_pick"], Value::Null);
    assert_eq!(kept.records[1]["outcome"], "took_rec");
    assert_eq!(kept.records[2]["id"], "question:interview-04");
    assert_eq!(kept.records[2]["outcome"], "own_words");
    assert_eq!(
        interview::summary(&kept),
        "kept 3 interview answers, always for training; 16 unanswered\nleft out 1 that looked personal and 0 that looked secret\n"
    );
}

#[test]
fn answers_refuse_a_file_with_no_answer() {
    let member = member();
    let filters = Filters::compile(&member.filters).unwrap();
    let questions = interview::check(interview::Form::Full, &weights(), &good_draft()).unwrap();
    let error = interview::answer(
        interview::Form::Full,
        &member,
        &filters,
        &questions,
        &answers(json!({"1": {}})),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), text::NO_ANSWERS);
    let error = interview::answer(
        interview::Form::Full,
        &member,
        &filters,
        &questions,
        &answers(json!({"1": {"pick": "Z"}})),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "round: question 1: pick is not an option"
    );
}

#[test]
fn report_weights_use_examples_and_zero_for_absent_areas() {
    use persona_builder_build::report::{Area, Level, Report, SCHEMA};
    let book = book(&member()).unwrap();
    assert_eq!(
        interview::weights_from_report(&book, None).unwrap(),
        interview::weights(&book, None).unwrap()
    );
    let report = Report {
        schema: SCHEMA.into(),
        built: "2026-10-25".into(),
        overall: Level::Thin,
        answered: 10,
        held: 0,
        test: None,
        areas: vec![
            Area {
                area: "scope".into(),
                level: Level::Thin,
                examples: 8,
                strengthen: vec![],
            },
            Area {
                area: "methods".into(),
                level: Level::Thin,
                examples: 2,
                strengthen: vec![],
            },
        ],
    };
    assert_eq!(
        interview::weights_from_report(&book, Some(&report)).unwrap(),
        vec![
            ("tooling".into(), Some(0)),
            ("methods".into(), Some(2)),
            ("scope".into(), Some(8))
        ]
    );
}

#[test]
fn forms_have_separate_files_and_rounds_at_the_boundary() {
    use interview::Form;
    assert_eq!(Form::for_answered(149), Form::Full);
    assert_eq!(Form::for_answered(150), Form::Short);
    assert_eq!(
        (
            Form::Full.step(),
            Form::Full.questions_file(),
            Form::Full.answers_file(),
            Form::Full.out()
        ),
        (
            "interview",
            "interview-questions.json",
            "interview-answers.json",
            "decisions-interview.jsonl"
        )
    );
    assert_eq!(
        (
            Form::Short.step(),
            Form::Short.questions_file(),
            Form::Short.answers_file(),
            Form::Short.out()
        ),
        (
            "interview-short",
            "interview-short-questions.json",
            "interview-short-answers.json",
            "decisions-interview-short.jsonl"
        )
    );
}

#[test]
fn thin_kinds_keep_only_counts_under_five_and_at_most_five_in_order() {
    let weights = vec![
        ("Scope".into(), Some(0)),
        ("Methods".into(), Some(3)),
        ("Go or no-go".into(), Some(4)),
        ("Timing".into(), Some(5)),
        ("Tools".into(), Some(40)),
    ];
    let mut book = book(&member()).unwrap();
    book.categories = vec![
        "Tools".into(),
        "Timing".into(),
        "Go or no-go".into(),
        "Methods".into(),
        "Scope".into(),
    ];
    let report: persona_builder_build::report::Report = serde_json::from_value(json!({
        "schema":"persona-builder/report/1", "built":"2026-10-24", "overall":"thin", "answered":150, "held":0, "test":null,
        "areas": weights.iter().map(|(kind, n)| json!({"area":kind,"examples":n,"level":"thin","strengthen":[]})).collect::<Vec<_>>()
    })).unwrap();
    let from_report = interview::weights_from_report(&book, Some(&report)).unwrap();
    assert_eq!(interview::thin_kinds(&from_report), weights[..3]);
    assert_eq!(interview::thin_kinds(&weights), weights[..3]);
    let seven = (0..7)
        .map(|n| (format!("Kind {n}"), Some(n / 2)))
        .collect::<Vec<_>>();
    assert_eq!(interview::thin_kinds(&seven), seven[..5]);
    assert!(interview::thin_kinds(&[("Scope".into(), None)]).is_empty());
}

#[test]
fn short_form_counts_covers_each_kind_and_keeps_its_own_round() {
    use interview::Form;
    let kinds = vec![("scope".into(), Some(0)), ("methods".into(), Some(3))];
    assert_eq!(Form::Short.count(&kinds), 8);
    assert_eq!(
        interview::schema(8)["properties"]["questions"]["minItems"],
        8
    );
    assert_eq!(
        interview::schema(8)["properties"]["questions"]["maxItems"],
        8
    );
    assert_eq!(
        interview::schema(8)["properties"]["questions"]["items"]["properties"]["n"]["maximum"],
        8
    );
    let prompt = interview::prompt(Form::Short, &book(&member()).unwrap(), &kinds);
    assert!(
        prompt.contains("Exactly 4 questions on each kind"),
        "{prompt}"
    );
    assert!(prompt.contains("fewest examples"), "{prompt}");
    assert!(!prompt.contains("tooling"));
    let mut draft = good_draft();
    draft["questions"].as_array_mut().unwrap().truncate(8);
    for q in draft["questions"].as_array_mut().unwrap() {
        q["header"] = json!("scope");
    }
    let error = interview::check(Form::Short, &kinds, &draft)
        .unwrap_err()
        .to_string();
    assert_eq!(error, "the kinds of decision listed each need a question");
    for q in &mut draft["questions"].as_array_mut().unwrap()[4..] {
        q["header"] = json!("methods");
    }
    let checked = interview::check(Form::Short, &kinds, &draft).unwrap();
    let member = member();
    let filters = Filters::compile(&member.filters).unwrap();
    let kept = interview::answer(
        Form::Short,
        &member,
        &filters,
        &checked,
        &answers(json!({"1": {"pick":"A"}})),
    )
    .unwrap();
    assert_eq!(kept.records[0]["id"], "question:interview-short-01");
    assert_eq!(kept.records[0]["project"], "interview");
}

#[test]
fn short_draft_retries_a_missing_kind_with_the_short_count_and_ledger_step() {
    use interview::Form;
    let kinds = vec![("scope".into(), Some(0)), ("methods".into(), Some(3))];
    let mut bad = good_draft();
    bad["questions"].as_array_mut().unwrap().truncate(8);
    for q in bad["questions"].as_array_mut().unwrap() {
        q["header"] = json!("scope");
    }
    let mut good = bad.clone();
    for q in &mut good["questions"].as_array_mut().unwrap()[4..] {
        q["header"] = json!("methods");
    }
    let home = Scratch::new();
    let stub = Stub::new(&[ok_reply(&bad), ok_reply(&good)]);
    let file = interview::draft(
        Form::Short,
        &stub.claude(),
        &session(&home.0, Cap::default()),
        "sonnet",
        &book(&member()).unwrap(),
        &kinds,
    )
    .unwrap();
    assert_eq!(file["questions"].as_array().unwrap().len(), 8);
    assert_eq!(stub.calls(), 2);
    assert!(stub.stdin(1).contains(
        "the kinds of decision listed each need a question. Draft all 8 questions again"
    ));
    assert_eq!(
        after(&stub.argv(0), "--json-schema"),
        interview::schema(8).to_string()
    );
    let ledger = fs::read_to_string(home.0.join(".claude/state/ada/runs.jsonl")).unwrap();
    assert!(
        ledger
            .lines()
            .all(|line| serde_json::from_str::<Value>(line).unwrap()["step"] == "interview-short")
    );
}
