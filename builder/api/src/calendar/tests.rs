use super::*;
use crate::dto::NewAssistantRequest;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, UNIX_EPOCH},
};
use voice_agent::{
    grants::{self, Capability, Window},
    ledger, text,
};
use voice_ingest::private;
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Home(PathBuf);
impl Home {
    fn new(state: bool) -> Self {
        let h = Self(std::env::temp_dir().join(format!(
            "voice-calendar-api-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&h.0).unwrap();
        crate::new_assistant_in(
            &h.0,
            NewAssistantRequest {
                name: "Ari".into(),
                pronouns: "they/them".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        if state {
            drop(private::lock_state(&h.0, "ari").unwrap());
        }
        fs::write(h.file(), "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:20261103T100000\nDTEND:20261103T110000\nSUMMARY:Team planning\nDESCRIPTION:canary-outside\nEND:VEVENT\nEND:VCALENDAR\n").unwrap();
        h
    }
    fn file(&self) -> PathBuf {
        self.0.join("calendar.ICS")
    }
    fn state(&self) -> PathBuf {
        self.0.join(".claude/state/ari")
    }
    fn choose(&self) {
        choose_calendar_export_in(&self.0, "ari", &self.file()).unwrap();
    }
    fn grant(&self) {
        grants::give(
            &self.0,
            "ari",
            Capability::CalendarRead,
            Window::DEFAULT,
            now(),
        )
        .unwrap();
    }
    fn audit(&self) -> String {
        fs::read_to_string(self.state().join("audit.jsonl")).unwrap()
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
// Keep the fixed clock's local day stable without changing the process-wide TZ during parallel tests.
fn in_utc() -> bool {
    if std::env::var_os("VOICE_CALENDAR_TEST_FIXED_ZONE").is_some() {
        assert_eq!(std::env::var("TZ").as_deref(), Ok("UTC"));
        return true;
    }
    let thread = std::thread::current();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", thread.name().unwrap(), "--nocapture"])
        .env("TZ", "UTC")
        .env("VOICE_CALENDAR_TEST_FIXED_ZONE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
    false
}
fn now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_620_800)
}
fn answer() -> CalendarRead {
    CalendarRead {
        events: vec![CalendarEvent {
            title: "Team planning".into(),
            place: Some("Room 4".into()),
            start: "2026-11-03T10:00".into(),
            end: "2026-11-03T11:00".into(),
            all_day: false,
            status: Some(EventStatus::Confirmed),
        }],
        from: "2026-11-02".into(),
        to: "2026-11-16".into(),
        private_left_out: 0,
        not_understood: 0,
        more: false,
    }
}

#[test]
fn choose_stores_exact_json_without_history_consent_and_reads_it_back() {
    let h = Home::new(true);
    let got = choose_calendar_export_in(&h.0, "ari", &h.file()).unwrap();
    assert_eq!(
        got,
        CalendarExport {
            name: "calendar.ICS".into(),
            folder: false
        }
    );
    assert_eq!(
        fs::read(h.state().join("calendar-export.json")).unwrap(),
        format!(r#"{{"path":"{}","folder":false}}"#, h.file().display()).as_bytes()
    );
    assert_eq!(calendar_export_in(&h.0, "ari").unwrap(), Some(got));
    let folder = h.0.join("calendars");
    fs::create_dir(&folder).unwrap();
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", &folder).unwrap(),
        CalendarExport {
            name: "calendars".into(),
            folder: true
        }
    );
    assert!(!crate::read_consent(&h.0, "ari").agreed);
}
#[test]
fn choose_refuses_bad_missing_and_denied_paths() {
    let h = Home::new(true);
    let txt = h.0.join("notes.txt");
    fs::write(&txt, "text").unwrap();
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", &txt)
            .unwrap_err()
            .message,
        NOT_ICS
    );
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", &h.0.join("missing.ics"))
            .unwrap_err()
            .message,
        GONE
    );
    let denied = h.0.join(".ssh");
    fs::create_dir(&denied).unwrap();
    fs::write(denied.join("calendar.ics"), "calendar").unwrap();
    assert!(choose_calendar_export_in(&h.0, "ari", &denied.join("calendar.ics")).is_err());
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", Path::new("relative.ics"))
            .unwrap_err()
            .message,
        NOT_ICS
    );
    assert!(!h.state().join("calendar-export.json").exists());
}
#[test]
fn choose_needs_existing_state_and_respects_the_lock() {
    let h = Home::new(false);
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", &h.file())
            .unwrap_err()
            .message,
        NO_STATE
    );
    assert!(!h.state().exists());
    let _lock = private::lock_state(&h.0, "ari").unwrap();
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", &h.file())
            .unwrap_err()
            .message,
        crate::history::BUSY
    );
}
#[test]
fn stored_choice_is_strict_and_missing_export_is_gone() {
    let h = Home::new(true);
    assert_eq!(calendar_export_in(&h.0, "ari").unwrap(), None);
    for raw in [
        b"not json".as_slice(),
        br#"{"path":"relative.ics","folder":false}"#,
        br#"{"path":"/a.ics","folder":false,"extra":true}"#,
    ] {
        private::write_private(&h.0, "ari", "calendar-export.json", raw).unwrap();
        assert_eq!(calendar_export_in(&h.0, "ari").unwrap_err().message, BAD);
    }
    h.choose();
    fs::remove_file(h.file()).unwrap();
    assert_eq!(calendar_export_in(&h.0, "ari").unwrap_err().message, GONE);
}
#[test]
fn broker_requires_choice_before_running_and_logs_a_grant_refusal() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    assert_eq!(
        read_calendar_through(&h.0, "ari", "cli", now(), |_| panic!("no choice"))
            .unwrap_err()
            .message,
        NONE
    );
    h.choose();
    assert_eq!(
        read_calendar_through(&h.0, "ari", "cli", now(), |_| panic!("no grant"))
            .unwrap_err()
            .message,
        "Voice doesn't have your permission to read your calendar for this assistant."
    );
    assert_eq!(
        h.audit(),
        "{\"at\":\"2026-11-02T12:00:00Z\",\"event\":\"refused\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"reason\":\"no-grant\"}\n"
    );
}
#[test]
fn broker_passes_the_local_day_and_zone_and_logs_only_the_item_count() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    h.choose();
    h.grant();
    let result = read_calendar_through(&h.0, "ari", "cli", now(), |request| {
        assert_eq!(request.slug, "ari");
        assert_eq!(request.today, ledger::local_day(now()));
        assert!(
            voice_ingest::calendar::zone::Zone::load(
                Path::new(voice_ingest::calendar::zone::ZONEINFO),
                &request.zone
            )
            .is_some()
        );
        Ok(answer())
    })
    .unwrap();
    assert_eq!(result, answer());
    assert_eq!(
        h.audit().lines().last().unwrap(),
        r#"{"at":"2026-11-02T12:00:00Z","event":"read","capability":"calendar.read","step":"cli","items":1}"#
    );
    assert!(!h.audit().contains("Team planning"));
}
#[test]
fn broker_rejects_each_untrusted_answer_without_logging_a_read() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    h.choose();
    h.grant();
    let mut bad = Vec::new();
    let mut a = answer();
    a.from = "2026-11-01".into();
    bad.push(a);
    let mut a = answer();
    a.to = "2026-11-17".into();
    bad.push(a);
    for (start, end, all_day) in [
        ("2026-11-17T10:00", "2026-11-17T11:00", false),
        ("2026-11-01T23:00", "2026-11-02T00:00", false),
        ("2026-11-03T11:00", "2026-11-03T10:00", false),
        ("2026-11-03", "2026-11-04", false),
        ("2026-11-03T10:00", "2026-11-04T10:00", true),
        ("2026-11-31T10:00", "2026-11-31T11:00", false),
        ("2026-11-03T24:00", "2026-11-04T01:00", false),
        ("éééééééé", "éééééééé", false),
    ] {
        let mut a = answer();
        a.events[0].start = start.into();
        a.events[0].end = end.into();
        a.events[0].all_day = all_day;
        bad.push(a);
    }
    for value in [
        "bad\u{202e}title".into(),
        "x".repeat(202),
        "bad\nline".into(),
    ] {
        let mut a = answer();
        a.events[0].title = value.clone();
        bad.push(a);
        let mut a = answer();
        a.events[0].place = Some(value);
        bad.push(a);
    }
    let mut a = answer();
    a.events.resize(2001, a.events[0].clone());
    bad.push(a);
    for a in bad {
        assert_eq!(
            read_calendar_through(&h.0, "ari", "cli", now(), |_| Ok(a))
                .unwrap_err()
                .message,
            reader::STOPPED
        );
    }
    assert_eq!(h.audit().lines().count(), 1);
}
#[test]
fn broker_accepts_overlap_and_zero_length_at_midnight() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    h.choose();
    h.grant();
    for (start, end, all_day) in [
        ("2026-11-01T23:00", "2026-11-02T01:00", false),
        ("2026-11-02T00:00", "2026-11-02T00:00", false),
        ("2026-11-01", "2026-11-02", true),
    ] {
        let mut a = answer();
        a.events[0].start = start.into();
        a.events[0].end = end.into();
        a.events[0].all_day = all_day;
        assert_eq!(
            read_calendar_through(&h.0, "ari", "cli", now(), |_| Ok(a.clone())).unwrap(),
            a
        );
    }
}
#[test]
fn broker_drops_the_answer_when_the_log_cannot_be_written() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    h.choose();
    h.grant();
    fs::remove_file(h.state().join("audit.jsonl")).unwrap();
    fs::create_dir(h.state().join("audit.jsonl")).unwrap();
    assert_eq!(
        read_calendar_through(&h.0, "ari", "cli", now(), |_| Ok(answer()))
            .unwrap_err()
            .message,
        LOG
    );
}
#[test]
fn reader_uses_its_own_grant_window_and_never_logs_or_reads_descriptions() {
    let h = Home::new(true);
    h.choose();
    let clock = SystemTime::now();
    let today = ledger::utc_stamp(clock)[..10].to_owned();
    let date = today.replace('-', "");
    fs::write(h.file(), format!("BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:{date}T100000\nSUMMARY:Team planning\nDESCRIPTION:canary-outside\nEND:VEVENT\nEND:VCALENDAR\n")).unwrap();
    let request = reader::CalendarStep {
        slug: "ari".into(),
        today: today.clone(),
        zone: "UTC".into(),
    };
    assert_eq!(
        read_calendar_in(&h.0, request.clone()).unwrap_err().message,
        text::no_grant(Capability::CalendarRead)
    );
    grants::give(
        &h.0,
        "ari",
        Capability::CalendarRead,
        Window::new(1, 1).unwrap(),
        now(),
    )
    .unwrap();
    let before = h.audit();
    let a = read_calendar_in(&h.0, request.clone()).unwrap();
    assert_eq!(
        a.from,
        ledger::utc_stamp(clock - Duration::from_secs(86400))[..10]
    );
    assert_eq!(
        a.to,
        ledger::utc_stamp(clock + Duration::from_secs(86400))[..10]
    );
    assert_eq!(a.events.len(), 1);
    assert_eq!(a.events[0].start, format!("{today}T10:00"));
    assert!(!format!("{a:?}").contains("canary-outside"));
    assert_eq!(h.audit(), before);
    private::write_private(&h.0, "ari", "grants.json", b"damaged").unwrap();
    assert_eq!(
        read_calendar_in(&h.0, request).unwrap_err().message,
        BAD_GRANTS
    );
}
#[test]
fn reader_refuses_bad_today_and_unknown_zone_before_permission() {
    let h = Home::new(true);
    for today in ["2026-02-30", "2026-2-03", "2000-01-01", "ééééé"] {
        assert_eq!(
            read_calendar_in(
                &h.0,
                reader::CalendarStep {
                    slug: "ari".into(),
                    today: today.into(),
                    zone: "UTC".into()
                }
            )
            .unwrap_err()
            .message,
            reader::REQUEST
        );
    }
    assert_eq!(
        read_calendar_in(
            &h.0,
            reader::CalendarStep {
                slug: "ari".into(),
                today: ledger::utc_stamp(SystemTime::now())[..10].into(),
                zone: "Invented/Missing".into()
            }
        )
        .unwrap_err()
        .message,
        ZONE
    );
}
#[test]
fn broker_propagates_reader_refusal_without_a_read_line() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    h.choose();
    h.grant();
    assert_eq!(
        read_calendar_through(&h.0, "ari", "cli", now(), |_| Err(failure(TOO_BIG)))
            .unwrap_err()
            .message,
        TOO_BIG
    );
    assert_eq!(h.audit().lines().count(), 1);
    // Keep the wire shape pinned alongside the generated DTOs.
    assert_eq!(
        serde_json::to_value(answer().events[0].clone()).unwrap(),
        json!({"title":"Team planning", "place":"Room 4", "start":"2026-11-03T10:00", "end":"2026-11-03T11:00", "allDay":false, "status":"confirmed"})
    );
}

#[test]
fn broker_refuses_hidden_bom_without_logging_a_read() {
    if !in_utc() {
        return;
    }
    let h = Home::new(true);
    h.choose();
    h.grant();
    let mut a = answer();
    a.events[0].title = "Team\u{feff}planning".into();
    assert_eq!(
        read_calendar_through(&h.0, "ari", "cli", now(), |_| Ok(a))
            .unwrap_err()
            .message,
        reader::STOPPED
    );
    let mut a = answer();
    a.events[0].place = Some("Room\u{feff}4".into());
    assert_eq!(
        read_calendar_through(&h.0, "ari", "cli", now(), |_| Ok(a))
            .unwrap_err()
            .message,
        reader::STOPPED
    );
    assert_eq!(h.audit().lines().count(), 1);
}

#[test]
fn choose_refuses_file_folder_and_dangling_links() {
    let h = Home::new(true);
    let folder = h.0.join("calendars");
    fs::create_dir(&folder).unwrap();
    for (name, target) in [
        ("linked.ics", h.file()),
        ("linked-folder", folder),
        ("dangling.ics", h.0.join("missing.ics")),
    ] {
        let link = h.0.join(name);
        std::os::unix::fs::symlink(target, &link).unwrap();
        assert_eq!(
            choose_calendar_export_in(&h.0, "ari", &link)
                .unwrap_err()
                .message,
            "Voice doesn't follow links. Choose the calendar file itself."
        );
    }
    assert!(!h.state().join("calendar-export.json").exists());
}
#[test]
fn choose_reports_save_when_the_state_lock_cannot_be_opened() {
    let h = Home::new(true);
    fs::remove_dir(h.state()).unwrap();
    fs::write(h.state(), "not a folder").unwrap();
    assert_eq!(
        choose_calendar_export_in(&h.0, "ari", &h.file())
            .unwrap_err()
            .message,
        history::SAVE
    );
    assert!(!h.state().join("calendar-export.json").exists());
}
