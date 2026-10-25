use persona_builder::crew::{NewMember, new_member};
use persona_builder_save::text;
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
    fn path(&self) -> &str {
        self.member.to_str().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
fn success(out: Output, expected: &str) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap(), expected);
    assert!(out.stderr.is_empty());
}
fn refused(out: Output, expected: &str) {
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        format!("persona-builder: {expected}\n")
    );
}

#[test]
fn options_without_a_repo_prints_choices_recommendation_and_note() {
    let fixture = Fixture::new();
    success(
        fixture.run(&["save", "options", fixture.path()]),
        &format!(
            "1. {} (recommended)\n2. {}\n{}\n",
            text::CHOICE_GITHUB,
            text::CHOICE_LOCAL,
            text::NO_GITHUB_ACCOUNT
        ),
    );
}

#[test]
fn choose_local_then_push_keeps_everything_on_this_mac() {
    let fixture = Fixture::new();
    success(
        fixture.run(&["save", "choose", fixture.path(), "local"]),
        &format!("{}\n", text::CHOSE_LOCAL),
    );
    assert_eq!(
        fs::read(fixture.home.join(".claude/state/wren/save.json")).unwrap(),
        b"{\n  \"version\": 1,\n  \"mode\": \"local\"\n}\n"
    );
    success(
        fixture.run(&["save", "push", fixture.path()]),
        &format!("{}\n", text::KEPT_LOCAL),
    );
}

fn github_record(fixture: &Fixture) {
    let state = fixture.home.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("save.json"), b"{\"version\":1,\"mode\":\"github\",\"owner\":\"rowan-vale\",\"name\":\"wren-assistant\",\"branch\":\"main\"}").unwrap();
}
#[test]
fn clean_github_save_passes_privacy_before_sign_in() {
    let fixture = Fixture::new();
    github_record(&fixture);
    refused(
        fixture.run(&["save", "push", fixture.path()]),
        text::SIGN_IN_FIRST,
    );
}
fn private_github_save(ignored: bool) {
    let fixture = Fixture::new();
    github_record(&fixture);
    let folder = fixture.member.parent().unwrap();
    let path = folder.join("decisions.md");
    let mut body = fs::read_to_string(&path).unwrap();
    body.push_str("We moved it because my doctor called.\n");
    let line = body.lines().count();
    fs::write(path, body).unwrap();
    if ignored {
        let out = Command::new("git")
            .arg("-C")
            .arg(&fixture.home)
            .args(["init", "-q"])
            .output()
            .unwrap();
        assert!(out.status.success());
        fs::write(folder.join(".gitignore"), "decisions.md\n").unwrap();
    }
    refused(
        fixture.run(&["save", "push", fixture.path()]),
        &text::privacy_hits(&format!("decisions.md:{line} (personal)")),
    );
}
#[test]
fn private_github_save_is_refused_before_sign_in() {
    private_github_save(false);
}
#[test]
fn ignored_untracked_private_file_is_still_refused() {
    private_github_save(true);
}

#[test]
fn choose_github_asks_for_sign_in_before_anything_else() {
    let fixture = Fixture::new();
    refused(
        fixture.run(&["save", "choose", fixture.path(), "github", "x"]),
        text::SIGN_IN_FIRST,
    );
    assert!(!fixture.home.join(".claude/state/wren/save.json").exists());
}
