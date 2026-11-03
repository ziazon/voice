use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use voice_agent::grants::{self, Capability, Event, Reason, Window};
use voice_ingest::private::{lock_state, write_private};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const CAP: Capability = Capability::CalendarRead;
const SAVED: &[u8] = br#"{"version":1,"grants":[{"capability":"calendar.read","given_on":"2026-11-02","past_days":0,"next_days":14}]}"#;
const GIVEN: &str = "{\"at\":\"2026-11-02T12:00:00Z\",\"event\":\"grant\",\"capability\":\"calendar.read\",\"past_days\":0,\"next_days\":14}\n";
const REVOKED: &str =
    "{\"at\":\"2026-11-02T12:00:00Z\",\"event\":\"revoke\",\"capability\":\"calendar.read\"}\n";
const READ: &str = "{\"at\":\"2026-11-02T12:00:00Z\",\"event\":\"read\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"items\":12}\n";
const REFUSED: &str = "{\"at\":\"2026-11-02T12:00:00Z\",\"event\":\"refused\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"reason\":\"no-grant\"}\n";
fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_620_800)
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "grants-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        drop(lock_state(&p, "ari").unwrap());
        Self(p)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(".claude/state/ari").join(name)
    }
    fn give(&self) {
        grants::give(&self.0, "ari", CAP, Window::DEFAULT, now()).unwrap();
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        write_private(&self.0, "ari", name, bytes).unwrap();
    }
    fn damaged(&self) -> String {
        format!(
            "{} is damaged, so Voice reads nothing it needs permission for. Delete it and give permission again.",
            self.path("grants.json").display()
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn give_load_and_formats() {
    let f = Fixture::new();
    let _lock = lock_state(&f.0, "ari").unwrap();
    f.give();
    let g = grants::load(&f.0, "ari").unwrap();
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].given_on, "2026-11-02");
    assert_eq!(g[0].window, Window::DEFAULT);
    assert_eq!(g[0].capability, CAP);
    assert_eq!(fs::read(f.path("grants.json")).unwrap(), SAVED);
    assert_eq!(
        fs::metadata(f.path("grants.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(fs::read_to_string(f.path("audit.jsonl")).unwrap(), GIVEN);
    assert_eq!(Capability::ALL, [CAP]);
    assert_eq!(CAP.id(), "calendar.read");
    assert_eq!(CAP.words(), "read your calendar");
    assert_eq!(CAP.source(), voice_core::ceilings::Source::Calendar);
    for bad in ["Calendar.read", "calendar.write", "calendar.read ", ""] {
        assert_eq!(Capability::from_id(bad), None);
    }
    assert_eq!(Capability::from_id("calendar.read"), Some(CAP));
}
#[test]
fn window_ranges() {
    for (p, n) in [(0, 1), (31, 92)] {
        let w = Window::new(p, n).unwrap();
        assert_eq!((w.past_days(), w.next_days()), (p, n));
    }
    for (p, n) in [(32, 14), (0, 0), (0, 93)] {
        assert_eq!(
            Window::new(p, n).unwrap_err().to_string(),
            "The calendar window goes from 0 to 31 days back and from 1 to 92 days ahead."
        );
    }
}
#[test]
fn replace_and_revoke() {
    let f = Fixture::new();
    let _lock = lock_state(&f.0, "ari").unwrap();
    f.give();
    let w = Window::new(3, 30).unwrap();
    grants::give(&f.0, "ari", CAP, w, now()).unwrap();
    let loaded = grants::load(&f.0, "ari").unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].window, w);
    assert!(grants::revoke(&f.0, "ari", CAP, now()).unwrap());
    assert!(!f.path("grants.json").exists());
    let before = fs::read(f.path("audit.jsonl")).unwrap();
    assert!(before.ends_with(REVOKED.as_bytes()));
    assert!(!grants::revoke(&f.0, "ari", CAP, now()).unwrap());
    assert_eq!(fs::read(f.path("audit.jsonl")).unwrap(), before);
}
#[test]
fn damaged_grants_never_become_missing() {
    let valid = String::from_utf8(SAVED.to_vec()).unwrap();
    let bads=[valid.replace("\"capability\":\"calendar.read\"", "\"capability\":{\"calendar.read\":null}"), valid.replace("\"version\":1","\"version\":0"), valid.replace("\"version\":1","\"version\":2"), valid.replace("\"version\":1","\"extra\":0,\"version\":1"), valid.replace("\"past_days\":0","\"extra\":0,\"past_days\":0"), valid.replace("calendar.read","calendar.write"), valid.replace("}]}","},{\"capability\":\"calendar.read\",\"given_on\":\"2026-11-02\",\"past_days\":0,\"next_days\":14}]}"), valid.replace("2026-11-02","2026-13-02"), valid.replace("2026-11-02","2026-11-00"), valid.replace("2026-11-02","2026/11/02"), valid.replace("\"next_days\":14","\"next_days\":93"), "not JSON".into()];
    for bad in bads {
        let f = Fixture::new();
        let _lock = lock_state(&f.0, "ari").unwrap();
        f.write("grants.json", bad.as_bytes());
        assert_eq!(
            grants::load(&f.0, "ari").unwrap_err().to_string(),
            f.damaged()
        );
        assert_eq!(
            grants::give(&f.0, "ari", CAP, Window::DEFAULT, now())
                .unwrap_err()
                .to_string(),
            f.damaged()
        );
        assert_eq!(fs::read(f.path("grants.json")).unwrap(), bad.as_bytes());
        assert_eq!(
            grants::require(&f.0, "ari", CAP, "cli", now())
                .unwrap_err()
                .to_string(),
            f.damaged()
        );
        assert_eq!(
            fs::read_to_string(f.path("audit.jsonl")).unwrap(),
            REFUSED.replace("no-grant", "damaged")
        );
    }
    let f = Fixture::new();
    f.write("grants.json", SAVED);
    fs::set_permissions(f.path("grants.json"), fs::Permissions::from_mode(0o640)).unwrap();
    assert_eq!(
        grants::load(&f.0, "ari").unwrap_err().to_string(),
        f.damaged()
    );
    assert_eq!(
        grants::give(&f.0, "ari", CAP, Window::DEFAULT, now())
            .unwrap_err()
            .to_string(),
        f.damaged()
    );
    assert_eq!(fs::read(f.path("grants.json")).unwrap(), SAVED);
    assert_eq!(
        grants::require(&f.0, "ari", CAP, "cli", now())
            .unwrap_err()
            .to_string(),
        f.damaged()
    );
    let f = Fixture::new();
    f.write("target.json", SAVED);
    symlink(f.path("target.json"), f.path("grants.json")).unwrap();
    assert_eq!(
        grants::load(&f.0, "ari").unwrap_err().to_string(),
        f.damaged()
    );
    assert_eq!(
        grants::give(&f.0, "ari", CAP, Window::DEFAULT, now())
            .unwrap_err()
            .to_string(),
        f.damaged()
    );
    assert!(
        fs::symlink_metadata(f.path("grants.json"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        grants::require(&f.0, "ari", CAP, "cli", now())
            .unwrap_err()
            .to_string(),
        f.damaged()
    );
    assert_eq!(fs::read(f.path("target.json")).unwrap(), SAVED);
}
#[test]
fn require_logs_refusals_only_and_record_read_logs_counts() {
    let f = Fixture::new();
    assert_eq!(
        grants::require(&f.0, "ari", CAP, "cli", now())
            .unwrap_err()
            .to_string(),
        "Voice doesn't have your permission to read your calendar for this assistant."
    );
    assert_eq!(fs::read_to_string(f.path("audit.jsonl")).unwrap(), REFUSED);
    f.give();
    let before = fs::read(f.path("audit.jsonl")).unwrap();
    assert_eq!(
        grants::require(&f.0, "ari", CAP, "cli", now())
            .unwrap()
            .capability,
        CAP
    );
    assert_eq!(fs::read(f.path("audit.jsonl")).unwrap(), before);
    grants::record_read(&f.0, "ari", CAP, "cli", 12, now()).unwrap();
    assert_eq!(
        fs::read_to_string(f.path("audit.jsonl")).unwrap(),
        format!("{REFUSED}{GIVEN}{READ}")
    );
}
#[test]
fn require_propagates_failed_refusal_audit() {
    for damaged in [false, true] {
        let f = Fixture::new();
        if damaged {
            f.write("grants.json", b"not JSON");
        }
        fs::create_dir(f.path("audit.jsonl")).unwrap();
        let error = grants::require(&f.0, "ari", CAP, "cli", now())
            .unwrap_err()
            .to_string();
        if damaged {
            assert_ne!(error, f.damaged());
            assert_eq!(fs::read(f.path("grants.json")).unwrap(), b"not JSON");
        } else {
            assert_ne!(
                error,
                "Voice doesn't have your permission to read your calendar for this assistant."
            );
            assert!(!f.path("grants.json").exists());
        }
    }
}
#[test]
fn rollback_on_failed_audit() {
    for existing in [false, true] {
        let f = Fixture::new();
        let _lock = lock_state(&f.0, "ari").unwrap();
        if existing {
            f.give();
            fs::remove_file(f.path("audit.jsonl")).unwrap();
        }
        fs::create_dir(f.path("audit.jsonl")).unwrap();
        assert!(grants::give(&f.0, "ari", CAP, Window::new(1, 2).unwrap(), now()).is_err());
        if existing {
            assert_eq!(fs::read(f.path("grants.json")).unwrap(), SAVED);
            assert!(grants::revoke(&f.0, "ari", CAP, now()).is_err());
            assert_eq!(fs::read(f.path("grants.json")).unwrap(), SAVED);
        } else {
            assert!(!f.path("grants.json").exists());
        }
        assert!(grants::require(&f.0, "absent", CAP, "cli", now()).is_err());
        assert!(!f.0.join(".claude/state/absent").exists());
    }
}
#[test]
fn invalid_steps_write_nothing() {
    let f = Fixture::new();
    for step in [
        "".into(),
        "a".repeat(33),
        "Cli".into(),
        "1x".into(),
        "a b".into(),
        "a/b".into(),
    ] {
        assert!(grants::require(&f.0, "ari", CAP, &step, now()).is_err());
        assert!(grants::record_read(&f.0, "ari", CAP, &step, 1, now()).is_err());
        assert!(!f.path("audit.jsonl").exists());
    }
    for step in ["a", "cli-2", &"a".repeat(32)] {
        grants::record_read(&f.0, "ari", CAP, step, 0, now()).unwrap();
    }
}
#[test]
fn audit_typed_events_bounds_and_bad_lines() {
    let f = Fixture::new();
    f.write(
        "audit.jsonl",
        format!("{GIVEN}{REVOKED}{READ}{REFUSED}").as_bytes(),
    );
    let a = grants::audit(&f.0, "ari", 50).unwrap();
    assert_eq!(a.unreadable, 0);
    assert!(!a.older);
    assert_eq!(a.entries.len(), 4);
    assert_eq!(a.entries[0].at, "2026-11-02T12:00:00Z");
    assert_eq!(
        a.entries[0].event,
        Event::Refused {
            capability: CAP,
            step: "cli".into(),
            reason: Reason::NoGrant
        }
    );
    assert_eq!(
        a.entries[1].event,
        Event::Read {
            capability: CAP,
            step: "cli".into(),
            items: 12
        }
    );
    assert_eq!(a.entries[2].event, Event::Revoke { capability: CAP });
    assert_eq!(
        a.entries[3].event,
        Event::Grant {
            capability: CAP,
            window: Window::DEFAULT
        }
    );
    let short = grants::audit(&f.0, "ari", 2).unwrap();
    assert_eq!(short.entries, a.entries[..2]);
    assert!(short.older);
    for last in [0, 1001] {
        assert_eq!(
            grants::audit(&f.0, "ari", last).unwrap_err().to_string(),
            "Show between 1 and 1,000 entries."
        );
    }
    let bads = [
        GIVEN.replace("\"event\":\"grant\"", "\"extra\":0,\"event\":\"grant\""),
        GIVEN.replace("grant", "write"),
        GIVEN.replace("calendar.read", "calendar.write"),
        GIVEN.replace(
            "\"capability\":\"calendar.read\"",
            "\"capability\":{\"calendar.read\":null}",
        ),
        REFUSED.replace("\"reason\":\"no-grant\"", "\"reason\":{\"no-grant\":null}"),
        GIVEN.replace("\"next_days\":14", "\"next_days\":0"),
        READ.replace("cli", "1x"),
        READ.replace("\"items\":12", "\"items\":-1"),
        REFUSED.replace("no-grant", "other"),
        GIVEN.replace("T12:00:00Z", " 12:00:00Z"),
        GIVEN.replace("T12:00:00Z", "T25:00:00Z"),
        "not json\n".into(),
        "\n".into(),
    ];
    f.write(
        "audit.jsonl",
        format!("{GIVEN}{}", bads.join("")).as_bytes(),
    );
    let a = grants::audit(&f.0, "ari", 50).unwrap();
    assert_eq!(a.entries.len(), 1);
    assert_eq!(a.unreadable, bads.len());
    f.write(
        "audit.jsonl",
        REFUSED.replace("no-grant", "damaged").as_bytes(),
    );
    assert_eq!(
        grants::audit(&f.0, "ari", 1).unwrap().entries[0].event,
        Event::Refused {
            capability: CAP,
            step: "cli".into(),
            reason: Reason::Damaged
        }
    );
}
#[test]
fn audit_tail_and_private_file_rules() {
    let f = Fixture::new();
    let mut bytes = GIVEN.repeat(1024 * 1024 / GIVEN.len() + 2);
    bytes.push_str(READ);
    f.write("audit.jsonl", bytes.as_bytes());
    let a = grants::audit(&f.0, "ari", 1000).unwrap();
    assert!(a.older);
    assert_eq!(a.unreadable, 0);
    assert_eq!(
        a.entries[0].event,
        Event::Read {
            capability: CAP,
            step: "cli".into(),
            items: 12
        }
    );
    fs::set_permissions(f.path("audit.jsonl"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(grants::audit(&f.0, "ari", 50).is_err());
    fs::remove_file(f.path("audit.jsonl")).unwrap();
    f.write("target.jsonl", GIVEN.as_bytes());
    symlink(f.path("target.jsonl"), f.path("audit.jsonl")).unwrap();
    assert!(grants::audit(&f.0, "ari", 50).is_err());
}
#[test]
fn forget_removes_grants_and_audit() {
    let f = Fixture::new();
    let member = f.0.join("ari");
    voice_core::crew::new_member_embedded(
        &member,
        &voice_core::crew::NewMember {
            decisions: voice_core::crew::Decisions::Python,
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/assistants",
            date: "2026-11-02",
        },
    )
    .unwrap();
    f.give();
    voice_core::forget::forget(&member, &f.0, true).unwrap();
    assert!(grants::load(&f.0, "ari").unwrap().is_empty());
    let a = grants::audit(&f.0, "ari", 50).unwrap();
    assert!(a.entries.is_empty());
    assert_eq!(a.unreadable, 0);
    assert!(!a.older);
    assert!(!f.path("grants.json").parent().unwrap().exists());
}
