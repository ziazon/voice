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
        Command::new(env!("CARGO_BIN_EXE_voice"))
            .env("HOME", &self.0)
            .env("CLAUDE_CONFIG_DIR", "")
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
        "You have no assistants yet. Make one with: voice new <name> --principal <your name>\n"
    );
    assert_eq!(
        home.success(&["new", "Wren", "--principal", "Robin"]),
        "Made Wren in ~/.claude/crew/wren/.\nNext: voice consent show, then voice consent agree ~/.claude/crew/wren\n"
    );
    let skill = fs::read_to_string(home.0.join(".claude/crew/wren/SKILL.md")).unwrap();
    // The CLI's shipped_cli() returns this binary itself; the test executable lives in deps/.
    let word = voice_core::cli_command::cli_word(
        &PathBuf::from(env!("CARGO_BIN_EXE_voice"))
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
    let decisions = fs::read_to_string(home.0.join(".claude/crew/wren/decisions.md")).unwrap();
    assert!(
        !decisions.contains("crew_decisions.py similar"),
        "app-made decisions.md names the Python look-up"
    );
    assert!(skill.contains(&format!(
        "{word} similar ~/.claude/crew/wren --stdin <<'END_OF_TEXT'"
    )));
    assert!(skill.contains(&format!(
        "{word} record ~/.claude/crew/wren --stdin <<'END_OF_TEXT'"
    )));
    let member =
        voice_core::member::read_member(&home.0.join(".claude/crew/wren/member.json")).unwrap();
    assert_eq!(member.pronouns.as_deref(), Some("they/them"));
    assert_eq!(member.principal_pronouns.as_deref(), Some("they/them"));
    assert_eq!(
        home.success(&["assistants"]),
        "Wren (wren): ~/.claude/crew/wren/ · permission not given yet\n"
    );
    let date = voice_agent::ledger::local_day(SystemTime::now());
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
    assert_eq!(
        record,
        serde_json::json!({"version": voice_core::consent::VERSION, "agreed_on": date})
    );
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
        "voice: You already have an assistant in ~/.claude/crew/wren/.\n"
    );
    assert_eq!(
        voice_core::member::read_member(&home.0.join(".claude/crew/wren/member.json"))
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
        format!(
            "{}Version {}.\n",
            voice_core::consent::plain_text(),
            voice_core::consent::VERSION
        )
    );
    assert!(text.starts_with("Before Voice reads anything\n\nWhat it reads\n"));
    assert!(text.contains("You can change your mind: Forget deletes the history it read and its working files. What it wrote into your assistant's folder stays there until you delete that folder.\n"));
    assert!(text.ends_with(&format!("Version {}.\n", voice_core::consent::VERSION)));
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
        let bad = bad.replace(
            "\"version\":1",
            &format!("\"version\":{}", voice_core::consent::VERSION),
        );
        fs::write(&record, &bad).unwrap();
        assert_eq!(
            home.success(&["assistants"]),
            "Wren (wren): ~/.claude/crew/wren/ · permission not given yet\n",
            "record: {bad}"
        );
    }
}

#[test]
fn new_rolls_back_when_clearing_fails() {
    let home = Home::new();
    let state = home.0.join(".claude/state/wren");
    fs::create_dir_all(state.join("consent.json")).unwrap();
    let output = home.run(&["new", "Wren", "--principal", "Robin"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: Voice couldn't make the assistant's folder. Check there's space on this computer, then try again.\n"
    );
    assert!(!home.0.join(".claude/crew/wren").exists());
    assert!(state.join("consent.json").is_dir());
}

#[test]
fn skill_commands_run_similar_and_record_against_private_state() {
    let home = Home::new();
    home.success(&["new", "Ari", "--principal", "Robin"]);
    home.success(&["consent", "agree", "~/.claude/crew/ari"]);
    let skill = fs::read_to_string(home.0.join(".claude/crew/ari/SKILL.md")).unwrap();
    // The CLI's shipped_cli() returns this binary itself; the test executable lives in deps/.
    let word = voice_core::cli_command::cli_word(
        &PathBuf::from(env!("CARGO_BIN_EXE_voice"))
            .canonicalize()
            .unwrap(),
        &home.0,
    )
    .unwrap();
    let binary = format!("\"{}\"", env!("CARGO_BIN_EXE_voice"));
    let state = home.0.join(".claude/state/ari");
    voice_ingest::private::write_private(&home.0, "ari", "decisions.jsonl",
        b"{\"kind\":\"message\",\"split\":\"train\",\"timestamp\":\"2026-10-25T00:00:00Z\",\"project\":\"garden\",\"text\":\"Choose durable garden benches before adding decorative flower pots outdoors.\"}\n").unwrap();
    let cwd = home.0.join("empty");
    fs::create_dir(&cwd).unwrap();
    let hostile = "$(touch pwned-dollar) `touch pwned-tick` ${HOME} \"double\" 'single' back\\slash café\nEND_OF_TEXT\nend";
    let query = format!("durable garden benches {hostile}");
    let escaped = |text: &str| {
        let json = serde_json::to_string(text).unwrap();
        json[1..json.len() - 1].to_owned()
    };
    let blocks: Vec<_> = skill
        .split("```bash\n")
        .skip(1)
        .map(|part| part.split("```").next().unwrap())
        .filter(|block| block.contains("--stdin"))
        .collect();
    assert_eq!(blocks.len(), 3, "similar and both record blocks use stdin");
    for block in blocks {
        let indent = block
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap();
        let script = block
            .lines()
            .map(|line| &line[indent.min(line.len())..])
            .collect::<Vec<_>>()
            .join("\n")
            .replacen(&word, &binary, 1)
            .replace("<the question and its options>", &escaped(&query));
        let script = [
            "<what you are working on>",
            "<work>",
            "<c>",
            "<q>",
            "<pick>",
            "<rules>",
            "<their answer>",
        ]
        .iter()
        .fold(script, |script, placeholder| {
            script.replace(placeholder, &escaped(hostile))
        })
        .replace("<t>", "medium");
        let output = Command::new("/bin/sh")
            .args(["-c", &script])
            .env("HOME", &home.0)
            .current_dir(&cwd)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if script.contains(" similar ") {
            assert!(
                String::from_utf8(output.stdout)
                    .unwrap()
                    .contains("Choose durable garden benches")
            );
        }
        for folder in [&cwd, &home.0] {
            for name in ["pwned-dollar", "pwned-tick"] {
                assert!(!folder.join(name).exists());
            }
        }
    }
    let log = fs::read_to_string(state.join("live.jsonl")).unwrap();
    let records: Vec<serde_json::Value> = log
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    for record in &records {
        for key in ["work", "question", "category", "ari_pick"] {
            assert_eq!(record[key], hostile);
        }
    }
    assert_eq!(records[0]["kind"], "decided");
    assert_eq!(records[0]["tier"], "high");
    assert!(records[0]["rule"].is_null());
    assert!(records[0]["robin_answer"].is_null());
    assert_eq!(records[1]["kind"], "escalated");
    assert_eq!(records[1]["tier"], "medium");
    assert_eq!(records[1]["rule"], hostile);
    assert_eq!(records[1]["robin_answer"], hostile);
}

#[test]
fn existing_reports_nothing_and_new_refuses_older_state_untouched() {
    let home = Home::new();
    assert_eq!(
        home.success(&["existing", "Ari"]),
        "Nothing from before is called Ari on this computer.\n"
    );
    let state = home.0.join(".claude/state/ari");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("decisions.jsonl"), b"older history\n").unwrap();
    let found = home.success(&["existing", "Ari"]);
    assert!(found.contains(&format!(
        "Private folder: {} (1 file, a put-together history (decisions.jsonl))",
        state.display()
    )));
    assert!(found.ends_with("Voice can back it up and bring it in: voice new Ari --bring-in\n"));
    let output = home.run(&["new", "Ari", "--principal", "Robin"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: Voice found an older Ari on this computer, so it made nothing. See what it found: voice existing Ari. To back it up and bring it in: voice new Ari --bring-in (with the same options).\n"
    );
    assert_eq!(
        fs::read(state.join("decisions.jsonl")).unwrap(),
        b"older history\n"
    );
    assert!(!home.0.join(".claude/crew/ari").exists());
}

#[test]
fn existing_reports_skill_lines_and_old_folder_in_custom_claude_home() {
    let home = Home::new();
    let claude = home.0.join("custom-claude");
    let skill = claude.join("skills/ari");
    fs::create_dir_all(&skill).unwrap();
    let old = voice_core::assistants::make(
        &home.0.join("old-kit"),
        &voice_core::assistants::New {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
        },
        "2026-11-12",
        "voice",
    )
    .unwrap();
    fs::write(
        skill.join("SKILL.md"),
        format!(
            "run --member {} similar",
            old.dir.join("member.json").display()
        ),
    )
    .unwrap();
    fs::write(claude.join("CLAUDE.md"), "first\nAsk /ari.\n").unwrap();
    let args = ["existing", "Ari", "--claude-home", claude.to_str().unwrap()];
    let found = home.success(&args);
    assert!(found.contains(&format!("Skill Voice didn't add: {}", skill.display())));
    assert!(found.contains(&format!(
        "CLAUDE.md lines naming /ari: {} lines 2",
        claude.join("CLAUDE.md").display()
    )));
    assert!(found.contains(&format!("Folder it was made in: {}", old.dir.display())));
    assert!(
        !home
            .run(&[
                "new",
                "Ari",
                "--principal",
                "Robin",
                "--claude-home",
                claude.to_str().unwrap()
            ])
            .status
            .success()
    );
    assert!(!home.0.join(".claude/crew/ari").exists());
}

#[test]
fn existing_reports_each_source_alone_and_rejects_bad_names_like_new() {
    for source in ["skill", "line", "file", "link"] {
        let home = Home::new();
        fs::create_dir_all(home.0.join(".claude/state")).unwrap();
        let expected = match source {
            "skill" => {
                fs::create_dir_all(home.0.join(".claude/skills/ari")).unwrap();
                "Skill Voice didn't add:"
            }
            "line" => {
                fs::write(home.0.join(".claude/CLAUDE.md"), "Ask /ari.\n").unwrap();
                "CLAUDE.md lines naming /ari:"
            }
            "file" => {
                fs::write(home.0.join(".claude/state/ari"), "keep").unwrap();
                "(a file, not a folder)"
            }
            _ => {
                std::os::unix::fs::symlink(
                    home.0.join("missing"),
                    home.0.join(".claude/state/ari"),
                )
                .unwrap();
                "(a link to somewhere else)"
            }
        };
        assert!(home.success(&["existing", "Ari"]).contains(expected));
    }
    let home = Home::new();
    for name in ["bad@", "???"] {
        let older = home.run(&["existing", name]);
        let make = home.run(&["new", name, "--principal", "Robin"]);
        assert_eq!(older.status.code(), Some(1));
        assert_eq!(older.stderr, make.stderr);
    }
}

#[test]
fn existing_reports_voice_assistant_before_looking_for_older_files() {
    let home = Home::new();
    home.success(&["new", "Ari", "--principal", "Robin"]);
    fs::create_dir_all(home.0.join(".claude/state/ari")).unwrap();
    fs::write(
        home.0.join(".claude/state/ari/decisions.jsonl"),
        "invented history",
    )
    .unwrap();
    fs::create_dir_all(home.0.join(".claude/skills/ari")).unwrap();
    assert_eq!(
        home.success(&["existing", "Ari"]),
        "You already have an assistant in ~/.claude/crew/ari/.\n"
    );
}

#[test]
fn new_bring_in_backs_up_the_older_history_before_making_the_assistant() {
    let home = Home::new();
    let state = home.0.join(".claude/state/ari");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("decisions.jsonl"), b"older history\n").unwrap();
    let text = home.success(&["new", "Ari", "--principal", "Robin", "--bring-in"]);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "Made Ari in ~/.claude/crew/ari/.");
    let backup = lines[1]
        .strip_prefix("Voice backed up the older Ari first, in ")
        .and_then(|line| line.strip_suffix('.'))
        .expect("backup line between Made and Next");
    assert!(backup.starts_with("~/.claude/voice-backups/ari/"));
    let backup = home.0.join(backup.strip_prefix("~/").unwrap());
    assert_eq!(
        fs::read(backup.join("state/decisions.jsonl")).unwrap(),
        b"older history\n"
    );
    assert_eq!(
        fs::read(state.join("decisions-before-voice.jsonl")).unwrap(),
        b"older history\n"
    );
    assert!(!state.join("decisions.jsonl").exists());
    assert!(home.0.join(".claude/crew/ari/member.json").is_file());
    assert_eq!(
        lines[2],
        "Next: voice consent show, then voice consent agree ~/.claude/crew/ari"
    );
}

#[test]
fn existing_explains_why_a_link_or_saved_history_cannot_be_brought_in() {
    for linked in [true, false] {
        let home = Home::new();
        let state = home.0.join(".claude/state/ari");
        fs::create_dir_all(state.parent().unwrap()).unwrap();
        let reason = if linked {
            std::os::unix::fs::symlink(home.0.join("missing"), &state).unwrap();
            "Voice can't bring in ~/.claude/state/ari because it's a link or a file, not a folder. Move it somewhere else, or pick another name."
        } else {
            fs::create_dir(&state).unwrap();
            fs::write(state.join("decisions-before-voice.jsonl"), "keep").unwrap();
            "Voice can't bring this one in: its private folder already holds a history from before Voice. Pick another name for now."
        };
        let text = home.success(&["existing", "Ari"]);
        assert!(
            text.ends_with(&format!("Voice can't bring this one in. {reason}\n")),
            "{text}"
        );
    }
}
