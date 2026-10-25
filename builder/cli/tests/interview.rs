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
            "here=\"$(cd \"$(dirname \"$0\")\" && pwd -P)\"\ncase \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":true}';; *) cat > /dev/null; echo x >> \"$here/calls\"; cat \"$here/reply\";; esac",
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
    let out = f.run(&["interview", "draft", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        format!("{}\n", persona_builder_build::text::enough(150))
    );
    assert_eq!(f.calls(), 0);
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
