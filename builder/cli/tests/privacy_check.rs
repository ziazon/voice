use persona_builder::crew::{NewMember, new_member};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    home: PathBuf,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "pb6-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let member = home.join("crew/wren/member.json");
        new_member(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crew/_template"),
            member.parent().unwrap(),
            &NewMember {
                decisions: persona_builder::crew::Decisions::Python,
                name: "Wren",
                pronouns: "she/her",
                principal: "Rowan Vale",
                principal_pronouns: "they/them",
                repo_shown: "~/ai",
                date: "2026-10-24",
            },
        )
        .unwrap();
        Self { home, member }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_persona-builder"))
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(&self.home)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
fn check(f: &Fixture) -> Output {
    f.run(&[
        "privacy-check",
        f.member.parent().unwrap().to_str().unwrap(),
    ])
}
#[test]
fn template_prints_clean() {
    let f = Fixture::new();
    let out = check(&f);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"clean\n");
    assert!(out.stderr.is_empty());
}
#[test]
fn one_hit_prints_only_place_and_summary() {
    let f = Fixture::new();
    fs::write(
        f.member.parent().unwrap().join("decisions.md"),
        "We moved it because my doctor called.\n",
    )
    .unwrap();
    let out = check(&f);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"decisions.md:1 (personal)\n1 hits\n");
    assert!(out.stderr.is_empty());
}
#[test]
fn advice_precedes_summary() {
    let f = Fixture::new();
    let raw = fs::read_to_string(&f.member)
        .unwrap()
        .replace("my ", "our ");
    assert_ne!(raw, fs::read_to_string(&f.member).unwrap());
    fs::write(&f.member, raw).unwrap();
    let out = check(&f);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!(
            "member.json:filters.personal (differs from crew/_template)\n{}\n1 hits\n",
            persona_builder_ingest::check::ADVICE
        )
    );
    assert!(out.stderr.is_empty());
}
#[test]
fn empty_folder_names_missing_member() {
    let f = Fixture::new();
    let out = f.run(&["privacy-check", f.home.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        format!(
            "persona-builder: {}/member.json: no member.json to check\n",
            f.home.canonicalize().unwrap().display()
        )
    );
}
