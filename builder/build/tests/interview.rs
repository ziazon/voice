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
    let plain = interview::prompt(&book, &weights());
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
    let weighted = interview::prompt(&book, &counted);
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
    let file = interview::check(&weights(), &draft).unwrap();
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
    let refused = |draft: Value| interview::check(&weights, &draft).unwrap_err().to_string();
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
        &stub.claude(),
        &session(&home.0, Cap::default()),
        "sonnet",
        &book,
        &weights(),
    )
    .unwrap();
    assert_eq!(file, interview::check(&weights(), &good_draft()).unwrap());
    assert_eq!(stub.calls(), 1);
    assert_eq!(stub.stdin(0), interview::prompt(&book, &weights()));
    let argv = stub.argv(0);
    assert_eq!(after(&argv, "--system-prompt"), interview::SYSTEM);
    assert_eq!(
        after(&argv, "--json-schema"),
        interview::schema().to_string()
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
    assert!(again.starts_with(&interview::prompt(&book, &weights())));
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
        &stub.claude(),
        &session,
        "sonnet",
        &book(&member()).unwrap(),
        &weights(),
    );
    assert!(first.is_ok());
    let error = interview::draft(
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
    let questions = interview::check(&weights(), &good_draft()).unwrap();
    let given = answers(json!({
        "1": {"pick": "B"},
        "2": {"pick": "A"},
        "3": {"own_words": "Ask my therapist first"},
        "4": {"own_words": "Neither: wait a week", "notes": "busy month"}
    }));
    let kept = interview::answer(&member, &filters, &questions, &given).unwrap();
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
    let questions = interview::check(&weights(), &good_draft()).unwrap();
    let error =
        interview::answer(&member, &filters, &questions, &answers(json!({"1": {}}))).unwrap_err();
    assert_eq!(error.to_string(), text::NO_ANSWERS);
    let error = interview::answer(
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
