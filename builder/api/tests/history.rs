use persona_builder_api::{dto::*, *};
use persona_builder_ingest::{
    claudeai, corpus,
    filters::Filters,
    mine::{self, Split},
    private::*,
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
const PERMISSION: &str =
    "Persona Builder needs your permission before it reads anything. Give it in the first step.";
const FIND: &str = "Persona Builder couldn't find that assistant. Close it and open it again.";
const BUSY: &str =
    "Persona Builder is already working on this assistant. Wait for it to finish, then try again.";
const EXPORT_BAD: &str = "Persona Builder couldn't read this download. Check it's the file claude.ai sent you, then choose it again.";
const EXPORT_GONE: &str =
    "Persona Builder can't find your claude.ai download any more. Choose it again.";
const NO_CHATS: &str = "Choose at least one chat first.";
const NOTHING: &str = "There's nothing to put together yet. Read your Claude Code history or your claude.ai chats first.";
struct Home(PathBuf);
impl Home {
    fn new(agree: bool) -> Self {
        let h = Self(std::env::temp_dir().join(format!(
            "pb-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&h.0).unwrap();
        new_assistant_in(
            &h.0,
            NewAssistantRequest {
                name: "Ari".into(),
                pronouns: "they/them".into(),
                principal: "Robin".into(),
                principal_pronouns: "they/them".into(),
            },
        )
        .unwrap();
        fs::write(
            h.0.join(".claude/crew/ari/private.json"),
            r#"{"filters":{"excluded_folders":"excluded"}}"#,
        )
        .unwrap();
        if agree {
            agree_consent_in(&h.0, slug("ari")).unwrap();
        }
        h
    }
    fn path(&self) -> &Path {
        &self.0
    }
    fn state(&self, name: &str) -> PathBuf {
        self.0.join(".claude/state/ari").join(name)
    }
    fn files(&self) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(self.state(""))
            .into_iter()
            .flatten()
            .map(|e| e.unwrap().file_name().to_str().unwrap().to_owned())
            .collect();
        names.sort();
        names
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn slug(s: &str) -> SlugRequest {
    SlugRequest { slug: s.into() }
}
fn read(dry_run: bool) -> ReadRequest {
    ReadRequest {
        slug: "ari".into(),
        dry_run,
    }
}
fn marks(uuids: &[&str], mine: bool, unmarked: bool) -> MarkChatsRequest {
    MarkChatsRequest {
        slug: "ari".into(),
        mine,
        uuids: uuids.iter().map(|s| (*s).into()).collect(),
        unmarked,
    }
}
fn safe<T: std::fmt::Debug>(h: &Home, result: Result<T, Failure>, expected: &str) {
    let message = result.unwrap_err().message;
    assert!(
        !message.contains(h.path().to_str().unwrap()),
        "private home leaked: {message}"
    );
    assert_eq!(message, expected);
}
fn every_command(h: &Home, s: &str, expected: &str) {
    let before = h.files();
    safe(h, can_read_in(h.path(), slug(s)), expected);
    safe(h, claude_code_folders_in(h.path(), slug(s)), expected);
    for dry_run in [true, false] {
        safe(
            h,
            read_claude_code_in(
                h.path(),
                ReadRequest {
                    slug: s.into(),
                    dry_run,
                },
                &mut |_| panic!("gate sent progress"),
            ),
            expected,
        );
    }
    safe(
        h,
        use_claudeai_export_in(h.path(), s, &h.path().join("absent")),
        expected,
    );
    safe(h, claudeai_export_status_in(h.path(), slug(s)), expected);
    safe(h, list_claudeai_chats_in(h.path(), slug(s)), expected);
    safe(
        h,
        mark_claudeai_chats_in(
            h.path(),
            MarkChatsRequest {
                slug: s.into(),
                ..marks(&[], false, false)
            },
        ),
        expected,
    );
    safe(
        h,
        read_claudeai_in(h.path(), slug(s), &mut |_| panic!("gate sent progress")),
        expected,
    );
    safe(h, merge_histories_in(h.path(), slug(s)), expected);
    assert_eq!(h.files(), before);
}
#[test]
fn every_command_requires_current_permission_without_writing() {
    let h = Home::new(false);
    every_command(&h, "ari", PERMISSION);
    write_private(
        h.path(),
        "ari",
        "consent.json",
        br#"{"version":0,"agreed_on":"2026-10-25"}"#,
    )
    .unwrap();
    every_command(&h, "ari", PERMISSION);
    for s in ["missing", "../x"] {
        every_command(&h, s, FIND);
    }
}
#[test]
fn every_command_refuses_history_elsewhere() {
    let h = Home::new(true);
    let p = h.path().join(".claude/crew/ari/private.json");
    fs::write(p, r#"{"state_dir":"/elsewhere"}"#).unwrap();
    every_command(
        &h,
        "ari",
        "This assistant keeps its history outside your Claude folder. Use the persona-builder command for it.",
    );
}
fn code_history(h: &Home, folders: usize) {
    for i in 0..folders {
        let folder = h.path().join(format!(".claude/projects/project-{i:04}"));
        fs::create_dir_all(&folder).unwrap();
        let records = [
            json!({"type":"assistant","message":{"content":"Which approach should we use?"}}),
            json!({"type":"user","timestamp":"2026-01-02T10:00:00.000Z","toolUseResult":{"questions":[{"question":"Choose an approach?","header":"Approach","options":[{"label":"Small","description":"Start small"}],"multiSelect":false}],"answers":{"Choose an approach?":"Small"}}}),
            json!({"type":"user","timestamp":"2026-01-03T10:00:00.000Z","origin":{"kind":"human"},"message":{"content":"Start with the smallest useful change."}}),
            json!({"type":"user","timestamp":"2026-01-04T10:00:00.000Z","origin":{"kind":"human"},"message":{"content":"My therapist asked about this."}}),
        ];
        fs::write(
            folder.join("session.jsonl"),
            records.iter().map(|r| format!("{r}\n")).collect::<String>(),
        )
        .unwrap();
    }
}
fn member(h: &Home) -> persona_builder::member::Member {
    persona_builder::member::read_member(&h.path().join(".claude/crew/ari/member.json")).unwrap()
}
#[test]
fn code_folders_count_and_exclusion() {
    let h = Home::new(true);
    assert_eq!(
        claude_code_folders_in(h.path(), slug("ari")).unwrap(),
        ClaudeCodeFolders {
            found: false,
            folders: vec![]
        }
    );
    code_history(&h, 1);
    let excluded = h.path().join(".claude/projects/excluded");
    fs::create_dir(&excluded).unwrap();
    for n in ["one.jsonl", "two.jsonl"] {
        fs::write(excluded.join(n), "").unwrap();
    }
    assert_eq!(
        claude_code_folders_in(h.path(), slug("ari")).unwrap(),
        ClaudeCodeFolders {
            found: true,
            folders: vec![
                ClaudeCodeFolder {
                    project: "excluded".into(),
                    conversations: 2,
                    read: false
                },
                ClaudeCodeFolder {
                    project: "project-0000".into(),
                    conversations: 1,
                    read: true
                }
            ]
        }
    );
}
#[test]
fn code_read_dry_run_parity_progress_and_lock() {
    let h = Home::new(true);
    let zeros = ReadCounts {
        questions: 0,
        held_out: 0,
        messages: 0,
        private_left_out: 0,
        first: None,
        last: None,
    };
    assert_eq!(
        read_claude_code_in(h.path(), read(false), &mut |_| {}).unwrap(),
        zeros
    );
    assert!(!h.state("decisions-claude-code.jsonl").exists());
    code_history(&h, 1);
    let before = h.files();
    let mut progress = vec![];
    let counts = read_claude_code_in(h.path(), read(true), &mut |p| progress.push(p)).unwrap();
    assert_eq!(h.files(), before);
    assert_eq!(
        counts,
        ReadCounts {
            questions: 1,
            held_out: 1,
            messages: 1,
            private_left_out: 1,
            first: Some("2026-01-02".into()),
            last: Some("2026-01-03".into())
        }
    );
    assert_eq!(
        progress,
        vec![
            ReadProgress { done: 0, total: 1 },
            ReadProgress { done: 1, total: 1 }
        ]
    );
    assert_eq!(
        read_claude_code_in(h.path(), read(false), &mut |_| {}).unwrap(),
        counts
    );
    let m = member(&h);
    let f = Filters::compile(&m.filters).unwrap();
    let expected = mine::mine(
        &m,
        &f,
        &h.path().join(".claude/projects"),
        h.path(),
        &Split::Holdout(0.15),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(h.state("decisions-claude-code.jsonl")).unwrap(),
        mine::to_jsonl(&expected.records)
    );
    assert_eq!(
        fs::metadata(h.state("decisions-claude-code.jsonl"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let _lock = lock_state(h.path(), "ari").unwrap();
    safe(
        &h,
        read_claude_code_in(h.path(), read(false), &mut |_| {}),
        BUSY,
    );
}
#[test]
fn code_progress_is_throttled_to_whole_percent() {
    let h = Home::new(true);
    code_history(&h, 250);
    let mut progress = vec![];
    read_claude_code_in(h.path(), read(true), &mut |p| progress.push(p)).unwrap();
    assert_eq!(
        progress.first(),
        Some(&ReadProgress {
            done: 0,
            total: 250
        })
    );
    assert_eq!(
        progress.last(),
        Some(&ReadProgress {
            done: 250,
            total: 250
        })
    );
    assert_eq!(progress.len(), 101);
    for pair in progress.windows(2) {
        assert_ne!(
            pair[0].done * 100 / pair[0].total,
            pair[1].done * 100 / pair[1].total
        );
    }
}
fn export(h: &Home) -> PathBuf {
    let folder = h.path().join("download");
    fs::create_dir(&folder).unwrap();
    let chats:Vec<_>=(0..3).map(|i|json!({"uuid":format!("chat-{i}"),"name":format!("Project {i}"),"created_at":"2026-02-01T10:00:00Z","chat_messages":[{"sender":"human","created_at":"2026-02-01T10:00:00Z","text":format!("Ship a small useful change in project {i}.")}]})).collect();
    fs::write(
        folder.join("conversations.json"),
        serde_json::to_vec(&chats).unwrap(),
    )
    .unwrap();
    folder
}
#[test]
fn export_status_and_safe_failures_never_contain_home_path() {
    let h = Home::new(true);
    assert_eq!(
        claudeai_export_status_in(h.path(), slug("ari")).unwrap(),
        None
    );
    let p = export(&h);
    let status = use_claudeai_export_in(h.path(), "ari", &p).unwrap();
    assert_eq!(
        status,
        ExportStatus {
            name: "download".into(),
            ready: true,
            chats: 3
        }
    );
    assert_eq!(
        claudeai_export_status_in(h.path(), slug("ari")).unwrap(),
        Some(status)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &read_private(h.path(), "ari", "claudeai-export.json").unwrap()
        )
        .unwrap(),
        json!({"path":p})
    );
    fs::rename(&p, h.path().join("moved")).unwrap();
    safe(
        &h,
        claudeai_export_status_in(h.path(), slug("ari")),
        EXPORT_GONE,
    );
    let bad = h.path().join("bad.txt");
    fs::write(&bad, "not an export").unwrap();
    safe(
        &h,
        use_claudeai_export_in(h.path(), "ari", &bad),
        EXPORT_BAD,
    );
    // A directory with no conversations.json makes the ingest error include the full path.
    let broken = h.path().join("broken");
    fs::create_dir(&broken).unwrap();
    write_private(
        h.path(),
        "ari",
        "claudeai-export.json",
        serde_json::to_vec(&json!({"path":broken}))
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    safe(
        &h,
        claudeai_export_status_in(h.path(), slug("ari")),
        EXPORT_BAD,
    );
    write_private(h.path(), "ari", "claudeai-export.json", b"broken record").unwrap();
    safe(
        &h,
        claudeai_export_status_in(h.path(), slug("ari")),
        EXPORT_BAD,
    );
}
#[test]
fn chat_review_round_trip_marks_survive_and_mark_is_locked() {
    let h = Home::new(true);
    let p = export(&h);
    safe(
        &h,
        list_claudeai_chats_in(h.path(), slug("ari")),
        EXPORT_GONE,
    );
    use_claudeai_export_in(h.path(), "ari", &p).unwrap();
    let listed = list_claudeai_chats_in(h.path(), slug("ari")).unwrap();
    assert_eq!((listed.mine, listed.not_mine, listed.unmarked), (0, 0, 3));
    assert_eq!(
        listed.chats[0],
        Chat {
            uuid: "chat-0".into(),
            title: "Project 0".into(),
            created: "2026-02-01".into(),
            messages: 1,
            mark: ChatMark::Unmarked
        }
    );
    let marked = mark_claudeai_chats_in(h.path(), marks(&["chat-0"], true, false)).unwrap();
    assert_eq!((marked.mine, marked.not_mine, marked.unmarked), (1, 0, 2));
    let marked = mark_claudeai_chats_in(h.path(), marks(&[], false, true)).unwrap();
    assert_eq!((marked.mine, marked.not_mine, marked.unmarked), (1, 2, 0));
    assert_eq!(
        list_claudeai_chats_in(h.path(), slug("ari")).unwrap(),
        marked
    );
    let rows = claudeai::read_review(
        "claudeai-review.tsv",
        &read_private(h.path(), "ari", "claudeai-review.tsv").unwrap(),
    )
    .unwrap();
    assert_eq!(
        rows.iter().map(|r| r.mine.as_str()).collect::<Vec<_>>(),
        vec!["y", "n", "n"]
    );
    safe(
        &h,
        mark_claudeai_chats_in(h.path(), marks(&[], true, false)),
        NO_CHATS,
    );
    let _lock = lock_state(h.path(), "ari").unwrap();
    safe(
        &h,
        mark_claudeai_chats_in(h.path(), marks(&["chat-0"], false, false)),
        BUSY,
    );
}
#[test]
fn writers_answer_busy_while_the_state_lock_is_held() {
    let h = Home::new(true);
    let p = export(&h);
    use_claudeai_export_in(h.path(), "ari", &p).unwrap();
    list_claudeai_chats_in(h.path(), slug("ari")).unwrap();
    mark_claudeai_chats_in(h.path(), marks(&["chat-1"], true, false)).unwrap();
    read_claudeai_in(h.path(), slug("ari"), &mut |_| {}).unwrap();
    code_history(&h, 1);
    read_claude_code_in(h.path(), read(false), &mut |_| {}).unwrap();
    merge_histories_in(h.path(), slug("ari")).unwrap();

    let _lock = lock_state(h.path(), "ari").unwrap();
    let before = h.files();
    let outputs: Vec<_> = [
        "claudeai-export.json",
        "claudeai-review.tsv",
        "decisions-claudeai.jsonl",
        "decisions.jsonl",
    ]
    .into_iter()
    .map(|name| (name, read_private(h.path(), "ari", name).unwrap()))
    .collect();
    let unchanged = || {
        assert_eq!(h.files(), before);
        for (name, bytes) in &outputs {
            assert_eq!(
                read_private(h.path(), "ari", name).unwrap(),
                *bytes,
                "{name} changed while locked"
            );
        }
    };
    safe(&h, use_claudeai_export_in(h.path(), "ari", &p), BUSY);
    unchanged();
    safe(&h, list_claudeai_chats_in(h.path(), slug("ari")), BUSY);
    unchanged();
    safe(
        &h,
        read_claudeai_in(h.path(), slug("ari"), &mut |_| {}),
        BUSY,
    );
    unchanged();
    safe(&h, merge_histories_in(h.path(), slug("ari")), BUSY);
    unchanged();
}
#[test]
fn reads_only_chosen_chats_and_merges_with_code_byte_for_byte() {
    let h = Home::new(true);
    safe(&h, merge_histories_in(h.path(), slug("ari")), NOTHING);
    safe(
        &h,
        read_claudeai_in(h.path(), slug("ari"), &mut |_| {}),
        EXPORT_GONE,
    );
    let p = export(&h);
    use_claudeai_export_in(h.path(), "ari", &p).unwrap();
    safe(
        &h,
        read_claudeai_in(h.path(), slug("ari"), &mut |_| {}),
        NO_CHATS,
    );
    list_claudeai_chats_in(h.path(), slug("ari")).unwrap();
    mark_claudeai_chats_in(h.path(), marks(&["chat-1"], true, false)).unwrap();
    let mut progress = vec![];
    assert_eq!(
        read_claudeai_in(h.path(), slug("ari"), &mut |p| progress.push(p)).unwrap(),
        ClaudeaiCounts {
            chats: 1,
            messages: 1,
            private_left_out: 0
        }
    );
    assert_eq!(progress.first(), Some(&ReadProgress { done: 0, total: 3 }));
    assert_eq!(progress.last(), Some(&ReadProgress { done: 3, total: 3 }));
    let bytes = read_private(h.path(), "ari", "decisions-claudeai.jsonl").unwrap();
    let records = corpus::read_records("decisions-claudeai.jsonl", &bytes).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].1["text"],
        "Ship a small useful change in project 1."
    );
    code_history(&h, 1);
    read_claude_code_in(h.path(), read(false), &mut |_| {}).unwrap();
    let counts = merge_histories_in(h.path(), slug("ari")).unwrap();
    assert_eq!(
        counts,
        MergeCounts {
            questions: 1,
            held_out: 1,
            messages: 2,
            duplicates: 0,
            private_left_out: 0
        }
    );
    let names = ["decisions-claude-code.jsonl", "decisions-claudeai.jsonl"];
    let bytes: Vec<_> = names
        .iter()
        .map(|n| read_private(h.path(), "ari", n).unwrap())
        .collect();
    let inputs: Vec<_> = names
        .iter()
        .zip(&bytes)
        .map(|(name, bytes)| corpus::Input {
            name,
            bytes,
            mode: corpus::Mode::History,
        })
        .collect();
    let m = member(&h);
    let merged = corpus::merge(
        &Filters::compile(&m.filters).unwrap(),
        &inputs,
        &Split::Holdout(0.15),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(h.state("decisions.jsonl")).unwrap(),
        mine::to_jsonl(&merged.records)
    );
}
#[test]
fn history_dtos_are_camel_case_and_requests_reject_unknown_fields() {
    assert_eq!(
        serde_json::to_value(read(true)).unwrap(),
        json!({"slug":"ari","dryRun":true})
    );
    assert_eq!(
        serde_json::to_value(ChatMark::NotMine).unwrap(),
        json!("notMine")
    );
    assert_eq!(
        serde_json::to_value(ReadCounts {
            questions: 1,
            held_out: 2,
            messages: 3,
            private_left_out: 4,
            first: None,
            last: None
        })
        .unwrap(),
        json!({"questions":1,"heldOut":2,"messages":3,"privateLeftOut":4,"first":null,"last":null})
    );
    assert_eq!(
        serde_json::to_value(marks(&["one"], true, false)).unwrap(),
        json!({"slug":"ari","mine":true,"uuids":["one"],"unmarked":false})
    );
    assert_eq!(
        serde_json::to_value(ChooseExportRequest {
            slug: "ari".into(),
            folder: true
        })
        .unwrap(),
        json!({"slug":"ari","folder":true})
    );
    assert!(
        serde_json::from_value::<ReadRequest>(json!({"slug":"ari","dryRun":true,"path":"bad"}))
            .is_err()
    );
    assert!(
        serde_json::from_value::<ChooseExportRequest>(
            json!({"slug":"ari","folder":true,"path":"bad"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<MarkChatsRequest>(
            json!({"slug":"ari","mine":true,"uuids":[],"unmarked":false,"path":"bad"})
        )
        .is_err()
    );
}
#[test]
fn export_status_hides_ingest_paths() {
    let h = Home::new(true);
    let broken = h.path().join("broken-export");
    fs::create_dir(&broken).unwrap();
    write_private(
        h.path(),
        "ari",
        "claudeai-export.json",
        &serde_json::to_vec(&json!({"path":broken})).unwrap(),
    )
    .unwrap();
    let error = claudeai_export_status_in(h.path(), slug("ari")).unwrap_err();
    assert!(
        !error.message.contains(h.path().to_str().unwrap()),
        "private home leaked: {}",
        error.message
    );
    assert_eq!(error.message, EXPORT_BAD);
}
