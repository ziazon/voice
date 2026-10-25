mod common;

use common::{Scratch, repo};
use persona_builder::member::read_member;
use persona_builder_ingest::{
    claudeai,
    filters::Filters,
    mine::{self, Split},
};
use serde_json::json;
use std::fs;

#[test]
fn claude_code_progress_counts_every_folder_including_excluded() {
    let s = Scratch::new("history-progress");
    let projects = s.0.join("projects");
    let mut member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    member.filters.excluded_folders = "excluded".into();
    for folder in ["app", "excluded", "kit"] {
        fs::create_dir_all(projects.join(folder)).unwrap();
        fs::write(
            projects.join(folder).join("session.jsonl"),
            json!({
                "type": "user", "timestamp": "2026-09-01",
                "origin": {"kind": "human"},
                "message": {"content": "Keep this scope small"}
            })
            .to_string(),
        )
        .unwrap();
    }
    let filters = Filters::compile(&member.filters).unwrap();
    let mut progress = vec![];
    let mined = mine::mine_with_progress(
        &member,
        &filters,
        &projects,
        &s.0,
        &Split::Holdout(mine::DEFAULT_HOLDOUT),
        &mut |done, total| progress.push((done, total)),
    )
    .unwrap();
    assert_eq!(progress, [(0, 3), (1, 3), (2, 3), (3, 3)]);
    assert_eq!(mined.excluded, 1);
    assert_eq!(mined.messages, 2);
    assert_eq!(
        mine::to_jsonl(&mined.records),
        mine::to_jsonl(
            &mine::mine(&member, &filters, &projects, &s.0, &Split::Holdout(0.15))
                .unwrap()
                .records
        )
    );
}

#[test]
fn claude_code_progress_reports_an_empty_history() {
    let s = Scratch::new("history-empty-progress");
    let member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    let filters = Filters::compile(&member.filters).unwrap();
    let mut progress = vec![];
    mine::mine_with_progress(
        &member,
        &filters,
        &s.0,
        &s.0,
        &Split::Holdout(0.15),
        &mut |done, total| progress.push((done, total)),
    )
    .unwrap();
    assert_eq!(progress, [(0, 0)]);
}

fn export() -> claudeai::Export {
    claudeai::Export {
        files: 1,
        conversations: json!([
            {"uuid": "chat-a", "name": "Kit plan", "created_at": "2026-03-01", "updated_at": "2026-03-01", "chat_messages": [
                {"sender": "human", "created_at": "2026-03-01", "text": "Keep this scope small"}]},
            {"uuid": "chat-b", "name": "Other plan", "created_at": "2026-03-02", "updated_at": "2026-03-02", "chat_messages": []},
            {"uuid": "chat-c", "name": "Later plan", "created_at": "2026-03-03", "updated_at": "2026-03-03", "chat_messages": []}
        ]),
    }
}

#[test]
fn claudeai_progress_counts_mine_not_mine_and_unmarked_conversations() {
    let member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    let filters = Filters::compile(&member.filters).unwrap();
    let export = export();
    let mut review = claudeai::titles(&filters, &export, &[], None).unwrap().rows;
    review.iter_mut().find(|r| r.uuid == "chat-a").unwrap().mine = " Yes ".into();
    review.iter_mut().find(|r| r.uuid == "chat-b").unwrap().mine = "n".into();
    let mut progress = vec![];
    let mined = claudeai::mine_with_progress(
        &filters,
        &export,
        &review,
        &Split::Holdout(0.15),
        &mut |done, total| progress.push((done, total)),
    )
    .unwrap();
    assert_eq!(progress, [(0, 3), (1, 3), (2, 3), (3, 3)]);
    assert_eq!((mined.read, mined.not_mine, mined.unmarked), (1, 1, 1));
    assert_eq!(mined.records.len(), 1);
    assert_eq!(
        mine::to_jsonl(&mined.records),
        mine::to_jsonl(
            &claudeai::mine(&filters, &export, &review, &Split::Holdout(0.15))
                .unwrap()
                .records
        )
    );
}

#[test]
fn claudeai_conversations_counts_all_and_refuses_a_non_list() {
    assert_eq!(claudeai::conversations(&export()).unwrap(), 3);
    assert_eq!(
        claudeai::conversations(&claudeai::Export {
            files: 1,
            conversations: json!([])
        })
        .unwrap(),
        0
    );
    assert!(
        claudeai::conversations(&claudeai::Export {
            files: 1,
            conversations: json!({})
        })
        .is_err()
    );
}

#[test]
fn mine_marks_use_the_miners_case_and_python_whitespace_rules() {
    for mark in ["y", " Yes ", "mine", "\u{1c}Y\u{1f}"] {
        assert!(claudeai::is_mine(mark), "{mark:?} should be mine");
    }
    for mark in ["n", "", "maybe"] {
        assert!(!claudeai::is_mine(mark), "{mark:?} should not be mine");
    }
}
