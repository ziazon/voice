#![cfg(target_os = "macos")]

use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
use voice_agent::reader::{Reader, Source};
use voice_api::{dto::*, *};
use voice_core::{deny::DenyList, member::read_member};
use voice_ingest::private;

static NEXT: AtomicUsize = AtomicUsize::new(0);
const CANARY: &str = "canary-outside";
const SKIP: &str =
    "sandbox_profile: skipped, already inside a sandbox (macOS refuses a sandbox inside one)";
const READ: &str = "Voice couldn't read your Claude Code history. Close it and open it again.";
const BUSY: &str =
    "Voice is already working on this assistant. Wait for it to finish, then try again.";
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
            "voice-reader-sandbox-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("home");
        fs::create_dir_all(&path).unwrap();
        let path = fs::canonicalize(path).unwrap();
        let runs = root.join("runs");
        let outside = root.join("outside");
        fs::create_dir(&runs).unwrap();
        fs::create_dir(&outside).unwrap();
        new_assistant_in(
            &path,
            &path.join(".claude"),
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
        // Reader::new needs the state folder even for the permission-refusal case.
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
    fn run(&self, step: &str, request: Value, sources: &[Source<'_>]) -> Output {
        let member_dir = self.path.join(".claude/crew/pebble");
        let member = read_member(&member_dir.join("member.json")).unwrap();
        let reader = Reader::new(
            Path::new(env!("CARGO_BIN_EXE_voice")),
            &self.path,
            &member_dir,
            &self.state(),
            &self.runs,
            sources,
            &DenyList::for_member(&member, &self.path).unwrap(),
        )
        .unwrap();
        let mut child = reader
            .command(&["reader", step])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.stderr.is_empty(), "{step}: {output:?}");
        no_canary(&output.stdout);
        no_canary_in(&self.state());
        output
    }
    fn code_history(&self) -> PathBuf {
        let projects = self.path.join(".claude/projects");
        let folder = projects.join("project-one");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("session.jsonl"),
            session("Keep the reading list short."),
        )
        .unwrap();
        let outside = self.outside.join("session.jsonl");
        fs::write(&outside, session(CANARY)).unwrap();
        fs::hard_link(outside, folder.join("linked.jsonl")).unwrap();
        projects
    }
    fn export(&self, linked: bool) -> PathBuf {
        let folder = self.path.join("Downloads/export");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("conversations.json");
        let text = json!([
            {"uuid":"c-1", "name":"Reading list", "created_at":"2026-03-01T09:00:00Z",
             "chat_messages":[{"sender":"human", "created_at":"2026-03-01T09:00:00Z",
                               "text":if linked { CANARY } else { "Keep the reading list short." }}]},
            {"uuid":"c-2", "name":"Notes", "created_at":"2026-03-02T09:00:00Z",
             "chat_messages":[{"sender":"human", "created_at":"2026-03-02T09:00:00Z",
                               "text":"Use a short outline before writing."}]}
        ])
        .to_string();
        if linked {
            let outside = self.outside.join("conversations.json");
            fs::write(&outside, text).unwrap();
            fs::hard_link(outside, &file).unwrap();
        } else {
            fs::write(file, text).unwrap();
        }
        folder
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn slug() -> SlugRequest {
    SlugRequest {
        slug: "pebble".into(),
    }
}
fn request() -> Value {
    json!({"slug":"pebble"})
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
fn lines(output: &Output) -> Vec<Value> {
    assert!(output.stdout.ends_with(b"\n"), "{output:?}");
    String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(|line| {
            assert!(line.len() <= 16 << 20);
            let line: Value = serde_json::from_str(line).unwrap();
            assert_eq!(line.as_object().unwrap().len(), 1);
            line
        })
        .collect()
}
fn result(output: Output) -> Value {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let lines = lines(&output);
    let (last, progress) = lines.split_last().unwrap();
    assert!(progress.iter().all(|line| line.get("progress").is_some()));
    last.get("result").expect("final result line").clone()
}
fn error(output: Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(lines(&output), vec![json!({"error":expected})]);
}

#[test]
fn claude_code_folders_dry_run_and_saved_read_match_the_api() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let projects = h.code_history();
    let sources = [Source::Folder(&projects)];
    let expected = claude_code_folders_in(&h.path, slug()).unwrap();
    assert_eq!(expected.folders[0].conversations, 1);
    assert_eq!(
        result(h.run("claude-code-folders", request(), &sources)),
        serde_json::to_value(expected).unwrap()
    );
    for dry_run in [true, false] {
        let expected = read_claude_code_in(&h.path, read(dry_run), &mut |_| {}).unwrap();
        assert_eq!(expected.private_left_out, 1);
        if !dry_run {
            fs::remove_file(h.state().join("decisions-claude-code.jsonl")).unwrap();
        }
        let output = h.run(
            "read-claude-code",
            json!({"slug":"pebble", "dryRun":dry_run}),
            &sources,
        );
        assert!(lines(&output).len() > 1);
        assert_eq!(result(output), serde_json::to_value(expected).unwrap());
        assert_eq!(
            h.state().join("decisions-claude-code.jsonl").exists(),
            !dry_run
        );
    }
}

#[test]
fn a_history_source_is_required_by_the_profile() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    h.code_history();
    assert!(read_claude_code_in(&h.path, read(true), &mut |_| {}).is_ok());
    error(
        h.run(
            "read-claude-code",
            json!({"slug":"pebble", "dryRun":true}),
            &[],
        ),
        READ,
    );
}

#[test]
fn lock_and_permission_checks_run_inside_the_reader() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let projects = h.code_history();
    let _lock = private::lock_state(&h.path, "pebble").unwrap();
    error(
        h.run(
            "read-claude-code",
            json!({"slug":"pebble", "dryRun":true}),
            &[Source::Folder(&projects)],
        ),
        BUSY,
    );
    let h = Home::new(false);
    let projects = h.code_history();
    error(
        h.run(
            "read-claude-code",
            json!({"slug":"pebble", "dryRun":true}),
            &[Source::Folder(&projects)],
        ),
        PERMISSION,
    );
}

#[test]
fn export_status_list_and_read_match_the_api() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let export = h.export(false);
    use_claudeai_export_in(&h.path, "pebble", &export, true).unwrap();
    let sources = [Source::Folder(&export)];
    let expected = claudeai_export_status_in(&h.path, slug()).unwrap();
    assert_eq!(
        result(h.run("claudeai-export-status", request(), &sources)),
        serde_json::to_value(expected).unwrap()
    );
    let expected = list_claudeai_chats_in(&h.path, slug()).unwrap();
    assert_eq!(expected.chats.len(), 2);
    assert_eq!(
        result(h.run("list-claudeai-chats", request(), &sources)),
        serde_json::to_value(expected).unwrap()
    );
    mark_claudeai_chats_in(
        &h.path,
        MarkChatsRequest {
            slug: "pebble".into(),
            mine: true,
            uuids: vec!["c-1".into(), "c-2".into()],
            unmarked: false,
        },
    )
    .unwrap();
    let expected = read_claudeai_in(&h.path, slug(), &mut |_| {}).unwrap();
    assert_eq!(
        result(h.run("read-claudeai", request(), &sources)),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn folder_exports_refuse_hard_links_but_a_chosen_file_is_allowed() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let export = h.export(false);
    use_claudeai_export_in(&h.path, "pebble", &export, true).unwrap();
    fs::remove_file(export.join("conversations.json")).unwrap();
    fs::remove_dir(&export).unwrap();
    let export = h.export(true);
    error(
        h.run(
            "claudeai-export-status",
            request(),
            &[Source::Folder(&export)],
        ),
        EXPORT_BAD,
    );
    let file = export.join("conversations.json");
    use_claudeai_export_in(&h.path, "pebble", &file, false).unwrap();
    assert_eq!(
        result(h.run("claudeai-export-status", request(), &[Source::File(&file)]))["ready"],
        true
    );
}

#[test]
fn skills_list_agree_and_refresh_leave_hard_links_out() {
    if !available() {
        return;
    }
    let h = Home::new(true);
    let skills = h.path.join(".claude/skills");
    for (name, text) in [("notes", "Use short notes."), ("writing", "Write clearly.")] {
        let folder = skills.join(name);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("SKILL.md"), text).unwrap();
    }
    let outside = h.outside.join("notes.md");
    fs::write(&outside, CANARY).unwrap();
    fs::hard_link(outside, skills.join("notes/notes.md")).unwrap();
    let sources = [Source::Folder(&skills)];
    let expected = list_skills_in(&h.path, slug()).unwrap();
    assert_eq!(expected.skills.len(), 2);
    assert_eq!(
        result(h.run(
            "skills",
            json!({"slug":"pebble", "action":"list"}),
            &sources
        )),
        serde_json::to_value(expected).unwrap()
    );
    let agreed = result(h.run(
        "skills",
        json!({"slug":"pebble", "action":"agree", "ids":["claude:notes"]}),
        &sources,
    ));
    assert!(
        agreed["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|skill| skill["id"] == "claude:notes" && skill["state"] == "agreed")
    );
    let refreshed = result(h.run(
        "skills",
        json!({"slug":"pebble", "action":"refresh"}),
        &sources,
    ));
    assert!(refreshed["lines"].as_array().unwrap().iter().any(|line| {
        let line = line.as_str().unwrap();
        line.contains("notes.md") && line.contains("a hard link")
    }));
    assert!(h.state().join("mirror").is_dir());
    no_canary_in(&h.state());
}
