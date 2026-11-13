//! Refresh drafts stay private; approved changes are tested before they change the member.
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_core::crew::{NewMember, new_member};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    home: PathBuf,
    member: PathBuf,
    bin: PathBuf,
}

fn python() -> Option<PathBuf> {
    let found = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));
    if found.is_none() && std::env::var("VOICE_REQUIRE_PARITY").as_deref() == Ok("1") {
        panic!("python3 is needed for the stub claude");
    }
    found
}

impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb-refresh-cli-{}-{}",
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
                decisions: voice_core::crew::Decisions::Python,
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
    /// A signed-in Claude Code that answers each step from its prompt (the build crate's stub).
    fn signed_in(&self, python: &std::path::Path) {
        let script = include_str!("../../build/tests/fixtures/claude_stub.py");
        self.write_stub(&format!("#!{} -I\n{script}", python.display()));
    }
    fn write_stub(&self, text: &str) {
        let program = self.bin.join("claude");
        fs::write(&program, text).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn calls(&self) -> usize {
        (0..)
            .take_while(|n| self.bin.join(format!("stdin{n}")).exists())
            .count()
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_voice"))
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

const HEADERS: [&str; 3] = ["Scope", "Methods", "Tooling"];

fn question(n: usize, split: &str) -> Value {
    json!({"id": format!("question:{split}{n:03}"), "kind": "question",
           "timestamp": format!("2026-09-{:02}T10:00:00.000Z", 1 + n % 28), "project": "kit",
           "header": HEADERS[n % 3],
           "question": format!("Should the reading list for week {n} stay short?"),
           "multiSelect": false,
           "options": [{"label": "Keep it short", "description": "Less to read"},
                       {"label": "Make it longer", "description": "More detail"}],
           "recommended": null, "picks": ["Keep it short"], "own_words": "", "notes": "",
           "outcome": "no_rec", "split": split, "wren_pick": null})
}

fn corpus(held: usize) -> String {
    let mut lines: Vec<Value> = (0..12).map(|n| question(n, "train")).collect();
    lines.push(json!({"id": "message:m0", "kind": "message", "timestamp": "2026-08-20T10:00:00.000Z",
                      "project": "kit", "text": "Please always keep the shared folders tidy and easy to find", "split": "train"}));
    lines.extend((0..held).map(|n| question(n, "heldout")));
    lines.iter().map(|l| format!("{l}\n")).collect()
}

impl Fixture {
    fn refresh_stub(&self, python: &std::path::Path) {
        let script = include_str!("../../grow/tests/fixtures/refresh_stub.py");
        self.write_stub(&format!("#!{} -I\n{script}", python.display()));
    }
    fn built(&self, python: &std::path::Path) {
        self.signed_in(python);
        self.write_state("decisions.jsonl", &corpus(3));
        let out = self.run(&["build", &self.member()]);
        assert!(out.status.success(), "{}", text(&out.stderr));
        self.refresh_stub(python);
    }
    fn keep_answers(&self) {
        for area in ["scope", "sequencing"] {
            self.write_state(
                "answer.json",
                &json!({"kind":"instruction", "area":area,
                "situation":format!("Choose the next classroom activity in {area}."),
                "wanted":"Start with a quiet exercise before a group task."})
                .to_string(),
            );
            let out = self.run(&["keep", &self.member(), "--signal", "answer.json"]);
            assert!(out.status.success(), "{}", text(&out.stderr));
        }
    }
    fn records(&self, prefix: &str) -> Vec<Value> {
        fs::read_dir(self.state().join("growth"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
            .map(|p| serde_json::from_slice(&fs::read(p).unwrap()).unwrap())
            .collect()
    }
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(path: &std::path::Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                } else {
                    out.push((path.clone(), fs::read(path).unwrap()));
                }
            }
        }
        let mut files = Vec::new();
        walk(&self.member, &mut files);
        files.sort();
        files
    }
}

#[test]
fn propose_show_approve_and_wait_without_changing_the_member() {
    let Some(python) = python() else { return };
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    let before = f.snapshot();
    let calls = f.calls();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(f.calls(), calls + 1);
    let proposals = f.records("prop-");
    assert_eq!(proposals.len(), 1);
    let id = proposals[0]["id"].as_str().unwrap();
    let shown = text(&out.stdout);
    assert!(
        shown.contains(proposals[0]["summary"].as_str().unwrap()),
        "{shown}"
    );
    assert!(shown.contains("--- a/decisions.md"), "{shown}");
    for command in ["approve", "reject"] {
        assert!(
            shown.contains(&format!("voice refresh {command} {} {id}", f.member())),
            "{shown}"
        );
    }
    assert!(shown.contains("Today, about $"), "{shown}");
    for named in [false, true] {
        // Paths and ids use the ordinary positional CLI form.
        let member = f.member();
        let mut words = vec!["refresh", "show", &member];
        if named {
            words.push(id);
        }
        let out = f.run(&words);
        assert!(out.status.success(), "{}", text(&out.stderr));
        let expected = shown
            .lines()
            .take_while(|line| !line.starts_with("Today, about $"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(text(&out.stdout), format!("{expected}\n"));
    }
    let out = f.run(&["refresh", "approve", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout).trim(),
        "Approved. Your assistant changes only after the change is tested on all your answers and nothing gets worse."
    );
    let approvals = f.records("appr-");
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0]["proposal"], id);
    assert_eq!(approvals[0]["decision"], "approved");
    assert_eq!(approvals[0]["role"], "subject");
    assert!(approvals[0].get("note").is_none());
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains(&format!(
            "You approved a change ({id}) that hasn't been tested and applied yet."
        )),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(f.calls(), calls + 1);
    assert_eq!(f.snapshot(), before);
}

#[test]
fn reject_saves_the_answer_and_leaves_no_open_change() {
    let Some(python) = python() else { return };
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    let before = f.snapshot();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let proposals = f.records("prop-");
    let id = proposals[0]["id"].as_str().unwrap();
    let out = f.run(&["refresh", "reject", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout).trim(),
        "Rejected. Nothing changed. Your answers stay saved for the next refresh."
    );
    assert_eq!(f.records("appr-")[0]["decision"], "rejected");
    let out = f.run(&["refresh", "show", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "No change is waiting for your answer.\n");
    assert_eq!(f.snapshot(), before);
}

#[test]
fn approve_with_note_keeps_trimmed_words_and_newlines_privately() {
    answer_with_note("approve", "approved", "notes/a.json");
}

#[test]
fn note_help_explains_the_private_json_file() {
    let f = Fixture::new();
    for command in ["approve", "reject"] {
        let out = f.run(&["refresh", command, "--help"]);
        assert!(out.status.success());
        let shown = text(&out.stdout);
        assert!(shown.contains("--note <FILE>"), "{shown}");
        assert!(shown.contains("Your words to keep with your answer: a JSON file in your assistant's private folder, NAME or FOLDER/NAME, holding {\"note\": \"...\"}"), "{shown}");
    }
}

#[test]
fn reject_with_note_keeps_trimmed_words_and_newlines_privately() {
    answer_with_note("reject", "rejected", "a.json");
}

fn answer_with_note(command: &str, decision: &str, name: &str) {
    let python = python().expect("python3 is required for the stub claude");
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let proposals = f.records("prop-");
    let id = proposals[0]["id"].as_str().unwrap();
    if let Some((folder, _)) = name.split_once('/') {
        fs::create_dir(f.state().join(folder)).unwrap();
        fs::set_permissions(f.state().join(folder), fs::Permissions::from_mode(0o700)).unwrap();
    }
    f.write_state(
        name,
        r#"{"note": "  Yes, try it.\nKeep the old rule too.  "}"#,
    );
    let out = f.run(&["refresh", command, &f.member(), id, "--note", name]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let expected = match command {
        "approve" => {
            "Approved. Your assistant changes only after the change is tested on all your answers and nothing gets worse.\n"
        }
        "reject" => "Rejected. Nothing changed. Your answers stay saved for the next refresh.\n",
        _ => unreachable!(),
    };
    assert_eq!(text(&out.stdout), expected);
    assert!(out.stderr.is_empty());
    let approvals = f.records("appr-");
    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0]["decision"], decision);
    assert_eq!(approvals[0]["note"], "Yes, try it.\nKeep the old rule too.");
}

fn refused_note(name: &str, contents: Option<&str>, sentence: &str) {
    let python = python().expect("python3 is required for the stub claude");
    // Both decisions must validate the note before saving any answer.
    for command in ["approve", "reject"] {
        let f = Fixture::new();
        f.built(&python);
        f.keep_answers();
        let out = f.run(&["refresh", "propose", &f.member()]);
        assert!(out.status.success(), "{}", text(&out.stderr));
        let proposals = f.records("prop-");
        let id = proposals[0]["id"].as_str().unwrap();
        if let Some(contents) = contents {
            f.write_state(name, contents);
        }
        let out = f.run(&["refresh", command, &f.member(), id, "--note", name]);
        let stderr = text(&out.stderr);
        assert!(!out.status.success(), "{command}: expected a refusal");
        assert!(stderr.contains(sentence), "{command}: {stderr}");
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(f.records("appr-").is_empty());
        let out = f.run(&["refresh", "approve", &f.member(), id]);
        assert!(out.status.success(), "{}", text(&out.stderr));
        assert_eq!(f.records("appr-").len(), 1);
    }
}

macro_rules! note_refusal {
    ($test:ident, $name:expr, $contents:expr, $sentence:expr) => {
        #[test]
        fn $test() {
            refused_note($name, $contents, $sentence);
        }
    };
}

note_refusal!(
    note_empty,
    "a.json",
    Some(r#"{"note":"   "}"#),
    "The note is empty."
);
note_refusal!(
    note_control,
    "a.json",
    Some(r#"{"note":"a\u0007b"}"#),
    "The note can't hold control characters."
);
note_refusal!(
    note_non_string,
    "a.json",
    Some(r#"{"note":1}"#),
    "The note file must hold {\"note\": \"...\"} and nothing else."
);
note_refusal!(
    note_extra_key,
    "a.json",
    Some(r#"{"note":"x","by":"y"}"#),
    "The note file must hold {\"note\": \"...\"} and nothing else."
);
note_refusal!(
    note_not_json,
    "a.json",
    Some("not json"),
    "The note file must hold {\"note\": \"...\"} and nothing else."
);
note_refusal!(
    note_missing_key,
    "a.json",
    Some("{}"),
    "The note file must hold {\"note\": \"...\"} and nothing else."
);
note_refusal!(
    note_array,
    "a.json",
    Some(r#"["x"]"#),
    "The note file must hold {\"note\": \"...\"} and nothing else."
);
note_refusal!(
    note_not_json_name,
    "a.txt",
    Some(r#"{"note":"x"}"#),
    "the file name must be a plain name ending .json"
);
note_refusal!(
    note_two_folders,
    "a/b/c.json",
    None,
    "a file here is NAME or FOLDER/NAME inside the assistant's private folder"
);
note_refusal!(note_missing_file, "missing.json", None, "doesn't exist");

#[test]
fn note_over_16_kib_is_refused_before_saving() {
    let contents = json!({"note": "x".repeat(16 * 1024)}).to_string();
    refused_note(
        "a.json",
        Some(&contents),
        "a.json is too big to read (over 16 KiB)",
    );
}

#[test]
fn nothing_kept_makes_no_model_call() {
    let Some(python) = python() else { return };
    let f = Fixture::new();
    f.built(&python);
    let before = f.snapshot();
    let calls = f.calls();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert!(
        shown.contains(
            "There's nothing new to learn from yet. Keep answering your assistant, then try again."
        ),
        "{shown}"
    );
    assert!(shown.contains("Today, about $"), "{shown}");
    assert_eq!(f.calls(), calls);
    assert_eq!(f.snapshot(), before);
}

#[test]
fn refresh_help_calls_the_id_a_change_and_explains_the_model() {
    let f = Fixture::new();
    for command in ["show", "approve", "reject"] {
        let out = f.run(&["refresh", command, "--help"]);
        assert!(out.status.success());
        let shown = text(&out.stdout);
        assert!(shown.contains("CHANGE"), "{shown}");
        assert!(
            shown.contains("The change's id, as `refresh show` prints it"),
            "{shown}"
        );
        assert!(!shown.contains("PROPOSAL"), "{shown}");
    }
    let out = f.run(&["refresh", "propose", "--help"]);
    assert!(out.status.success());
    assert!(text(&out.stdout).contains("The Claude model to draft with"));
}

#[test]
fn approved_change_is_applied_reported_and_resumes_without_calls() {
    let Some(python) = python() else { return };
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let proposals = f.records("prop-");
    let id = proposals[0]["id"].as_str().unwrap();
    let out = f.run(&["refresh", "approve", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let script = include_str!("../../grow/tests/fixtures/apply_stub.py");
    f.write_stub(&format!("#!{} -I\n{script}", python.display()));
    let out = f.run(&["refresh", "apply", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    let kept = "Kept. Your assistant now works this way.";
    let reason = "Every new test now passes, and none of the others got worse.";
    let growth = "Since the build: 1 change kept and 0 undone. Your assistant passes 2 of 2 tests from your answers.";
    assert!(shown.starts_with(&format!(
        "Testing your assistant as it is now on 2 tests.\nTesting it with the change on 2 tests.\nChecking the written answers against what you wanted.\n{kept}\n{reason}\n"
    )), "{shown}");
    assert!(shown.contains(growth), "{shown}");
    assert!(shown.find(growth).unwrap() < shown.find("Today, about $").unwrap());
    let out = f.run(&["report", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains(growth), "{}", text(&out.stdout));
    let calls = f.calls();
    let before = f.snapshot();
    let out = f.run(&["refresh", "apply", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).starts_with(&format!(
            "This change ({id}) was already tested and kept.\n"
        )),
        "{}",
        text(&out.stdout)
    );
    assert_eq!(f.calls(), calls);
    assert_eq!(f.snapshot(), before);
    fs::remove_file(f.member.join("measurements.md")).unwrap();
    let out = f.run(&["refresh", "apply", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).starts_with("This change was already tested; finished what was left.\n")
    );
    assert!(f.member.join("measurements.md").is_file());
    assert_eq!(f.calls(), calls);
    fs::remove_file(f.state().join("report.json")).unwrap();
    let out = f.run(&["report", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert!(shown.contains(growth), "{shown}");
    assert!(shown.contains("There's no report yet."), "{shown}");
    let out = f.run(&["refresh", "apply", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains(growth), "{}", text(&out.stdout));
}

#[test]
fn applying_an_unanswered_change_says_it_is_waiting_without_calls() {
    let Some(python) = python() else { return };
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let proposals = f.records("prop-");
    let id = proposals[0]["id"].as_str().unwrap();
    let calls = f.calls();
    let before = f.snapshot();
    let out = f.run(&["refresh", "apply", &f.member(), id]);
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains(&format!("That change ({id}) is still waiting for your answer. Look at it with `voice refresh show`, then approve or reject it.")), "{}", text(&out.stderr));
    assert_eq!(f.calls(), calls);
    assert_eq!(f.snapshot(), before);
}

#[test]
fn apply_help_explains_the_model_is_for_testing() {
    let f = Fixture::new();
    let out = f.run(&["refresh", "apply", "--help"]);
    assert!(out.status.success());
    assert!(text(&out.stdout).contains("The Claude model to test with"));
}

const FOLDER_KEPT_HOME: &str = "Nothing was sent. This step uses your assistant's own folder, which you keep on your own machines, and the provider for this step, Claude, through your own Claude Code, sends what it's given to Anthropic. To let it go there, run: voice ceilings set <assistant folder> crew-folder anthropic To run it without the data leaving your machines, set up a model server with `voice self-hosted set`.";

fn keep_folder_local(member: &std::path::Path) {
    let path = member.join("member.json");
    let mut settings: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    settings["ceilings"] = json!({"crew-folder": "local-only"});
    fs::write(path, serde_json::to_vec(&settings).unwrap()).unwrap();
}

#[test]
fn refresh_propose_honors_the_members_local_folder_ceiling() {
    let python = python().expect("python3 is required for the stub claude");
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    keep_folder_local(&f.member);
    let calls = f.calls();
    let ledger = fs::read(f.state().join("runs.jsonl")).unwrap();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(!out.status.success());
    assert_eq!(text(&out.stderr), format!("voice: {FOLDER_KEPT_HOME}\n"));
    assert_eq!(f.calls(), calls);
    assert_eq!(fs::read(f.state().join("runs.jsonl")).unwrap(), ledger);
}

#[test]
fn refresh_apply_honors_the_members_local_folder_ceiling() {
    let python = python().expect("python3 is required for the stub claude");
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let proposals = f.records("prop-");
    let id = proposals[0]["id"].as_str().unwrap();
    let out = f.run(&["refresh", "approve", &f.member(), id]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let script = include_str!("../../grow/tests/fixtures/apply_stub.py");
    f.write_stub(&format!("#!{} -I\n{script}", python.display()));
    keep_folder_local(&f.member);
    let calls = f.calls();
    let ledger = fs::read(f.state().join("runs.jsonl")).unwrap();
    let out = f.run(&["refresh", "apply", &f.member(), id]);
    assert!(!out.status.success());
    assert_eq!(text(&out.stderr), format!("voice: {FOLDER_KEPT_HOME}\n"));
    assert_eq!(f.calls(), calls);
    assert_eq!(fs::read(f.state().join("runs.jsonl")).unwrap(), ledger);
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_refresh_propose_closes_with_usage() {
    let python = python().expect("python3 is required for the stub");
    let f = Fixture::new();
    f.built(&python);
    f.keep_answers();
    f.write_state("runs.jsonl", "");
    let script =
        include_str!("../../grow/tests/fixtures/refresh_stub.py").replace("api_key", "claude.ai");
    f.write_stub(&format!("#!{} -I\n{script}", python.display()));
    let out = f.run(&["refresh", "propose", &f.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout).lines().last(),
        Some("Today, 1 call to Claude and about 1 thousand tokens, on your Claude subscription.")
    );
}

#[cfg(feature = "claude-subscription")]
#[test]
fn subscription_refresh_ignores_daily_limit_in_both_flows() {
    let python = python().expect("python3 is required for the stub");
    for apply in [false, true] {
        for method in ["claude.ai", "api_key"] {
            let f = Fixture::new();
            f.built(&python);
            f.keep_answers();
            let mut id = String::new();
            if apply {
                let out = f.run(&["refresh", "propose", &f.member()]);
                assert!(out.status.success(), "{}", text(&out.stderr));
                id = f.records("prop-")[0]["id"].as_str().unwrap().to_owned();
                let out = f.run(&["refresh", "approve", &f.member(), &id]);
                assert!(out.status.success(), "{}", text(&out.stderr));
            }
            let script = if apply {
                include_str!("../../grow/tests/fixtures/apply_stub.py")
            } else {
                include_str!("../../grow/tests/fixtures/refresh_stub.py")
            }
            .replace("api_key", method);
            f.write_stub(&format!("#!{} -I\n{script}", python.display()));
            f.write_state("limit.json", r#"{"daily_usd":1,"call_usd":0.5}"#);
            f.write_state("runs.jsonl", &format!("{}\n", json!({"day":voice_agent::ledger::local_day(std::time::SystemTime::now()),"cost_usd":2})));
            let before = f.calls();
            let member = f.member();
            let args = if apply {
                vec!["refresh", "apply", &member, &id]
            } else {
                vec!["refresh", "propose", &member]
            };
            let out = f.run(&args);
            if method == "claude.ai" {
                assert!(out.status.success(), "{}", text(&out.stderr));
                assert!(f.calls() > before);
            } else {
                assert_eq!(
                    text(&out.stderr),
                    "voice: Today's spending limit for this assistant is reached: about $2.00 of $1.00, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit.\n"
                );
                assert_eq!(f.calls(), before);
            }
        }
    }
}
