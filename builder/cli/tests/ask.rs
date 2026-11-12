use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_api::dto::*;

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    home: PathBuf,
    bin: PathBuf,
    question: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "voice-cli-ask-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let home = home.canonicalize().unwrap();
        voice_api::new_assistant_in(
            &home,
            &home.join(".claude"),
            NewAssistantRequest {
                name: "Wren".into(),
                pronouns: "she/her".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        voice_api::agree_consent_in(
            &home,
            SlugRequest {
                slug: "wren".into(),
            },
        )
        .unwrap();
        fs::write(
            home.join(".claude/crew/wren/decisions.md"),
            "Keep changes small.",
        )
        .unwrap();
        let question = home.join("q.txt");
        fs::write(&question, "What comes first?\nPlease suggest one thing.").unwrap();
        let bin = home.join("bin");
        fs::create_dir(&bin).unwrap();
        let python = Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        fs::write(bin.join("claude"),format!("#!{} -I\nimport json, os, sys\np = os.path.dirname(os.path.realpath(__file__))\na = sys.argv[3:]\nwith open(os.path.join(p, 'checked'), 'a') as f: f.write('run\\n')\nif a[:1] == ['--version']: print('2.1.292 (Claude Code)')\nelif a[:1] == ['auth']: print(json.dumps({{'loggedIn':True,'authMethod':'api_key'}}))\nelse:\n with open(os.path.join(p, 'stdin'), 'w') as f: f.write(sys.stdin.read())\n print(json.dumps({{'type':'result','subtype':'success','is_error':False,'total_cost_usd':0.02,'structured_output':{{'answer':'Try the smaller change.'}}}}))\n",String::from_utf8(python.stdout).unwrap().trim())).unwrap();
        fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            home,
            bin,
            question,
        }
    }
    fn run(&self, question: &std::path::Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_voice"))
            .env("HOME", &self.home)
            .env("PATH", &self.bin)
            .args(["ask"])
            .arg(self.home.join(".claude/crew/wren"))
            .arg("--question-file")
            .arg(question)
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
fn failure(output: Output, sentence: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("voice: {sentence}\n")
    );
}
#[test]
fn prints_answer_blank_line_and_disclosure_and_reads_whole_question() {
    let f = Fixture::new();
    let output = f.run(&f.question, &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Try the smaller change.\n\nClaude answered, through your own Claude Code. It was given Wren's decisions and your question, nothing else.\n"
    );
    assert!(
        fs::read_to_string(f.bin.join("stdin"))
            .unwrap()
            .contains("```text question\nWhat comes first?\nPlease suggest one thing.\n```")
    );
}
#[test]
fn missing_and_denied_question_files_have_one_fixed_sentence() {
    let f = Fixture::new();
    failure(
        f.run(&f.home.join("missing.txt"), &[]),
        voice_api::ask::text::READ_INPUT,
    );
    let denied = f.home.join(".ssh");
    fs::create_dir(&denied).unwrap();
    let denied = denied.join("q.txt");
    fs::write(&denied, "never read").unwrap();
    failure(f.run(&denied, &[]), voice_api::ask::text::READ_INPUT);
    assert!(!f.bin.join("stdin").exists());
}
#[test]
fn question_file_honours_members_denied_paths() {
    let f = Fixture::new();
    let file = f.home.join(".claude/crew/wren/member.json");
    let mut member: serde_json::Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    member["filters"]["denied_paths"] = serde_json::json!(["notes"]);
    fs::write(file, serde_json::to_vec(&member).unwrap()).unwrap();
    let notes = f.home.join("notes");
    fs::create_dir(&notes).unwrap();
    let question = notes.join("q.txt");
    fs::write(&question, "What comes first?").unwrap();

    failure(f.run(&question, &[]), voice_api::ask::text::READ_INPUT);
    assert!(!f.bin.join("stdin").exists());
    assert!(!f.bin.join("checked").exists());
}
#[test]
fn unknown_capability_uses_fixed_sentence() {
    let f = Fixture::new();
    failure(
        f.run(&f.question, &["--with", "mail.read"]),
        voice_api::permissions::UNKNOWN,
    );
    assert!(!f.bin.join("stdin").exists());
}
#[test]
fn repeated_with_is_accepted_and_guard_refuses_calendar_before_reading() {
    let f = Fixture::new();
    let output = f.run(
        &f.question,
        &["--with", "calendar.read", "--with", "calendar.read"],
    );
    let sentence = format!(
        "{} {}",
        voice_agent::text::kept_home(
            voice_core::ceilings::Source::Calendar,
            voice_core::ceilings::Ceiling::LocalOnly,
            voice_agent::claude::AGENT_NAME,
            voice_agent::provider::Reach::Anthropic
        ),
        voice_agent::text::SELF_HOSTED_HINT
    );
    failure(output, &sentence);
    assert!(!f.bin.join("stdin").exists());
}
