#![cfg(target_os = "macos")]

use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_api::{dto::*, reader_client, reader_client::Place, *};
use voice_ingest::private;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const CANARY: &str = "canary-outside";
const SKIP: &str =
    "sandbox_profile: skipped, already inside a sandbox (macOS refuses a sandbox inside one)";
const PERMISSION: &str =
    "Voice needs your permission before it reads anything. Give it in the first step.";
const EXPORT_BAD: &str = "Voice couldn't read this download. Check it's the file claude.ai sent you, then choose it again.";

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

struct Home {
    root: PathBuf,
    path: PathBuf,
    runs: PathBuf,
    outside: PathBuf,
}
impl Home {
    fn new(permission: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "voice-reader-app-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("home");
        fs::create_dir_all(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        let runs = root.join("runs");
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        new_assistant_in(
            &path,
            NewAssistantRequest {
                name: "Pebble".into(),
                pronouns: "they/them".into(),
                principal: "Rowan Vale".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        if permission {
            agree_consent_in(&path, slug()).unwrap();
        }
        drop(private::lock_state(&path, "pebble").unwrap());
        Self {
            root,
            path,
            runs,
            outside,
        }
    }
    fn state(&self) -> PathBuf {
        self.path.join(".claude/state/pebble")
    }
    fn place(&self) -> Place<'_> {
        Place {
            program: Path::new(env!("CARGO_BIN_EXE_voice")),
            runs: &self.runs,
        }
    }
    fn no_runs(&self) {
        match fs::read_dir(&self.runs) {
            Ok(mut entries) => assert!(entries.next().is_none(), "reader left a run folder"),
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::NotFound),
        }
    }
    fn code_history(&self) {
        let folder = self.path.join(".claude/projects/project-one");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("session.jsonl"),
            session("Keep the reading list short."),
        )
        .unwrap();
        let outside = self.outside.join("session.jsonl");
        fs::write(&outside, session(CANARY)).unwrap();
        fs::hard_link(outside, folder.join("linked.jsonl")).unwrap();
    }
    fn export(&self) -> PathBuf {
        let folder = self.path.join("Downloads/export");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("conversations.json"),
            json!([
                {"uuid":"c-1", "name":"Reading list", "created_at":"2026-03-01T09:00:00Z",
                 "chat_messages":[{"sender":"human", "created_at":"2026-03-01T09:00:00Z",
                                   "text":"Keep the reading list short."}]},
                {"uuid":"c-2", "name":"Notes", "created_at":"2026-03-02T09:00:00Z",
                 "chat_messages":[{"sender":"human", "created_at":"2026-03-02T09:00:00Z",
                                   "text":"Use a short outline before writing."}]}
            ])
            .to_string(),
        )
        .unwrap();
        folder
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.no_runs();
            no_canary_in(&self.state());
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn slug() -> SlugRequest {
    SlugRequest {
        slug: "pebble".into(),
    }
}
fn read(dry_run: bool) -> ReadRequest {
    ReadRequest {
        slug: "pebble".into(),
        dry_run,
    }
}
fn session(text: &str) -> String {
    format!(
        "{}\n",
        json!({"type":"user", "timestamp":"2026-01-03T10:00:00.000Z",
               "origin":{"kind":"human"}, "message":{"content":text}})
    )
}
fn no_canary(bytes: &[u8]) {
    assert!(
        !bytes
            .windows(CANARY.len())
            .any(|word| word == CANARY.as_bytes())
    );
}
fn no_canary_in(folder: &Path) {
    for entry in fs::read_dir(folder).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            no_canary_in(&entry.path());
        } else if kind.is_file() {
            no_canary(&fs::read(entry.path()).unwrap());
        }
    }
}
fn progress_finished(progress: &[ReadProgress]) {
    let last = progress.last().expect("reader relayed progress");
    assert_eq!(last.done, last.total);
}

#[test]
fn claude_code_folders_dry_run_and_saved_read_match_the_api() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    h.code_history();
    let expected = claude_code_folders_in(&h.path, slug()).unwrap();
    assert_eq!(expected.folders[0].conversations, 1);
    let answer = reader_client::claude_code_folders(&h.path, &h.place(), slug()).unwrap();
    no_canary(&serde_json::to_vec(&answer).unwrap());
    assert_eq!(answer, expected);
    h.no_runs();
    for dry_run in [true, false] {
        let expected = read_claude_code_in(&h.path, read(dry_run), &mut |_| {}).unwrap();
        assert_eq!(expected.private_left_out, 1);
        let saved = h.state().join("decisions-claude-code.jsonl");
        let expected_saved = (!dry_run).then(|| fs::read(&saved).unwrap());
        if !dry_run {
            fs::remove_file(&saved).unwrap();
        }
        let mut progress = vec![];
        let answer =
            reader_client::read_claude_code(&h.path, &h.place(), read(dry_run), &mut |p| {
                progress.push(p)
            })
            .unwrap();
        no_canary(&serde_json::to_vec(&answer).unwrap());
        assert_eq!(answer, expected);
        progress_finished(&progress);
        assert_eq!(saved.exists(), !dry_run);
        if let Some(expected) = expected_saved {
            assert_eq!(fs::read(saved).unwrap(), expected);
        }
        no_canary_in(&h.state());
        h.no_runs();
    }
}

#[test]
fn missing_projects_returns_empty_answers_without_a_run_folder() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    assert_eq!(
        reader_client::claude_code_folders(&h.path, &h.place(), slug()).unwrap(),
        ClaudeCodeFolders {
            found: false,
            folders: vec![]
        }
    );
    assert_eq!(
        reader_client::read_claude_code(&h.path, &h.place(), read(false), &mut |_| {}).unwrap(),
        ReadCounts {
            questions: 0,
            held_out: 0,
            messages: 0,
            private_left_out: 0,
            unreadable: 0,
            first: None,
            last: None,
        }
    );
    h.no_runs();
}

#[test]
fn every_step_checks_permission_before_starting_the_reader() {
    if !available() {
        return;
    }
    let h = Home::new(false);
    h.code_history();
    assert_eq!(
        reader_client::claude_code_folders(&h.path, &h.place(), slug())
            .unwrap_err()
            .message,
        PERMISSION
    );
    assert_eq!(
        reader_client::read_claude_code(&h.path, &h.place(), read(true), &mut |_| {})
            .unwrap_err()
            .message,
        PERMISSION
    );
    assert_eq!(
        reader_client::claudeai_export_status(&h.path, &h.place(), slug())
            .unwrap_err()
            .message,
        PERMISSION
    );
    assert_eq!(
        reader_client::list_claudeai_chats(&h.path, &h.place(), slug())
            .unwrap_err()
            .message,
        PERMISSION
    );
    assert_eq!(
        reader_client::read_claudeai(&h.path, &h.place(), slug(), &mut |_| {})
            .unwrap_err()
            .message,
        PERMISSION
    );
    assert!(!h.runs.exists());
}

#[test]
fn export_choice_status_list_and_read_match_the_api() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let export = h.export();
    let expected = use_claudeai_export_in(&h.path, "pebble", &export, true).unwrap();
    fs::remove_file(h.state().join("claudeai-export.json")).unwrap();
    assert_eq!(
        reader_client::use_claudeai_export(&h.path, &h.place(), "pebble", &export, true).unwrap(),
        expected
    );
    assert_eq!(
        reader_client::claudeai_export_status(&h.path, &h.place(), slug()).unwrap(),
        claudeai_export_status_in(&h.path, slug()).unwrap()
    );
    let expected = list_claudeai_chats_in(&h.path, slug()).unwrap();
    assert_eq!(expected.chats.len(), 2);
    fs::remove_file(h.state().join("claudeai-review.tsv")).unwrap();
    assert_eq!(
        reader_client::list_claudeai_chats(&h.path, &h.place(), slug()).unwrap(),
        expected
    );
    let marked = mark_claudeai_chats_in(
        &h.path,
        MarkChatsRequest {
            slug: "pebble".into(),
            mine: true,
            uuids: vec!["c-1".into(), "c-2".into()],
            unmarked: false,
        },
    )
    .unwrap();
    assert_eq!(
        reader_client::list_claudeai_chats(&h.path, &h.place(), slug()).unwrap(),
        marked
    );
    let expected = read_claudeai_in(&h.path, slug(), &mut |_| {}).unwrap();
    let saved = h.state().join("decisions-claudeai.jsonl");
    let expected_saved = fs::read(&saved).unwrap();
    fs::remove_file(&saved).unwrap();
    let mut progress = vec![];
    assert_eq!(
        reader_client::read_claudeai(&h.path, &h.place(), slug(), &mut |p| progress.push(p))
            .unwrap(),
        expected
    );
    progress_finished(&progress);
    assert_eq!(fs::read(saved).unwrap(), expected_saved);
}

#[test]
fn export_kind_is_checked_before_saving_and_again_before_reading() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let folder = h.export();
    let file = folder.join("conversations.json");
    for (path, kind) in [(&file, true), (&folder, false)] {
        assert_eq!(
            choose_claudeai_export_in(&h.path, "pebble", path, kind)
                .unwrap_err()
                .message,
            EXPORT_BAD
        );
        assert!(!h.state().join("claudeai-export.json").exists());
    }
    reader_client::use_claudeai_export(&h.path, &h.place(), "pebble", &folder, true).unwrap();
    let bytes = fs::read(file).unwrap();
    fs::remove_dir_all(&folder).unwrap();
    fs::write(&folder, bytes).unwrap();
    assert_eq!(
        reader_client::claudeai_export_status(&h.path, &h.place(), slug())
            .unwrap_err()
            .message,
        EXPORT_BAD
    );
}

#[test]
fn an_export_choice_without_its_kind_must_be_chosen_again() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let export = h.export();
    private::write_private(
        &h.path,
        "pebble",
        "claudeai-export.json",
        &serde_json::to_vec(&json!({"path":export})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        reader_client::claudeai_export_status(&h.path, &h.place(), slug())
            .unwrap_err()
            .message,
        EXPORT_BAD
    );
}

#[test]
fn a_successful_exit_without_a_protocol_answer_is_not_trusted() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    h.code_history();
    let program = h.outside.join("reader");
    fs::copy("/usr/bin/true", &program).unwrap();
    assert_eq!(
        reader_client::claude_code_folders(
            &h.path,
            &Place {
                program: &program,
                runs: &h.runs,
            },
            slug(),
        )
        .unwrap_err()
        .message,
        voice_api::reader::STOPPED
    );
}
