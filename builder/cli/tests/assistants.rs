use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Home(PathBuf);

impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-assistants-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", &self.0)
            .args(args)
            .output()
            .unwrap()
    }

    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn new_list_and_agree_use_the_claude_folders_and_final_wording() {
    let home = Home::new();
    assert_eq!(
        home.success(&["assistants"]),
        "You have no assistants yet. Make one with: persona-builder new <name> --principal <your name>\n"
    );
    assert_eq!(
        home.success(&["new", "Wren", "--principal", "Robin"]),
        "Made Wren in ~/.claude/crew/wren/.\nNext: persona-builder consent show, then persona-builder consent agree ~/.claude/crew/wren\n"
    );
    let skill = fs::read_to_string(home.0.join(".claude/crew/wren/SKILL.md")).unwrap();
    // The CLI's shipped_cli() returns this binary itself; the test executable lives in deps/.
    let word = persona_builder::cli_command::cli_word(
        &PathBuf::from(env!("CARGO_BIN_EXE_persona-builder"))
            .canonicalize()
            .unwrap(),
        &home.0,
    )
    .unwrap();
    assert!(!skill.contains("{{"), "unfilled placeholder in SKILL.md");
    assert!(
        !skill.contains("crew_decisions.py"),
        "app-made assistant names Python"
    );
    assert!(skill.contains(&format!("{word} similar ~/.claude/crew/wren --query")));
    assert!(skill.contains(&format!("{word} record ~/.claude/crew/wren {}", '\\')));
    let member =
        persona_builder::member::read_member(&home.0.join(".claude/crew/wren/member.json"))
            .unwrap();
    assert_eq!(member.pronouns.as_deref(), Some("they/them"));
    assert_eq!(member.principal_pronouns.as_deref(), Some("they/them"));
    assert_eq!(
        home.success(&["assistants"]),
        "Wren (wren): ~/.claude/crew/wren/ · permission not given yet\n"
    );
    let date = persona_builder_agent::ledger::local_day(SystemTime::now());
    assert_eq!(
        home.success(&["consent", "agree", "~/.claude/crew/wren"]),
        format!("Thank you. Permission given on {date}.\n")
    );
    assert_eq!(
        home.success(&["assistants"]),
        format!("Wren (wren): ~/.claude/crew/wren/ · permission given {date}\n")
    );
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(home.0.join(".claude/state/wren/consent.json")).unwrap())
            .unwrap();
    assert_eq!(record, serde_json::json!({"version": 1, "agreed_on": date}));
    assert!(!home.0.join(".claude/crew/wren/consent.json").exists());
}

#[test]
fn duplicate_new_exits_one_and_keeps_the_existing_assistant() {
    let home = Home::new();
    home.success(&["new", "Wren", "--principal", "Robin"]);
    let output = home.run(&["new", "Wren", "--principal", "Alex"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "persona-builder: You already have an assistant in ~/.claude/crew/wren/.\n"
    );
    assert_eq!(
        persona_builder::member::read_member(&home.0.join(".claude/crew/wren/member.json"))
            .unwrap()
            .principal,
        "Robin"
    );
}

#[test]
fn consent_show_prints_the_final_wording_and_version() {
    let text = Home::new().success(&["consent", "show"]);
    assert_eq!(
        text,
        format!("{}Version 1.\n", persona_builder::consent::plain_text())
    );
    assert!(text.starts_with("Before Persona Builder reads anything\n\nWhat it reads\n"));
    assert!(text.contains("You can change your mind: Forget deletes the history it read and its working files. What it wrote into your assistant's folder stays there until you delete that folder.\n"));
    assert!(text.ends_with("Version 1.\n"));
}

#[test]
fn listing_does_not_count_bad_or_outdated_records_as_permission() {
    let home = Home::new();
    home.success(&["new", "Wren", "--principal", "Robin"]);
    home.success(&["consent", "agree", "~/.claude/crew/wren"]);
    let record = home.0.join(".claude/state/wren/consent.json");
    for bad in [
        r#"{"version":0,"version":1,"agreed_on":"2026-10-25"}"#,
        r#"{"version":1,"agreed_on":"2026-10-24","agreed_on":"2026-10-25"}"#,
        r#"{"version":0,"agreed_on":"2026-10-25"}"#,
        r#"{"version":1,"agreed_on":"2026-10-25","extra":true}"#,
        r#"{"version":1,"agreed_on":"2026-02-30"}"#,
        r#"{"version":1,"agreed_on":"not a date"}"#,
        r#"{"version":1,"agreed_on":42}"#,
        "{broken",
    ] {
        fs::write(&record, bad).unwrap();
        assert_eq!(
            home.success(&["assistants"]),
            "Wren (wren): ~/.claude/crew/wren/ · permission not given yet\n",
            "record: {bad}"
        );
    }
}

#[test]
fn new_clears_old_permission_and_rolls_back_when_clearing_fails() {
    let home = Home::new();
    let state = home.0.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    let record = state.join("consent.json");
    fs::write(&record, r#"{"version":1,"agreed_on":"2026-10-25"}"#).unwrap();
    home.success(&["new", "Wren", "--principal", "Robin"]);
    assert!(!record.exists());
    assert!(
        home.success(&["assistants"])
            .contains("permission not given yet")
    );
    fs::remove_dir_all(home.0.join(".claude/crew/wren")).unwrap();
    fs::create_dir(&record).unwrap();
    let output = home.run(&["new", "Wren", "--principal", "Robin"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "persona-builder: Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again.\n"
    );
    assert!(!home.0.join(".claude/crew/wren").exists());
}

#[test]
fn skill_commands_run_similar_and_record_against_private_state() {
    let home = Home::new();
    home.success(&["new", "Ari", "--principal", "Robin"]);
    home.success(&["consent", "agree", "~/.claude/crew/ari"]);
    let skill = fs::read_to_string(home.0.join(".claude/crew/ari/SKILL.md")).unwrap();
    // The CLI's shipped_cli() returns this binary itself; the test executable lives in deps/.
    let word = persona_builder::cli_command::cli_word(
        &PathBuf::from(env!("CARGO_BIN_EXE_persona-builder"))
            .canonicalize()
            .unwrap(),
        &home.0,
    )
    .unwrap();
    let binary = format!("\"{}\"", env!("CARGO_BIN_EXE_persona-builder"));
    let similar = skill
        .lines()
        .find(|line| line.trim_start().starts_with(&format!("{word} similar ")))
        .expect("SKILL.md names the shipped similar command");
    let state = home.0.join(".claude/state/ari");
    persona_builder_ingest::private::write_private(&home.0, "ari", "decisions.jsonl",
        b"{\"kind\":\"message\",\"split\":\"train\",\"timestamp\":\"2026-10-25T00:00:00Z\",\"project\":\"garden\",\"text\":\"Choose durable garden benches before adding decorative flower pots outdoors.\"}\n").unwrap();
    let run = |script: &str| {
        let output = Command::new("/bin/sh")
            .args(["-c", script])
            .env("HOME", &home.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    assert!(
        run(&similar
            .replacen(&word, &binary, 1)
            .replace("<the question and its options>", "durable garden benches"))
        .contains("Choose durable garden benches")
    );
    let mut lines = skill.lines();
    let record = lines
        .find(|line| line.trim_start().starts_with(&format!("{word} record ")))
        .expect("SKILL.md names the shipped record command");
    let command = format!(
        "{record}\n{}\n{}",
        lines.next().unwrap(),
        lines.next().unwrap()
    )
    .replacen(&word, &binary, 1)
    .replace("<what you are working on>", "garden")
    .replace("<c>", "design")
    .replace("<q>", "Choose durable garden benches?")
    .replace("<pick>", "durable benches");
    assert_eq!(
        run(&command),
        "recorded a decided design decision (high) for garden\n"
    );
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("live.jsonl")).unwrap()).unwrap();
    assert_eq!(record["question"], "Choose durable garden benches?");
    assert_eq!(record["ari_pick"], "durable benches");
    assert_eq!(record["rule"], serde_json::Value::Null);
    assert_eq!(record["robin_answer"], serde_json::Value::Null);
}
