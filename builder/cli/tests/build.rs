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
        "How well your assistant knows you: Thin.",
        "- scope: Strong.",
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
    let report = fixture.state().join("report.json");
    assert_eq!(
        fs::metadata(&report).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let report: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
    assert_eq!(report["overall"], "thin");
    assert_eq!(report["held"], 32);
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

#[test]
fn build_checks_the_member_folder_before_spending() {
    let Some(python) = python() else { return };
    let fixture = Fixture::new();
    fixture.signed_in(&python);
    fixture.write_state("decisions.jsonl", &corpus(32));
    fs::remove_file(fixture.member.join("measurements.md")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", fixture.member.join("measurements.md")).unwrap();
    let out = fixture.run(&["build", &fixture.member()]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("is a link"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(fixture.calls(), 0);
}

#[test]
fn report_shows_the_last_build() {
    let Some(python) = python() else { return };
    let fixture = Fixture::new();
    fixture.signed_in(&python);
    let out = fixture.run(&["report", &fixture.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        format!(
            "There's no report yet. Build your assistant first: persona-builder build {}\n",
            fixture.member()
        )
    );
    fixture.write_state("decisions.jsonl", &corpus(32));
    let built = fixture.run(&["build", &fixture.member()]);
    assert!(built.status.success(), "{}", text(&built.stderr));
    let out = fixture.run(&["report", &fixture.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.starts_with("From the build on "), "{stdout}");
    let build_stdout = text(&built.stdout);
    let report_lines = build_stdout
        .split_once("How well your assistant knows you:")
        .unwrap()
        .1;
    let report_lines = report_lines
        .lines()
        .take_while(|line| !line.starts_with("Today, about $"))
        .collect::<Vec<_>>();
    let shown = stdout.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert_eq!(
        shown,
        format!(
            "How well your assistant knows you:{}",
            report_lines.join("\n")
        )
    );
    assert_eq!(fixture.calls(), 6);
}

#[test]
fn build_with_too_few_held_out_questions_says_so_honestly() {
    let Some(python) = python() else { return };
    let fixture = Fixture::new();
    fixture.signed_in(&python);
    fixture.write_state("decisions.jsonl", &corpus(10));
    let out = fixture.run(&["build", &fixture.member()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.contains("Only 10 questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own.\n"), "{stdout}");
    assert!(
        stdout
            .contains("The interview fills out a thin history: persona-builder interview draft\n"),
        "{stdout}"
    );
    assert!(!stdout.contains(": Strong."), "{stdout}");
}

#[test]
fn build_refuses_bad_report_names_before_changing_member_files() {
    let Some(python) = python() else { return };
    let fixture = Fixture::new();
    fixture.signed_in(&python);
    let bad = "s".repeat(101);
    let config = fixture.member.join("member.json");
    let settings = fs::read_to_string(&config)
        .unwrap()
        .replace("\"scope\"", &format!("\"{bad}\""));
    fs::write(config, settings).unwrap();
    fixture.write_state("decisions.jsonl", &corpus(10).replace("Scope", &bad));
    fixture.write_state("report.json", "previous report");
    let before: Vec<_> = ["decisions.md", "measurements.md"]
        .map(|name| (name, fs::read(fixture.member.join(name)).unwrap()))
        .into();
    let out = fixture.run(&["build", &fixture.member()]);
    assert!(!out.status.success(), "{}", text(&out.stdout));
    assert_eq!(
        text(&out.stderr).trim(),
        "persona-builder: A kind of decision in your assistant's settings (codebook.categories) has a name that is empty, longer than 100 characters, or holds a character that can't be shown, so nothing was saved. Rename it, then try again."
    );
    for (name, bytes) in before {
        assert_eq!(
            fs::read(fixture.member.join(name)).unwrap(),
            bytes,
            "{name} changed"
        );
    }
    assert_eq!(
        fs::read(fixture.state().join("report.json")).unwrap(),
        b"previous report"
    );
}
