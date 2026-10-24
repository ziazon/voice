mod common;

use common::{Scratch, parity_tools_available, repo};
use persona_builder::{json::to_python_json, member::read_member};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const MEMBER: &str = r#"{
  "name": "Ari",
  "slug": "ari",
  "pronouns": "they/them",
  "principal": "Robin",
  "principal_slug": "robin",
  "principal_pronouns": "they/them",
  "state_dir": "~/.claude/state/ari",
  "mark_word": "Ari",
  "filters": {
    "excluded_folders": "private",
    "personal": "family",
    "secret": "password",
    "pasted_prefixes": ["You are "]
  }
}"#;

fn python_read(path: &Path) -> Output {
    Command::new("python3")
        .args([
            "-I",
            "-c",
            r#"
import importlib.util, json, logging, sys
# Optional hashlib algorithms can log during import on minimal Python builds.
# Only the member reader's Failed message belongs on stderr for this oracle.
logging.disable(logging.CRITICAL)
spec = importlib.util.spec_from_file_location("parity_crew_decisions", sys.argv[1])
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
try:
    print(json.dumps(module.read_member(sys.argv[2]), indent=2))
except module.Failed as exc:
    print(str(exc), file=sys.stderr)
    sys.exit(1)
"#,
        ])
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg(path)
        .output()
        .unwrap()
}

fn valid_case(label: &str, member: &str, private: Option<&str>) {
    if !parity_tools_available() {
        return;
    }
    let scratch = Scratch::new(label);
    let path = scratch.0.join("member.json");
    fs::write(&path, member).unwrap();
    if let Some(private) = private {
        fs::write(scratch.0.join("private.json"), private).unwrap();
    }
    let python = python_read(&path);
    assert!(
        python.status.success(),
        "{}",
        String::from_utf8_lossy(&python.stderr)
    );
    let rust = read_member(&path);
    assert!(
        rust.is_ok(),
        "Rust must accept Python's valid config: {rust:?}"
    );
    assert_eq!(
        to_python_json(&rust.unwrap()).unwrap().as_bytes(),
        python.stdout,
        "Rust and Python member JSON must be byte-identical"
    );
}

#[test]
fn plain_member_matches_python_bytes() {
    valid_case("plain", MEMBER, None);
}

#[test]
fn private_overlay_matches_python_bytes() {
    valid_case(
        "private",
        MEMBER,
        Some(
            r#"{
      "filters": {"personal": "household", "pasted_prefixes": ["Caveat:"]},
      "codebook": {"categories": ["scope"], "domains": ["work"],
                   "escalates": ["privacy"], "signals": ["ruling"],
                   "groups": {"g1": ["scope"]}}
    }"#,
        ),
    );
}

#[test]
fn queue_answerer_matches_python_bytes() {
    let mut value: serde_json::Value = serde_json::from_str(MEMBER).unwrap();
    value["queue_answerer"] = serde_json::json!("Ari");
    valid_case("queue", &serde_json::to_string(&value).unwrap(), None);
}

fn invalid_case(label: &str, config: &str) {
    if !parity_tools_available() {
        return;
    }
    let scratch = Scratch::new(label);
    let path = scratch.0.join("member.json");
    fs::write(&path, config).unwrap();
    let python = python_read(&path);
    assert!(
        !python.status.success(),
        "Python must refuse the invalid config"
    );
    let rust = read_member(&path).expect_err("Rust must refuse the invalid config");
    assert_eq!(
        format!("{rust}\n").as_bytes(),
        python.stderr,
        "Rust error must equal Python's Failed text"
    );
}

#[test]
fn invalid_filters_matches_python_failed_text() {
    invalid_case("invalid-filters", r#"{"filters": []}"#);
}

#[test]
fn invalid_queue_answerer_matches_python_failed_text() {
    invalid_case("invalid-answerer", r#"{"queue_answerer": ""}"#);
}

#[test]
fn missing_tools_fail_only_when_parity_is_required() {
    for required in [true, false] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "plain_member_matches_python_bytes",
                "--nocapture",
            ])
            .env("PATH", "/nonexistent-dir");
        if required {
            command.env("PERSONA_BUILDER_REQUIRE_PARITY", "1");
        } else {
            command.env_remove("PERSONA_BUILDER_REQUIRE_PARITY");
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.success(),
            !required,
            "required={required}: {output:?}"
        );
        let message = if required {
            "parity tools missing: python3, git or bash not on PATH"
        } else {
            "skipped: python3, git or bash not found"
        };
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
}
