use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};
use voice_api::{dto::*, *};
use voice_ingest::{
    anonymize::Rules,
    private,
    skills::{self, Agreements},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new() -> Self {
        let h = Self(std::env::temp_dir().join(format!(
            "voice-api-skills-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&h.0).unwrap();
        new_assistant_in(
            &h.0,
            NewAssistantRequest {
                name: "Pebble".into(),
                pronouns: "they/them".into(),
                principal: "Rowan Vale".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        h
    }
    fn member(&self) -> voice_core::member::Member {
        voice_core::member::read_member(&self.0.join(".claude/crew/pebble/member.json")).unwrap()
    }
    fn state(&self) -> PathBuf {
        self.0.join(".claude/state/pebble")
    }
    fn rules(&self, value: serde_json::Value) {
        fs::write(
            self.0.join(".claude/crew/pebble/private.json"),
            serde_json::json!({"anonymizer":value}).to_string(),
        )
        .unwrap();
    }
    fn skill(&self, name: &str, text: &str) {
        let p = self.0.join(".claude/skills").join(name);
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("SKILL.md"), text).unwrap();
    }
    fn agree(&self, ids: &[&str]) {
        let _lock = private::lock_state(&self.0, "pebble").unwrap();
        let rules = Rules::for_member(&self.member(), &self.0).unwrap();
        let found = skills::discover(&self.0, &rules).unwrap();
        let mut a = Agreements::load(&self.0, "pebble").unwrap();
        for id in ids {
            a.agree(
                found.skills.iter().find(|s| &s.id == id).unwrap(),
                "2026-01-02",
            );
        }
        a.save(&self.0, "pebble").unwrap();
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn slug() -> SlugRequest {
    SlugRequest {
        slug: "pebble".into(),
    }
}
fn request(ids: &[&str]) -> SkillsRequest {
    SkillsRequest {
        slug: "pebble".into(),
        ids: ids.iter().map(|s| (*s).into()).collect(),
    }
}
const READ: &str =
    "Voice couldn't look through the skills on this computer. Close it and open it again.";
const SAVE: &str =
    "Voice couldn't save your choice. Check there's space on this computer, then try again.";
const GONE: &str = "That skill isn't on this computer any more. Look again.";
const NONE: &str = "Choose a skill first.";
const STALE: &str = "Voice couldn't bring your assistant's copy of your skills up to date, so it marked the copy as out of date and left its files alone. To see what needs fixing, run voice skills refresh in a terminal.";
const REFRESH: &str = "Voice couldn't bring your assistant's copy of your skills up to date, and it may be out of date. To see why, run voice skills refresh in a terminal.";
const BUSY: &str =
    "Voice is already working on this assistant. Wait for it to finish, then try again.";
fn safe(h: &Home, f: Failure, expected: &str) {
    assert_eq!(f.message, expected);
    for private in [h.0.to_str().unwrap(), "pebble", "Pebble Freight"] {
        assert!(!f.message.contains(private));
    }
}
#[test]
fn list_orders_waiting_first_with_cli_numbers_and_dates_without_history_permission() {
    let h = Home::new();
    assert!(!h.state().exists());
    assert!(list_skills_in(&h.0, slug()).unwrap().skills.is_empty());
    assert!(h.state().is_dir());
    h.skill("notes", "Use short notes.");
    h.skill("writing", "Write clearly.");
    h.skill("archive", "Keep records.");
    h.rules(serde_json::json!({"never_copy":["archive"]}));
    h.agree(&["claude:writing"]);
    let got = list_skills_in(&h.0, slug()).unwrap();
    assert_eq!(
        got,
        Skills {
            skills: vec![
                SkillRow {
                    id: "claude:notes".into(),
                    name: "notes".into(),
                    place: "~/.claude/skills/notes".into(),
                    state: SkillState::Waiting,
                    agreed_on: None,
                    number: 2
                },
                SkillRow {
                    id: "claude:archive".into(),
                    name: "archive".into(),
                    place: "~/.claude/skills/archive".into(),
                    state: SkillState::NeverCopied,
                    agreed_on: None,
                    number: 1
                },
                SkillRow {
                    id: "claude:writing".into(),
                    name: "writing".into(),
                    place: "~/.claude/skills/writing".into(),
                    state: SkillState::Agreed,
                    agreed_on: Some("2026-01-02".into()),
                    number: 3
                },
            ],
            notes: vec![]
        }
    );
}
#[test]
fn agreement_validates_all_ids_before_save_and_uses_today() {
    let h = Home::new();
    h.skill("notes", "Use short notes.");
    h.agree(&["claude:notes"]);
    let file = h.state().join("anonymizer/skills.json");
    let before = fs::read(&file).unwrap();
    safe(
        &h,
        agree_skills_in(&h.0, request(&["claude:notes", "claude:missing"])).unwrap_err(),
        GONE,
    );
    assert_eq!(fs::read(&file).unwrap(), before);
    safe(&h, agree_skills_in(&h.0, request(&[])).unwrap_err(), NONE);
    safe(
        &h,
        withdraw_skills_in(&h.0, request(&[])).unwrap_err(),
        NONE,
    );
    h.skill("writing", "Write clearly.");
    let got = agree_skills_in(&h.0, request(&["claude:writing"])).unwrap();
    let row = got
        .skills
        .iter()
        .find(|s| s.id == "claude:writing")
        .unwrap();
    assert_eq!(row.state, SkillState::Agreed);
    assert_eq!(
        row.agreed_on,
        Some(voice_agent::ledger::local_day(SystemTime::now()))
    );
}
#[test]
fn withdrawal_removes_only_after_a_record_and_preserves_protected_links() {
    use std::os::unix::fs::symlink;
    let h = Home::new();
    h.skill("notes", "Use short notes.");
    h.agree(&["claude:notes"]);
    let date = voice_agent::ledger::local_day(SystemTime::now());
    let mirror = skills::refresh(&h.0, &h.member(), &date).unwrap().mirror;
    let got = withdraw_skills_in(&h.0, request(&["claude:notes"])).unwrap();
    assert_eq!(got.mirror, MirrorAfter::Removed);
    assert!(!mirror.exists());
    assert_eq!(got.skills.skills[0].state, SkillState::Waiting);
    h.agree(&["claude:notes"]);
    assert_eq!(
        withdraw_skills_in(&h.0, request(&["claude:notes"]))
            .unwrap()
            .mirror,
        MirrorAfter::NotThere
    );
    h.agree(&["claude:notes"]);
    skills::refresh(&h.0, &h.member(), &date).unwrap();
    assert_eq!(
        withdraw_skills_in(&h.0, request(&["claude:missing"]))
            .unwrap()
            .mirror,
        MirrorAfter::NotThere
    );
    assert!(mirror.join(".manifest.json").is_file());
    fs::create_dir(mirror.join("release.key")).unwrap();
    fs::write(mirror.join("release.key/SKILL.md"), "Keep this text.").unwrap();
    symlink(
        mirror.join("release.key"),
        h.0.join(".claude/skills/release.key"),
    )
    .unwrap();
    assert_eq!(
        withdraw_skills_in(&h.0, request(&["claude:notes"]))
            .unwrap()
            .mirror,
        MirrorAfter::NotRemoved
    );
    assert_eq!(
        fs::read_to_string(mirror.join("release.key/SKILL.md")).unwrap(),
        "Keep this text."
    );
    assert!(!mirror.join(".manifest.json").exists());
}
#[test]
fn refresh_returns_the_ingest_cli_report_counts_and_partial_ok() {
    let h = Home::new();
    h.skill("notes", "Use short notes.");
    h.skill("writing", "Write clearly.");
    h.agree(&["claude:notes", "claude:writing"]);
    let date = voice_agent::ledger::local_day(SystemTime::now());
    let expected = skills::refresh(&h.0, &h.member(), &date).unwrap();
    fs::remove_dir_all(&expected.mirror).unwrap();
    let got = refresh_skills_in(&h.0, slug()).unwrap();
    assert_eq!(
        got,
        SkillsRefreshed {
            lines: expected
                .copied
                .lines
                .split('\n')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            copied: 2,
            agreed: 2,
            waiting: 0,
            new: 2,
            updated: 0,
            unchanged: 0,
            removed: 0
        }
    );
    let second = refresh_skills_in(&h.0, slug()).unwrap();
    assert_eq!(
        (second.new, second.updated, second.unchanged, second.removed),
        (0, 0, 2, 0)
    );
    h.rules(serde_json::json!({"terms":["writing"]}));
    let partial = refresh_skills_in(&h.0, slug()).unwrap();
    assert_eq!((partial.copied, partial.agreed), (1, 2));
    assert!(partial.lines.iter().any(|s|s=="A skill you agreed to: not copied, its name holds a private detail (number 2 in voice skills list)"));
}
#[test]
fn refresh_failure_outcomes_are_fixed_sentences_and_busy_is_shared() {
    let h = Home::new();
    h.skill("notes", "Use short notes.");
    h.agree(&["claude:notes"]);
    skills::refresh(&h.0, &h.member(), "2026-01-02").unwrap();
    h.rules(serde_json::json!({"unknown":"Pebble Freight"}));
    safe(&h, refresh_skills_in(&h.0, slug()).unwrap_err(), STALE);
    h.rules(serde_json::json!({"projects":["~/.claude/state/pebble/mirror/proj"]}));
    let path = h.state().join("mirror/proj");
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("work.txt"), "Keep this text.").unwrap();
    safe(&h, refresh_skills_in(&h.0, slug()).unwrap_err(), REFRESH);
    assert_eq!(
        fs::read_to_string(path.join("work.txt")).unwrap(),
        "Keep this text."
    );
    let _lock = private::lock_state(&h.0, "pebble").unwrap();
    safe(&h, refresh_skills_in(&h.0, slug()).unwrap_err(), BUSY);
}
struct Mode(PathBuf, fs::Permissions);
impl Mode {
    fn set(path: PathBuf, mode: u32) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let old = fs::metadata(&path).unwrap().permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        Self(path, old)
    }
}
impl Drop for Mode {
    fn drop(&mut self) {
        fs::set_permissions(&self.0, self.1.clone()).unwrap();
    }
}
#[test]
fn failures_never_expose_paths_names_or_private_details() {
    use std::os::unix::fs::MetadataExt;
    let h = Home::new();
    h.skill("notes", "Use short notes.");
    h.agree(&["claude:notes"]);
    for call in [
        list_skills_in(
            &h.0,
            SlugRequest {
                slug: "missing".into(),
            },
        )
        .map(|_| ()),
        agree_skills_in(
            &h.0,
            SkillsRequest {
                slug: "missing".into(),
                ids: vec!["claude:notes".into()],
            },
        )
        .map(|_| ()),
        withdraw_skills_in(
            &h.0,
            SkillsRequest {
                slug: "missing".into(),
                ids: vec!["claude:notes".into()],
            },
        )
        .map(|_| ()),
        refresh_skills_in(
            &h.0,
            SlugRequest {
                slug: "missing".into(),
            },
        )
        .map(|_| ()),
    ] {
        safe(
            &h,
            call.unwrap_err(),
            "Voice couldn't find that assistant. Close it and open it again.",
        );
    }
    h.rules(serde_json::json!({"unknown":"Pebble Freight"}));
    safe(&h, list_skills_in(&h.0, slug()).unwrap_err(), READ);
    safe(
        &h,
        agree_skills_in(&h.0, request(&["claude:notes"])).unwrap_err(),
        READ,
    );
    safe(
        &h,
        withdraw_skills_in(&h.0, request(&["claude:notes"])).unwrap_err(),
        READ,
    );
    h.rules(serde_json::json!({}));
    {
        let _lock = private::lock_state(&h.0, "pebble").unwrap();
        safe(&h, list_skills_in(&h.0, slug()).unwrap_err(), BUSY);
        safe(
            &h,
            agree_skills_in(&h.0, request(&["claude:notes"])).unwrap_err(),
            BUSY,
        );
        safe(
            &h,
            withdraw_skills_in(&h.0, request(&["claude:notes"])).unwrap_err(),
            BUSY,
        );
    }
    if fs::metadata(&h.0).unwrap().uid() == 0 {
        return;
    }
    {
        let _mode = Mode::set(h.state().join("anonymizer"), 0o500);
        safe(
            &h,
            agree_skills_in(&h.0, request(&["claude:notes"])).unwrap_err(),
            SAVE,
        );
        safe(
            &h,
            withdraw_skills_in(&h.0, request(&["claude:notes"])).unwrap_err(),
            SAVE,
        );
    }
    let folder = h.0.join("Pebble Freight/.claude/skills");
    fs::create_dir_all(&folder).unwrap();
    h.rules(serde_json::json!({"projects":["~/Pebble Freight"]}));
    let _mode = Mode::set(folder, 0);
    assert_eq!(
        list_skills_in(&h.0, slug()).unwrap().notes,
        vec![skills::UNREADABLE_NOTE]
    );
    assert_eq!(
        agree_skills_in(&h.0, request(&["claude:notes"]))
            .unwrap()
            .notes,
        vec![skills::UNREADABLE_NOTE]
    );
    assert_eq!(
        withdraw_skills_in(&h.0, request(&["claude:notes"]))
            .unwrap()
            .skills
            .notes,
        vec![skills::UNREADABLE_NOTE]
    );
    assert!(refresh_skills_in(&h.0, slug()).is_ok());
}
#[test]
fn refresh_removed_outcome_is_fixed_and_dto_contract_is_strict() {
    use serde_json::{from_value, json, to_value};
    use std::os::unix::fs::MetadataExt;
    assert!(
        from_value::<SkillsRequest>(json!({"slug":"pebble","ids":[],"path":"private"})).is_err()
    );
    assert_eq!(
        to_value(request(&["claude:notes"])).unwrap(),
        json!({"slug":"pebble","ids":["claude:notes"]})
    );
    for (state, word) in [
        (SkillState::Waiting, "waiting"),
        (SkillState::Agreed, "agreed"),
        (SkillState::Moved, "moved"),
        (SkillState::NotOffered, "notOffered"),
        (SkillState::NeverCopied, "neverCopied"),
    ] {
        assert_eq!(to_value(state).unwrap(), json!(word));
    }
    for (state, word) in [
        (MirrorAfter::NotThere, "notThere"),
        (MirrorAfter::Removed, "removed"),
        (MirrorAfter::NotRemoved, "notRemoved"),
    ] {
        assert_eq!(to_value(state).unwrap(), json!(word));
    }
    let h = Home::new();
    if fs::metadata(&h.0).unwrap().uid() == 0 {
        return;
    }
    h.skill("notes", "Use short notes.");
    h.agree(&["claude:notes"]);
    skills::refresh(&h.0, &h.member(), "2026-01-02").unwrap();
    let _mode = Mode::set(h.state().join("anonymizer"), 0o500);
    safe(
        &h,
        refresh_skills_in(&h.0, slug()).unwrap_err(),
        "Voice couldn't bring your assistant's copy of your skills up to date, so it removed the copy. To see what needs fixing, run voice skills refresh in a terminal.",
    );
    assert!(!h.state().join("mirror").exists());
}
