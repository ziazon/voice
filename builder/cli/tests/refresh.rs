//! Refresh drafts and answers stay private, and never change the member files.
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

fn python() -> Option<PathBuf> {
    let found = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));
    if found.is_none() && std::env::var("PERSONA_BUILDER_REQUIRE_PARITY").as_deref() == Ok("1") {
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
            shown.contains(&format!(
                "persona-builder refresh {command} {} {id}",
                f.member()
            )),
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
