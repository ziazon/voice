mod common;
use common::{Scratch, parity_tools_available, repo};
use persona_builder::{error::Error, member::read_member};
use persona_builder_ingest::{
    filters::Filters,
    privacy::{filtered_text, privacy},
};
use serde_json::{Value, json};
use std::{fs, process::Command};

#[test]
fn records_match_the_real_python() {
    if !parity_tools_available() {
        return;
    }
    let scratch = Scratch::new("records");
    let member = scratch.0.join("member.json");
    fs::copy(repo().join("crew/_template/member.json"), &member).unwrap();
    let filters = Filters::compile(&read_member(&member).unwrap().filters).unwrap();
    let mut records = vec![];
    for text in [
        "Keep the scope small.",
        "MY DOCTOR called.",
        "wren@example.org",
        "my doctor wren@example.org",
        "Café 日本語",
        "café\nmy doctor",
    ] {
        records.push(json!({"kind":"message", "text":text}));
    }
    for (field, text) in [
        ("question", "Which scope?"),
        ("context", "my doctor"),
        ("project", "wren@example.org"),
        ("header", "my doctor"),
        ("description", "wren@example.org"),
        ("notes", "my doctor wren@example.org"),
    ] {
        let mut r = json!({"kind":"question", "question":"Quelle portée?", "options":[{"label":"Small", "description":"Ship a slice"}, {"label":"Wide", "description":"Wait"}], "picks":["Small"], "own_words":"Keep it clear", "notes":"", "header":"Scope"});
        if field == "description" {
            r["options"][0][field] = json!(text);
        } else {
            r[field] = json!(text);
        }
        records.push(r);
    }
    let input = scratch.0.join("records.json");
    fs::write(&input, serde_json::to_vec(&records).unwrap()).unwrap();
    let out = Command::new("python3")
        .args([
            "-I",
            "-c",
            r#"
import importlib.util, json, logging, sys
logging.disable(logging.CRITICAL)
spec = importlib.util.spec_from_file_location("oracle", sys.argv[1])
m = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = m
spec.loader.exec_module(m)
member = m.load_member(json.load(open(sys.argv[2])))
print(json.dumps([[m.filtered(r), m.privacy(member, r)] for r in json.load(open(sys.argv[3]))]))
"#,
        ])
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg(member)
        .arg(input)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let expected: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
    for (record, expected) in records.iter().zip(expected) {
        assert_eq!(
            filtered_text(record).unwrap(),
            expected[0].as_str().unwrap()
        );
        assert_eq!(
            privacy(&filters, record).unwrap().map(|k| k.as_str()),
            expected[1].as_str()
        );
    }
}

#[test]
fn missing_and_wrong_types_name_the_required_key() {
    let err = filtered_text(&json!({})).unwrap_err();
    assert!(matches!(err, Error::Invalid(ref s) if s == "a record lacks kind"));
    let question = json!({"kind":"question", "question":"Q", "options":[{"label":"A", "description":"D"}], "picks":[], "own_words":"", "notes":"", "header":"H"});
    for key in [
        "kind",
        "question",
        "options",
        "picks",
        "own_words",
        "notes",
        "header",
    ] {
        for missing in [true, false] {
            let mut r = question.clone();
            if missing {
                r.as_object_mut().unwrap().remove(key);
            } else {
                r[key] = json!(42);
            }
            assert_eq!(
                filtered_text(&r).unwrap_err().to_string(),
                format!("a record lacks {key}")
            );
        }
    }
    for key in ["label", "description"] {
        let mut r = question.clone();
        r["options"][0].as_object_mut().unwrap().remove(key);
        assert_eq!(
            filtered_text(&r).unwrap_err().to_string(),
            format!("a record lacks {key}")
        );
    }
    assert_eq!(
        filtered_text(&json!({"kind":"message"}))
            .unwrap_err()
            .to_string(),
        "a record lacks text"
    );
}
