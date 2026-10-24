mod common;
use common::{Scratch, parity_tools_available, repo};
use persona_builder::{
    error::Result,
    member::{Member, read_member},
};
use persona_builder_ingest::{
    corpus::{self, Input, Mode},
    filters::Filters,
    mine::Split,
};
use serde_json::{Value, json};
use std::{fs, process::Command};
fn member() -> Member {
    read_member(&repo().join("crew/_template/member.json")).unwrap()
}
fn error<T: std::fmt::Debug>(result: Result<T>, expected: &str) {
    let message = result.unwrap_err().to_string();
    assert_eq!(message, expected);
    assert!(!message.contains("sentinel"));
}
fn merged(bytes: &[u8], mode: Mode) -> Result<corpus::Merged> {
    corpus::merge(
        &Filters::compile(&member().filters).unwrap(),
        &[Input {
            name: "x.jsonl",
            mode,
            bytes,
        }],
        &Split::Holdout(0.5),
    )
}
fn record() -> Value {
    json!({"id":"sentinel", "kind":"message", "timestamp":"2026", "text":"sentinel"})
}
fn python(bytes: &[u8], command: &str) -> std::process::Output {
    let s = Scratch::new("corpus-errors");
    fs::copy(
        repo().join("crew/_template/member.json"),
        s.0.join("member.json"),
    )
    .unwrap();
    fs::write(s.0.join("x.jsonl"), bytes).unwrap();
    let mut cmd = Command::new("python3");
    cmd.arg("-I")
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(s.0.join("member.json"));
    match command {
        "split" => {
            cmd.args([
                "split",
                "--corpus",
                "x.jsonl",
                "--questions",
                "q.jsonl",
                "--answers",
                "a.jsonl",
            ]);
        }
        "round" => {
            cmd.args(["merge", "--rounds", "x.jsonl", "--out", "o.jsonl"]);
        }
        _ => {
            cmd.args(["merge", "--in", "x.jsonl", "--out", "o.jsonl"]);
        }
    }
    cmd.current_dir(&s.0).output().unwrap()
}
#[test]
fn reader_errors_and_duplicate_key_order() {
    error(
        corpus::read_records("x.jsonl", b"\xff"),
        "x.jsonl is not UTF-8 text",
    );
    for bytes in [b"sentinel".as_slice(), b"[\"sentinel\"]", b"null"] {
        let message = if bytes == b"sentinel" {
            "a line is not valid JSON"
        } else {
            "a line is not a JSON object"
        };
        error(
            corpus::read_records("x.jsonl", bytes),
            &format!("x.jsonl:1: {message}"),
        );
        if parity_tools_available() {
            assert_eq!(python(bytes, "merge").status.code(), Some(1));
        }
    }
    let records =
        corpus::read_records("x.jsonl", b"\n \x1c\r\n{\"a\":1,\"b\":2,\"a\":3}\r\n").unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, 3);
    assert_eq!(
        records[0].1.keys().map(String::as_str).collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(records[0].1["a"], 3);
}
#[test]
fn numbers_are_exact_integers_only_including_nested_values_and_negative_zero() {
    for number in [
        "1.0",
        "1e16",
        "-0",
        "18446744073709551616",
        "-9223372036854775809",
    ] {
        let bytes = format!(r#"{{"sentinel":[{{"nested":{number}}}]}}"#);
        error(
            corpus::read_records("x.jsonl", bytes.as_bytes()),
            "x.jsonl:1: a record holds a number Persona Builder can't copy exactly",
        );
        if parity_tools_available() {
            let b = format!(
                r#"{{"id":"sentinel","kind":"message","timestamp":"now","text":"safe","extra":{number}}}"#
            );
            assert!(python(b.as_bytes(), "merge").status.success());
        }
    }
    assert!(
        corpus::read_records(
            "x.jsonl",
            br#"{"n":[-9223372036854775808,18446744073709551615,0]}"#
        )
        .is_ok()
    );
}
#[test]
fn json_python_accepts_but_serde_refuses() {
    for value in [
        "NaN".to_owned(),
        "Infinity".to_owned(),
        r#""\ud800""#.to_owned(),
        format!("{}0{}", "[".repeat(130), "]".repeat(130)),
    ] {
        let bytes = format!(
            r#"{{"id":"sentinel","kind":"message","timestamp":"now","text":"safe","extra":{value}}}"#
        );
        error(
            corpus::read_records("x.jsonl", bytes.as_bytes()),
            "x.jsonl:1: a line is not valid JSON",
        );
        if parity_tools_available() {
            let parsed = Command::new("python3")
                .args(["-I", "-c", "import json,sys;json.loads(sys.argv[1])"])
                .arg(&bytes)
                .output()
                .unwrap();
            assert!(parsed.status.success());
            // Python parses lone surrogates, but cannot write one as UTF-8.
            assert_eq!(
                python(bytes.as_bytes(), "merge").status.success(),
                value != r#""\ud800""#
            );
        }
    }
}
#[test]
fn merge_checks_round_then_id_then_privacy_then_timestamp() {
    for (key, mode) in [
        ("split", Mode::Round),
        ("id", Mode::History),
        ("timestamp", Mode::History),
        ("kind", Mode::History),
    ] {
        let mut r = record();
        r.as_object_mut().unwrap().remove(key);
        let bytes = serde_json::to_vec(&r).unwrap();
        let message = if key == "split" {
            "a round record's split must be train or heldout".into()
        } else if key == "kind" {
            "a record has an unexpected shape (kind)".into()
        } else {
            format!("a record has an unexpected shape ({key})")
        };
        error(merged(&bytes, mode), &format!("x.jsonl:1: {message}"));
        if parity_tools_available() {
            assert_eq!(
                python(
                    &bytes,
                    if mode == Mode::Round {
                        "round"
                    } else {
                        "merge"
                    }
                )
                .status
                .code(),
                Some(1)
            );
        }
    }
    for value in [json!(null), json!("other"), json!(1)] {
        let mut r = record();
        r["split"] = value;
        error(
            merged(&serde_json::to_vec(&r).unwrap(), Mode::Round),
            "x.jsonl:1: a round record's split must be train or heldout",
        );
    }
    let mut r = record();
    r["text"] = json!("my therapist");
    r.as_object_mut().unwrap().remove("timestamp");
    let mut m = member();
    m.filters.personal = "my therapist".into();
    let bytes = serde_json::to_vec(&r).unwrap();
    let f = Filters::compile(&m.filters).unwrap();
    let result = corpus::merge(
        &f,
        &[Input {
            name: "x.jsonl",
            mode: Mode::History,
            bytes: &bytes,
        }],
        &Split::Holdout(0.5),
    )
    .unwrap();
    assert_eq!(result.personal, 1);
    assert!(result.records.is_empty());
    let mut r = record();
    r["text"] = json!(["sentinel"]);
    error(
        merged(&serde_json::to_vec(&r).unwrap(), Mode::History),
        "x.jsonl:1: a record lacks text",
    );
}
#[test]
fn odd_types_are_explicit_differences() {
    for (key, value) in [
        ("id", json!(1)),
        ("id", json!(true)),
        ("timestamp", json!(1)),
        ("kind", json!(1)),
    ] {
        let mut r = record();
        r[key] = value;
        let bytes = serde_json::to_vec(&r).unwrap();
        error(
            merged(&bytes, Mode::History),
            &format!("x.jsonl:1: a record has an unexpected shape ({key})"),
        );
        if key != "kind" && parity_tools_available() {
            assert!(python(&bytes, "merge").status.success());
        }
    }
    let r = json!({"id":"sentinel","kind":"question","timestamp":"now","question":"safe","header":"scope","options":[],"picks":"A","own_words":"","notes":""});
    let bytes = serde_json::to_vec(&r).unwrap();
    error(
        merged(&bytes, Mode::History),
        "x.jsonl:1: a record lacks picks",
    );
    if parity_tools_available() {
        assert!(python(&bytes, "merge").status.success());
    }
}
#[test]
fn discover_and_duplicates_across_modes_and_empty_inputs() {
    let names: Vec<String> = [
        "decisions-z.jsonl",
        "decisions-A.jsonl",
        "decisions-interview.jsonl",
        "decisions-round-b.jsonl",
        "decisions-round-a.jsonl",
        "other.jsonl",
        "decisions.jsonl",
        "decisions-out.jsonl",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        corpus::discover(&names, "decisions-out.jsonl"),
        vec![
            ("decisions-A.jsonl".into(), Mode::History),
            ("decisions-z.jsonl".into(), Mode::History),
            ("decisions-interview.jsonl".into(), Mode::Train),
            ("decisions-round-a.jsonl".into(), Mode::Round),
            ("decisions-round-b.jsonl".into(), Mode::Round)
        ]
    );
    let f = Filters::compile(&member().filters).unwrap();
    error(
        corpus::merge(&f, &[], &Split::Holdout(0.5)),
        "merge needs at least one input",
    );
    let a = serde_json::to_vec(&record()).unwrap();
    let mut r = record();
    r["split"] = json!("train");
    let b = serde_json::to_vec(&r).unwrap();
    let result = corpus::merge(
        &f,
        &[
            Input {
                name: "r.jsonl",
                mode: Mode::Round,
                bytes: &b,
            },
            Input {
                name: "h.jsonl",
                mode: Mode::History,
                bytes: &a,
            },
        ],
        &Split::Cutoff("".into()),
    )
    .unwrap();
    assert_eq!(result.duplicates, 1);
    assert_eq!(result.records[0]["split"], "heldout");
}
#[test]
fn split_checks_shapes_and_copies_fields_in_order() {
    let m = member();
    for (r, key) in [
        (json!({"sentinel":true}), "kind"),
        (json!({"kind":1,"sentinel":true}), "kind"),
        (json!({"kind":"question","sentinel":true}), "split"),
        (
            json!({"kind":"question","split":"heldout","sentinel":true}),
            "id",
        ),
    ] {
        let bytes = serde_json::to_vec(&r).unwrap();
        error(
            corpus::split(&m, "x.jsonl", &bytes),
            &format!("x.jsonl:1: a record has an unexpected shape ({key})"),
        );
        if (key != "kind" || r.get("kind").is_none()) && parity_tools_available() {
            assert_eq!(python(&bytes, "split").status.code(), Some(1));
        }
    }
    let keys = [
        "id",
        "timestamp",
        "project",
        "header",
        "question",
        "multiSelect",
        "options",
        "recommended",
    ];
    let mut r = json!({"kind":"question","split":"heldout"});
    for key in keys.into_iter().chain([
        m.pick_field().as_str(),
        "picks",
        "own_words",
        "notes",
        "outcome",
    ]) {
        r[key] = json!(null);
    }
    for key in keys.into_iter().chain([
        m.pick_field().as_str(),
        "picks",
        "own_words",
        "notes",
        "outcome",
    ]) {
        let mut bad = r.clone();
        bad.as_object_mut().unwrap().remove(key);
        let bytes = serde_json::to_vec(&bad).unwrap();
        error(
            corpus::split(&m, "x.jsonl", &bytes),
            &format!("x.jsonl:1: a record has an unexpected shape ({key})"),
        );
        if parity_tools_available() {
            assert_eq!(python(&bytes, "split").status.code(), Some(1));
        }
    }
    let held = corpus::split(&m, "x.jsonl", &serde_json::to_vec(&r).unwrap()).unwrap();
    assert_eq!(
        held.questions[0]
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        keys
    );
    assert_eq!(
        held.answers[0]
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "id",
            "recommended",
            m.pick_field().as_str(),
            "picks",
            "own_words",
            "notes",
            "outcome",
            "multiSelect"
        ]
    );
    for r in [
        json!({"kind":"message"}),
        json!({"kind":"question","split":true}),
    ] {
        assert!(
            corpus::split(&m, "x.jsonl", &serde_json::to_vec(&r).unwrap())
                .unwrap()
                .questions
                .is_empty()
        );
    }
}

#[test]
fn universal_newlines_count_blank_lines_in_errors() {
    // The third record is on line four after universal-newline translation.
    let bytes = b"{}\r\r{}\r\n[\"sentinel\"]\n";
    error(
        corpus::read_records("x.jsonl", bytes),
        "x.jsonl:4: a line is not a JSON object",
    );
}
