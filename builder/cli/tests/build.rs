//! `persona-builder build` against a stub `claude` on `PATH`, on an invented member.
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
            "pb-build-cli-{}-{}",
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

#[test]
fn build_writes_how_they_decide_and_the_tests_result() {
    let Some(python) = python() else { return };
    let fixture = Fixture::new();
    fixture.signed_in(&python);
    fixture.write_state("decisions.jsonl", &corpus(32));
    let before = fs::read_to_string(fixture.member.join("measurements.md")).unwrap();
    let out = fixture.run(&["build", &fixture.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(fixture.calls(), 6);
    let stdout = text(&out.stdout);
    for line in [
        "Your history holds 44 answered questions. Fewer than 150 is thin: persona-builder interview draft adds twenty questions to fill it out.",
        "Reading your history: 44 answered questions, 32 of them kept back to test your assistant.",
        "Sorting your decisions by kind, part 2 of 2: done.",
        "Testing your assistant on 32 questions it hasn't seen, batch 1 of 1: done.",
        "Checking the draft for anything private before it's saved.",
        "Your assistant passed the test on: scope, methods, tooling. It can decide those on its own where you allow it.",
    ] {
        assert!(stdout.contains(&format!("{line}\n")), "{stdout}");
    }
    assert!(
        stdout.contains(&format!(
            "Saved how you decide in {}/decisions.md",
            fixture.member()
        )),
        "{stdout}"
    );
    let decisions = fs::read_to_string(fixture.member.join("decisions.md")).unwrap();
    assert!(
        decisions.starts_with(
            "---\nmodel: Robin's decisions\nmember: Wren (she/her)\nmined_through: 2026-09-12\n"
        ),
        "{decisions}"
    );
    assert!(
        decisions.contains("\n### scope\n")
            && decisions.contains("\n| scope | 11/11 | — | — | yes |\n"),
        "{decisions}"
    );
    let measurements = fs::read_to_string(fixture.member.join("measurements.md")).unwrap();
    assert!(
        measurements.starts_with(before.lines().next().unwrap()),
        "{measurements}"
    );
    assert!(
        measurements.contains(": build backtest\n\n- **Set:** 32 held-out questions"),
        "{measurements}"
    );
    assert!(!measurements.contains("YYYY-MM-DD"), "{measurements}");
    // The checked replies are kept, private, for a rerun.
    let kept = fixture.state().join("build");
    assert_eq!(
        fs::metadata(&kept).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(fs::read_dir(&kept).unwrap().count(), 6);
}

#[test]
fn build_spends_nothing_and_writes_nothing_when_claude_code_is_signed_out() {
    let fixture = Fixture::new();
    fixture.write_stub(
        "#!/bin/sh\nhere=\"$(cd \"$(dirname \"$0\")\" && pwd -P)\"\ncase \"$1\" in --version) echo '2.1.292 (Claude Code)';; auth) echo '{\"loggedIn\":false}';; *) echo x > \"$here/stdin0\";; esac",
    );
    fixture.write_state("decisions.jsonl", &corpus(32));
    let before = fs::read_to_string(fixture.member.join("decisions.md")).unwrap();
    let out = fixture.run(&["build", &fixture.member()]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("Claude Code isn't signed in."),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(fixture.calls(), 0);
    assert_eq!(
        fs::read_to_string(fixture.member.join("decisions.md")).unwrap(),
        before
    );
    assert!(!fixture.state().join("runs.jsonl").exists());
}

#[test]
fn build_needs_the_codebook_brief() {
    let fixture = Fixture::new();
    fixture.write_state("decisions.jsonl", &corpus(0));
    fs::remove_file(fixture.member.join("briefs/codebook.md")).unwrap();
    let out = fixture.run(&["build", &fixture.member()]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("has no briefs/codebook.md"),
        "{}",
        text(&out.stderr)
    );
}
