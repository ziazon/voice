mod common;
use common::{Scratch, parity_tools_available, repo};
use persona_builder::member::read_member;
use persona_builder_ingest::{
    filters::Filters,
    mine::{Split, mine, summary, to_jsonl},
};
use std::{fs, process::Command};
#[test]
fn basic_real_python_parity() {
    if !parity_tools_available() {
        return;
    }
    let s = Scratch::new("mine-parity");
    let projects = s.0.join("projects");
    fs::create_dir_all(projects.join("-home-sam-projects-kit")).unwrap();
    fs::write(projects.join("-home-sam-projects-kit/a.jsonl"),"{\"type\":\"user\",\"timestamp\":\"2026-09-01\",\"origin\":{\"kind\":\"human\"},\"message\":{\"content\":\"Keep this scope small\"}}\n").unwrap();
    let config = s.0.join("member.json");
    fs::copy(repo().join("crew/_template/member.json"), &config).unwrap();
    let member = read_member(&config).unwrap();
    let f = Filters::compile(&member.filters).unwrap();
    let out = s.0.join("expected.jsonl");
    let py = Command::new("python3")
        .args(["-I"])
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(&config)
        .arg("mine")
        .arg("--projects")
        .arg(&projects)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        py.status.success(),
        "{}",
        String::from_utf8_lossy(&py.stderr)
    );
    let mined = mine(&member, &f, &projects, &s.0, &Split::Holdout(0.15)).unwrap();
    assert_eq!(to_jsonl(&mined.records), fs::read_to_string(out).unwrap());
    assert_eq!(summary(&mined), String::from_utf8(py.stdout).unwrap());
}

fn config(ari: bool) -> serde_json::Value {
    serde_json::json!({"name":if ari{"Ari"}else{"Ada"},"slug":if ari{"ari"}else{"ada"},"pronouns":"they/them","principal":if ari{"Robin"}else{"Sam"},"principal_slug":if ari{"robin"}else{"sam"},"state_dir":"/fictional/state","mark_word":if ari{"Ari"}else{"Ada"},"filters":{"excluded_folders":if ari{r"hoa-board|^-private-"}else{r"garden-club|^-private-"},"personal":if ari{r"\bmy doctor\b"}else{r"\bmy therapist\b"},"secret":if ari{r"\b\d{7,}\b|[\w.+-]+@[\w-]+\.[a-z]{2,}"}else{r"\b\d{9,}\b"},"pasted_prefixes":[if ari{"Pasted: "}else{"You are "}]}})
}
fn fixture(case: &str, s: &Scratch) -> std::path::PathBuf {
    let p = s.0.join("projects");
    let out = Command::new("python3")
        .arg("-I")
        .arg(repo().join("tests/crew_mine_fixtures.py"))
        .arg(case)
        .arg(&p)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "fixture {case}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    p
}
fn compare(case: &str, split: Split, flags: &[&str]) {
    let s = Scratch::new("mining-cases");
    let p = fixture(case, &s);
    let c = s.0.join("member.json");
    fs::write(
        &c,
        serde_json::to_vec(&config(
            ["another_member", "private_overlay"].contains(&case),
        ))
        .unwrap(),
    )
    .unwrap();
    if case == "private_overlay" {
        fs::write(s.0.join("private.json"),r#"{"filters":{"excluded_folders":"garden-club","personal":"\\bmy cat\\b","secret":"\\bpin \\d{4}\\b","pasted_prefixes":["Quoted: "]}}"#).unwrap()
    }
    let m = read_member(&c).unwrap();
    let f = Filters::compile(&m.filters).unwrap();
    let output = s.0.join("oracle.jsonl");
    let home = if case == "dotted_home" {
        "/Users/jane.doe"
    } else {
        "/Users/p"
    };
    let py = Command::new("python3")
        .env("HOME", home)
        .arg("-I")
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(&c)
        .arg("mine")
        .arg("--projects")
        .arg(&p)
        .arg("--out")
        .arg(&output)
        .args(flags)
        .output()
        .unwrap();
    assert!(
        py.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&py.stderr)
    );
    let result = mine(&m, &f, &p, std::path::Path::new(home), &split).unwrap();
    assert_eq!(
        to_jsonl(&result.records).as_bytes(),
        fs::read(output).unwrap(),
        "JSONL case {case} flags {flags:?}"
    );
    assert_eq!(
        summary(&result).as_bytes(),
        py.stdout,
        "summary {case} flags {flags:?}"
    );
}
#[test]
fn all_accepted_fixtures_match_python_bytes_and_summary() {
    if !parity_tools_available() {
        return;
    }
    for case in [
        "ada_fixture",
        "five_questions",
        "another_member",
        "multi_select",
        "list_answers",
        "single_select",
        "notes",
        "tolerant_questions",
        "typed_text",
        "context",
        "bytes",
        "timestamps",
        "folder_names",
        "dotted_home",
        "duplicates",
        "empty",
        "no_sessions",
        "private_overlay",
    ] {
        compare(case, Split::Holdout(0.15), &[])
    }
    for value in ["0", "1", "0.5"] {
        compare(
            "ada_fixture",
            Split::Holdout(value.parse().unwrap()),
            &["--holdout", value],
        );
    }
    compare("five_questions", Split::Holdout(0.5), &["--holdout", "0.5"]);
    for value in ["2026-09-02T10:00:00Z", "9999", ""] {
        compare(
            "ada_fixture",
            Split::Cutoff(value.into()),
            &["--cutoff", value],
        );
    }
}
#[test]
fn malformed_records_are_skipped_and_counted() {
    if !parity_tools_available() {
        return;
    }
    for key in [
        "record",
        "message",
        "timestamp",
        "content",
        "origin",
        "options",
        "answer",
    ] {
        let s = Scratch::new("shape");
        let p = fixture(&format!("error_{key}"), &s);
        let c = s.0.join("member.json");
        fs::write(&c, serde_json::to_vec(&config(false)).unwrap()).unwrap();
        let m = read_member(&c).unwrap();
        let f = Filters::compile(&m.filters).unwrap();
        let mined = mine(
            &m,
            &f,
            &p,
            std::path::Path::new("/Users/p"),
            &Split::Holdout(0.15),
        )
        .unwrap();
        assert_eq!(mined.unreadable, 1);
        assert!(!to_jsonl(&mined.records).contains("private record sentinel"));
        assert!(
            summary(&mined)
                .lines()
                .nth(1)
                .unwrap()
                .ends_with("; skipped 1 records of an unexpected shape")
        );
        let out = s.0.join("not-written.jsonl");
        let py = Command::new("python3")
            .arg("-I")
            .arg(repo().join("scripts/crew_decisions.py"))
            .arg("--member")
            .arg(&c)
            .arg("mine")
            .arg("--projects")
            .arg(p)
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert_eq!(py.status.code(), Some(1), "{key}");
        if key != "timestamp" {
            assert!(!out.exists(), "{key}");
        }
    }
}

#[test]
fn empty_options_object_is_an_explicit_accepted_difference() {
    if !parity_tools_available() {
        return;
    }
    let s = Scratch::new("empty-options");
    let projects = fixture("error_options", &s);
    let session = projects.join("-Users-p-projects-app/s1.jsonl");
    let mut record: serde_json::Value =
        serde_json::from_slice(&fs::read(&session).unwrap()).unwrap();
    record["toolUseResult"]["questions"][0]["options"] = serde_json::json!({});
    fs::write(&session, serde_json::to_vec(&record).unwrap()).unwrap();
    let c = s.0.join("member.json");
    fs::write(&c, serde_json::to_vec(&config(false)).unwrap()).unwrap();
    let m = read_member(&c).unwrap();
    let f = Filters::compile(&m.filters).unwrap();
    let mined = mine(
        &m,
        &f,
        &projects,
        std::path::Path::new("/Users/p"),
        &Split::Holdout(0.15),
    )
    .unwrap();
    assert_eq!(mined.unreadable, 1);
    assert!(mined.records.is_empty());
    assert!(summary(&mined).contains("; skipped 1 records of an unexpected shape"));
    let output = s.0.join("python.jsonl");
    let py = Command::new("python3")
        .arg("-I")
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(c)
        .arg("mine")
        .arg("--projects")
        .arg(projects)
        .arg("--out")
        .arg(&output)
        .output()
        .unwrap();
    assert!(py.status.success());
    assert!(!fs::read(output).unwrap().is_empty());
}

#[test]
fn empty_mark_word_matches_python() {
    if !parity_tools_available() {
        return;
    }
    let s = Scratch::new("empty-mark");
    let projects = s.0.join("projects");
    let folder = projects.join("projects-app");
    fs::create_dir_all(&folder).unwrap();
    let record = serde_json::json!({"type":"user","timestamp":"2026-09-01","toolUseResult":{"questions":[{"question":"Which scope?","options":[{"label":"A ()"}]}],"answers":{"Which scope?":"A ()"}}});
    fs::write(folder.join("s.jsonl"), serde_json::to_vec(&record).unwrap()).unwrap();
    let mut config = config(false);
    config["mark_word"] = serde_json::json!("");
    let c = s.0.join("member.json");
    fs::write(&c, serde_json::to_vec(&config).unwrap()).unwrap();
    let m = read_member(&c).unwrap();
    let f = Filters::compile(&m.filters).unwrap();
    let expected = s.0.join("expected.jsonl");
    let py = Command::new("python3")
        .env("HOME", "/Users/p")
        .arg("-I")
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(c)
        .arg("mine")
        .arg("--projects")
        .arg(&projects)
        .arg("--out")
        .arg(&expected)
        .output()
        .unwrap();
    assert!(py.status.success());
    let mined = mine(
        &m,
        &f,
        &projects,
        std::path::Path::new("/Users/p"),
        &Split::Holdout(0.15),
    )
    .unwrap();
    assert_eq!(
        to_jsonl(&mined.records),
        fs::read_to_string(expected).unwrap()
    );
}
