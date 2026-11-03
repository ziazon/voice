use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_core::crew::{Decisions, NewMember, new_member_embedded};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    home: PathBuf,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "cli-grants-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let dir = home.join("ari");
        new_member_embedded(
            &dir,
            &NewMember {
                decisions: Decisions::Python,
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/assistants",
                date: "2026-10-31",
            },
        )
        .unwrap();
        Self { home, dir }
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_voice"))
            .env("HOME", self.home.join("home"))
            .env("CLAUDE_CONFIG_DIR", self.home.join("home/.claude"))
            .args(["grants", command])
            .arg(&self.dir)
            .args(args)
            .output()
            .unwrap()
    }
    fn state_home(&self) -> PathBuf {
        self.home.join("home")
    }
    fn settings(&self) -> PathBuf {
        self.state_home().join(".claude/state/ari/grants.json")
    }
    fn state(&self) {
        fs::create_dir_all(self.state_home()).unwrap();
        drop(voice_ingest::private::lock_state(&self.state_home(), "ari").unwrap());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
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
fn audit(f: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_voice"))
        .env("HOME", f.state_home())
        .env("CLAUDE_CONFIG_DIR", f.state_home().join(".claude"))
        .args(["audit", "show"])
        .arg(&f.dir)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn grants_exact_output_and_date_shape() {
    let f = Fixture::new();
    let before = fs::read(f.dir.join("member.json")).unwrap();
    assert_eq!(
        success(f.run("show", &[])),
        "calendar.read\toff\tread your calendar\n"
    );
    assert!(!f.state_home().exists());
    f.state();
    assert_eq!(
        success(f.run("give", &["calendar.read"])),
        "Voice may now read your calendar for Ari, from today to 14 days ahead.\n"
    );
    let shown = success(f.run("show", &[]));
    let day = shown
        .strip_prefix("calendar.read\ton\tread your calendar, from today to 14 days ahead, given ")
        .unwrap()
        .strip_suffix('\n')
        .unwrap();
    assert_eq!(day.len(), 10);
    assert!(day.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 {
        b == b'-'
    } else {
        b.is_ascii_digit()
    }));
    assert_eq!(
        success(f.run(
            "give",
            &["calendar.read", "--past-days", "3", "--next-days", "30"]
        )),
        "Voice may now read your calendar for Ari, from 3 days back to 30 days ahead.\n"
    );
    assert_eq!(
        success(f.run(
            "give",
            &["calendar.read", "--past-days", "1", "--next-days", "1"]
        )),
        "Voice may now read your calendar for Ari, from 1 day back to 1 day ahead.\n"
    );
    assert_eq!(
        success(f.run("revoke", &["calendar.read"])),
        "Voice may no longer read your calendar for Ari.\n"
    );
    assert_eq!(
        success(f.run("revoke", &["calendar.read"])),
        "Voice didn't have permission to read your calendar for Ari, so nothing changed.\n"
    );
    assert_eq!(
        success(f.run("show", &[])),
        "calendar.read\toff\tread your calendar\n"
    );
    assert!(!f.settings().exists());
    assert_eq!(fs::read(f.dir.join("member.json")).unwrap(), before);
}
#[test]
fn invalid_args_missing_state_and_locking() {
    let f = Fixture::new();
    failure(
        f.run("give", &["calendar.write"]),
        "calendar.write isn't something Voice can be given permission for. It can be given: calendar.read.",
    );
    failure(
        f.run("give", &["calendar.read", "--next-days", "0"]),
        "The calendar window goes from 0 to 31 days back and from 1 to 92 days ahead.",
    );
    failure(
        f.run("give", &["calendar.read"]),
        "This assistant has no private state yet. Build it once, then give permission.",
    );
    assert!(!f.state_home().exists());
    f.state();
    let _lock = voice_ingest::private::lock_existing_state(&f.state_home(), "ari").unwrap();
    for action in ["give", "revoke"] {
        failure(
            f.run(action, &["calendar.read"]),
            voice_ingest::private::BUSY,
        );
    }
    assert!(!f.settings().exists());
    assert_eq!(
        success(f.run("show", &[])),
        "calendar.read\toff\tread your calendar\n"
    );
}
#[test]
fn audit_output_newest_first_bounds_and_empty() {
    let f = Fixture::new();
    assert_eq!(
        success(audit(&f, &[])),
        "Nothing has been logged for Ari yet.\n"
    );
    assert!(!f.state_home().exists());
    failure(
        audit(&f, &["--last", "1001"]),
        "Show between 1 and 1,000 entries.",
    );
    f.state();
    success(f.run("give", &["calendar.read"]));
    success(f.run("revoke", &["calendar.read"]));
    let output = success(audit(&f, &[]));
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert_eq!(&line[16..22], " UTC  ");
        assert_eq!(line.as_bytes()[10], b' ');
    }
    assert_eq!(&lines[0][22..], "taken back: read your calendar");
    assert_eq!(
        &lines[1][22..],
        "given: read your calendar, from today to 14 days ahead"
    );
    let last = success(audit(&f, &["--last", "1"]));
    assert_eq!(last, format!("{}\nOlder entries aren't shown.\n", lines[0]));
}
#[test]
fn audit_exact_read_refusal_and_unreadable_sentences() {
    let f = Fixture::new();
    f.state();
    let bytes=b"{\"at\":\"2026-11-02T12:00:00Z\",\"event\":\"read\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"items\":1}\n{\"at\":\"2026-11-02T12:01:00Z\",\"event\":\"read\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"items\":12}\n{\"at\":\"2026-11-02T12:02:00Z\",\"event\":\"refused\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"reason\":\"no-grant\"}\n{\"at\":\"2026-11-02T12:03:00Z\",\"event\":\"refused\",\"capability\":\"calendar.read\",\"step\":\"cli\",\"reason\":\"damaged\"}\nbad\n";
    voice_ingest::private::write_private(&f.state_home(), "ari", "audit.jsonl", bytes).unwrap();
    assert_eq!(
        success(audit(&f, &[])),
        "2026-11-02 12:03 UTC  refused: read your calendar, for cli, the permissions file is damaged\n2026-11-02 12:02 UTC  refused: read your calendar, for cli, no permission\n2026-11-02 12:01 UTC  read: read your calendar, 12 items, for cli\n2026-11-02 12:00 UTC  read: read your calendar, 1 item, for cli\n1 entry couldn't be read.\n"
    );
    voice_ingest::private::write_private(&f.state_home(), "ari", "audit.jsonl", b"bad\nbad\n")
        .unwrap();
    assert_eq!(success(audit(&f, &[])), "2 entries couldn't be read.\n");
}
