use persona_builder::crew::{Decisions, NewMember, new_member_embedded};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    home: PathBuf,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "cli-ceilings-{}-{}",
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
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env("HOME", self.home.join("home"))
            .env("CLAUDE_CONFIG_DIR", self.home.join("home/.claude"))
            .args(["ceilings", command])
            .arg(&self.dir)
            .args(args)
            .output()
            .unwrap()
    }
    fn member_bytes(&self) -> Vec<u8> {
        fs::read(self.dir.join("member.json")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
fn expected_show(mail: &str) -> String {
    format!(
        "claude-history\tanthropic\tdefault\tyour Claude history (your Claude Code sessions and your claude.ai export)\n\
crew-folder\tanthropic\tdefault\tyour assistant's own folder\n\
typed\tanthropic\tdefault\twhat you type into Persona Builder (interview answers, notes on answers, a panel's question)\n\
skill-mirror\tanthropic\tdefault\tthe copy of your skills (Persona Builder doesn't read this yet)\n\
{mail}\n\
calendar\tlocal-only\tdefault\tyour calendar (Persona Builder doesn't read this yet)\n\
contacts\tlocal-only\tdefault\tyour contacts (Persona Builder doesn't read this yet)\n\
family-skills\tlocal-only\tdefault\tyour family skills (Persona Builder doesn't read this yet)\n\
legal-skills\tlocal-only\tdefault\tyour legal skills (Persona Builder doesn't read this yet)\n\
Claude, through your own Claude Code: sends what it's given to Anthropic.\n"
    )
}

#[test]
fn ceilings_show_prints_the_nine_sources_and_provider_exactly() {
    let f = Fixture::new();
    let output = f.run("show", &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        expected_show("mail\tlocal-only\tdefault\tyour mail")
    );
}

#[test]
fn ceilings_set_writes_only_member_settings_and_show_marks_mail_set() {
    let f = Fixture::new();
    let private = f.dir.join("private.json");
    let private_bytes = br#"{"private_marker":"invented-private-setting","filters":{"personal":"invented-private-pattern"}}"#;
    fs::write(&private, private_bytes).unwrap();
    let before: Value = serde_json::from_slice(&f.member_bytes()).unwrap();
    let output = f.run("set", &["mail", "anthropic"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        b"your mail now stays on your own machines or with Anthropic.\n"
    );
    let mut expected = before;
    expected["ceilings"] = json!({"mail": "anthropic"});
    assert_eq!(
        serde_json::from_slice::<Value>(&f.member_bytes()).unwrap(),
        expected
    );
    assert_eq!(fs::read(private).unwrap(), private_bytes);
    let shown = f.run("show", &[]);
    assert!(shown.status.success());
    assert!(shown.stderr.is_empty());
    assert_eq!(
        String::from_utf8(shown.stdout).unwrap(),
        expected_show("mail\tanthropic\tset\tyour mail")
    );
}

#[test]
fn ceilings_set_unknown_source_is_refused_without_changing_member_json() {
    let f = Fixture::new();
    let before = f.member_bytes();
    let output = f.run("set", &["Mail", "anthropic"]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "persona-builder: member config: {}: ceilings names \"Mail\", which isn't a source. The sources are claude-history, crew-folder, typed, skill-mirror, mail, calendar, contacts, family-skills, legal-skills.\n",
            f.dir.join("member.json").display()
        )
    );
    assert!(output.stdout.is_empty());
    assert_eq!(f.member_bytes(), before);
}

#[test]
fn ceilings_set_unknown_ceiling_is_refused_without_changing_member_json() {
    let f = Fixture::new();
    let before = f.member_bytes();
    let output = f.run("set", &["mail", "elsewhere"]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "persona-builder: member config: {}: ceilings.mail must be local-only, anthropic or any-consented\n",
            f.dir.join("member.json").display()
        )
    );
    assert!(output.stdout.is_empty());
    assert_eq!(f.member_bytes(), before);
}

#[test]
fn ceilings_set_refuses_links_and_atomically_preserves_mode_without_staged_files() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    let f = Fixture::new();
    let path = f.dir.join("member.json");
    let target = f.home.join("target.json");
    fs::rename(&path, &target).unwrap();
    let original = fs::read(&target).unwrap();
    symlink(&target, &path).unwrap();
    let refused = f.run("set", &["mail", "anthropic"]);
    assert!(!refused.status.success());
    assert_eq!(fs::read(&target).unwrap(), original);
    fs::remove_file(&path).unwrap();
    fs::rename(&target, &path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    // Holding the old descriptor proves replacement rather than truncation in place.
    let old = fs::File::open(&path).unwrap();
    let mut names = fs::read_dir(&f.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    let output = f.run("set", &["mail", "anthropic"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let mut after = fs::read_dir(&f.dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect::<Vec<_>>();
    after.sort();
    assert_eq!(after, names, "no staged sibling remains");
    assert_ne!(
        old.metadata().unwrap().ino(),
        fs::metadata(&path).unwrap().ino(),
        "member.json must be staged and renamed"
    );
}

#[test]
fn ceilings_set_takes_the_builds_state_lock() {
    let f = Fixture::new();
    let home = f.home.join("home");
    fs::create_dir(&home).unwrap();
    let _lock = persona_builder_ingest::private::lock_state(&home, "ari").unwrap();
    let before = f.member_bytes();
    let output = f.run("set", &["mail", "anthropic"]);
    assert!(
        !output.status.success(),
        "must refuse while a build holds the lock"
    );
    assert_eq!(f.member_bytes(), before);
}
