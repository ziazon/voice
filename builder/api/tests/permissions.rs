use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};
use voice_agent::{
    grants::{self, Capability, Event, Window},
    ledger, text,
};
use voice_api::{
    dto::*,
    permissions::{AUDIT_READ, DAMAGED, PERMISSION_SAVE, UNKNOWN},
    *,
};
use voice_ingest::private;
const BUSY: &str =
    "Voice is already working on this assistant. Wait for it to finish, then try again.";
const FIND: &str = "Voice couldn't find that assistant. Close it and open it again.";
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new(state: bool) -> Self {
        let h = Self(std::env::temp_dir().join(format!(
            "voice-api-permissions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&h.0).unwrap();
        new_assistant_in(
            &h.0,
            &h.0.join(".claude"),
            NewAssistantRequest {
                name: "Pebble".into(),
                pronouns: "they/them".into(),
                principal: "Rowan Vale".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        if state {
            drop(private::lock_state(&h.0, "pebble").unwrap());
        }
        h
    }
    fn state(&self) -> PathBuf {
        self.0.join(".claude/state/pebble")
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        let path = self.state().join(name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
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
fn give(past_days: u16, next_days: u16) -> GivePermissionRequest {
    GivePermissionRequest {
        slug: "pebble".into(),
        id: "calendar.read".into(),
        past_days,
        next_days,
    }
}
fn permission() -> PermissionRequest {
    PermissionRequest {
        slug: "pebble".into(),
        id: "calendar.read".into(),
    }
}
fn audit(last: u32) -> AuditRequest {
    AuditRequest {
        slug: "pebble".into(),
        last,
    }
}
fn off() -> Permissions {
    Permissions {
        permissions: vec![Permission {
            id: "calendar.read".into(),
            words: "read your calendar".into(),
            on: false,
            given_on: None,
            past_days: 0,
            next_days: 14,
            window: None,
        }],
        calendar: None,
        calendar_problem: None,
    }
}
fn safe(h: &Home, failure: Failure, expected: &str) {
    assert_eq!(failure.message, expected);
    assert!(
        [
            UNKNOWN,
            DAMAGED,
            PERMISSION_SAVE,
            AUDIT_READ,
            text::WINDOW_RANGE,
            text::NO_STATE_FOR_GRANTS,
            text::AUDIT_LAST,
            calendar::NO_STATE,
            BUSY,
            FIND,
            calendar::NOT_ICS
        ]
        .contains(&expected)
    );
    for private in [h.0.to_str().unwrap(), "grants.json", "pebble"] {
        assert!(
            !failure.message.contains(private),
            "private data in failure"
        );
    }
}
#[test]
fn list_without_grants_or_state_creates_nothing() {
    for state in [false, true] {
        let h = Home::new(state);
        assert_eq!(list_permissions_in(&h.0, slug()).unwrap(), off());
        assert_eq!(h.state().exists(), state);
        assert!(!h.state().join("grants.json").exists());
        assert!(!h.state().join("audit.jsonl").exists());
    }
}
#[test]
fn give_dates_and_replaces_the_grant_and_logs_it() {
    let h = Home::new(true);
    let before = ledger::local_day(SystemTime::now());
    let got = give_permission_in(&h.0, give(3, 30)).unwrap();
    let saved = grants::load(&h.0, "pebble").unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].capability, Capability::CalendarRead);
    assert_eq!(saved[0].window, Window::new(3, 30).unwrap());
    assert!([before, ledger::local_day(SystemTime::now())].contains(&saved[0].given_on));
    assert_eq!(got.said, text::grant_given("Pebble", &saved[0]));
    assert_eq!(
        got.said,
        "Voice may now read your calendar for Pebble, from 3 days back to 30 days ahead."
    );
    assert_eq!(
        got.permissions,
        Permissions {
            permissions: vec![Permission {
                id: "calendar.read".into(),
                words: "read your calendar".into(),
                on: true,
                given_on: Some(saved[0].given_on.clone()),
                past_days: 3,
                next_days: 30,
                window: Some("from 3 days back to 30 days ahead".into())
            }],
            calendar: None,
            calendar_problem: None
        }
    );
    let log = grants::audit(&h.0, "pebble", 50).unwrap();
    assert_eq!(log.entries.len(), 1);
    assert_eq!(
        log.entries[0].event,
        Event::Grant {
            capability: Capability::CalendarRead,
            window: Window::new(3, 30).unwrap()
        }
    );
    let got = give_permission_in(&h.0, give(0, 7)).unwrap();
    let saved = grants::load(&h.0, "pebble").unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].window, Window::new(0, 7).unwrap());
    assert_eq!(
        got.permissions.permissions[0].window.as_deref(),
        Some("from today to 7 days ahead")
    );
    assert_eq!(
        got.permissions.permissions[0].given_on.as_deref(),
        Some(saved[0].given_on.as_str())
    );
}
#[test]
fn turn_off_reports_both_outcomes_and_logs_only_a_change() {
    let h = Home::new(true);
    give_permission_in(&h.0, give(0, 14)).unwrap();
    let got = turn_off_permission_in(&h.0, permission()).unwrap();
    assert_eq!(
        got.said,
        text::grant_revoked("Pebble", Capability::CalendarRead, true)
    );
    assert_eq!(
        got.said,
        "Voice may no longer read your calendar for Pebble."
    );
    assert_eq!(got.permissions, off());
    let log = grants::audit(&h.0, "pebble", 50).unwrap();
    assert_eq!(log.entries.len(), 2);
    assert_eq!(
        log.entries[0].event,
        Event::Revoke {
            capability: Capability::CalendarRead
        }
    );
    let bytes = fs::read(h.state().join("audit.jsonl")).unwrap();
    let got = turn_off_permission_in(&h.0, permission()).unwrap();
    assert_eq!(
        got.said,
        text::grant_revoked("Pebble", Capability::CalendarRead, false)
    );
    assert_eq!(
        got.said,
        "Voice didn't have permission to read your calendar for Pebble, so nothing changed."
    );
    assert_eq!(got.permissions, off());
    assert_eq!(fs::read(h.state().join("audit.jsonl")).unwrap(), bytes);
}
#[test]
fn refusals_are_fixed_and_write_nothing() {
    let h = Home::new(true);
    let mut unknown = give(0, 14);
    unknown.id = "mail.read".into();
    let error = give_permission_in(&h.0, unknown).unwrap_err();
    assert!(!error.message.contains("mail.read"));
    safe(&h, error, UNKNOWN);
    let mut unknown = permission();
    unknown.id = "mail.read".into();
    safe(
        &h,
        turn_off_permission_in(&h.0, unknown).unwrap_err(),
        UNKNOWN,
    );
    for request in [give(32, 14), give(0, 0)] {
        safe(
            &h,
            give_permission_in(&h.0, request).unwrap_err(),
            text::WINDOW_RANGE,
        );
    }
    {
        let _lock = private::lock_existing_state(&h.0, "pebble").unwrap();
        safe(&h, give_permission_in(&h.0, give(0, 14)).unwrap_err(), BUSY);
        safe(
            &h,
            turn_off_permission_in(&h.0, permission()).unwrap_err(),
            BUSY,
        );
    }
    assert!(!h.state().join("grants.json").exists());
    assert!(!h.state().join("audit.jsonl").exists());
    let absent = Home::new(false);
    safe(
        &absent,
        give_permission_in(&absent.0, give(0, 14)).unwrap_err(),
        text::NO_STATE_FOR_GRANTS,
    );
    safe(
        &absent,
        turn_off_permission_in(&absent.0, permission()).unwrap_err(),
        text::NO_STATE_FOR_GRANTS,
    );
    safe(
        &absent,
        check_calendar_choice_in(&absent.0, slug()).unwrap_err(),
        calendar::NO_STATE,
    );
    assert!(!absent.state().exists());
}
#[test]
fn damaged_grants_fail_closed_without_changing_bytes() {
    let h = Home::new(true);
    let bytes = br#"{"version":2,"grants":[]}"#;
    h.write("grants.json", bytes);
    safe(&h, list_permissions_in(&h.0, slug()).unwrap_err(), DAMAGED);
    safe(
        &h,
        give_permission_in(&h.0, give(0, 14)).unwrap_err(),
        DAMAGED,
    );
    safe(
        &h,
        turn_off_permission_in(&h.0, permission()).unwrap_err(),
        DAMAGED,
    );
    assert_eq!(fs::read(h.state().join("grants.json")).unwrap(), bytes);
    assert!(!h.state().join("audit.jsonl").exists());
}
#[test]
fn calendar_choice_returns_only_the_cleaned_name_and_kind() {
    let h = Home::new(true);
    check_calendar_choice_in(&h.0, slug()).unwrap();
    for (name, folder) in [
        ("team.ics".to_owned(), false),
        ("exports".to_owned(), true),
        (format!("{}\u{200b}.ics", "x".repeat(70)), false),
    ] {
        let path = h.0.join(&name);
        if folder {
            fs::create_dir(&path).unwrap();
        } else {
            fs::write(&path, "BEGIN:VCALENDAR\nEND:VCALENDAR\n").unwrap();
        }
        let expected = CalendarExport {
            name: text::typed_name(&name),
            folder,
        };
        assert_eq!(
            use_calendar_export_in(&h.0, "pebble", &path).unwrap(),
            expected
        );
        assert_eq!(
            list_permissions_in(&h.0, slug()).unwrap().calendar,
            Some(expected)
        );
    }
    let missing = h.0.join("notes.txt");
    fs::write(&missing, "text").unwrap();
    safe(
        &h,
        use_calendar_export_in(&h.0, "pebble", &missing).unwrap_err(),
        calendar::NOT_ICS,
    );
}
#[test]
fn missing_calendar_keeps_permissions_available_and_give_succeeds() {
    let h = Home::new(true);
    let path = h.0.join("team.ics");
    fs::write(&path, "BEGIN:VCALENDAR\nEND:VCALENDAR\n").unwrap();
    use_calendar_export_in(&h.0, "pebble", &path).unwrap();
    fs::remove_file(path).unwrap();

    let result = list_permissions_in(&h.0, slug());
    assert!(result.is_ok(), "{result:?}");
    let list = result.unwrap();
    assert_eq!(list.calendar, None);
    assert_eq!(
        serde_json::to_value(&list).unwrap()["calendarProblem"],
        calendar::GONE
    );
    let result = give_permission_in(&h.0, give(3, 30));
    assert!(result.is_ok(), "{result:?}");
    let changed = result.unwrap();
    assert_eq!(
        changed.said,
        "Voice may now read your calendar for Pebble, from 3 days back to 30 days ahead."
    );
    assert!(changed.permissions.permissions[0].on);
    assert_eq!(changed.permissions.calendar, None);
    assert_eq!(
        serde_json::to_value(&changed.permissions).unwrap()["calendarProblem"],
        calendar::GONE
    );
}

#[test]
fn damaged_calendar_choice_keeps_permissions_available() {
    let h = Home::new(true);
    h.write("calendar-export.json", b"{}");
    let result = list_permissions_in(&h.0, slug());
    assert!(result.is_ok(), "{result:?}");
    let list = result.unwrap();
    assert_eq!(list.calendar, None);
    assert_eq!(
        serde_json::to_value(&list).unwrap()["calendarProblem"],
        calendar::BAD
    );
}

#[test]
fn audit_uses_shared_words_newest_first_counts_damage_and_validates_bounds() {
    let h = Home::new(true);
    assert_eq!(
        audit_log_in(&h.0, audit(50)).unwrap(),
        AuditLog {
            lines: vec![],
            unreadable: 0,
            older: false
        }
    );
    give_permission_in(&h.0, give(0, 14)).unwrap();
    turn_off_permission_in(&h.0, permission()).unwrap();
    let log = grants::audit(&h.0, "pebble", 50).unwrap();
    let got = audit_log_in(&h.0, audit(50)).unwrap();
    assert_eq!(
        got.lines,
        log.entries
            .iter()
            .map(text::audit_entry)
            .collect::<Vec<_>>()
    );
    assert_eq!(got.lines.len(), 2);
    assert!(got.lines[0].contains("taken back:"));
    assert!(got.lines[1].contains("given:"));
    assert_eq!(got.unreadable, 0);
    assert!(!got.older);
    let short = audit_log_in(&h.0, audit(1)).unwrap();
    assert_eq!(short.lines, got.lines[..1]);
    assert!(short.older);
    for last in [0, 1001] {
        safe(
            &h,
            audit_log_in(&h.0, audit(last)).unwrap_err(),
            text::AUDIT_LAST,
        );
    }
    let mut bytes = fs::read(h.state().join("audit.jsonl")).unwrap();
    bytes.extend_from_slice(b"not JSON\n");
    h.write("audit.jsonl", &bytes);
    let got = audit_log_in(&h.0, audit(50)).unwrap();
    assert_eq!(got.unreadable, 1);
    assert_eq!(got.lines.len(), 2);
    fs::remove_file(h.state().join("audit.jsonl")).unwrap();
    fs::create_dir(h.state().join("audit.jsonl")).unwrap();
    safe(&h, audit_log_in(&h.0, audit(50)).unwrap_err(), AUDIT_READ);
}
#[test]
fn permission_requests_are_strict_and_use_camel_case() {
    assert_eq!(UNKNOWN, "Voice can't be given that permission.");
    assert_eq!(
        DAMAGED,
        "Voice's permissions file for this assistant is damaged, so it reads nothing that needs your permission. To see where it is, run voice grants show in a terminal, delete it, then give permission again."
    );
    assert_eq!(
        PERMISSION_SAVE,
        "Voice couldn't save this. Check there's space on this computer, then try again."
    );
    assert_eq!(
        AUDIT_READ,
        "Voice couldn't read this assistant's log. Try again."
    );
    use serde_json::{from_value, json, to_value};
    assert_eq!(
        to_value(give(3, 30)).unwrap(),
        json!({"slug":"pebble","id":"calendar.read","pastDays":3,"nextDays":30})
    );
    assert!(from_value::<GivePermissionRequest>(json!({"slug":"pebble","id":"calendar.read","pastDays":0,"nextDays":14,"path":"private"})).is_err());
    assert!(
        from_value::<PermissionRequest>(
            json!({"slug":"pebble","id":"calendar.read","path":"private"})
        )
        .is_err()
    );
    assert!(
        from_value::<ChooseCalendarRequest>(
            json!({"slug":"pebble","folder":false,"path":"private"})
        )
        .is_err()
    );
    assert!(
        from_value::<AuditRequest>(json!({"slug":"pebble","last":50,"path":"private"})).is_err()
    );
}
