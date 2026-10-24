mod common;
use common::{Scratch, parity_tools_available, repo};
use persona_builder::member::read_member;
use persona_builder_ingest::{
    claudeai::{self, ReviewRow, Selector},
    filters::Filters,
    mine::{Split, to_jsonl},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Case {
    dir: PathBuf,
    spec: Value,
    config: PathBuf,
    filters: Filters,
}
impl Case {
    fn new(name: &str, s: &Scratch) -> Self {
        let dir = s.0.join(name);
        let fixture = Command::new("python3")
            .arg("-I")
            .arg(repo().join("tests/crew_claudeai_fixtures.py"))
            .arg(name)
            .arg(&dir)
            .output()
            .unwrap();
        assert!(
            fixture.status.success(),
            "fixture {name}: {}",
            String::from_utf8_lossy(&fixture.stderr)
        );
        let spec: Value = serde_json::from_slice(&fixture.stdout).unwrap();
        let config = dir.join("member.json");
        let py_config = Command::new("python3").arg("-I").arg("-c").arg(
            "import sys,json;sys.path.insert(0,sys.argv[1]);from crew_decisions_test import ADA,ARI;c=dict(ARI if sys.argv[2]=='ari' else ADA);c['state_dir']='/fictional/state';print(json.dumps(c))")
            .arg(repo().join("tests")).arg(spec["member"].as_str().unwrap()).output().unwrap();
        assert!(py_config.status.success());
        fs::write(&config, py_config.stdout).unwrap();
        let member = read_member(&config).unwrap();
        let filters = Filters::compile(&member.filters).unwrap();
        Self {
            dir,
            spec,
            config,
            filters,
        }
    }
    fn python(&self, args: &[&str]) -> Output {
        Command::new("python3")
            .arg("-I")
            .arg(repo().join("scripts/crew_decisions.py"))
            .arg("--member")
            .arg(&self.config)
            .arg("claudeai")
            .args(args)
            .output()
            .unwrap()
    }
    fn path(&self, name: &str) -> String {
        self.dir.join(name).to_str().unwrap().to_owned()
    }
    fn strings(&self, key: &str) -> Vec<String> {
        self.spec[key]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
            .unwrap_or_default()
    }
    fn lists(&self, key: &str) -> Vec<Vec<String>> {
        self.spec[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect()
            })
            .collect()
    }
}
fn ok(case: &str, step: &str, out: &Output) -> String {
    assert!(
        out.status.success(),
        "{case} {step}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout.clone()).unwrap()
}
fn strs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}
fn split_of(args: &[String]) -> Split {
    match strs(args)[..] {
        [] => Split::Holdout(0.15),
        ["--holdout", h] => Split::Holdout(h.parse().unwrap()),
        ["--cutoff", c] => Split::Cutoff(c.into()),
        _ => panic!("mine args {args:?}"),
    }
}
fn selector_of(args: &[String]) -> (Selector, String) {
    let mut s = Selector::default();
    let mut mark = String::new();
    let mut i = 0;
    while i < args.len() {
        let value = || args[i + 1].clone();
        let given = |v: String| Some(v).filter(|v| !v.is_empty());
        match args[i].as_str() {
            "--uuid" => s.uuids.push(value()),
            "--project" => s.project = given(value()),
            "--title" => s.title = given(value()).map(|t| claudeai::title_pattern(&t).unwrap()),
            "--before" => s.before = given(value()),
            "--after" => s.after = given(value()),
            "--as" => mark = value(),
            "--unmarked" => {
                s.unmarked = true;
                i += 1;
                continue;
            }
            "--all" => {
                s.all = true;
                i += 1;
                continue;
            }
            other => panic!("mark arg {other}"),
        }
        i += 2;
    }
    (s, mark)
}
fn compare_mine(c: &Case, name: &str, export: &claudeai::Export, rows: &[ReviewRow], review: &str) {
    for (n, args) in c.lists("mine").iter().enumerate() {
        let out = c.path(&format!("oracle-{n}.jsonl"));
        let export_path = c.path(c.spec["export"].as_str().unwrap());
        let mut py_args = vec!["mine", "--export", &export_path];
        py_args.extend(["--review", review, "--out", &out]);
        py_args.extend(strs(args));
        let stdout = ok(name, "mine", &c.python(&py_args));
        let mined = claudeai::mine(&c.filters, export, rows, &split_of(args)).unwrap();
        assert_eq!(
            to_jsonl(&mined.records),
            fs::read_to_string(&out).unwrap(),
            "{name} mine {args:?}"
        );
        assert_eq!(
            claudeai::mine_summary(&mined),
            stdout,
            "{name} mine {args:?}"
        );
    }
}
fn compare(name: &str) {
    if !parity_tools_available() {
        return;
    }
    let s = Scratch::new("claudeai-parity");
    let c = Case::new(name, &s);
    let export_path = c.path(c.spec["export"].as_str().unwrap());
    let export = claudeai::read_export(Path::new(&export_path)).unwrap();
    let inspection = claudeai::inspect(&export).unwrap();
    let py = ok(
        name,
        "inspect",
        &c.python(&["inspect", "--export", &export_path]),
    );
    assert!(
        inspection.missing.is_empty(),
        "{name}: {:?}",
        inspection.missing
    );
    assert_eq!(
        format!(
            "{}ok: every field the adapter reads is present\n",
            inspection.text
        ),
        py,
        "{name} inspect"
    );
    if let Some(broken) = c.spec["broken"].as_str() {
        let broken = c.path(broken);
        let py = c.python(&["inspect", "--export", &broken]);
        assert_eq!(py.status.code(), Some(1), "{name} broken inspect");
        let inspection =
            claudeai::inspect(&claudeai::read_export(Path::new(&broken)).unwrap()).unwrap();
        assert_eq!(inspection.text, String::from_utf8(py.stdout).unwrap());
        assert_eq!(
            inspection.missing.join(", "),
            ".created_at, .chat_messages[].sender, .chat_messages[].text or .chat_messages[].content[].text"
        );
        assert!(String::from_utf8_lossy(&py.stderr).contains(
            "missing: .created_at, .chat_messages[].sender, .chat_messages[].text or .chat_messages[].content[].text"
        ));
    }
    if c.spec["titles"].is_null() {
        return;
    }
    let review = c.path("oracle.tsv");
    let mut rows = vec![];
    if let Some(hand) = c.spec["review"].as_str() {
        fs::copy(c.path(hand), &review).unwrap();
        rows = claudeai::read_review(hand, &fs::read(&review).unwrap()).unwrap();
        compare_mine(&c, name, &export, &rows, &review);
    }
    let mut titles_args = vec!["titles", "--export", &export_path, "--out", &review];
    let extra = c.strings("titles");
    titles_args.extend(strs(&extra));
    let py = ok(name, "titles", &c.python(&titles_args));
    let project_field = extra.get(1).map(String::as_str);
    let titles = claudeai::titles(&c.filters, &export, &rows, project_field).unwrap();
    assert_eq!(
        claudeai::titles_summary(&titles, &review),
        py,
        "{name} titles"
    );
    let mut rust_review = claudeai::review_text(&titles.rows);
    assert_eq!(
        rust_review,
        fs::read_to_string(&review).unwrap(),
        "{name} review"
    );
    for args in c.lists("marks") {
        let mut mark_args = vec!["mark", "--review", &review];
        mark_args.extend(strs(&args));
        let py = ok(name, "mark", &c.python(&mark_args));
        let mut rows = claudeai::read_review("review", rust_review.as_bytes()).unwrap();
        let (selector, mark) = selector_of(&args);
        let changed = claudeai::mark(&mut rows, &selector, &mark);
        assert_eq!(
            format!("marked {changed} conversations {mark}\n"),
            py,
            "{name} mark {args:?}"
        );
        rust_review = claudeai::review_text(&rows);
        assert_eq!(
            rust_review,
            fs::read_to_string(&review).unwrap(),
            "{name} mark {args:?}"
        );
    }
    let rows = claudeai::read_review("review", rust_review.as_bytes()).unwrap();
    compare_mine(&c, name, &export, &rows, &review);
}

#[test]
fn basic_zip_with_a_project_field() {
    compare("basic");
}
#[test]
fn an_export_folder() {
    compare("folder");
}
#[test]
fn a_bare_conversations_file() {
    compare("plain");
}
#[test]
fn conversations_nested_in_the_zip() {
    compare("nested_zip");
}
#[test]
fn untitled_conversations_show_opening_words() {
    compare("untitled");
}
#[test]
fn timestamps_in_every_shared_form() {
    compare("timestamps");
}
#[test]
fn text_and_content_blocks() {
    compare("blocks");
}
#[test]
fn messages_through_the_filters() {
    compare("messages");
}
#[test]
fn messages_through_another_members_filters() {
    compare("messages_ada");
}
#[test]
fn a_hand_edited_review_file() {
    compare("marks");
}
#[test]
fn unicode_titles_and_text() {
    compare("unicode");
}
#[test]
fn inspect_on_odd_shapes() {
    compare("inspect_shapes");
}
