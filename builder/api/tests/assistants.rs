use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};
use voice_agent::{ledger::local_day, text};
use voice_api::{dto::*, *};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "pb-api-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn request() -> NewAssistantRequest {
    NewAssistantRequest {
        name: "Ari".into(),
        pronouns: "they/them".into(),
        principal: "Robin".into(),
        principal_pronouns: "they/them".into(),
    }
}
fn slug(value: &str) -> SlugRequest {
    SlugRequest { slug: value.into() }
}
#[test]
fn new_list_and_private_consent_round_trip() {
    let home = Home::new();
    let h = home.path();
    assert!(list_assistants_in(h).unwrap().is_empty());
    let made = new_assistant_in(h, &h.join(".claude"), request()).unwrap();
    assert_eq!(made.slug, "ari");
    let skill = fs::read_to_string(h.join(".claude/crew/ari/SKILL.md")).unwrap();
    let word =
        voice_core::cli_command::cli_word(&voice_core::cli_command::shipped_cli().unwrap(), h)
            .unwrap();
    assert!(!skill.contains("{{"), "unfilled placeholder in SKILL.md");
    assert!(
        !skill.contains("crew_decisions.py"),
        "app-made assistant names Python"
    );
    assert!(skill.contains(&format!(
        "{word} similar ~/.claude/crew/ari --stdin <<'END_OF_TEXT'"
    )));
    assert!(skill.contains(&format!(
        "{word} record ~/.claude/crew/ari --stdin <<'END_OF_TEXT'"
    )));

    assert!(!made.consent.agreed);
    assert_eq!(list_assistants_in(h).unwrap(), vec![made]);
    let agreed = agree_consent_in(h, slug("ari")).unwrap();
    assert_eq!(
        agreed,
        Consent {
            agreed: true,
            agreed_on: Some(local_day(SystemTime::now()))
        }
    );
    assert_eq!(list_assistants_in(h).unwrap()[0].consent, agreed);
    let path = h.join(".claude/state/ari/consent.json");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        record,
        serde_json::json!({"version":voice_core::consent::VERSION,"agreed_on":agreed.agreed_on})
    );
    for content in [
        r#"{"version":0,"version":1,"agreed_on":"2026-10-25"}"#,
        r#"{"version":1,"agreed_on":"2026-10-24","agreed_on":"2026-10-25"}"#,
        r#"{"version":0,"agreed_on":"2026-10-25"}"#,
        r#"{"version":1}"#,
        r#"{"version":1,"agreed_on":null}"#,
        r#"{"version":1,"agreed_on":"2026-10-25","extra":true}"#,
        r#"{"version":1,"agreed_on":"not-a-date"}"#,
        r#"{"version":1,"agreed_on":"2026-99-99"}"#,
        "bad JSON",
    ] {
        let content = content.replace(
            "\"version\":1",
            &format!("\"version\":{}", voice_core::consent::VERSION),
        );
        fs::write(&path, &content).unwrap();
        assert_eq!(
            list_assistants_in(h).unwrap()[0].consent,
            Consent {
                agreed: false,
                agreed_on: None
            },
            "{content}"
        );
    }
}
#[test]
fn fixed_failure_messages_hide_core_details() {
    let home = Home::new();
    let h = home.path();
    for (field, title) in [
        (0, "Your assistant's name"),
        (1, "Its pronouns"),
        (2, "Your name"),
        (3, "Your pronouns"),
    ] {
        let mut req = request();
        *match field {
            0 => &mut req.name,
            1 => &mut req.pronouns,
            2 => &mut req.principal,
            _ => &mut req.principal_pronouns,
        } = "bad@input".into();
        assert_eq!(
            new_assistant_in(h, &h.join(".claude"), req)
                .unwrap_err()
                .message,
            format!(
                "{title} can be up to 64 characters on one line, without @ < > {{ }} [ ] ( ) # \\ or `."
            )
        );
    }
    let mut req = request();
    req.name = "!!!".into();
    assert_eq!(
        new_assistant_in(h, &h.join(".claude"), req)
            .unwrap_err()
            .message,
        "Your assistant's name needs at least one letter or number."
    );
    new_assistant_in(h, &h.join(".claude"), request()).unwrap();
    assert_eq!(
        new_assistant_in(h, &h.join(".claude"), request())
            .unwrap_err()
            .message,
        "You already have an assistant called Ari. Pick another name, or open that one."
    );
    let mut req = request();
    req.name = "ARI".into();
    assert_eq!(
        new_assistant_in(h, &h.join(".claude"), req)
            .unwrap_err()
            .message,
        "You already have an assistant called ARI. Pick another name, or open that one."
    );
    for s in ["../x", "A", "absent"] {
        let expected = "Voice couldn't find that assistant. Close it and open it again.";
        assert_eq!(agree_consent_in(h, slug(s)).unwrap_err().message, expected);
        assert_eq!(
            show_assistant_folder_in(h, slug(s), |_| panic!("invalid folder opened"))
                .unwrap_err()
                .message,
            expected
        );
    }
    fs::create_dir_all(h.join(".claude/crew/blocked")).unwrap();
    let mut req = request();
    req.name = "Blocked".into();
    assert_eq!(
        new_assistant_in(h, &h.join(".claude"), req)
            .unwrap_err()
            .message,
        "You already have an assistant called Blocked. Pick another name, or open that one."
    );
    let broken = Home::new();
    fs::write(broken.path().join(".claude"), "file").unwrap();
    assert_eq!(
        list_assistants_in(broken.path()).unwrap_err().message,
        "Voice couldn't look for your assistants. Close it and open it again."
    );
    assert_eq!(
        new_assistant_in(broken.path(), &broken.path().join(".claude"), request())
            .unwrap_err()
            .message,
        "Voice couldn't look for an older assistant with that name. Close it and open it again."
    );
    fs::create_dir_all(h.join(".claude/state")).unwrap();
    fs::write(h.join(".claude/state/ari"), "file").unwrap();
    assert_eq!(
        agree_consent_in(h, slug("ari")).unwrap_err().message,
        "Voice couldn't keep your answer. Check there's space on this computer, then try again."
    );
}
#[test]
fn folder_opener_gets_canonical_folder_and_safe_errors() {
    let home = Home::new();
    let h = home.path();
    new_assistant_in(h, &h.join(".claude"), request()).unwrap();
    show_assistant_folder_in(h, slug("ari"), |p| {
        assert_eq!(p, h.join(".claude/crew/ari").canonicalize().unwrap());
        Ok(())
    })
    .unwrap();
    assert_eq!(
        show_assistant_folder_in(h, slug("ari"), |_| Err(std::io::Error::other(
            "private path"
        )))
        .unwrap_err()
        .message,
        "Voice couldn't open the folder."
    );
}
#[test]
fn claude_status_uses_only_safe_messages() {
    const CHILD: &str = "PB_TEST_API_KEY_NOTE";
    if std::env::var_os(CHILD).is_none() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "claude_status_uses_only_safe_messages"])
            .env(CHILD, "1")
            .env("ANTHROPIC_API_KEY", "not-a-real-key")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        return;
    }
    let home = Home::new();
    let h = home.path();
    assert_eq!(
        claude_check_in(h, Some(std::ffi::OsStr::new("")), &[]).unwrap(),
        ClaudeStatus {
            subscription: false,
            ready: false,
            message: text::NOT_INSTALLED.into(),
            note: None,
            can_sign_in: false,
            can_sign_in_with_subscription: false,
        }
    );
    let bin = h.join("bin");
    fs::create_dir(&bin).unwrap();
    let program = bin.join("claude");
    let signed_out = if !cfg!(target_os = "macos") {
        "Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with an Anthropic Console account."
    } else if voice_agent::claude::SUBSCRIPTION_OFFERED {
        "Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with your Claude subscription."
    } else {
        "Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself. Signing in with a Console account doesn't work on a Mac yet. A later update adds it."
    };
    for (auth, expected) in [
        ("true", text::ready("2.1.292")),
        ("false", signed_out.into()),
    ] {
        fs::write(&program,format!("#!/bin/sh\nshift 2\nif [ \"$1\" = --version ]; then echo '2.1.292 (Claude Code)'; else echo '{{\"loggedIn\":{auth},\"authMethod\":\"api_key\"}}'; fi\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let status = claude_check_in(h, Some(bin.as_os_str()), &[]).unwrap();
        assert_eq!(status.ready, auth == "true");
        assert_eq!(status.message, expected);
        assert_eq!(status.can_sign_in, auth == "false");
        assert!(!status.message.contains(h.to_str().unwrap()));
        assert_eq!(status.note, None);
    }
    fs::write(&program, "#!/bin/sh\nshift 2\necho private-detail\n").unwrap();
    assert_eq!(
        claude_check_in(h, None, std::slice::from_ref(&program))
            .unwrap()
            .message,
        text::DOES_NOT_RUN
    );
    assert!(!claude_check_in(h, None, &[program]).unwrap().can_sign_in);
}
#[test]
fn find_existing_request_refuses_unknown_fields() {
    assert!(
        serde_json::from_value::<FindExistingRequest>(serde_json::json!({"name":"Ari","extra":1}))
            .is_err()
    );
}

#[test]
fn dto_json_and_wording_match_contract() {
    let req = request();
    assert_eq!(
        serde_json::to_value(req).unwrap(),
        serde_json::json!({"name":"Ari","pronouns":"they/them","principal":"Robin","principalPronouns":"they/them"})
    );
    assert!(serde_json::from_value::<NewAssistantRequest>(serde_json::json!({"name":"Ari","pronouns":"they/them","principal":"Robin","principalPronouns":"they/them","extra":1})).is_err());
    assert!(
        serde_json::from_value::<SlugRequest>(serde_json::json!({"slug":"ari","extra":1})).is_err()
    );
    assert_eq!(
        serde_json::to_value(Consent {
            agreed: true,
            agreed_on: Some("2026-10-25".into())
        })
        .unwrap(),
        serde_json::json!({"agreed":true,"agreedOn":"2026-10-25"})
    );
    let wording = consent_text().unwrap();
    assert_eq!(wording.title, voice_core::consent::TITLE);
    assert_eq!(wording.version, voice_core::consent::VERSION);
    assert_eq!(wording.sections.len(), voice_core::consent::SECTIONS.len());
    for (got, expected) in wording.sections.iter().zip(voice_core::consent::SECTIONS) {
        assert_eq!(got.heading, expected.heading);
        assert_eq!(got.lines, expected.lines);
    }
}

#[test]
fn new_refuses_orphaned_state_without_changing_it() {
    let home = Home::new();
    let h = home.path();
    let path = h.join(".claude/state/ari/decisions.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"older history\n").unwrap();
    assert_eq!(
        new_assistant_in(h, &h.join(".claude"), request())
            .unwrap_err()
            .message,
        "Voice found an older Ari on this computer, so it made nothing and changed nothing. Choose Back it up and bring it in, or pick another name."
    );
    assert_eq!(fs::read(path).unwrap(), b"older history\n");
    assert!(!h.join(".claude/crew/ari").exists());
}

#[test]
fn unlisted_folder_is_never_opened() {
    let home = Home::new();
    fs::create_dir_all(home.path().join(".claude/crew/ari")).unwrap();
    assert_eq!(
        show_assistant_folder_in(home.path(), slug("ari"), |_| panic!(
            "unlisted folder opened"
        ))
        .unwrap_err()
        .message,
        "Voice couldn't find that assistant. Close it and open it again."
    );
}

#[test]
fn failed_consent_clear_removes_only_the_new_assistant() {
    let home = Home::new();
    let state = home.path().join(".claude/state/ari");
    fs::create_dir_all(state.join("consent.json")).unwrap();
    assert_eq!(
        new_assistant_in(home.path(), &home.path().join(".claude"), request())
            .unwrap_err()
            .message,
        "Voice couldn't make the assistant's folder. Check there's space on this computer, then try again."
    );
    assert!(!home.path().join(".claude/crew/ari").exists());
    assert!(state.join("consent.json").is_dir());
}

#[test]
fn missing_home_claude_check_returns_a_status() {
    const CHILD: &str = "PB_TEST_MISSING_HOME";
    if std::env::var_os(CHILD).is_some() {
        assert_eq!(
            claude_check().unwrap(),
            ClaudeStatus {
                subscription: false,
                ready: false,
                message: text::FAILED.into(),
                note: None,
                can_sign_in: false,
                can_sign_in_with_subscription: false,
            }
        );
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "missing_home_claude_check_returns_a_status"])
        .env(CHILD, "1")
        .env_remove("HOME")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn unsafe_app_folder_shows_the_move_sentence() {
    const CHILD: &str = "PB_TEST_UNSAFE_APP_FOLDER";
    if std::env::var_os(CHILD).is_some() {
        let home = Home::new();
        assert_eq!(
            new_assistant_in(home.path(), &home.path().join(".claude"), request())
                .unwrap_err()
                .message,
            voice_core::cli_command::CLI_FOLDER
        );
        assert!(!home.path().join(".claude/crew/ari").exists());
        return;
    }
    let scratch = Home::new();
    let dir = scratch.path().join("Voice $folder.app");
    fs::create_dir(&dir).unwrap();
    let executable = dir.join("app-test");
    fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
    let output = std::process::Command::new(executable)
        .args(["--exact", "unsafe_app_folder_shows_the_move_sentence"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn version_one_consent_is_no_permission_given() {
    let home = Home::new();
    let h = home.path();
    new_assistant_in(h, &h.join(".claude"), request()).unwrap();
    agree_consent_in(h, slug("ari")).unwrap();
    fs::write(
        h.join(".claude/state/ari/consent.json"),
        r#"{"version":1,"agreed_on":"2026-10-25"}"#,
    )
    .unwrap();
    assert_eq!(
        list_assistants_in(h).unwrap()[0].consent,
        Consent {
            agreed: false,
            agreed_on: None
        }
    );
}

#[test]
fn find_existing_reports_fixed_sentences_without_private_paths() {
    let home = Home::new();
    let h = home.path();
    let claude = h.join("custom-claude");
    let request = || FindExistingRequest { name: "Ari".into() };
    assert_eq!(
        find_existing_in(h, &claude, request()).unwrap(),
        ExistingAssistant {
            found: false,
            can_bring_in: false,
            files: vec![],
            lines: vec![]
        }
    );
    let state = h.join(".claude/state/ari");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("decisions.jsonl"), "keep").unwrap();
    fs::write(state.join("live.jsonl"), "keep too").unwrap();
    let skill = claude.join("skills/ari");
    fs::create_dir_all(&skill).unwrap();
    let old = voice_core::assistants::make(
        &h.join("old-kit"),
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
    fs::write(claude.join("CLAUDE.md"), "Ask /ari.\nrun `/ARI`.\n").unwrap();
    let found = find_existing_in(h, &claude, request()).unwrap();
    assert!(found.found);
    assert_eq!(
        found.lines,
        [
            "A private folder from before, ~/.claude/state/ari/, holds 2 files.",
            "One of them is a put-together history that Voice would replace.",
            "One of them is a log of answers you recorded.",
            "Claude Code has a skill called ari that Voice didn't add.",
            "Your CLAUDE.md mentions /ari on 2 lines that Voice didn't add.",
            "That skill points to the folder the assistant was made in, with its own files.",
        ]
    );
    assert!(
        !serde_json::to_string(&found)
            .unwrap()
            .contains(h.to_str().unwrap())
    );
}

#[test]
fn find_existing_name_validation_and_errors_are_fixed() {
    let home = Home::new();
    let h = home.path();
    for name in ["bad@", "???"] {
        let mut req = request();
        req.name = name.into();
        assert_eq!(
            find_existing_in(
                h,
                &h.join(".claude"),
                FindExistingRequest { name: name.into() }
            )
            .unwrap_err(),
            new_assistant_in(h, &h.join(".claude"), req).unwrap_err()
        );
    }
    fs::create_dir_all(h.join(".claude")).unwrap();
    fs::write(h.join(".claude/CLAUDE.md"), vec![b'x'; 4 * 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        find_existing_in(
            h,
            &h.join(".claude"),
            FindExistingRequest { name: "Ari".into() }
        )
        .unwrap_err()
        .message,
        "Voice couldn't look for an older assistant with that name. Close it and open it again."
    );
    assert_eq!(
        new_assistant_in(h, &h.join(".claude"), request())
            .unwrap_err()
            .message,
        "Voice couldn't look for an older assistant with that name. Close it and open it again."
    );
    assert!(!h.join(".claude/crew").exists());
}

#[test]
fn voice_assistant_is_not_older_and_cannot_be_made_twice() {
    let home = Home::new();
    let h = home.path();
    let claude = h.join(".claude");
    new_assistant_in(h, &claude, request()).unwrap();
    fs::create_dir_all(claude.join("state/ari")).unwrap();
    fs::write(claude.join("state/ari/decisions.jsonl"), "invented history").unwrap();
    fs::create_dir_all(claude.join("skills/ari")).unwrap();
    assert_eq!(
        find_existing_in(h, &claude, FindExistingRequest { name: "Ari".into() }).unwrap(),
        ExistingAssistant {
            found: false,
            can_bring_in: false,
            files: vec![],
            lines: vec![]
        }
    );
    assert_eq!(
        new_assistant_in(h, &claude, request()).unwrap_err().message,
        "You already have an assistant called Ari. Pick another name, or open that one."
    );
}

fn old_state(h: &Path) -> PathBuf {
    let state = h.join(".claude/state/ari");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("decisions.jsonl"), b"older history\n").unwrap();
    state
}
fn snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, (String, Vec<u8>)> {
    fn walk(
        root: &Path,
        at: &Path,
        out: &mut std::collections::BTreeMap<PathBuf, (String, Vec<u8>)>,
    ) {
        let meta = fs::symlink_metadata(at).unwrap();
        let (kind, bytes) = if meta.is_symlink() {
            (
                "link",
                fs::read_link(at)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec(),
            )
        } else if meta.is_dir() {
            ("folder", vec![])
        } else if meta.is_file() {
            ("file", fs::read(at).unwrap())
        } else {
            ("special", vec![])
        };
        out.insert(
            at.strip_prefix(root).unwrap().to_owned(),
            (kind.into(), bytes),
        );
        if meta.is_dir() {
            for entry in fs::read_dir(at).unwrap() {
                walk(root, &entry.unwrap().path(), out);
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}
#[test]
fn bring_in_backs_up_all_four_and_keeps_older_files() {
    let home = Home::new();
    let h = home.path();
    let claude = h.join("custom-claude");
    let state = old_state(h);
    fs::write(state.join("live.jsonl"), "older answers\n").unwrap();
    fs::write(state.join("other"), "other bytes").unwrap();
    fs::write(state.join("consent.json"), "old agreement").unwrap();
    let skill = claude.join("skills/ari");
    fs::create_dir_all(&skill).unwrap();
    let old = voice_core::assistants::make(
        &h.join("old-kit"),
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
    fs::write(claude.join("CLAUDE.md"), "Ask /ari.\n").unwrap();
    let before_state = snapshot(&state);
    let before_skill = snapshot(&skill);
    let before_member = snapshot(&old.dir);
    let result = bring_in_assistant_in(h, &claude, bringing(&[])).unwrap();
    let shown = result.backup.unwrap();
    assert!(shown.starts_with("~/.claude/voice-backups/ari/"));
    assert!(!shown.contains(h.to_str().unwrap()));
    let backup = h.join(shown.strip_prefix("~/").unwrap());
    assert_eq!(snapshot(&backup.join("state")), before_state);
    assert_eq!(snapshot(&backup.join("skill")), before_skill);
    assert_eq!(snapshot(&backup.join("member")), before_member);
    assert_eq!(fs::read(backup.join("CLAUDE.md")).unwrap(), b"Ask /ari.\n");
    assert_eq!(
        fs::read(state.join("decisions-before-voice.jsonl")).unwrap(),
        b"older history\n"
    );
    assert!(!state.join("decisions.jsonl").exists());
    assert!(!state.join("consent.json").exists());
    assert!(h.join(".claude/crew/ari/member.json").exists());
    assert!(!result.assistant.consent.agreed);
    assert_eq!(snapshot(&skill), before_skill);
    assert_eq!(snapshot(&old.dir), before_member);
    assert_eq!(fs::read(claude.join("CLAUDE.md")).unwrap(), b"Ask /ari.\n");
    assert!(
        !find_existing_in(h, &claude, FindExistingRequest { name: "Ari".into() })
            .unwrap()
            .found
    );
}
#[test]
fn bring_in_preserves_all_questions_when_histories_are_put_together() {
    let home = Home::new();
    let h = home.path();
    let state = old_state(h);
    let records = |ids: &[&str]| {
        ids.iter()
            .map(|id| {
                serde_json::json!({
                    "id": id, "kind": "question", "timestamp": "2026-01-01T10:00:00Z",
                    "question": "Which approach should we use?", "header": "Approach",
                "options": [{"label": "Small", "description": "Start small"}],
                "picks": ["Small"], "own_words": "", "notes": ""
                })
                .to_string()
                    + "\n"
            })
            .collect::<String>()
    };
    fs::write(state.join("decisions.jsonl"), records(&["q1", "q2"])).unwrap();
    fs::write(
        state.join("decisions-prev-2026-01-01.jsonl"),
        records(&["q3", "q4"]),
    )
    .unwrap();
    bring_in_assistant_in(h, &h.join(".claude"), bringing(&[])).unwrap();
    agree_consent_in(h, slug("ari")).unwrap();
    assert_eq!(merge_histories_in(h, slug("ari")).unwrap().questions, 4);
    let bytes = fs::read(state.join("decisions.jsonl")).unwrap();
    let mut ids: Vec<_> = voice_ingest::corpus::read_records("decisions.jsonl", &bytes)
        .unwrap()
        .into_iter()
        .map(|(_, r)| r["id"].as_str().unwrap().to_owned())
        .collect();
    ids.sort();
    assert_eq!(ids, ["q1", "q2", "q3", "q4"]);
}
fn refused_tree(kind: &str, sentence: &str) {
    let home = Home::new();
    let h = home.path();
    fs::create_dir_all(h.join(".claude/state")).unwrap();
    match kind {
        "link" => {
            std::os::unix::fs::symlink(h.join("missing"), h.join(".claude/state/ari")).unwrap()
        }
        "file" => fs::write(h.join(".claude/state/ari"), "plain file").unwrap(),
        "before" => {
            let state = old_state(h);
            fs::write(state.join("decisions-before-voice.jsonl"), "keep").unwrap();
        }
        "corpus-link" => {
            let state = old_state(h);
            fs::remove_file(state.join("decisions.jsonl")).unwrap();
            std::os::unix::fs::symlink(h.join("missing"), state.join("decisions.jsonl")).unwrap();
        }
        _ => unreachable!(),
    }
    let before = snapshot(h);
    let err = bring_in_assistant_in(h, &h.join(".claude"), bringing(&[])).unwrap_err();
    assert_eq!(err.message, sentence);
    assert_eq!(snapshot(h), before);
    assert!(!h.join(".claude/voice-backups").exists());
    assert!(
        !find_existing_in(
            h,
            &h.join(".claude"),
            FindExistingRequest { name: "Ari".into() }
        )
        .unwrap()
        .can_bring_in
    );
}
const STATE_REFUSAL: &str = "Voice can't bring in ~/.claude/state/ari because it's a link or a file, not a folder. Move it somewhere else, or pick another name.";
const HISTORY_REFUSAL: &str = "Voice can't bring this one in: its private folder already holds a history from before Voice. Pick another name for now.";
#[test]
fn bring_in_refuses_linked_state_without_writing() {
    refused_tree("link", STATE_REFUSAL);
}
#[test]
fn bring_in_refuses_file_state_without_writing() {
    refused_tree("file", STATE_REFUSAL);
}
#[test]
fn bring_in_refuses_existing_before_history_without_writing() {
    refused_tree("before", HISTORY_REFUSAL);
}
#[test]
fn bring_in_refuses_linked_corpus_without_writing() {
    refused_tree("corpus-link", CORPUS_REFUSAL);
}
#[test]
fn bring_in_rolls_back_history_and_backup_when_make_fails() {
    let home = Home::new();
    let h = home.path();
    let state = old_state(h);
    fs::write(h.join(".claude/crew"), "plain file").unwrap();
    let before = snapshot(h);
    assert_eq!(
        bring_in_assistant_in(h, &h.join(".claude"), bringing(&[]))
            .unwrap_err()
            .message,
        "Voice couldn't make the assistant's folder. Check there's space on this computer, then try again."
    );
    let mut after = snapshot(h);
    after.remove(Path::new(".claude/voice-backups"));
    after.remove(Path::new(".claude/voice-backups/ari"));
    assert_eq!(after, before);
    assert_eq!(
        fs::read(state.join("decisions.jsonl")).unwrap(),
        b"older history\n"
    );
}
#[test]
fn bring_in_without_older_files_makes_an_assistant_without_backup() {
    let home = Home::new();
    let h = home.path();
    let result = bring_in_assistant_in(h, &h.join(".claude"), bringing(&[])).unwrap();
    assert_eq!(result.backup, None);
    assert_eq!(result.assistant.name, "Ari");
    assert!(!result.assistant.consent.agreed);
    assert!(h.join(".claude/crew/ari/member.json").exists());
    assert!(!h.join(".claude/voice-backups").exists());
}
#[test]
fn bring_in_reports_a_plain_old_state_as_available() {
    let home = Home::new();
    let h = home.path();
    old_state(h);
    let result = find_existing_in(
        h,
        &h.join(".claude"),
        FindExistingRequest { name: "Ari".into() },
    )
    .unwrap();
    assert!(result.can_bring_in);
    assert_eq!(serde_json::to_value(result).unwrap()["canBringIn"], true);
}

#[test]
fn bring_in_refuses_empty_corpus_folder_with_an_older_skill() {
    let home = Home::new();
    let h = home.path();
    let claude = h.join(".claude");
    fs::create_dir_all(claude.join("skills/ari")).unwrap();
    fs::create_dir_all(claude.join("state/ari/decisions.jsonl")).unwrap();
    let before = snapshot(h);
    let found = find_existing_in(h, &claude, FindExistingRequest { name: "Ari".into() }).unwrap();
    assert!(found.found);
    assert!(!found.can_bring_in);
    assert_eq!(
        bring_in_assistant_in(h, &claude, bringing(&[]))
            .unwrap_err()
            .message,
        CORPUS_REFUSAL
    );
    assert_eq!(snapshot(h), before);
    assert!(!claude.join("voice-backups").exists());
}

#[test]
fn bring_in_reports_fifo_refusal_without_changing_old_files() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let home = Home::new();
    let h = home.path();
    let state = old_state(h);
    let fifo = CString::new(state.join("pipe").as_os_str().as_bytes()).unwrap();
    // SAFETY: fifo is a live, NUL-terminated path and mode is a valid permission mask.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(
        find_existing_in(
            h,
            &h.join(".claude"),
            FindExistingRequest { name: "Ari".into() }
        )
        .unwrap()
        .can_bring_in
    );
    let before = snapshot(h);
    let error = bring_in_assistant_in(h, &h.join(".claude"), bringing(&[])).unwrap_err();
    assert_eq!(
        error.message,
        "pipe isn't a plain file, so Voice backed nothing up."
    );
    let mut after = snapshot(h);
    assert_eq!(
        after.remove(Path::new(".claude/voice-backups")),
        Some(("folder".into(), vec![]))
    );
    assert_eq!(
        after.remove(Path::new(".claude/voice-backups/ari")),
        Some(("folder".into(), vec![]))
    );
    assert_eq!(after, before);
}

const CORPUS_REFUSAL: &str = "Voice can't bring this one in: the decisions.jsonl in its private folder is a link or a folder, not a file. Pick another name for now.";

fn old_member_files(home: &Path) -> PathBuf {
    let old = voice_core::assistants::make(
        &home.join("old-kit"),
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
    for name in [
        "IDENTITY.md",
        "interview.md",
        "lens.md",
        "decisions.md",
        "measurements.md",
    ] {
        fs::write(old.dir.join(name), format!("Older {name}\n")).unwrap();
    }
    fs::write(old.dir.join("autonomy.md"), "- [x] `release`: Ask first.\n").unwrap();
    fs::remove_dir_all(old.dir.join("briefs")).unwrap();
    fs::create_dir(old.dir.join("briefs")).unwrap();
    fs::write(old.dir.join("briefs/old.md"), "Older brief\n").unwrap();
    fs::write(
        old.dir.join("private.json"),
        r#"{"filters":{"personal":"invented-private-pattern"}}"#,
    )
    .unwrap();
    let skill = home.join(".claude/skills/ari");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        format!("--member {}", old.dir.join("member.json").display()),
    )
    .unwrap();
    old.dir
}
fn fresh_template(home: &Path) -> PathBuf {
    voice_core::assistants::make(
        &home.join("baseline"),
        &voice_core::assistants::New {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
        },
        &local_day(SystemTime::now()),
        &voice_core::cli_command::cli_word(&voice_core::cli_command::shipped_cli().unwrap(), home)
            .unwrap(),
    )
    .unwrap()
    .dir
}

fn bringing(files: &[&str]) -> BringInRequest {
    BringInRequest {
        assistant: request(),
        files: files.iter().map(|name| (*name).into()).collect(),
    }
}
#[test]
fn pb87c_brings_all_own_files_over_template_with_private_modes() {
    let home = Home::new();
    let h = home.path();
    let old = old_member_files(h);
    let result = bring_in_assistant_in(
        h,
        &h.join(".claude"),
        bringing(&voice_core::old_files::NAMES),
    )
    .unwrap();
    assert_eq!(result.files, voice_core::old_files::NAMES);
    let dir = h.join(".claude/crew/ari");
    for name in voice_core::old_files::NAMES {
        let file = if name == "briefs" {
            "briefs/old.md"
        } else {
            name
        };
        assert_eq!(
            fs::read(dir.join(file)).unwrap(),
            fs::read(old.join(file)).unwrap(),
            "{file}"
        );
        assert_eq!(
            fs::metadata(dir.join(file)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert!(dir.join("briefs/codebook.md").exists());
    let baseline = fresh_template(h);
    let mut expected_briefs = snapshot(&baseline.join("briefs"));
    expected_briefs.insert(
        PathBuf::from("old.md"),
        ("file".into(), b"Older brief\n".to_vec()),
    );
    assert_eq!(snapshot(&dir.join("briefs")), expected_briefs);
    for name in ["SKILL.md", "member.json"] {
        assert_eq!(
            fs::read(dir.join(name)).unwrap(),
            fs::read(baseline.join(name)).unwrap()
        );
    }
    let member = voice_core::member::read_member(&dir.join("member.json")).unwrap();
    assert!(member.filters.personal.contains("invented-private-pattern"));
}
#[test]
fn pb87c_only_selected_file_replaces_template_and_empty_keeps_template() {
    for selected in [vec!["IDENTITY.md"], vec![]] {
        let home = Home::new();
        let h = home.path();
        let old = old_member_files(h);
        let result = bring_in_assistant_in(h, &h.join(".claude"), bringing(&selected)).unwrap();
        assert_eq!(result.files, selected);
        let dir = h.join(".claude/crew/ari");
        let baseline = fresh_template(h);
        if selected.is_empty() {
            assert_eq!(snapshot(&dir), snapshot(&baseline));
        } else {
            assert_eq!(
                fs::read(dir.join("IDENTITY.md")).unwrap(),
                fs::read(old.join("IDENTITY.md")).unwrap()
            );
        }
        assert_eq!(
            fs::read(dir.join("autonomy.md")).unwrap(),
            fs::read(baseline.join("autonomy.md")).unwrap()
        );
    }
}
#[test]
fn pb87c_bad_ticks_refuse_before_any_writes() {
    for (kind, file) in [
        ("unknown", "nope.md"),
        ("absent", "lens.md"),
        ("private", "private.json"),
        ("no-folder", "IDENTITY.md"),
        ("duplicate", "IDENTITY.md"),
    ] {
        let home = Home::new();
        let h = home.path();
        if kind != "no-folder" {
            let old = old_member_files(h);
            if kind == "absent" {
                fs::remove_file(old.join("lens.md")).unwrap();
            }
            if kind == "private" {
                fs::write(old.join("private.json"), r#"{"state_dir":"elsewhere"}"#).unwrap();
            }
        }
        let before = snapshot(h);
        let ticks = if kind == "duplicate" {
            vec![file, file]
        } else {
            vec![file]
        };
        let error = bring_in_assistant_in(h, &h.join(".claude"), bringing(&ticks)).unwrap_err();
        assert_eq!(
            error.message,
            format!(
                "Voice can't bring in {file}, so it changed nothing. Leave it out and try again."
            )
        );
        assert_eq!(snapshot(h), before, "{kind}");
        assert!(!h.join(".claude/voice-backups").exists());
    }
}
#[test]
fn pb87c_bad_merged_private_file_refuses_before_backup() {
    assert_private_refuses(r#"{"filters":{"personal":7}}"#);
}
fn assert_private_refuses(value: &str) {
    let home = Home::new();
    let h = home.path();
    let old = old_member_files(h);
    fs::write(old.join("private.json"), value).unwrap();
    let found = find_existing_in(
        h,
        &h.join(".claude"),
        FindExistingRequest { name: "Ari".into() },
    )
    .unwrap();
    assert_eq!(
        found
            .files
            .iter()
            .find(|f| f.name == "private.json")
            .unwrap()
            .problem
            .as_deref(),
        Some("It changes more than the privacy settings, so Voice leaves it."),
        "{value}"
    );
    let before = snapshot(h);
    let error =
        bring_in_assistant_in(h, &h.join(".claude"), bringing(&["private.json"])).unwrap_err();
    assert_eq!(
        error.message,
        "Voice can't bring in private.json, so it changed nothing. Leave it out and try again."
    );
    assert_eq!(snapshot(h), before);
    assert!(!h.join(".claude/voice-backups").exists());
}
#[test]
fn fixup_private_rules_refuse_before_backup() {
    for value in [
        r#"{"filters":{"secret":"("}}"#,
        r#"{"filters":{"secret":""}}"#,
        r#"{"anonymizer":7}"#,
        r#"{"anonymizer":{"bogus":1}}"#,
        r#"{"filters":7}"#,
        r#"{"filters":{"denied_paths":[""]}}"#,
        r#"{"filters":{"pasted_prefixes":[7]}}"#,
    ] {
        assert_private_refuses(value);
    }
}
#[test]
fn fixup_returns_files_in_names_order() {
    let home = Home::new();
    let h = home.path();
    old_member_files(h);
    let result = bring_in_assistant_in(
        h,
        &h.join(".claude"),
        bringing(&["private.json", "lens.md", "IDENTITY.md"]),
    )
    .unwrap();
    assert_eq!(result.files, ["IDENTITY.md", "lens.md", "private.json"]);
}
#[test]
fn pb87c_existing_lists_files_and_problem_sentences() {
    let home = Home::new();
    let h = home.path();
    let old = old_member_files(h);
    fs::write(old.join("private.json"), r#"{"name":"Other"}"#).unwrap();
    let found = find_existing_in(
        h,
        &h.join(".claude"),
        FindExistingRequest { name: "Ari".into() },
    )
    .unwrap();
    assert_eq!(
        found
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        voice_core::old_files::NAMES
    );
    assert_eq!(
        found.files[7].problem.as_deref(),
        Some("It changes more than the privacy settings, so Voice leaves it.")
    );
}
#[test]
fn pb87c_bring_request_refuses_unknown_fields() {
    let value = serde_json::json!({"assistant": request(), "files": [], "extra": true});
    assert!(serde_json::from_value::<BringInRequest>(value).is_err());
}

#[test]
fn fixup_refuses_pattern_that_breaks_the_merged_filter() {
    assert_private_refuses(r#"{"filters":{"secret":"(?x)word# comment"}}"#);
}

#[test]
fn fixup3_empty_group_refuses_before_backup() {
    assert_private_refuses(r#"{"filters":{"secret":"(?:)"}}"#);
}
#[test]
fn fixup3_empty_alternative_refuses_before_backup() {
    assert_private_refuses(r#"{"filters":{"secret":"|"}}"#);
}
#[test]
fn fixup3_zero_or_more_refuses_before_backup() {
    assert_private_refuses(r#"{"filters":{"personal":"a*"}}"#);
}
#[test]
fn fixup3_optional_folder_refuses_before_backup() {
    assert_private_refuses(r#"{"filters":{"excluded_folders":"x?"}}"#);
}

#[test]
fn fixup3_unknown_filter_is_not_compiled() {
    let home = Home::new();
    let h = home.path();
    let old = old_member_files(h);
    let private = r#"{"filters":{"future":"("}}"#;
    fs::write(old.join("private.json"), private).unwrap();
    let found = find_existing_in(
        h,
        &h.join(".claude"),
        FindExistingRequest { name: "Ari".into() },
    )
    .unwrap();
    assert_eq!(
        found
            .files
            .iter()
            .find(|file| file.name == "private.json")
            .unwrap()
            .problem,
        None
    );
    let result = bring_in_assistant_in(h, &h.join(".claude"), bringing(&["private.json"])).unwrap();
    assert_eq!(result.files, ["private.json"]);
    assert_eq!(
        fs::read_to_string(h.join(".claude/crew/ari/private.json")).unwrap(),
        private
    );
}
