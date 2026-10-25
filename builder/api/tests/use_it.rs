use persona_builder::install::{self, Places};
use persona_builder_api::{dto::*, *};
use persona_builder_ingest::private::{lock_state, private_folder};
use persona_builder_save::{git::Git, keychain::MemoryStore};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

const FIND: &str = "Persona Builder couldn't find that assistant. Close it and open it again.";
const CHANGED_ON: &str = "What's on in Claude Code has changed. Look again, then choose.";
const FORGET_CHANGED: &str =
    "There's more to forget than when you looked. Look again, then confirm.";
const BUSY: &str =
    "Persona Builder is already working on Ari. Wait for it to finish, then try again.";
const NO_KINDS: &str =
    "Ari doesn't know any kinds of question yet. Build Ari first, then make the claude.ai files.";
const DECISIONS: &str = "# Decisions\n\n## By kind of question\n\n### writing-voice\n\nUse everyday words.\nLead with the choice.\n\n### scope\n\nKeep the first draft small.\nAdd detail after feedback.\n";
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pb-api-use-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let home = Self(path);
        home.make("Ari");
        home
    }
    fn make(&self, name: &str) {
        new_assistant_in(
            &self.0,
            NewAssistantRequest {
                name: name.into(),
                pronouns: "they/them".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
    }
    fn claude(&self) -> PathBuf {
        self.0.join(".claude")
    }
    fn member(&self) -> PathBuf {
        self.claude().join("crew/ari")
    }
    fn state(&self) -> PathBuf {
        self.claude().join("state/ari")
    }
    fn places(&self) -> Places {
        Places {
            home: self.0.clone(),
            claude_home: self.claude(),
        }
    }
    fn on(&self) {
        install::turn_on(&self.member(), &self.places()).unwrap();
    }
    fn kinds(&self) {
        fs::write(self.member().join("decisions.md"), DECISIONS).unwrap();
    }
    fn packed(&self) {
        self.kinds();
        // Set up through the core, so each API assertion is independently reachable in red.
        let out = private_folder(&self.0, "ari", "claude-ai").unwrap();
        persona_builder::pack::pack(&self.member(), &out).unwrap();
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn slug(s: &str) -> SlugRequest {
    SlugRequest { slug: s.into() }
}
fn off(s: &str) -> TurnOffRequest {
    TurnOffRequest { slug: s.into() }
}
fn forget_request(files: u32) -> ForgetRequest {
    ForgetRequest {
        slug: "ari".into(),
        files,
    }
}
fn snapshot(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    if !folder.exists() {
        return vec![];
    }
    let mut files = vec![];
    for entry in fs::read_dir(folder).unwrap() {
        let path = entry.unwrap().path();
        let name = PathBuf::from(path.file_name().unwrap());
        if fs::symlink_metadata(&path).unwrap().is_dir() {
            files.push((name.clone(), vec![]));
            files.extend(
                snapshot(&path)
                    .into_iter()
                    .map(|(child, bytes)| (name.join(child), bytes)),
            );
        } else {
            files.push((name, fs::read(path).unwrap()));
        }
    }
    files.sort();
    files
}
fn succeeds<T: std::fmt::Debug>(result: std::result::Result<T, Failure>) -> T {
    assert!(
        result.is_ok(),
        "expected a successful answer, got {result:?}"
    );
    result.unwrap()
}
fn status(h: &Home) -> ClaudeCodeStatus {
    succeeds(claude_code_status_in(&h.0, &h.claude(), slug("ari")))
}
fn copy_ari(h: &Home) -> PathBuf {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let path = entry.unwrap().path();
            let target = to.join(path.file_name().unwrap());
            if path.is_dir() {
                copy(&path, &target);
            } else {
                fs::copy(path, target).unwrap();
            }
        }
    }
    let dir = h.0.join("kit/crew/ari");
    copy(&h.member(), &dir);
    dir
}
#[test]
fn status_not_set_up_and_off() {
    let h = Home::new();
    assert_eq!(
        succeeds(claude_code_status_in(
            &h.0,
            &h.0.join("missing"),
            slug("ari")
        )),
        ClaudeCodeStatus {
            state: OnState::NotSetUp,
            on_name: None,
            on_slug: None,
            out_of_date: false,
            changed: false
        }
    );
    assert_eq!(status(&h).state, OnState::Off);
}
#[test]
fn status_this_out_of_date_and_changed() {
    let h = Home::new();
    h.on();
    assert_eq!(
        status(&h),
        ClaudeCodeStatus {
            state: OnState::This,
            on_name: Some("Ari".into()),
            on_slug: Some("ari".into()),
            out_of_date: false,
            changed: false
        }
    );
    let skill = h.member().join("SKILL.md");
    let text = fs::read_to_string(&skill).unwrap();
    fs::write(&skill, format!("{text}\nMore guidance.\n")).unwrap();
    assert!(status(&h).out_of_date);
    fs::write(h.claude().join("CLAUDE.md"), "").unwrap();
    assert!(status(&h).changed);
    fs::remove_file(h.claude().join("skills/ari/SKILL.md")).unwrap();
    assert!(status(&h).changed);
}
#[test]
fn status_other_assistant() {
    let h = Home::new();
    h.make("Mo Lee");
    install::turn_on(&h.claude().join("crew/mo-lee"), &h.places()).unwrap();
    assert_eq!(
        status(&h),
        ClaudeCodeStatus {
            state: OnState::Other,
            on_name: Some("Mo Lee".into()),
            on_slug: Some("mo-lee".into()),
            out_of_date: false,
            changed: false
        }
    );
}
#[test]
fn kit_copy_is_other_even_with_same_slug() {
    let h = Home::new();
    let copy = copy_ari(&h);
    install::turn_on(&copy, &h.places()).unwrap();
    assert_eq!(status(&h).state, OnState::Other);
    let before = snapshot(&h.claude());
    assert_eq!(
        turn_on_in(&h.0, &h.claude(), slug("ari"))
            .unwrap_err()
            .message,
        "Ari is on in Claude Code. Turn Ari off first, then turn Ari on."
    );
    assert_eq!(snapshot(&h.claude()), before);
}
#[test]
fn on_installs_and_refreshes() {
    let h = Home::new();
    assert_eq!(
        succeeds(turn_on_in(&h.0, &h.claude(), slug("ari"))).message,
        "Ari is on. Start a new Claude Code session to use Ari."
    );
    assert!(h.claude().join("skills/ari/SKILL.md").is_file());
    let skill = h.member().join("SKILL.md");
    let text = fs::read_to_string(&skill).unwrap();
    fs::write(&skill, format!("{text}\nMore guidance.\n")).unwrap();
    assert!(status(&h).out_of_date);
    succeeds(turn_on_in(&h.0, &h.claude(), slug("ari")));
    assert!(!status(&h).out_of_date);
    assert_eq!(
        fs::read(skill).unwrap(),
        fs::read(h.claude().join("skills/ari/SKILL.md")).unwrap()
    );
}
#[test]
fn on_refuses_other_without_changing_bytes() {
    let h = Home::new();
    h.make("Mo Lee");
    install::turn_on(&h.claude().join("crew/mo-lee"), &h.places()).unwrap();
    let before = snapshot(&h.claude());
    assert_eq!(
        turn_on_in(&h.0, &h.claude(), slug("ari"))
            .unwrap_err()
            .message,
        "Mo Lee is on in Claude Code. Turn Mo Lee off first, then turn Ari on."
    );
    assert_eq!(snapshot(&h.claude()), before);
}
#[test]
fn on_not_set_up_and_failure_are_safe() {
    let h = Home::new();
    assert_eq!(
        turn_on_in(&h.0, &h.0.join("missing"), slug("ari"))
            .unwrap_err()
            .message,
        "Claude Code isn't set up on this computer yet. Install it and sign in, then come back."
    );
    fs::create_dir_all(h.claude().join("skills/ari")).unwrap();
    let before = snapshot(&h.claude());
    assert_eq!(
        turn_on_in(&h.0, &h.claude(), slug("ari"))
            .unwrap_err()
            .message,
        "Persona Builder couldn't turn Ari on, so nothing in your Claude folder was changed."
    );
    assert_eq!(snapshot(&h.claude()), before);
}
#[test]
fn off_nothing_and_this() {
    let h = Home::new();
    assert_eq!(
        succeeds(turn_off_in(&h.0, &h.claude(), off("ari"))).message,
        "No assistant is on in Claude Code."
    );
    h.on();
    assert_eq!(
        succeeds(turn_off_in(&h.0, &h.claude(), off("ari"))).message,
        "Ari is off. Everything Persona Builder added is gone."
    );
    assert_eq!(status(&h).state, OnState::Off);
}
#[test]
fn off_changed_on_refuses_stale_slug() {
    let h = Home::new();
    h.on();
    let before = snapshot(&h.claude());
    for value in ["mo-lee", "../x", ""] {
        assert_eq!(
            turn_off_in(&h.0, &h.claude(), off(value))
                .unwrap_err()
                .message,
            CHANGED_ON
        );
        assert_eq!(snapshot(&h.claude()), before);
    }
    assert_eq!(status(&h).state, OnState::This);
}
#[test]
fn off_leaves_changed_files() {
    let h = Home::new();
    h.on();
    fs::write(h.claude().join("skills/ari/SKILL.md"), "changed").unwrap();
    assert_eq!(
        succeeds(turn_off_in(&h.0, &h.claude(), off("ari"))).message,
        "Ari is off. A few things in your Claude folder were changed after Ari was turned on, so Persona Builder left them where they are."
    );
    assert_eq!(
        fs::read_to_string(h.claude().join("skills/ari/SKILL.md")).unwrap(),
        "changed"
    );
}
#[test]
fn record_names_are_checked_before_shown() {
    for (name, shown) in [("Ari", Some("Ari")), ("Ari\n@~/.ssh/x", None), ("", None)] {
        let h = Home::new();
        h.on();
        let path = h.claude().join("persona-builder/installed.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        record["name"] = name.into();
        fs::write(path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert_eq!(status(&h).on_name.as_deref(), shown);
        if shown.is_none() {
            assert_eq!(
                succeeds(turn_off_in(&h.0, &h.claude(), off("ari"))).message,
                "The assistant is off. Everything Persona Builder added is gone."
            );
        }
    }
}
#[test]
fn pack_no_kinds_and_empty_listing() {
    let h = Home::new();
    assert_eq!(
        succeeds(claude_ai_files_in(&h.0, slug("ari"))).files,
        Vec::<String>::new()
    );
    assert_eq!(
        make_claude_ai_files_in(&h.0, slug("ari"))
            .unwrap_err()
            .message,
        NO_KINDS
    );
    assert!(!h.state().join("claude-ai").exists());
}
#[test]
fn pack_private_sorted_and_replaced_whole() {
    let h = Home::new();
    h.kinds();
    let files = succeeds(make_claude_ai_files_in(&h.0, slug("ari")));
    assert_eq!(
        files.files,
        [
            "instructions.md",
            "decisions-scope.md",
            "decisions-writing-voice.md"
        ]
    );
    assert_eq!(
        fs::metadata(h.state().join("claude-ai"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    for file in &files.files {
        assert_eq!(
            fs::metadata(h.state().join("claude-ai").join(file))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    assert_eq!(succeeds(claude_ai_files_in(&h.0, slug("ari"))), files);
    fs::write(
        h.member().join("decisions.md"),
        "## By kind of question\n### writing-voice\nUse everyday words.\n",
    )
    .unwrap();
    assert_eq!(
        succeeds(make_claude_ai_files_in(&h.0, slug("ari"))).files,
        ["instructions.md", "decisions-writing-voice.md"]
    );
    assert!(!h.state().join("claude-ai/decisions-scope.md").exists());
}
#[test]
fn pack_failure_keeps_old_files() {
    let h = Home::new();
    h.packed();
    let before = snapshot(&h.state().join("claude-ai"));
    fs::write(h.member().join("decisions.md"), "# No areas\n").unwrap();
    assert_eq!(
        make_claude_ai_files_in(&h.0, slug("ari"))
            .unwrap_err()
            .message,
        NO_KINDS
    );
    assert_eq!(snapshot(&h.state().join("claude-ai")), before);
}
#[test]
fn pack_clears_leftover_and_busy_refuses() {
    let h = Home::new();
    h.kinds();
    let new = private_folder(&h.0, "ari", "claude-ai-new").unwrap();
    fs::write(new.join("leftover"), "old").unwrap();
    assert!(make_claude_ai_files_in(&h.0, slug("ari")).is_ok());
    assert!(!new.exists());
    let _lock = lock_state(&h.0, "ari").unwrap();
    let before = snapshot(&h.state());
    assert_eq!(
        make_claude_ai_files_in(&h.0, slug("ari"))
            .unwrap_err()
            .message,
        BUSY
    );
    assert_eq!(snapshot(&h.state()), before);
}
#[test]
fn listing_never_names_planted_files_or_links() {
    let h = Home::new();
    h.packed();
    let out = h.state().join("claude-ai");
    for name in [
        "notes.txt",
        "secret.md",
        "decisions-.md",
        "decisions-UPPER.md",
        "decisions-space here.md",
    ] {
        fs::write(out.join(name), "private").unwrap();
    }
    fs::write(
        out.join(format!("decisions-{}.md", "a".repeat(65))),
        "private",
    )
    .unwrap();
    symlink(out.join("instructions.md"), out.join("decisions-x.md")).unwrap();
    fs::create_dir(out.join("decisions-dir.md")).unwrap();
    assert_eq!(
        succeeds(claude_ai_files_in(&h.0, slug("ari"))).files,
        [
            "instructions.md",
            "decisions-scope.md",
            "decisions-writing-voice.md"
        ]
    );
}
#[test]
fn show_requires_pack_canonicalizes_and_hides_opener_errors() {
    let h = Home::new();
    assert_eq!(
        show_claude_ai_files_in(&h.0, slug("ari"), |_| panic!("opened before make"))
            .unwrap_err()
            .message,
        "Make the claude.ai files first."
    );
    h.packed();
    let mut opened = false;
    show_claude_ai_files_in(&h.0, slug("ari"), |path| {
        assert_eq!(path, h.state().join("claude-ai").canonicalize().unwrap());
        opened = true;
        Ok(())
    })
    .unwrap();
    assert!(opened);
    assert_eq!(
        show_claude_ai_files_in(&h.0, slug("ari"), |_| Err(std::io::Error::other(
            "private path"
        )))
        .unwrap_err()
        .message,
        "Persona Builder couldn't open the folder."
    );
}
#[test]
fn forget_removes_all_state_preserves_assistant_and_resets_choices() {
    let h = Home::new();
    agree_consent_in(&h.0, slug("ari")).unwrap();
    save_choose_in(
        &h.0,
        SaveChoiceRequest {
            slug: "ari".into(),
            choice: 1,
            kind: SaveKind::Local,
            name: None,
        },
        &Git::system(),
        None,
        &MemoryStore::new(),
    )
    .unwrap();
    h.packed();
    fs::write(h.state().join("history.jsonl"), "invented").unwrap();
    let before = snapshot(&h.member());
    assert_eq!(succeeds(forget_preview_in(&h.0, slug("ari"))).files, 6);
    assert_eq!(
        succeeds(forget_in(&h.0, forget_request(6))).message,
        "Deleted 6 files. Ari's own folder is untouched."
    );
    assert!(!h.state().exists());
    assert_eq!(snapshot(&h.member()), before);
    assert!(!list_assistants_in(&h.0).unwrap()[0].consent.agreed);
    assert_eq!(
        save_options_in(&h.0, slug("ari"), &Git::system(), &MemoryStore::new())
            .unwrap()
            .chosen,
        None
    );
}
#[test]
fn forget_changed_count_keeps_everything() {
    let h = Home::new();
    h.packed();
    let before = snapshot(&h.state());
    assert_eq!(
        forget_in(&h.0, forget_request(2)).unwrap_err().message,
        FORGET_CHANGED
    );
    assert_eq!(snapshot(&h.state()), before);
}
#[test]
fn forget_busy_keeps_everything() {
    let h = Home::new();
    h.packed();
    let _lock = lock_state(&h.0, "ari").unwrap();
    let before = snapshot(&h.state());
    assert_eq!(
        forget_in(&h.0, forget_request(3)).unwrap_err().message,
        BUSY
    );
    assert_eq!(snapshot(&h.state()), before);
}
#[test]
fn forget_missing_creates_nothing_and_singular_message() {
    let h = Home::new();
    assert_eq!(succeeds(forget_preview_in(&h.0, slug("ari"))).files, 0);
    assert_eq!(
        succeeds(forget_in(&h.0, forget_request(0))).message,
        "There's nothing to forget: Persona Builder hasn't kept anything for Ari yet."
    );
    assert!(!h.state().exists());
    agree_consent_in(&h.0, slug("ari")).unwrap();
    assert_eq!(
        succeeds(forget_in(&h.0, forget_request(1))).message,
        "Deleted 1 file. Ari's own folder is untouched."
    );
}
#[test]
fn all_member_commands_require_listed_slug() {
    let h = Home::new();
    fs::create_dir_all(h.claude().join("crew/absent")).unwrap();
    for value in ["absent", "../x", ""] {
        assert_eq!(
            claude_code_status_in(&h.0, &h.claude(), slug(value))
                .unwrap_err()
                .message,
            FIND
        );
        assert_eq!(
            turn_on_in(&h.0, &h.claude(), slug(value))
                .unwrap_err()
                .message,
            FIND
        );
        assert_eq!(
            claude_ai_files_in(&h.0, slug(value)).unwrap_err().message,
            FIND
        );
        assert_eq!(
            make_claude_ai_files_in(&h.0, slug(value))
                .unwrap_err()
                .message,
            FIND
        );
        assert_eq!(
            show_claude_ai_files_in(&h.0, slug(value), |_| panic!("opened invalid"))
                .unwrap_err()
                .message,
            FIND
        );
        assert_eq!(
            forget_preview_in(&h.0, slug(value)).unwrap_err().message,
            FIND
        );
        assert_eq!(
            forget_in(
                &h.0,
                ForgetRequest {
                    slug: value.into(),
                    files: 0
                }
            )
            .unwrap_err()
            .message,
            FIND
        );
    }
}
#[test]
fn dto_contract() {
    use serde_json::{from_value, json, to_value};
    assert_eq!(
        to_value(ClaudeCodeStatus {
            state: OnState::This,
            on_name: Some("Ari".into()),
            on_slug: Some("ari".into()),
            out_of_date: true,
            changed: true
        })
        .unwrap(),
        json!({"state":"this","onName":"Ari","onSlug":"ari","outOfDate":true,"changed":true})
    );
    for (state, text) in [
        (OnState::NotSetUp, "notSetUp"),
        (OnState::Off, "off"),
        (OnState::This, "this"),
        (OnState::Other, "other"),
    ] {
        assert_eq!(to_value(state).unwrap(), json!(text));
    }
    assert_eq!(
        to_value(forget_request(3)).unwrap(),
        json!({"slug":"ari","files":3})
    );
    assert_eq!(to_value(off("ari")).unwrap(), json!({"slug":"ari"}));
    assert_eq!(
        to_value(ClaudeAiFiles {
            files: vec!["instructions.md".into()]
        })
        .unwrap(),
        json!({"files":["instructions.md"]})
    );
    assert_eq!(
        to_value(ForgetPreview { files: 3 }).unwrap(),
        json!({"files":3})
    );
    assert!(from_value::<TurnOffRequest>(json!({"slug":"ari","extra":1})).is_err());
    assert!(from_value::<ForgetRequest>(json!({"slug":"ari","files":3,"extra":1})).is_err());
}
