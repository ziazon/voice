//! `persona-builder interview draft|answer` against a stub `claude` on `PATH`, on an invented member.
use persona_builder::crew::{NewMember, new_member};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    home: PathBuf,
    member: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-interview-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        let member = home.join("crew/wren");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            &member,
            &NewMember {
                name: "Wren",
                pronouns: "she/her",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-10-24",
            },
        )
        .unwrap();
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        Self { home, member, bin }
    }
    /// A signed-in Claude Code whose model calls print `reply` and count themselves.
    fn signed_in(&self, reply: &str) {
        fs::write(self.bin.join("reply"), reply).unwrap();
        self.stub(
            "here=\"$(cd \"$(dirname \"$0\")\" && pwd -P)\"\ncase \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":true}';; *) cat > \"$here/stdin\"; echo x >> \"$here/calls\"; cat \"$here/reply\";; esac",
        );
    }
    fn stub(&self, body: &str) {
        let program = self.bin.join("claude");
        fs::write(&program, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn calls(&self) -> usize {
        fs::read_to_string(self.bin.join("calls"))
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", &self.home)
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env_remove("ANTHROPIC_API_KEY")
            .args(args)
            .output()
            .unwrap()
    }
    fn member(&self) -> String {
        self.member.to_str().unwrap().to_owned()
    }
    fn state(&self) -> PathBuf {
        self.home.join(".claude/state/wren")
    }
    fn write_state(&self, name: &str, text: &str) {
        fs::create_dir_all(self.state()).unwrap();
        fs::set_permissions(self.state(), fs::Permissions::from_mode(0o700)).unwrap();
        let path = self.state().join(name);
        fs::write(&path, text).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Twenty questions over the template's kinds of decision.
fn draft() -> Value {
    let kinds = ["scope", "sequencing", "methods", "verification", "go-no-go"];
    let questions: Vec<Value> = (1..=20)
        .map(|n| {
            json!({"n": n, "header": kinds[(n - 1) % kinds.len()], "question": format!("Situation {n}: what first?"),
                   "multiSelect": false, "recommended": "A",
                   "options": [{"label": "Cut it", "description": "Less work"},
                               {"label": "Keep it", "description": "More work"}]})
        })
        .collect();
    json!({"questions": questions})
}

fn ok_reply(output: &Value) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": "",
           "total_cost_usd": 0.02, "usage": {"input_tokens": 900, "output_tokens": 2000},
           "structured_output": output})
    .to_string()
}

fn corpus(questions: usize) -> String {
    (0..questions)
        .map(|n| {
            format!(
                "{}\n",
                json!({"id": format!("question:{n}"), "kind": "question", "timestamp": "2026-09-01T10:00:00.000Z",
                       "project": "kit", "header": "Scope", "question": "Keep the queue?", "multiSelect": false,
                       "options": [{"label": "Yes", "description": ""}, {"label": "No", "description": ""}],
                       "recommended": null, "picks": ["Yes"], "own_words": "", "notes": "",
                       "outcome": "no_rec", "split": "train", "wren_pick": null})
            )
        })
        .collect()
}

#[test]
fn draft_asks_nothing_when_the_history_is_not_thin() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    f.write_state("decisions.jsonl", &corpus(150));
    f.write_state("report.json", &long_history_report(false));
    f.write_state("interview-short-questions.json", &short_draft().to_string());
    let out = f.run(&["interview", "draft", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        "Your history already holds 150 answered questions, enough to build from, and no kind of decision is thin.\n"
    );
    assert_eq!(f.calls(), 0);
    assert!(!f.state().join("runs.jsonl").exists());
    assert!(!f.state().join("interview-questions.json").exists());
}

#[test]
fn draft_saves_twenty_questions_for_a_thin_history() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    f.write_state("decisions.jsonl", &corpus(3));
    let out = f.run(&["interview", "draft", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(f.calls(), 1);
    let saved = f.state().join("interview-questions.json");
    assert_eq!(
        fs::metadata(&saved).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let file: Value = serde_json::from_str(&fs::read_to_string(&saved).unwrap()).unwrap();
    assert_eq!(file["questions"].as_array().unwrap().len(), 20);
    assert_eq!(file["questions"][0]["header"], "scope");
    let member = f.member();
    assert_eq!(
        text(&out.stdout),
        format!(
            "{}\nsaved to ~/.claude/state/wren/interview-questions.json\n{}\n{}\n",
            persona_builder_build::text::drafted(20, false),
            persona_builder_build::text::swap(&member),
            persona_builder_agent::text::spend(0.02, 50.0)
        )
    );
}

#[test]
fn draft_never_replaces_waiting_questions_unless_asked() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    f.write_state("interview-questions.json", "{\"questions\": []}\n");
    let out = f.run(&["interview", "draft", &f.member()]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        format!(
            "persona-builder: {}\n",
            persona_builder_build::text::ALREADY_DRAFTED
        )
    );
    assert_eq!(f.calls(), 0);
    let out = f.run(&["interview", "draft", &f.member(), "--replace"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(f.calls(), 1);
}

#[test]
fn draft_says_how_to_sign_in_and_spends_nothing() {
    let f = Fixture::new();
    f.stub("case \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":false}';; *) echo x >> \"$(dirname \"$0\")/calls\";; esac");
    let out = f.run(&["interview", "draft", &f.member()]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        format!(
            "persona-builder: {}\n",
            persona_builder_agent::text::SIGNED_OUT
        )
    );
    assert_eq!(f.calls(), 0);
    assert!(!f.state().join("interview-questions.json").exists());
}

#[test]
fn draft_weights_by_an_earlier_builds_labels() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    f.write_state("decisions.jsonl", &corpus(2));
    f.write_state(
        "labels.json",
        "{\"question:0\": {\"category\": \"scope\"}, \"question:1\": {\"category\": \"scope\"}}",
    );
    let out = f.run(&["interview", "draft", &f.member(), "--labels", "labels.json"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).starts_with(&persona_builder_build::text::drafted(20, true)),
        "{}",
        text(&out.stdout)
    );
    let asked = fs::read_to_string(f.bin.join("stdin")).unwrap();
    assert!(
        asked.contains("- sequencing: 0 past decisions\n"),
        "{asked}"
    );
    assert!(asked.contains("- scope: 2 past decisions\n"), "{asked}");
}

#[test]
fn answer_keeps_the_answers_as_training_and_merge_picks_them_up() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    let out = f.run(&["interview", "draft", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    f.write_state(
        "interview-answers.json",
        "{\"answered\": \"2026-10-24\", \"answers\": {\"1\": {\"pick\": \"A\"}, \"2\": {\"own_words\": \"Wait a week\"}}}",
    );
    let out = f.run(&[
        "interview",
        "answer",
        &f.member(),
        "--answers",
        "interview-answers.json",
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let member = f.member();
    assert_eq!(
        text(&out.stdout),
        format!(
            "kept 2 interview answers, always for training; 18 unanswered\nleft out 0 that looked personal and 0 that looked secret\nsaved to ~/.claude/state/wren/decisions-interview.jsonl\n{}\n",
            persona_builder_build::text::merge_next(&member)
        )
    );
    let saved = f.state().join("decisions-interview.jsonl");
    assert_eq!(
        fs::metadata(&saved).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let lines: Vec<Value> = fs::read_to_string(&saved)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], "question:interview-01");
    assert_eq!(lines[0]["project"], "interview");
    assert_eq!(lines[0]["outcome"], "took_rec");
    assert_eq!(lines[1]["outcome"], "own_words");

    let again = [
        "interview",
        "answer",
        &member,
        "--answers",
        "interview-answers.json",
    ];
    let out = f.run(&again);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        format!(
            "persona-builder: {}\n",
            persona_builder_build::text::ALREADY_ANSWERED
        )
    );
    let out = f.run(&[&again[..], &["--replace"]].concat());
    assert!(out.status.success(), "{}", text(&out.stderr));

    let out = f.run(&["merge", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).starts_with("merging: decisions-interview.jsonl (always training)\n"),
        "{}",
        text(&out.stdout)
    );
}

#[test]
fn answer_with_nothing_answered_writes_nothing() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    assert!(f.run(&["interview", "draft", &f.member()]).status.success());
    f.write_state(
        "interview-answers.json",
        "{\"answered\": \"2026-10-24\", \"answers\": {}}",
    );
    let out = f.run(&[
        "interview",
        "answer",
        &f.member(),
        "--answers",
        "interview-answers.json",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        format!(
            "persona-builder: {}\n",
            persona_builder_build::text::NO_ANSWERS
        )
    );
    assert!(!f.state().join("decisions-interview.jsonl").exists());
}

#[test]
fn draft_weights_by_the_last_report_without_labels() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&draft()));
    f.write_state("report.json", r#"{"schema":"persona-builder/report/1","built":"2026-10-24","overall":"thin","answered":3,"held":0,"test":null,"areas":[{"area":"scope","level":"thin","examples":3,"strengthen":[]}]}"#);
    let out = f.run(&["interview", "draft", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let asked = fs::read_to_string(f.bin.join("stdin")).unwrap();
    assert!(asked.contains("- scope: 3 past decisions\n"), "{asked}");
    assert!(
        asked.contains("- sequencing: 0 past decisions\n"),
        "{asked}"
    );
    assert!(text(&out.stdout).starts_with(&persona_builder_build::text::drafted(20, true)));
}

fn long_history_report(thin: bool) -> String {
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
    let report = json!({"schema":"persona-builder/report/1","built":"2026-10-24","overall":"thin","answered":150,"held":0,"test":null,
        "areas":kinds.iter().enumerate().map(|(n, kind)|json!({"area":kind,"level":if thin && n < 2 {"thin"} else {"strong"},"examples":if thin && n < 2 {n * 3} else {40},"strengthen":[]})).collect::<Vec<_>>()}).to_string();
    serde_json::from_str::<persona_builder_build::report::Report>(&report).unwrap();
    report
}

fn short_draft() -> Value {
    let mut output = draft();
    let questions = output["questions"].as_array_mut().unwrap();
    questions.truncate(8);
    for (at, q) in questions.iter_mut().enumerate() {
        q["header"] = json!(["scope", "sequencing"][at % 2]);
    }
    output
}

#[test]
fn long_history_drafts_only_the_thin_kinds() {
    let f = Fixture::new();
    f.signed_in(&ok_reply(&short_draft()));
    f.write_state("decisions.jsonl", &corpus(150));
    f.write_state("report.json", &long_history_report(true));
    let out = f.run(&["interview", "draft", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(f.calls(), 1);
    let saved: Value = serde_json::from_slice(
        &fs::read(f.state().join("interview-short-questions.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["questions"].as_array().unwrap().len(), 8);
    assert!(!f.state().join("interview-questions.json").exists());
    assert!(text(&out.stdout).starts_with("Drafted 8 questions on the kinds of decision your history holds the fewest examples of: scope, sequencing.\nsaved to ~/.claude/state/wren/interview-short-questions.json\n"), "{}", text(&out.stdout));
    let ledger = fs::read_to_string(f.state().join("runs.jsonl")).unwrap();
    assert!(
        ledger
            .lines()
            .all(|line| serde_json::from_str::<Value>(line).unwrap()["step"] == "interview-short")
    );
}

#[test]
fn long_history_answers_use_the_short_round_without_replacing_full_answers() {
    let f = Fixture::new();
    f.write_state("decisions.jsonl", &corpus(150));
    f.write_state("interview-short-questions.json", &short_draft().to_string());
    let full = corpus(2);
    f.write_state("decisions-interview.jsonl", &full);
    f.write_state(
        "interview-short-answers.json",
        r#"{"answered":"2026-10-24","answers":{"1":{"pick":"A"},"2":{"own_words":"Wait a week"}}}"#,
    );
    let out = f.run(&[
        "interview",
        "answer",
        &f.member(),
        "--answers",
        "interview-short-answers.json",
        "--replace",
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        fs::read(f.state().join("decisions-interview.jsonl")).unwrap(),
        full.as_bytes()
    );
    let kept: Vec<Value> = fs::read_to_string(f.state().join("decisions-interview-short.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(kept.len(), 2);
    for (at, record) in kept.iter().enumerate() {
        assert_eq!(
            record["id"],
            format!("question:interview-short-{:02}", at + 1)
        );
        assert_eq!(record["project"], "interview");
        assert_eq!(record["split"], "train");
    }
    assert_eq!(
        fs::read(f.state().join("decisions-interview.jsonl")).unwrap(),
        full.as_bytes()
    );
    let out = f.run(&[
        "interview",
        "answer",
        &f.member(),
        "--answers",
        "interview-short-answers.json",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains(persona_builder_build::text::ALREADY_ANSWERED));
    let out = f.run(&[
        "interview",
        "answer",
        &f.member(),
        "--answers",
        "interview-short-answers.json",
        "--replace",
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        fs::read(f.state().join("decisions-interview.jsonl")).unwrap(),
        full.as_bytes()
    );
}

#[test]
fn answer_uses_the_short_form_from_a_custom_corpus() {
    let f = Fixture::new();
    f.write_state("other.jsonl", &corpus(150));
    assert!(!f.state().join("decisions.jsonl").exists());
    f.write_state("interview-short-questions.json", &short_draft().to_string());
    f.write_state(
        "interview-short-answers.json",
        r#"{"answered":"2026-10-24","answers":{"1":{"pick":"A"}}}"#,
    );
    let out = f.run(&[
        "interview",
        "answer",
        &f.member(),
        "--corpus",
        "other.jsonl",
        "--answers",
        "interview-short-answers.json",
    ]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let kept: Value = serde_json::from_str(
        fs::read_to_string(f.state().join("decisions-interview-short.jsonl"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(kept["id"], "question:interview-short-01");
    assert_eq!(kept["project"], "interview");
    assert_eq!(kept["split"], "train");
    assert!(!f.state().join("decisions-interview.jsonl").exists());
    assert_eq!(f.calls(), 0);
}
