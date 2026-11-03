#![cfg(target_os = "macos")]

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};
use voice_core::crew::{Decisions, NewMember, new_member_embedded};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const SKIP: &str =
    "sandbox_profile: skipped, already inside a sandbox (macOS refuses a sandbox inside one)";

fn available() -> bool {
    let output = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .output()
        .expect("run sandbox-exec availability check");
    if output.status.success() {
        return true;
    }
    assert!(
        std::env::var("VOICE_REQUIRE_SANDBOX").as_deref() != Ok("1"),
        "{SKIP}"
    );
    eprintln!("{SKIP}");
    false
}

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cli-calendar-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let home = root.join("home");
        let member = home.join(".claude/crew/ari");
        fs::create_dir_all(member.parent().unwrap()).unwrap();
        new_member_embedded(
            &member,
            &NewMember {
                decisions: Decisions::Python,
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/assistants",
                date: "2026-11-02",
            },
        )
        .unwrap();
        drop(voice_ingest::private::lock_state(&home, "ari").unwrap());
        Self { root, home, member }
    }
    fn run(&self, command: &str, action: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_voice"))
            .env("HOME", &self.home)
            .env("CLAUDE_CONFIG_DIR", self.home.join(".claude"))
            .args([command, action])
            .arg(&self.member)
            .args(args)
            .output()
            .unwrap()
    }
    fn export(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.home.join(name);
        fs::write(&path, contents).unwrap();
        path
    }
    fn choose(&self, path: &std::path::Path) -> String {
        success(self.run("calendar", "choose", &[path.to_str().unwrap()]))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}
fn failure(output: Output, sentence: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("voice: {sentence}\n")
    );
}

#[test]
fn choose_file_and_folder_print_exact_lines() {
    let f = Fixture::new();
    let file = f.export("calendar.ICS", "BEGIN:VCALENDAR\nEND:VCALENDAR\n");
    assert_eq!(
        f.choose(&file),
        "Voice will read your calendar for Ari from calendar.ICS.\n"
    );
    let folder = f.home.join("exports");
    fs::create_dir(&folder).unwrap();
    assert_eq!(
        f.choose(&folder),
        "Voice will read your calendar for Ari from exports.\n"
    );
}

#[test]
fn choose_cleans_the_file_name_for_one_line() {
    let f = Fixture::new();
    let file = f.export("cal\n\u{202e}endar.ics", "BEGIN:VCALENDAR\nEND:VCALENDAR\n");
    assert_eq!(
        f.choose(&file),
        "Voice will read your calendar for Ari from cal  endar.ics.\n"
    );
}

#[test]
fn show_without_choice_asks_for_one() {
    if !available() {
        return;
    }
    let f = Fixture::new();
    failure(
        f.run("calendar", "show", &[]),
        "Choose the calendar file you exported first.",
    );
}

#[test]
fn show_without_grant_logs_one_refusal() {
    if !available() {
        return;
    }
    let f = Fixture::new();
    let file = f.export("calendar.ics", "BEGIN:VCALENDAR\nEND:VCALENDAR\n");
    f.choose(&file);
    failure(
        f.run("calendar", "show", &[]),
        "Voice doesn't have your permission to read your calendar for this assistant.",
    );
    let audit = success(f.run("audit", "show", &[]));
    assert_eq!(audit.lines().count(), 1);
    assert!(audit.ends_with("refused: read your calendar, for cli, no permission\n"));
}

fn day(offset: i64) -> String {
    let today = voice_agent::ledger::local_day(SystemTime::now());
    let y = today[..4].parse().unwrap();
    let m = today[5..7].parse().unwrap();
    let d = today[8..].parse().unwrap();
    let (y, m, d) = voice_ingest::dates::civil_from_days(
        voice_ingest::dates::days_from_civil(y, m, d) + offset,
    );
    format!("{y:04}-{m:02}-{d:02}")
}
fn compact(day: &str) -> String {
    day.replace('-', "")
}

#[test]
fn show_uses_singular_and_plural_for_entries_not_understood() {
    if !available() {
        return;
    }
    for (count, sentence) in [
        (1, "1 couldn't be understood, so Voice left it out."),
        (2, "2 couldn't be understood, so Voice left them out."),
    ] {
        let f = Fixture::new();
        let entry = format!(
            "BEGIN:VEVENT\nDTSTART:{}T100000\nRRULE:FREQ=HOURLY\nSUMMARY:Team planning\nEND:VEVENT\n",
            compact(&day(1))
        );
        let file = f.export(
            "calendar.ics",
            &format!("BEGIN:VCALENDAR\n{}END:VCALENDAR\n", entry.repeat(count)),
        );
        f.choose(&file);
        success(f.run("grants", "give", &["calendar.read"]));
        assert_eq!(
            success(f.run("calendar", "show", &[])),
            format!("No events from today to 14 days ahead.\n{sentence}\n")
        );
    }
}

#[test]
fn show_reads_only_allowed_fields_and_window_and_logs_four_items() {
    if !available() {
        return;
    }
    let f = Fixture::new();
    let first = day(1);
    let second = day(2);
    let third = day(3);
    let fourth = day(4);
    let outside = day(15);
    let contents = format!(
        "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART:{}T100000\nDTEND:{}T110000\nSUMMARY:Team planning\nDESCRIPTION:canary-outside\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:{}\nSUMMARY:Reading day\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:{}\nDTEND;VALUE=DATE:{}\nSUMMARY:Workshop\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:{}T140000\nDTEND:{}T150000\nSUMMARY:Draft review\nLOCATION:Room 4\nSTATUS:TENTATIVE\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:{}T100000\nSUMMARY:Outside window\nEND:VEVENT\nBEGIN:VEVENT\nDTSTART:{}T160000\nSUMMARY:Private appointment\nEND:VEVENT\nEND:VCALENDAR\n",
        compact(&first),
        compact(&first),
        compact(&second),
        compact(&second),
        compact(&fourth),
        compact(&third),
        compact(&third),
        compact(&outside),
        compact(&third)
    );
    let config = f.member.join("member.json");
    let mut member: serde_json::Value =
        serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    member["filters"]["personal"] = "Private appointment".into();
    fs::write(config, serde_json::to_vec(&member).unwrap()).unwrap();
    let file = f.export("calendar.ics", &contents);
    f.choose(&file);
    success(f.run("grants", "give", &["calendar.read"]));
    let output = f.run("calendar", "show", &[]);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("canary-outside"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("canary-outside"));
    assert_eq!(
        success(output),
        format!(
            "{first} 10:00–11:00  Team planning\n{second}  all day  Reading day\n{second} to {third}  all day  Workshop\n{third} 14:00–15:00  Draft review, Room 4 (tentative)\n4 events from today to 14 days ahead.\n1 left out because it looks private.\n"
        )
    );
    let audit = success(f.run("audit", "show", &[]));
    assert!(
        audit
            .lines()
            .next()
            .unwrap()
            .ends_with("read: read your calendar, 4 items, for cli")
    );
    assert_eq!(audit.lines().count(), 2);
}
