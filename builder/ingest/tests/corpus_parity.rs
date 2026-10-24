mod common;
use common::{Scratch, parity_tools_available, repo};
use persona_builder::member::read_member;
use persona_builder_ingest::{
    corpus::{self, Input, Mode},
    filters::Filters,
    mine::{Split, to_jsonl},
};
use std::{fs, process::Command};

fn compare(case: &str, ari: bool, flags: &[&str]) {
    let s = Scratch::new("corpus-parity");
    let fixture = Command::new("python3")
        .arg("-I")
        .arg(repo().join("tests/crew_merge_fixtures.py"))
        .arg(case)
        .arg(&s.0)
        .output()
        .unwrap();
    assert!(
        fixture.status.success(),
        "{}",
        String::from_utf8_lossy(&fixture.stderr)
    );
    let mut args: Vec<String> = serde_json::from_slice(&fixture.stdout).unwrap();
    args.extend(flags.iter().map(|s| s.to_string()));
    let config = s.0.join("member.json");
    let py_config = Command::new("python3").arg("-I").arg("-c").arg(
        "import sys,json;sys.path.insert(0,sys.argv[1]);from crew_decisions_test import ADA,ARI;c=dict(ARI if sys.argv[2]=='ari' else ADA);c['state_dir']='/fictional/state';print(json.dumps(c))")
        .arg(repo().join("tests")).arg(if ari {"ari"} else {"ada"}).output().unwrap();
    assert!(py_config.status.success());
    fs::write(&config, py_config.stdout).unwrap();
    let member = read_member(&config).unwrap();
    let filters = Filters::compile(&member.filters).unwrap();
    let output = s.0.join("oracle.jsonl");
    let py = Command::new("python3")
        .arg("-I")
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(&config)
        .arg("merge")
        .args(&args)
        .arg("--out")
        .arg(&output)
        .current_dir(&s.0)
        .output()
        .unwrap();
    assert!(
        py.status.success(),
        "{case}: {}",
        String::from_utf8_lossy(&py.stderr)
    );
    let mut mode = Mode::History;
    let mut names = Vec::new();
    let mut split = Split::Holdout(0.15);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--in" => mode = Mode::History,
            "--train" => mode = Mode::Train,
            "--rounds" => mode = Mode::Round,
            "--holdout" => {
                i += 1;
                split = Split::Holdout(args[i].parse().unwrap());
            }
            "--cutoff" => {
                i += 1;
                split = Split::Cutoff(args[i].clone());
            }
            name => names.push((name.to_owned(), mode, fs::read(s.0.join(name)).unwrap())),
        }
        i += 1;
    }
    let inputs: Vec<_> = names
        .iter()
        .map(|(name, mode, bytes)| Input {
            name,
            mode: *mode,
            bytes,
        })
        .collect();
    let merged = corpus::merge(&filters, &inputs, &split).unwrap();
    assert_eq!(
        to_jsonl(&merged.records).as_bytes(),
        fs::read(&output).unwrap(),
        "{case} {flags:?}"
    );
    assert_eq!(
        corpus::merge_summary(&merged).as_bytes(),
        py.stdout,
        "{case} {flags:?}"
    );
    let q = s.0.join("q.jsonl");
    let a = s.0.join("answers.jsonl");
    let py = Command::new("python3")
        .arg("-I")
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(config)
        .arg("split")
        .arg("--corpus")
        .arg(&output)
        .arg("--questions")
        .arg(&q)
        .arg("--answers")
        .arg(&a)
        .output()
        .unwrap();
    assert!(py.status.success());
    let held = corpus::split(&member, "oracle.jsonl", &fs::read(output).unwrap()).unwrap();
    assert_eq!(to_jsonl(&held.questions).as_bytes(), fs::read(q).unwrap());
    assert_eq!(to_jsonl(&held.answers).as_bytes(), fs::read(a).unwrap());
    assert_eq!(corpus::split_summary(&held).as_bytes(), py.stdout);
}
#[test]
fn every_fixture_and_split_matches_python() {
    if !parity_tools_available() {
        return;
    }
    for ari in [false, true] {
        for case in [
            "two_machines",
            "interview_train",
            "rounds_keep_split",
            "rounds_only",
            "messages_only",
            "refilter",
            "ties",
            "unicode",
            "shapes",
            "whitespace_lines",
            "bare_cr",
        ] {
            compare(case, ari, &[]);
            if case != "rounds_only" {
                for flags in [
                    ["--holdout", "0"],
                    ["--holdout", "1"],
                    ["--holdout", "0.5"],
                    ["--cutoff", "2026-09-02T10:00:00Z"],
                    ["--cutoff", ""],
                ] {
                    compare(case, ari, &flags);
                }
            }
        }
    }
}

#[test]
fn bare_cr_matches_python() {
    if !parity_tools_available() {
        return;
    }
    compare("bare_cr", false, &[]);
}
