use persona_builder_agent::{ledger::local_day, text};
use persona_builder_api::{dto::*, *};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};
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
    let made = new_assistant_in(h, request()).unwrap();
    assert_eq!(made.slug, "ari");
    let skill = fs::read_to_string(h.join(".claude/crew/ari/SKILL.md")).unwrap();
    let word = persona_builder::cli_command::cli_word(
        &persona_builder::cli_command::shipped_cli().unwrap(),
        h,
    )
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
        serde_json::json!({"version":1,"agreed_on":agreed.agreed_on})
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
        fs::write(&path, content).unwrap();
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
            new_assistant_in(h, req).unwrap_err().message,
            format!(
                "{title} can be up to 64 characters on one line, without @ < > {{ }} [ ] ( ) # \\ or `."
            )
        );
    }
    let mut req = request();
    req.name = "!!!".into();
    assert_eq!(
        new_assistant_in(h, req).unwrap_err().message,
        "Your assistant's name needs at least one letter or number."
    );
    new_assistant_in(h, request()).unwrap();
    assert_eq!(
        new_assistant_in(h, request()).unwrap_err().message,
        "You already have an assistant called Ari. Pick another name, or open that one."
    );
    let mut req = request();
    req.name = "ARI".into();
    assert_eq!(
        new_assistant_in(h, req).unwrap_err().message,
        "You already have an assistant called ARI. Pick another name, or open that one."
    );
    for s in ["../x", "A", "absent"] {
        let expected = "Persona Builder couldn't find that assistant. Close it and open it again.";
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
        new_assistant_in(h, req).unwrap_err().message,
        "You already have an assistant called Blocked. Pick another name, or open that one."
    );
    let broken = Home::new();
    fs::write(broken.path().join(".claude"), "file").unwrap();
    assert_eq!(
        list_assistants_in(broken.path()).unwrap_err().message,
        "Persona Builder couldn't look for your assistants. Close it and open it again."
    );
    assert_eq!(
        new_assistant_in(broken.path(), request())
            .unwrap_err()
            .message,
        "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again."
    );
    fs::create_dir_all(h.join(".claude/state")).unwrap();
    fs::write(h.join(".claude/state/ari"), "file").unwrap();
    assert_eq!(
        agree_consent_in(h, slug("ari")).unwrap_err().message,
        "Persona Builder couldn't keep your answer. Check there's space on this computer, then try again."
    );
}
#[test]
fn folder_opener_gets_canonical_folder_and_safe_errors() {
    let home = Home::new();
    let h = home.path();
    new_assistant_in(h, request()).unwrap();
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
        "Persona Builder couldn't open the folder."
    );
}
#[test]
fn claude_status_uses_only_safe_messages() {
    let home = Home::new();
    let h = home.path();
    assert_eq!(
        claude_check_in(h, Some(std::ffi::OsStr::new("")), &[]).unwrap(),
        ClaudeStatus {
            ready: false,
            message: text::NOT_INSTALLED.into(),
            note: None
        }
    );
    let bin = h.join("bin");
    fs::create_dir(&bin).unwrap();
    let program = bin.join("claude");
    for (auth, expected) in [
        ("true", text::ready("2.1.292")),
        ("false", text::SIGNED_OUT.into()),
    ] {
        fs::write(&program,format!("#!/bin/sh\nif [ \"$1\" = --version ]; then echo '2.1.292 (Claude Code)'; else echo '{{\"loggedIn\":{auth}}}'; fi\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let status = claude_check_in(h, Some(bin.as_os_str()), &[]).unwrap();
        assert_eq!(status.ready, auth == "true");
        assert_eq!(status.message, expected);
        if status.ready {
            assert_eq!(
                status.note,
                std::env::var_os("ANTHROPIC_API_KEY")
                    .filter(|v| !v.is_empty())
                    .map(|_| text::API_KEY_SET.into())
            );
        }
    }
    fs::write(&program, "#!/bin/sh\necho private-detail\n").unwrap();
    assert_eq!(
        claude_check_in(h, None, &[program]).unwrap().message,
        text::DOES_NOT_RUN
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
    assert_eq!(wording.title, persona_builder::consent::TITLE);
    assert_eq!(wording.version, persona_builder::consent::VERSION);
    assert_eq!(
        wording.sections.len(),
        persona_builder::consent::SECTIONS.len()
    );
    for (got, expected) in wording
        .sections
        .iter()
        .zip(persona_builder::consent::SECTIONS)
    {
        assert_eq!(got.heading, expected.heading);
        assert_eq!(got.lines, expected.lines);
    }
}

#[test]
fn new_answer_does_not_reuse_orphaned_state() {
    let home = Home::new();
    let h = home.path();
    let path = h.join(".claude/state/ari/consent.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, r#"{"version":1,"agreed_on":"2026-10-25"}"#).unwrap();
    assert_eq!(
        new_assistant_in(h, request()).unwrap().consent,
        Consent {
            agreed: false,
            agreed_on: None
        }
    );
    assert!(!list_assistants_in(h).unwrap()[0].consent.agreed);
    assert!(!path.exists());
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
        "Persona Builder couldn't find that assistant. Close it and open it again."
    );
}

#[test]
fn failed_consent_clear_removes_only_the_new_assistant() {
    let home = Home::new();
    let state = home.path().join(".claude/state");
    fs::create_dir_all(&state).unwrap();
    let elsewhere = home.path().join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, state.join("ari")).unwrap();
    assert_eq!(
        new_assistant_in(home.path(), request())
            .unwrap_err()
            .message,
        "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again."
    );
    assert!(!home.path().join(".claude/crew/ari").exists());
    assert!(state.join("ari").is_symlink());
}

#[test]
fn missing_home_claude_check_returns_a_status() {
    const CHILD: &str = "PB_TEST_MISSING_HOME";
    if std::env::var_os(CHILD).is_some() {
        assert_eq!(
            claude_check().unwrap(),
            ClaudeStatus {
                ready: false,
                message: text::FAILED.into(),
                note: None,
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
            new_assistant_in(home.path(), request())
                .unwrap_err()
                .message,
            persona_builder::cli_command::CLI_FOLDER
        );
        assert!(!home.path().join(".claude/crew/ari").exists());
        return;
    }
    let scratch = Home::new();
    let dir = scratch.path().join("Persona Builder $folder.app");
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
