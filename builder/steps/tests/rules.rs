//! The steps' own rules: every refusal's words, no record text in any error, and each place the
//! Rust port refuses what the Python only tolerates (asserted beside the Python where it differs).
use persona_builder::member::Member;
use persona_builder_ingest::{corpus::read_records, filters::Filters};
use persona_builder_steps::{
    book::book,
    chunk::chunk,
    distil::distil_in,
    input::{read_json, read_object, read_objects},
    labels::{Mode, check_labels, label_problems, tally},
    round::round,
    score::score,
    shingle::overlaps,
    similar::similar,
};
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A word that must never appear in an error: it sits in every bad record's text.
const SENTINEL: &str = "zebracorn";
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn member_json() -> Value {
    json!({"name": "Ada", "slug": "ada", "principal": "Sam", "principal_slug": "sam",
           "principal_pronouns": "they/them", "state_dir": "~/.claude/state/ada", "mark_word": "Ada",
           "filters": {"excluded_folders": "^-private-", "personal": "\\bmy therapist\\b",
                       "secret": "\\b\\d{9,}\\b", "pasted_prefixes": ["You are "]},
           "codebook": {"categories": ["scope", "other"], "escalates": ["submit"],
                        "signals": ["ruling"], "groups": {"g1": ["scope"], "g2": ["other"]}}})
}
fn member() -> Member {
    Member::from_value(member_json(), "member.json").unwrap()
}
fn member_with(edit: impl FnOnce(&mut Value)) -> Member {
    let mut value = member_json();
    edit(&mut value);
    Member::from_value(value, "member.json").unwrap()
}
fn records(lines: &[Value]) -> Vec<(usize, Map<String, Value>)> {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    read_records("c.jsonl", text.as_bytes()).unwrap()
}
fn question(id: &str, split: &str) -> Value {
    json!({"id": id, "kind": "question", "timestamp": "2026-09-01T10:00:00.000Z", "project": "kit",
           "header": "Scope", "question": format!("Keep the {SENTINEL} queue?"), "multiSelect": false,
           "options": [{"label": "Keep (Recommended)", "description": ""}, {"label": "Drop", "description": "x"}],
           "recommended": "Keep (Recommended)", "ada_pick": null, "picks": ["Keep (Recommended)"],
           "own_words": "", "notes": "", "outcome": "took_rec", "split": split})
}
fn message(id: &str, split: &str, text: &str) -> Value {
    json!({"id": id, "kind": "message", "timestamp": "2026-09-02T10:00:00.000Z", "project": "kit",
           "text": text, "context": "", "split": split})
}
fn labels(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}
fn error<T>(result: persona_builder::error::Result<T>) -> String {
    let Err(error) = result else {
        panic!("expected an error")
    };
    let error = error.to_string();
    assert!(
        !error.contains(SENTINEL),
        "record text in an error: {error}"
    );
    error
}

/// The real Python on one command, with the member and files written to a scratch folder.
fn python(files: &[(&str, String)], args: &[&str]) -> i32 {
    let dir = std::env::temp_dir().join(format!(
        "pb-steps-rules-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    let mut config = member_json();
    config["state_dir"] = Value::from(dir.join("state").to_str().unwrap());
    fs::write(dir.join("member.json"), config.to_string()).unwrap();
    for (name, text) in files {
        fs::write(dir.join(name), text).unwrap();
    }
    let repo: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("python3")
        .arg("-I")
        .arg(repo.join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(dir.join("member.json"))
        .args(args)
        .current_dir(&dir)
        .output()
        .unwrap();
    fs::remove_dir_all(&dir).unwrap();
    output.status.code().unwrap()
}
fn jsonl(lines: &[Value]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

#[test]
fn json_inputs_are_checked_whole() {
    assert_eq!(
        error(read_json("l.json", b"\xff")),
        "l.json is not UTF-8 text"
    );
    assert_eq!(
        error(read_json("l.json", b"{\"a\": NaN}")),
        "l.json is not valid JSON"
    );
    assert_eq!(
        error(read_json("l.json", b"{\"a\": 1.5}")),
        "l.json holds a number Persona Builder can't copy exactly"
    );
    assert_eq!(
        error(read_object("l.json", b"[]")),
        "l.json must be a JSON object"
    );
    assert_eq!(
        error(read_objects("l.json", b"{\"question:a\": \"zebracorn\"}")),
        "l.json: the entry for question:a is not a JSON object"
    );
    // The Python takes NaN and floats: both are refused here.
    let corpus = jsonl(&[question("question:a", "train")]);
    assert_eq!(
        python(
            &[
                ("c.jsonl", corpus),
                (
                    "l.json",
                    "{\"question:a\": {\"category\": \"scope\", \"x\": NaN}}".into()
                )
            ],
            &["tally", "--corpus", "c.jsonl", "--labels", "l.json"]
        ),
        0
    );
}

#[test]
fn chunk_refuses_a_record_without_a_split_and_a_size_of_zero() {
    let mut bad = question("question:a", "train");
    bad.as_object_mut().unwrap().remove("split");
    assert_eq!(
        error(chunk("c.jsonl", &records(&[bad.clone()]), 3)),
        "c.jsonl:1: a record has an unexpected shape (split)"
    );
    assert_eq!(
        python(
            &[("c.jsonl", jsonl(&[bad]))],
            &["chunk", "--corpus", "c.jsonl", "--out-dir", "o"]
        ),
        1
    );
    assert_eq!(
        error(chunk(
            "c.jsonl",
            &records(&[question("question:a", "train")]),
            0
        )),
        "the chunk size must be at least 1"
    );
}

#[test]
fn check_labels_reports_bad_shapes_instead_of_failing() {
    let book = book(&member()).unwrap();
    let corpus = records(&[
        question("question:a", "train"),
        question("question:b", "train"),
    ]);
    let given = labels(json!({"question:a": SENTINEL,
                               "question:b": {"category": "scope", "escalates": "submit", "reason": "fine"}}));
    let problems = label_problems(&book, "c.jsonl", &corpus, &given, Mode::Train).unwrap();
    assert_eq!(problems, ["question:a label", "question:b escalates"]);
    // The Python crashes on a label that is not an object.
    let files = [
        ("c.jsonl", jsonl(&[question("question:a", "train")])),
        ("l.json", json!({"question:a": SENTINEL}).to_string()),
    ];
    assert_eq!(
        python(
            &files,
            &[
                "check-labels",
                "--chunk",
                "c.jsonl",
                "--labels",
                "l.json",
                "--mode",
                "train"
            ]
        ),
        1
    );
}

#[test]
fn tally_refuses_odd_outcomes_and_categories() {
    let mut odd = question("question:a", "train");
    odd["outcome"] = Value::from("questions");
    let corpus = records(&[odd.clone()]);
    let given = labels(json!({"question:a": {"category": "scope"}}));
    assert_eq!(
        error(tally("c.jsonl", &corpus, "l.json", &given)),
        "c.jsonl:1: a record has an unexpected shape (outcome)"
    );
    // The Python adds one to the row's "questions" instead.
    let files = [
        ("c.jsonl", jsonl(&[odd])),
        (
            "l.json",
            json!({"question:a": {"category": "scope"}}).to_string(),
        ),
    ];
    assert_eq!(
        python(
            &files,
            &["tally", "--corpus", "c.jsonl", "--labels", "l.json"]
        ),
        0
    );
    let corpus = records(&[question("question:a", "train")]);
    assert_eq!(
        error(tally(
            "c.jsonl",
            &corpus,
            "l.json",
            &labels(json!({"question:a": {"category": 3}}))
        )),
        "l.json: the label for question:a has a category that is not text"
    );
    assert_eq!(
        error(tally(
            "c.jsonl",
            &corpus,
            "l.json",
            &labels(json!({"question:a": {}}))
        )),
        "l.json: the label for question:a has no category"
    );
    assert_eq!(
        error(check_labels(
            "c.jsonl",
            &corpus,
            &labels(json!({"question:z": {"category": "scope"}}))
        )),
        "labels do not match the corpus: unlabelled question:a; unknown question:z"
    );
}

#[test]
fn the_codebook_is_checked_before_use() {
    let without = member_with(|m| {
        m.as_object_mut().unwrap().remove("codebook");
    });
    assert_eq!(error(book(&without)), "member config: missing codebook");
    let empty = member_with(|m| m["codebook"]["categories"] = json!([]));
    assert_eq!(
        error(book(&empty)),
        "member config: missing codebook.categories"
    );
    let slashed = member_with(|m| m["codebook"]["groups"] = json!({"../g": ["scope"]}));
    assert_eq!(
        error(book(&slashed)),
        "member config: codebook.groups: ../g can't be a file name; use letters, digits and hyphens"
    );
    let letters = member_with(|m| m["codebook"]["groups"] = json!({"g1": "scope"}));
    assert_eq!(
        error(book(&letters)),
        "member config: codebook.groups.g1 must be a list of categories"
    );
}

#[test]
fn distil_in_names_what_a_label_lacks() {
    let book = book(&member()).unwrap();
    let corpus = records(&[message("message:a", "train", SENTINEL)]);
    assert_eq!(
        error(distil_in(
            &book,
            "c.jsonl",
            &corpus,
            "l.json",
            &labels(json!({"message:a": {"category": "scope", "reason": "r"}}))
        )),
        "l.json: the label for message:a has no signal"
    );
    assert_eq!(
        error(distil_in(
            &book,
            "c.jsonl",
            &corpus,
            "l.json",
            &labels(json!({}))
        )),
        "l.json: no label for message:a"
    );
    assert_eq!(
        error(distil_in(
            &book,
            "c.jsonl",
            &corpus,
            "l.json",
            &labels(json!({"message:a": {"category": "nope"}}))
        )),
        "category nope is in no codebook group"
    );
}

#[test]
fn score_refuses_missing_predictions_bad_tiers_and_odd_picks() {
    let m = member();
    let answer = json!({"id": "question:a", "recommended": "Keep (Recommended)", "ada_pick": null,
                        "picks": ["Keep (Recommended)"], "own_words": "", "notes": "", "outcome": "took_rec",
                        "multiSelect": false});
    let answers = records(std::slice::from_ref(&answer));
    let none = labels(json!({}));
    assert_eq!(
        error(score(&m, "a.jsonl", &answers, "l.json", &none, &none)),
        "no prediction for question:a"
    );
    let guesses = labels(json!({"question:a": {"pick": "Keep", "confidence": "sure"}}));
    assert_eq!(
        error(score(&m, "a.jsonl", &answers, "l.json", &none, &guesses)),
        "question:a: confidence must be one of high, medium, low"
    );
    let mut led = answer.clone();
    led["ada_pick"] = json!([SENTINEL]);
    let guesses = labels(json!({"question:a": {"pick": "Keep", "confidence": "high"}}));
    assert_eq!(
        error(score(
            &m,
            "a.jsonl",
            &records(&[led]),
            "l.json",
            &none,
            &guesses
        )),
        "a.jsonl:1: a record has an unexpected shape (ada_pick)"
    );
    let result = score(&m, "a.jsonl", &answers, "l.json", &none, &guesses).unwrap();
    assert_eq!(
        (result.overall.n, result.overall.hits),
        (1, 1),
        "marks are ignored in a pick"
    );
}

#[test]
fn similar_reads_fields_only_where_the_python_does() {
    let m = member();
    let mut held = message("message:h", "heldout", "short");
    held.as_object_mut().unwrap().remove("text");
    let long = "keep the review queue small and the partner note first always";
    let corpus = records(&[held, message("message:a", "train", long)]);
    let hits = similar(&m, "c.jsonl", &corpus, "review queue partner", 6).unwrap();
    assert_eq!(
        hits.len(),
        1,
        "a held-out record without text is never read"
    );
    assert!(
        similar(&m, "c.jsonl", &corpus, "review queue partner", 0)
            .unwrap()
            .is_empty()
    );
    let mut odd = question("question:a", "train");
    odd["header"] = json!(7);
    assert_eq!(
        error(similar(&m, "c.jsonl", &records(&[odd]), "keep queue", 6)),
        "c.jsonl:1: a record has an unexpected shape (header)"
    );
}

#[test]
fn shingle_refuses_odd_fields() {
    let mut odd = message("message:a", "train", SENTINEL);
    odd["notes"] = json!(["a", "list"]);
    let corpus = records(&[odd]);
    assert_eq!(
        error(overlaps("a b c", &[("c.jsonl", &corpus)], 2)),
        "c.jsonl:1: a record has an unexpected shape (notes)"
    );
    let mut options = message("message:a", "train", "x");
    options["options"] = json!("letters");
    assert_eq!(
        error(overlaps("a b c", &[("c.jsonl", &records(&[options]))], 2)),
        "c.jsonl:1: a record has an unexpected shape (options)"
    );
}

fn round_files(questions: Value, answers: Value) -> (Map<String, Value>, Map<String, Value>) {
    (labels(questions), labels(answers))
}
fn one_question(extra: Value) -> Value {
    let mut q = json!({"n": 1, "question": format!("Keep the {SENTINEL}?"), "options": [{"label": "Yes"}, {"label": "No"}]});
    for (k, v) in extra.as_object().unwrap() {
        q[k] = v.clone();
    }
    json!({"questions": [q]})
}

#[test]
fn round_refuses_each_bad_shape_in_the_pythons_words() {
    let m = member();
    let filters = Filters::compile(&m.filters).unwrap();
    let ok_answers = json!({"answered": "2026-10-02", "answers": {"1": {"pick": "A"}}});
    let cases = [
        (
            one_question(json!({"n": 0})),
            ok_answers.clone(),
            "round: question 0: n must be a distinct positive integer",
        ),
        (
            one_question(json!({"n": "one"})),
            ok_answers.clone(),
            "round: question ?: n must be a distinct positive integer",
        ),
        (
            one_question(json!({"question": "  "})),
            ok_answers.clone(),
            "round: question 1: question must be a non-empty string",
        ),
        (
            one_question(json!({"header": 3})),
            ok_answers.clone(),
            "round: question 1: header must be a string",
        ),
        (
            one_question(json!({"multiSelect": 1})),
            ok_answers.clone(),
            "round: question 1: multiSelect must be a bool",
        ),
        (
            one_question(json!({"options": [{"label": "Only"}]})),
            ok_answers.clone(),
            "round: question 1: options must contain 2 to 4 options",
        ),
        (
            one_question(json!({"options": ["Yes", "No"]})),
            ok_answers.clone(),
            "round: question 1: options must be objects with a label",
        ),
        (
            one_question(json!({"options": [{"label": "Yes"}, {"label": "Yes"}]})),
            ok_answers.clone(),
            "round: question 1: label must be distinct",
        ),
        (
            one_question(json!({"recommended": "C"})),
            ok_answers.clone(),
            "round: question 1: recommended must be an option letter",
        ),
        (
            json!({"questions": {}}),
            ok_answers.clone(),
            "round: questions must be a list",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-2", "answers": {}}),
            "round: answered must be YYYY-MM-DD",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-02-30", "answers": {}}),
            "round: answered must be a valid date",
        ),
        (
            one_question(json!({})),
            json!({"answered": "0000-01-01", "answers": {}}),
            "round: answered must be a valid date",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": []}),
            "round: answers must be an object",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": {"2": {}}}),
            "round: question 2: answer has no question",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": {"zebracorn": {}}}),
            "round: question ?: answer has no question",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": {"1": "A"}}),
            "round: question 1: answer must be an object",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": {"1": {"pick": "C"}}}),
            "round: question 1: pick is not an option",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": {"1": {"pick": 1}}}),
            "round: question 1: pick is not an option",
        ),
        (
            one_question(json!({})),
            json!({"answered": "2026-10-02", "answers": {"1": {"pick": ["A", "B"]}}}),
            "round: question 1: pick must have one option for a single-select question",
        ),
        (
            one_question(json!({"n": 18446744073709551615u64})),
            json!({"answered": "9999-12-31", "answers": {"18446744073709551615": {"pick": "A"}}}),
            "round: question 18446744073709551615: n exceeds the timestamp range",
        ),
    ];
    for (questions, answers, expected) in cases {
        let (q, a) = round_files(questions.clone(), answers.clone());
        assert_eq!(error(round(&m, &filters, "pilot", &q, &a, false)), expected);
        let files = [
            ("q.json", questions.to_string()),
            ("a.json", answers.to_string()),
        ];
        assert_eq!(
            python(
                &files,
                &[
                    "round",
                    "--name",
                    "pilot",
                    "--questions",
                    "q.json",
                    "--answers",
                    "a.json"
                ]
            ),
            1,
            "{expected}"
        );
    }
    let (q, a) = round_files(one_question(json!({})), ok_answers);
    assert_eq!(
        error(round(&m, &filters, "Pilot", &q, &a, false)),
        "round: name must be 1 to 40 lowercase letters, digits or hyphens, starting with a letter or digit"
    );
    assert!(round(&m, &filters, &"a".repeat(40), &q, &a, false).is_ok());
    assert!(round(&m, &filters, &"a".repeat(41), &q, &a, false).is_err());
    assert!(round(&m, &filters, "-pilot", &q, &a, false).is_err());
}

#[test]
fn round_refuses_other_scripts_digits_and_reaches_the_last_second() {
    let m = member();
    let filters = Filters::compile(&m.filters).unwrap();
    let (q, a) = round_files(
        one_question(json!({})),
        json!({"answered": "٢٠٢٦-١٠-٠٢", "answers": {"1": {"pick": "A"}}}),
    );
    assert_eq!(
        error(round(&m, &filters, "pilot", &q, &a, false)),
        "round: answered must be YYYY-MM-DD"
    );
    // All of them in another script's digits: the Python's pattern takes them, strptime refuses them.
    let files = [
        ("q.json", one_question(json!({})).to_string()),
        (
            "a.json",
            json!({"answered": "٢٠٢٦-١٠-٠٢", "answers": {"1": {"pick": "A"}}}).to_string(),
        ),
    ];
    assert_eq!(
        python(
            &files,
            &[
                "round",
                "--name",
                "pilot",
                "--questions",
                "q.json",
                "--answers",
                "a.json"
            ]
        ),
        1
    );
    let (q, a) = round_files(
        one_question(json!({"n": 43199})),
        json!({"answered": "9999-12-31", "answers": {"43199": {"pick": "A"}}}),
    );
    let last = round(&m, &filters, "pilot", &q, &a, false).unwrap();
    assert_eq!(last.records[0]["timestamp"], "9999-12-31T23:59:59.000Z");
    assert_eq!(last.records[0]["id"], "question:pilot-43199");
}

#[test]
fn round_refuses_other_scripts_digits_the_python_takes_in_the_year() {
    let m = member();
    let filters = Filters::compile(&m.filters).unwrap();
    let answers = json!({"answered": "٢٠٢٦-10-1٢", "answers": {"1": {"pick": "A"}}});
    let (q, a) = round_files(one_question(json!({})), answers.clone());
    assert_eq!(
        error(round(&m, &filters, "pilot", &q, &a, false)),
        "round: answered must be YYYY-MM-DD"
    );
    let files = [
        ("q.json", one_question(json!({})).to_string()),
        ("a.json", answers.to_string()),
    ];
    assert_eq!(
        python(
            &files,
            &[
                "round",
                "--name",
                "pilot",
                "--questions",
                "q.json",
                "--answers",
                "a.json"
            ]
        ),
        0
    );
}

#[test]
fn score_refuses_picks_given_as_text_and_never_panics_on_a_bad_prediction() {
    let m = member();
    let mut answer = json!({"id": "question:a", "recommended": "Keep (Recommended)", "ada_pick": null,
                            "picks": "Keep (Recommended)", "own_words": "", "notes": "", "outcome": "took_rec",
                            "multiSelect": false});
    let guesses = labels(json!({"question:a": {"pick": "Keep", "confidence": "high"}}));
    let none = labels(json!({}));
    assert_eq!(
        error(score(
            &m,
            "a.jsonl",
            &records(std::slice::from_ref(&answer)),
            "l.json",
            &none,
            &guesses
        )),
        "a.jsonl:1: a record has an unexpected shape (picks)"
    );
    // The Python reads the text's letters as picks and finishes.
    let files = [
        ("a.jsonl", jsonl(std::slice::from_ref(&answer))),
        ("l.json", "{}".into()),
        ("p.json", Value::Object(guesses).to_string()),
    ];
    assert_eq!(
        python(
            &files,
            &[
                "score",
                "--answers",
                "a.jsonl",
                "--labels",
                "l.json",
                "--predictions",
                "p.json"
            ]
        ),
        0
    );
    answer["picks"] = json!(["Keep (Recommended)"]);
    let unchecked = labels(json!({"question:a": 5}));
    assert_eq!(
        error(score(
            &m,
            "a.jsonl",
            &records(&[answer]),
            "l.json",
            &none,
            &unchecked
        )),
        "the prediction for question:a is not a JSON object"
    );
}

#[test]
fn similar_reads_only_the_option_labels() {
    let m = member();
    let mut q = question("question:a", "train");
    for option in q["options"].as_array_mut().unwrap() {
        option.as_object_mut().unwrap().remove("description");
    }
    let hits = similar(&m, "c.jsonl", &records(&[q]), "keep queue drop", 6).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["options"], json!(["Keep (Recommended)", "Drop"]));
}

#[test]
fn distil_in_names_a_label_that_is_not_an_object() {
    let book = book(&member()).unwrap();
    let corpus = records(&[message("message:a", "train", SENTINEL)]);
    assert_eq!(
        error(distil_in(
            &book,
            "c.jsonl",
            &corpus,
            "l.json",
            &labels(json!({"message:a": "scope"}))
        )),
        "l.json: the label for message:a is not a JSON object"
    );
}
