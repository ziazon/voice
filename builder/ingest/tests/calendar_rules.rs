mod common;
use common::Scratch;
use std::{fs, path::Path};
use voice_ingest::calendar::zone::{ZONEINFO, Zone};

#[test]
fn zone_names_and_database_limits() {
    for name in ["UTC", "America/New_York", "Etc/GMT+4", &"a".repeat(64)] {
        assert!(Zone::valid_name(name), "{name}");
    }
    for name in [
        "",
        "../etc",
        "a//b",
        "a b",
        "/UTC",
        "a/",
        ".",
        "..",
        &"a".repeat(65),
    ] {
        assert!(!Zone::valid_name(name), "{name}");
        assert!(Zone::load(Path::new(ZONEINFO), name).is_none());
    }
    assert_eq!(Zone::utc().offset_at(0), Some(0));
    assert_eq!(Zone::utc().to_utc(-1), Some(-1));
    let utc = Zone::load(Path::new(ZONEINFO), "UTC");
    assert!(utc.is_some());
    assert_eq!(utc.unwrap().offset_at(2_208_988_800), Some(0));
    let tokyo = Zone::load(Path::new(ZONEINFO), "Asia/Tokyo");
    assert!(tokyo.is_some());
    assert_eq!(tokyo.unwrap().offset_at(2_208_988_800), Some(32_400));
    let ny = Zone::load(Path::new(ZONEINFO), "America/New_York");
    assert!(ny.is_some());
    assert_eq!(ny.unwrap().offset_at(2_208_988_800), None);
}

#[test]
fn zone_gap_and_overlap_use_the_rfc_instants() {
    let zone = Zone::load(Path::new(ZONEINFO), "America/New_York");
    assert!(zone.is_some());
    let zone = zone.unwrap();
    // 2026-03-08 02:30 wall -> 07:30 UTC; 2026-11-01 01:30 wall -> 05:30 UTC.
    assert_eq!(zone.to_utc(1_772_937_000), Some(1_772_955_000));
    assert_eq!(zone.to_utc(1_793_496_600), Some(1_793_511_000));
}

#[test]
fn zone_files_are_checked_before_parsing() {
    let scratch = Scratch::new("calendar-zones");
    let bytes = fs::read(Path::new(ZONEINFO).join("America/New_York")).unwrap();
    fs::write(scratch.0.join("Good"), &bytes).unwrap();
    assert!(Zone::load(&scratch.0, "Good").is_some());
    for (n, truncated) in [0, 4, 43, bytes.len() - 1].into_iter().enumerate() {
        let name = format!("Truncated{n}");
        fs::write(scratch.0.join(&name), &bytes[..truncated]).unwrap();
        assert!(Zone::load(&scratch.0, &name).is_none());
    }
    fs::write(scratch.0.join("Huge"), vec![0; 512 * 1024 + 1]).unwrap();
    assert!(Zone::load(&scratch.0, "Huge").is_none());
    fs::create_dir(scratch.0.join("Folder")).unwrap();
    assert!(Zone::load(&scratch.0, "Folder").is_none());
    assert!(Zone::load(&scratch.0, "Missing").is_none());
}

use voice_ingest::{
    calendar::{self, Calendar, Date, Event, Status, Window},
    filters::Filters,
};
fn filters(personal: &str, secret: &str) -> Filters {
    Filters::compile(&voice_core::member::Filters {
        personal: personal.into(),
        secret: secret.into(),
        excluded_folders: "a^".into(),
        pasted_prefixes: Vec::new(),
        denied_paths: Vec::new(),
        extra: Default::default(),
    })
    .unwrap()
}
fn window() -> Window {
    Window {
        today: Date {
            year: 2026,
            month: 11,
            day: 2,
        },
        past_days: 0,
        next_days: 14,
    }
}
fn parse_at(body: &str, zone: &str, window: Window) -> Calendar {
    let local = Zone::load(Path::new(ZONEINFO), zone).unwrap();
    calendar::read(
        &[format!("BEGIN:VCALENDAR\n{body}END:VCALENDAR\n").into_bytes()],
        &local,
        Path::new(ZONEINFO),
        window,
        &filters("a^", "a^"),
    )
    .unwrap()
}
fn parse(body: &str) -> Calendar {
    parse_at(body, "America/New_York", window())
}
fn entry(properties: &str) -> String {
    format!("BEGIN:VEVENT\n{properties}\nEND:VEVENT\n")
}
fn event(title: &str, start: &str, end: &str, all_day: bool) -> Event {
    Event {
        title: title.into(),
        place: None,
        start: start.into(),
        end: end.into(),
        all_day,
        status: None,
    }
}
fn starts(calendar: &Calendar) -> Vec<&str> {
    calendar.events.iter().map(|e| e.start.as_str()).collect()
}

#[test]
fn utc_floating_named_and_all_day_have_exact_local_fields() {
    let body = [
        entry("DTSTART:20261103T150000Z\nDTEND:20261103T160000Z\nSUMMARY:UTC\nLOCATION:Room 4\nSTATUS:confirmed"),
        entry("DTSTART:20261103T110000\nDURATION:PT30M\nSUMMARY:Floating\nSTATUS:TENTATIVE"),
        entry("DTSTART;TZID=Europe/Berlin:20261103T160000\nSUMMARY:Berlin"),
        entry("DTSTART;TZID=Asia/Tokyo:20261104T090000\nSUMMARY:Tokyo\nSTATUS:CANCELLED"),
        entry("DTSTART;VALUE=DATE:20261105\nDTEND;VALUE=DATE:20261108\nSUMMARY:Days"),
    ].concat();
    let mut utc = event("UTC", "2026-11-03T10:00", "2026-11-03T11:00", false);
    utc.place = Some("Room 4".into());
    utc.status = Some(Status::Confirmed);
    let mut floating = event("Floating", "2026-11-03T11:00", "2026-11-03T11:30", false);
    floating.status = Some(Status::Tentative);
    let mut tokyo = event("Tokyo", "2026-11-03T19:00", "2026-11-03T19:00", false);
    tokyo.status = Some(Status::Cancelled);
    let c = parse(&body);
    assert_eq!(
        c.events,
        vec![
            event("Berlin", "2026-11-03T10:00", "2026-11-03T10:00", false),
            utc,
            floating,
            tokyo,
            event("Days", "2026-11-05", "2026-11-07", true)
        ]
    );
    assert_eq!(
        (c.from.to_string(), c.to.to_string()),
        ("2026-11-02".into(), "2026-11-16".into())
    );
    assert_eq!(
        (c.private_left_out, c.not_understood, c.more),
        (0, 0, false)
    );
}
#[test]
fn content_lines_unfold_unescape_and_skip_nested_components() {
    let raw = "\u{feff}begin:vcalendar\r\nbegin:vtimezone\nBEGIN:VEVENT\nDTSTART:20261103T100000\nSUMMARY:Hidden\nEND:VEVENT\nend:vtimezone\nbegin:vevent\r\ndtstart:20261103T100060\r\nsummary;X-NOTE=\"with:colon;semi\":Team\\, plan\\; \\\\ \\nnext\\Nlast\\q\r\n line\n\tend\nlocation:Room\\, 4\nstatus:other\nBEGIN:VALARM\nSUMMARY:Hidden\nEND:VALARM\nend:vevent\nend:vcalendar\n";
    let c = calendar::read(
        &[raw.as_bytes().to_vec()],
        &Zone::utc(),
        Path::new(ZONEINFO),
        window(),
        &filters("a^", "a^"),
    )
    .unwrap();
    let mut expected = event(
        "Team, plan; \\ next lastqlineend",
        "2026-11-03T10:00",
        "2026-11-03T10:00",
        false,
    );
    expected.place = Some("Room, 4".into());
    assert_eq!(c.events, vec![expected]);
}
#[test]
fn private_properties_never_leave_the_parser() {
    let mut body = "DTSTART:20261103T100000\nSUMMARY:Team planning\n".to_owned();
    for property in [
        "DESCRIPTION",
        "ATTENDEE",
        "ORGANIZER",
        "URL",
        "CONTACT",
        "COMMENT",
        "ATTACH",
        "X-NOTE",
    ] {
        body += &format!("{property}:canary-outside\n");
    }
    let c = parse(&entry(&body));
    assert_eq!(c.events.len(), 1);
    assert!(!format!("{c:?}").contains("canary-outside"));
}
#[test]
fn daily_count_is_before_exdates() {
    let c = parse(&entry(
        "DTSTART:20261102T100000\nRRULE:FREQ=DAILY;COUNT=3\nEXDATE:20261103T100000\nSUMMARY:Team planning",
    ));
    assert_eq!(starts(&c), ["2026-11-02T10:00", "2026-11-04T10:00"]);
}
#[test]
fn weekly_interval_and_week_start() {
    let body = "DTSTART:20261101T100000\nRRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE";
    let mut w = window();
    w.past_days = 1;
    let c = parse_at(&entry(body), "America/New_York", w);
    assert_eq!(
        starts(&c),
        ["2026-11-01T10:00", "2026-11-09T10:00", "2026-11-11T10:00"]
    );
    let c = parse_at(&entry(&format!("{body};WKST=SU")), "America/New_York", w);
    assert_eq!(
        starts(&c),
        [
            "2026-11-01T10:00",
            "2026-11-02T10:00",
            "2026-11-04T10:00",
            "2026-11-16T10:00"
        ]
    );
}
#[test]
fn monthly_days_skip_short_months_and_ordinals_skip_missing_weekdays() {
    let mut w = window();
    w.next_days = 92;
    let c = parse_at(
        &entry("DTSTART:20261031T100000\nRRULE:FREQ=MONTHLY;BYMONTHDAY=31"),
        "UTC",
        w,
    );
    assert_eq!(starts(&c), ["2026-12-31T10:00", "2027-01-31T10:00"]);
    let c = parse_at(
        &entry("DTSTART:20261030T100000\nRRULE:FREQ=MONTHLY;BYDAY=-1FR"),
        "UTC",
        w,
    );
    assert_eq!(
        starts(&c),
        ["2026-11-27T10:00", "2026-12-25T10:00", "2027-01-29T10:00"]
    );
    let c = parse_at(
        &entry("DTSTART:20261030T100000\nRRULE:FREQ=MONTHLY;BYDAY=5MO"),
        "UTC",
        w,
    );
    assert_eq!(starts(&c), ["2026-11-30T10:00"]);
}
#[test]
fn week_start_is_allowed_and_ignored_on_nonweekly_rules() {
    for (rule, expected) in [
        (
            "FREQ=MONTHLY;BYMONTHDAY=3;COUNT=3",
            vec!["2026-11-03T10:00", "2026-12-03T10:00", "2027-01-03T10:00"],
        ),
        (
            "FREQ=DAILY;COUNT=2",
            vec!["2026-11-03T10:00", "2026-11-04T10:00"],
        ),
        ("FREQ=YEARLY;COUNT=2", vec!["2026-11-03T10:00"]),
    ] {
        let mut w = window();
        w.next_days = 92;
        for wkst in ["", ";WKST=SU"] {
            let c = parse_at(
                &entry(&format!("DTSTART:20261103T100000\nRRULE:{rule}{wkst}")),
                "UTC",
                w,
            );
            assert_eq!(starts(&c), expected, "{rule}{wkst}");
            assert_eq!(c.not_understood, 0);
        }
    }
}
#[test]
fn yearly_leap_day_skips_non_leap_years() {
    let w = Window {
        today: Date {
            year: 2027,
            month: 2,
            day: 27,
        },
        past_days: 0,
        next_days: 370,
    };
    for by in ["", ";BYMONTH=2"] {
        let c = parse_at(
            &entry(&format!(
                "DTSTART;VALUE=DATE:20240229\nRRULE:FREQ=YEARLY;COUNT=3{by}"
            )),
            "UTC",
            w,
        );
        assert_eq!(starts(&c), ["2028-02-29"]);
    }
}
#[test]
fn until_is_inclusive_in_utc_and_exdates_keep_the_start_frame() {
    let c = parse(&entry(
        "DTSTART;TZID=Europe/Berlin:20261102T160000\nRRULE:FREQ=DAILY;UNTIL=20261104T150000Z\nEXDATE;TZID=Europe/Berlin:20261103T160000",
    ));
    assert_eq!(starts(&c), ["2026-11-02T10:00", "2026-11-04T10:00"]);
    let c = parse(&entry(
        "DTSTART;TZID=Europe/Berlin:20261102T160000\nRRULE:FREQ=DAILY;UNTIL=20261104T160000\nEXDATE;TZID=Europe/Berlin:20261102T160000,20261103T160000\nEXDATE;TZID=Europe/Berlin:20261104T160000",
    ));
    assert_eq!((c.events.len(), c.not_understood), (0, 0));
}
#[test]
fn overrides_replace_one_occurrence_and_keep_cancellations() {
    let c = parse(&[
        entry("UID:series\nDTSTART:20261102T100000\nDURATION:PT1H\nRRULE:FREQ=DAILY;COUNT=3\nSUMMARY:Team planning"),
        entry("UID:series\nRECURRENCE-ID:20261103T100000\nDTSTART:20261103T140000\nSUMMARY:Moved"),
        entry("UID:series\nRECURRENCE-ID:20261104T100000\nDTSTART:20261104T100000\nSTATUS:CANCELLED\nSUMMARY:Cancelled"),
    ].concat());
    assert_eq!(
        starts(&c),
        ["2026-11-02T10:00", "2026-11-03T14:00", "2026-11-04T10:00"]
    );
    assert_eq!(c.events[1].title, "Moved");
    assert_eq!(c.events[2].status, Some(Status::Cancelled));
}
#[test]
fn an_override_without_its_series_is_a_single_event() {
    let c = parse(&entry(
        "UID:invitation\nRECURRENCE-ID:20261103T100000\nDTSTART:20261103T140000\nDTEND:20261103T150000\nSUMMARY:Team planning\nLOCATION:Room 4\nSTATUS:TENTATIVE",
    ));
    assert_eq!(
        c.events,
        [Event {
            title: "Team planning".into(),
            place: Some("Room 4".into()),
            start: "2026-11-03T14:00".into(),
            end: "2026-11-03T15:00".into(),
            all_day: false,
            status: Some(Status::Tentative),
        }]
    );
    assert_eq!(c.not_understood, 0);
}
#[test]
fn an_unmatched_override_with_a_series_present_is_not_understood() {
    for rule in ["FREQ=DAILY;COUNT=1", "FREQ=HOURLY"] {
        let c = parse(&[
            entry(&format!("UID:series\nDTSTART:20261102T100000\nRRULE:{rule}")),
            entry("UID:series\nRECURRENCE-ID:20261103T100000\nDTSTART:20261103T140000\nSUMMARY:Unmatched"),
        ].concat());
        assert!(!c.events.iter().any(|event| event.title == "Unmatched"));
        assert_eq!(c.not_understood, if rule == "FREQ=HOURLY" { 2 } else { 1 });
    }
}
#[test]
fn daylight_changes_convert_gaps_and_first_overlap_and_keep_weekly_wall_time() {
    let march = Window {
        today: Date {
            year: 2026,
            month: 3,
            day: 8,
        },
        past_days: 0,
        next_days: 14,
    };
    let c = parse_at(
        &entry("DTSTART;TZID=America/New_York:20260308T023000\nDURATION:PT1H"),
        "America/New_York",
        march,
    );
    assert_eq!(
        c.events,
        [event("", "2026-03-08T03:30", "2026-03-08T04:30", false)]
    );
    let nov = Window {
        today: Date {
            year: 2026,
            month: 11,
            day: 1,
        },
        past_days: 0,
        next_days: 14,
    };
    let c = parse_at(
        &entry("DTSTART;TZID=America/New_York:20261101T013000"),
        "UTC",
        nov,
    );
    assert_eq!(starts(&c), ["2026-11-01T05:30"]);
    let c = parse_at(
        &entry("DTSTART;TZID=America/New_York:20261025T100000\nRRULE:FREQ=WEEKLY;COUNT=3"),
        "America/New_York",
        nov,
    );
    assert_eq!(starts(&c), ["2026-11-01T10:00", "2026-11-08T10:00"]);
}
#[test]
fn windows_overlap_inclusive_days_and_exclude_midnight_end() {
    let c = parse(
        &[
            entry("DTSTART;VALUE=DATE:20261102"),
            entry("DTSTART;VALUE=DATE:20261116"),
            entry("DTSTART;VALUE=DATE:20261101"),
            entry("DTSTART;VALUE=DATE:20261117"),
            entry("DTSTART;VALUE=DATE:20261030\nDTEND;VALUE=DATE:20261102"),
            entry("DTSTART;VALUE=DATE:20261030\nDTEND;VALUE=DATE:20261104"),
            entry("DTSTART:20261101T220000\nDTEND:20261102T000000"),
            entry("DTSTART:20261102T000000"),
        ]
        .concat(),
    );
    assert_eq!(
        c.events,
        [
            event("", "2026-10-30", "2026-11-03", true),
            event("", "2026-11-02", "2026-11-02", true),
            event("", "2026-11-02T00:00", "2026-11-02T00:00", false),
            event("", "2026-11-16", "2026-11-16", true)
        ]
    );
}
#[test]
fn unsupported_entries_count_only_when_they_could_touch_the_window() {
    let bads = [
        "RRULE:FREQ=HOURLY",
        "RRULE:FREQ=DAILY;BYHOUR=10",
        "RRULE:FREQ=DAILY;BYSETPOS=1",
        "RRULE:FREQ=DAILY;RSCALE=GREGORIAN",
        "RRULE:FREQ=DAILY;SKIP=FORWARD",
        "RRULE:FREQ=DAILY;COUNT=2;UNTIL=20261104T100000",
        "RRULE:FREQ=WEEKLY;BYDAY=1MO",
        "RRULE:FREQ=MONTHLY;BYDAY=MO",
        "RRULE:FREQ=MONTHLY;BYDAY=0MO",
        "RRULE:FREQ=MONTHLY;BYDAY=6MO",
        "RRULE:FREQ=MONTHLY;BYDAY=1MO;BYMONTHDAY=1",
        "RRULE:FREQ=YEARLY;BYMONTH=12",
        "RRULE:FREQ=DAILY;INTERVAL=0",
        "RRULE:FREQ=DAILY;INTERVAL=1001",
        "RRULE:FREQ=DAILY;COUNT=0",
        "RRULE:FREQ=DAILY;COUNT=100001",
        "RDATE:20261104T100000",
        "EXRULE:FREQ=DAILY",
        "RECURRENCE-ID;RANGE=THISANDFUTURE:20261103T100000",
        "RECURRENCE-ID:20261103T100000",
        "UID:series\nRECURRENCE-ID:20261103T100000\nRRULE:FREQ=DAILY",
        "DTSTART:20261104T100000",
        "DTEND;VALUE=DATE:20261104",
        "DTEND:20261102T090000",
        "DURATION:-PT1H",
    ];
    for bad in bads {
        let c = parse(&entry(&format!("DTSTART:20261102T100000\n{bad}")));
        assert_eq!((c.events.len(), c.not_understood), (0, 1), "{bad}");
    }
    let c = parse(
        &[
            entry("DTSTART;TZID=Eastern Standard Time:20261103T100000"),
            entry("DTSTART:20190101T100000\nRRULE:FREQ=HOURLY"),
            entry("DTSTART:20190101T100000\nDURATION:-PT1H"),
            entry("DTSTART:not-a-date"),
            entry("SUMMARY:Missing start"),
        ]
        .concat(),
    );
    assert_eq!((c.events.len(), c.not_understood), (0, 4));
}
#[test]
fn dates_ends_durations_and_seconds_are_validated() {
    let c = parse(
        &[
            entry("DTSTART:20261103\nDURATION:P1W"),
            entry("DTSTART:20261103T100000\nDURATION:+P1DT2H3M4S"),
            entry("DTSTART:20261103T100000\nDTEND:20261103T110000\nDURATION:PT2H"),
        ]
        .concat(),
    );
    assert_eq!(
        c.events,
        [
            event("", "2026-11-03", "2026-11-09", true),
            event("", "2026-11-03T10:00", "2026-11-03T11:00", false),
            event("", "2026-11-03T10:00", "2026-11-04T12:03", false)
        ]
    );
}
#[test]
fn private_titles_and_places_are_counted_per_occurrence() {
    let bytes = format!(
        "BEGIN:VCALENDAR\n{}{}{}END:VCALENDAR\n",
        entry("DTSTART:20261103T100000\nSUMMARY:Home errand"),
        entry("DTSTART:20261104T100000\nLOCATION:Vault code"),
        entry("DTSTART:20261105T100000\nSUMMARY:Team planning")
    );
    let c = calendar::read(
        &[bytes.into_bytes()],
        &Zone::utc(),
        Path::new(ZONEINFO),
        window(),
        &filters("Home errand", "Vault code"),
    )
    .unwrap();
    assert_eq!((c.events.len(), c.private_left_out), (1, 2));
}
#[test]
fn cleaning_is_bounded_and_removes_hidden_controls() {
    let c = parse(&[entry("DTSTART:20261103T100000\nSUMMARY:  Team\u{1}\u{2028}\u{202e} planning \\n next  \nLOCATION: \\n "),
        entry(&format!("DTSTART:20261104T100000\nSUMMARY:{}\nLOCATION:{}", "é".repeat(250), "室".repeat(201)))].concat());
    assert_eq!(c.events.len(), 2);
    assert_eq!(c.events[0].title, "Team planning next");
    assert_eq!(c.events[0].place, None);
    assert_eq!(c.events[1].title, format!("{}…", "é".repeat(200)));
    assert_eq!(c.events[1].place, Some(format!("{}…", "室".repeat(200))));
}
#[test]
fn soft_hyphens_are_dropped_and_tag_characters_blanked() {
    let c = parse(&[
        entry("DTSTART:20261103T100000\nSUMMARY:Ver\u{ad}sicherung"),
        entry("DTSTART:20261104T100000\nSUMMARY:Lunch\u{e0069}\u{e0067}\u{e006e}\nLOCATION:Work\u{e0100}room"),
    ].concat());
    let mut lunch = event("Lunch", "2026-11-04T10:00", "2026-11-04T10:00", false);
    lunch.place = Some("Work room".into());
    assert_eq!(
        c.events,
        vec![
            event(
                "Versicherung",
                "2026-11-03T10:00",
                "2026-11-03T10:00",
                false
            ),
            lunch
        ]
    );
}
#[test]
fn output_is_sorted_and_cut_after_filtering() {
    let mut body = String::new();
    for n in (0..22).rev() {
        let count = if n == 21 { 90 } else { 91 };
        body += &entry(&format!(
            "DTSTART:20261102T100000\nRRULE:FREQ=DAILY;COUNT={count}\nSUMMARY:Team {n:02}"
        ));
    }
    let mut w = window();
    w.next_days = 92;
    let c = parse_at(&body, "UTC", w);
    assert_eq!(c.events.len(), 2000);
    assert!(c.more);
    assert_eq!(
        (c.events[0].start.as_str(), c.events[0].title.as_str()),
        ("2026-11-02T10:00", "Team 00")
    );
    assert_eq!(
        (c.events[1999].start.as_str(), c.events[1999].title.as_str()),
        ("2027-01-31T10:00", "Team 19")
    );
}
#[test]
fn bad_files_refuse_the_whole_read_and_long_entry_lines_are_counted() {
    let long = "x".repeat(65537);
    for bytes in [
        vec![0xff],
        b"not a calendar".to_vec(),
        b"BEGIN:VCALENDAR\nBEGIN:VEVENT\nEND:VCALENDAR".to_vec(),
        b"BEGIN:VCALENDAR\nEND:VEVENT\nEND:VCALENDAR".to_vec(),
        format!("BEGIN:VCALENDAR\n{long}\nEND:VCALENDAR").into_bytes(),
    ] {
        let result = calendar::read(
            &[bytes],
            &Zone::utc(),
            Path::new(ZONEINFO),
            window(),
            &filters("a^", "a^"),
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Voice couldn't read this calendar. Check it's a calendar you exported (an .ics file), then choose it again."
        );
    }
    let c = parse(&entry(&format!(
        "DTSTART:20261103T100000\nDESCRIPTION:{long}"
    )));
    assert_eq!((c.events.len(), c.not_understood), (0, 1));
}
#[test]
fn ancient_rules_skip_ahead_and_component_caps_are_enforced() {
    let c = parse(&entry("DTSTART:00010101T100000\nRRULE:FREQ=DAILY"));
    assert_eq!((c.events.len(), c.not_understood), (15, 0));
    let body = "BEGIN:X\nEND:X\n".repeat(200_000);
    let result = calendar::read(
        &[format!("BEGIN:VCALENDAR\n{body}END:VCALENDAR\n").into_bytes()],
        &Zone::utc(),
        Path::new(ZONEINFO),
        window(),
        &filters("a^", "a^"),
    );
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "This calendar is bigger than Voice reads (128 MB). Export a shorter stretch of time, then choose it again."
    );
}
#[test]
fn occurrences_past_a_dst_footer_are_not_understood() {
    let w = Window {
        today: Date {
            year: 2040,
            month: 1,
            day: 1,
        },
        past_days: 0,
        next_days: 14,
    };
    let c = parse_at(
        &entry("DTSTART;TZID=America/New_York:20400102T100000"),
        "UTC",
        w,
    );
    assert_eq!((c.events.len(), c.not_understood), (0, 1));
}

#[test]
fn exclusions_also_remove_moved_occurrences_with_old_recurrence_ids() {
    let c = parse(&[
        entry("UID:series\nDTSTART:20261001T100000\nRRULE:FREQ=DAILY;COUNT=2\nEXDATE:20261002T100000"),
        entry("UID:series\nRECURRENCE-ID:20261002T100000\nDTSTART:20261104T140000\nSUMMARY:Moved"),
    ].concat());
    assert_eq!(c.events.len(), 0);
}
#[test]
fn filters_check_the_whole_cleaned_title_and_place_before_display_cutoff() {
    let body = [
        entry(&format!(
            "DTSTART:20261103T100000\nSUMMARY:{} private errand",
            "x".repeat(210)
        )),
        entry(&format!(
            "DTSTART:20261104T100000\nLOCATION:{} vault code",
            "x".repeat(210)
        )),
    ]
    .concat();
    let c = calendar::read(
        &[format!("BEGIN:VCALENDAR\n{body}END:VCALENDAR\n").into_bytes()],
        &Zone::utc(),
        Path::new(ZONEINFO),
        window(),
        &filters("private errand", "vault code"),
    )
    .unwrap();
    assert_eq!((c.events.len(), c.private_left_out), (0, 2));
}
#[test]
fn until_stops_old_series_before_the_candidate_cap_or_a_moved_override() {
    let c = parse(&entry(
        "DTSTART:00010101T100000\nRRULE:FREQ=DAILY;UNTIL=00010103T100000",
    ));
    assert_eq!((c.events.len(), c.not_understood), (0, 0));
    let c = parse(&[
        entry("UID:series\nDTSTART:20261001T100000\nRRULE:FREQ=DAILY;UNTIL=20261003T100000"),
        entry("UID:series\nRECURRENCE-ID:20261010T100000\nDTSTART:20261103T100000\nSUMMARY:Not in series"),
    ].concat());
    assert_eq!(c.events.len(), 0);
}
#[test]
fn an_override_moved_into_the_window_from_after_the_expansion_horizon_is_kept() {
    let c = parse(&[
        entry("UID:series\nDTSTART:20261102T100000\nRRULE:FREQ=DAILY;COUNT=30"),
        entry("UID:series\nRECURRENCE-ID:20261125T100000\nDTSTART:20261103T120000\nSUMMARY:Moved in"),
    ].concat());
    assert_eq!(c.events.iter().filter(|e| e.title == "Moved in").count(), 1);
    assert_eq!(c.not_understood, 0);
}
#[test]
fn malformed_zone_footers_are_refused() {
    let scratch = Scratch::new("calendar-zone-footer");
    let bytes = fs::read(Path::new(ZONEINFO).join("America/New_York")).unwrap();
    let footer = bytes[..bytes.len() - 1]
        .iter()
        .rposition(|b| *b == b'\n')
        .unwrap();
    let mut bad = bytes[..footer + 1].to_vec();
    bad.extend_from_slice(b"nonsense,wat\n");
    fs::write(scratch.0.join("Malformed"), bad).unwrap();
    assert!(Zone::load(&scratch.0, "Malformed").is_none());
}
#[test]
fn an_empty_zone_footer_leaves_future_offsets_unavailable() {
    let scratch = Scratch::new("calendar-empty-footer");
    for transitions in [0u32, 1] {
        let mut bytes = Vec::new();
        for width in [4, 8] {
            bytes.extend_from_slice(b"TZif2");
            bytes.extend_from_slice(&[0; 15]);
            for count in [0u32, 0, 0, transitions, 1, 4] {
                bytes.extend_from_slice(&count.to_be_bytes());
            }
            if transitions == 1 {
                if width == 4 {
                    bytes.extend_from_slice(&100i32.to_be_bytes());
                } else {
                    bytes.extend_from_slice(&100i64.to_be_bytes());
                }
                bytes.push(0);
            }
            bytes.extend_from_slice(&3600i32.to_be_bytes());
            bytes.extend_from_slice(&[0, 0]);
            bytes.extend_from_slice(b"STD\0");
        }
        bytes.extend_from_slice(b"\n\n");
        fs::write(scratch.0.join("Empty"), bytes).unwrap();
        let zone = Zone::load(&scratch.0, "Empty").unwrap();
        assert_eq!(zone.offset_at(101), None);
        if transitions == 0 {
            assert_eq!(zone.offset_at(-1), None);
            assert_eq!(zone.to_utc(0), None);
        } else {
            assert_eq!(zone.offset_at(99), Some(3600));
            assert_eq!(zone.offset_at(100), Some(3600));
        }
    }
}
#[test]
fn oversized_footer_numbers_are_refused_without_panicking() {
    let scratch = Scratch::new("calendar-zone-number");
    let bytes = fs::read(Path::new(ZONEINFO).join("UTC")).unwrap();
    let footer = bytes[..bytes.len() - 1]
        .iter()
        .rposition(|b| *b == b'\n')
        .unwrap();
    let mut bad = bytes[..footer + 1].to_vec();
    bad.extend_from_slice(b"UTC2147483647\n");
    fs::write(scratch.0.join("Malformed"), bad).unwrap();
    let loaded = std::panic::catch_unwind(|| Zone::load(&scratch.0, "Malformed"));
    assert!(loaded.is_ok(), "malformed zone file must never panic");
    assert!(loaded.unwrap().is_none());
}

#[test]
fn a_read_shares_one_budget_and_counts_remaining_entries_by_window() {
    let mut body = entry("DTSTART:10000101T000000Z\nRRULE:FREQ=DAILY;COUNT=100000").repeat(51);
    body.push_str(&entry("DTSTART:20261103T100000Z\nSUMMARY:After budget"));
    body.push_str(&entry("DTSTART:20190101T100000Z\nSUMMARY:Old history"));
    let c = parse_at(&body, "UTC", window());
    assert_eq!(c.events.len(), 0);
    assert_eq!(c.not_understood, 28);
}

#[test]
fn properties_with_more_than_sixteen_parameters_are_not_understood() {
    for n in [16, 17] {
        let params: String = (0..n).map(|i| format!(";P{i}=value")).collect();
        let c = parse(&entry(&format!(
            "DTSTART{params}:20261103T100000\nSUMMARY:Team planning"
        )));
        assert_eq!(
            (c.events.len(), c.not_understood),
            if n == 16 { (1, 0) } else { (0, 1) }
        );
    }
}

#[test]
fn exdate_values_are_capped_across_all_properties_before_deduplication() {
    for n in [10_000, 10_001] {
        let mut body = String::from("DTSTART:20261103T100000\nRRULE:FREQ=DAILY;COUNT=3\n");
        // Each line stays under the independent 64 KiB unfolded-line cap.
        for chunk in (0..n).collect::<Vec<_>>().chunks(1000) {
            body.push_str("EXDATE:");
            body.push_str(&vec!["20261104T100000"; chunk.len()].join(","));
            body.push('\n');
        }
        let c = parse(&entry(&body));
        assert_eq!(
            (c.events.len(), c.not_understood),
            if n == 10_000 { (2, 0) } else { (0, 1) }
        );
    }
}

#[test]
fn every_new_hidden_character_becomes_a_space_in_titles() {
    for c in [
        '\u{061c}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{2061}', '\u{2062}',
        '\u{2063}', '\u{2064}', '\u{206a}', '\u{206b}', '\u{206c}', '\u{206d}', '\u{206e}',
        '\u{206f}', '\u{feff}',
    ] {
        let c = parse(&entry(&format!(
            "DTSTART:20261103T100000\nSUMMARY:Team{c}planning\nLOCATION:Room{c}4"
        )));
        assert_eq!(c.events[0].title, "Team planning");
        assert_eq!(c.events[0].place.as_deref(), Some("Room 4"));
    }
}

#[test]
fn overrides_match_berlin_occurrences_by_utc_instant() {
    let c = parse(&[
        entry("UID:series\nDTSTART;TZID=Europe/Berlin:20261103T100000\nRRULE:FREQ=DAILY;COUNT=3\nSUMMARY:Team planning"),
        entry("UID:series\nRECURRENCE-ID:20261105T090000Z\nDTSTART:20261105T090000Z\nSUMMARY:Cancelled planning\nSTATUS:CANCELLED"),
    ].concat());
    assert_eq!(c.not_understood, 0);
    assert_eq!(c.events.len(), 3);
    assert_eq!(
        c.events[2],
        Event {
            status: Some(Status::Cancelled),
            ..event(
                "Cancelled planning",
                "2026-11-05T04:00",
                "2026-11-05T04:00",
                false
            )
        }
    );
}

#[test]
fn utc_exdates_remove_berlin_occurrences_by_instant() {
    let c = parse(&entry(
        "DTSTART;TZID=Europe/Berlin:20261103T100000\nRRULE:FREQ=DAILY;COUNT=3\nEXDATE:20261104T090000Z\nSUMMARY:Team planning",
    ));
    assert_eq!(c.not_understood, 0);
    assert_eq!(starts(&c), ["2026-11-03T04:00", "2026-11-05T04:00"]);
}

#[test]
fn exception_value_types_must_match_the_series() {
    for (start, exdate) in [
        ("DTSTART;VALUE=DATE:20261103", "EXDATE:20261104T090000Z"),
        ("DTSTART:20261103T090000Z", "EXDATE;VALUE=DATE:20261104"),
    ] {
        let c = parse(&entry(&format!(
            "{start}\nRRULE:FREQ=DAILY;COUNT=3\n{exdate}"
        )));
        assert_eq!((c.events.len(), c.not_understood), (0, 1));
    }
}

#[test]
fn yearly_by_monthday_matches_the_start_day_with_or_without_bymonth() {
    let w = Window {
        next_days: 92,
        ..window()
    };
    for rule in [
        "FREQ=YEARLY;BYMONTHDAY=5",
        "FREQ=YEARLY;BYMONTH=11;BYMONTHDAY=5",
    ] {
        let c = parse_at(
            &entry(&format!(
                "DTSTART:20241105T100000\nRRULE:{rule}\nSUMMARY:Planning day"
            )),
            "UTC",
            w,
        );
        assert_eq!(c.not_understood, 0);
        assert_eq!(starts(&c), ["2026-11-05T10:00"]);
    }
}

#[test]
fn monthly_negative_monthdays_and_mixed_lists_skip_short_months() {
    let w = Window {
        today: Date {
            year: 2026,
            month: 1,
            day: 1,
        },
        next_days: 92,
        past_days: 0,
    };
    for (days, expected) in [
        (
            "-1",
            vec![
                "2026-01-01T10:00",
                "2026-01-31T10:00",
                "2026-02-28T10:00",
                "2026-03-31T10:00",
            ],
        ),
        ("-31", vec!["2026-01-01T10:00", "2026-03-01T10:00"]),
        (
            "1,-1,31",
            vec![
                "2026-01-01T10:00",
                "2026-01-31T10:00",
                "2026-02-01T10:00",
                "2026-02-28T10:00",
                "2026-03-01T10:00",
                "2026-03-31T10:00",
                "2026-04-01T10:00",
            ],
        ),
    ] {
        let c = parse_at(
            &entry(&format!(
                "DTSTART:20260101T100000\nRRULE:FREQ=MONTHLY;BYMONTHDAY={days};UNTIL=20260402T100000Z"
            )),
            "UTC",
            w,
        );
        assert_eq!(c.not_understood, 0);
        assert_eq!(starts(&c), expected);
    }
}
