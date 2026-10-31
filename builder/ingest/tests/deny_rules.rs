mod common;

use common::{Scratch, repo};
use persona_builder::{deny::DenyList, member::read_member};
use persona_builder_ingest::{
    claudeai::read_export,
    filters::Filters,
    mine::{Split, mine, summary},
};
use std::{fs, path::Path};

fn run(
    projects: &Path,
    home: &Path,
) -> persona_builder::error::Result<persona_builder_ingest::mine::Mined> {
    let member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    mine(
        &member,
        &Filters::compile(&member.filters).unwrap(),
        projects,
        home,
        &Split::Holdout(0.15),
    )
}
fn session(home: &Path) -> std::path::PathBuf {
    let projects = home.join("projects");
    fs::create_dir_all(projects.join("app")).unwrap();
    fs::write(projects.join("app/good.jsonl"), serde_json::json!({"type":"user", "timestamp":"2026-09-01T10:00:00Z", "origin":{"kind":"human"}, "message":{"content":"Keep the reading list short."}}).to_string()).unwrap();
    projects
}
fn sentence(shown: &str) -> String {
    format!(
        "{shown} is a kind of file that holds passwords or keys, so Persona Builder never opens it. Choose another file."
    )
}

#[test]
fn mining_refuses_projects_under_ssh() {
    let home = Scratch::new("deny-projects");
    let projects = home.0.join(".ssh/projects");
    fs::create_dir_all(&projects).unwrap();
    assert_eq!(
        run(&projects, &home.0).unwrap_err().to_string(),
        sentence("~/.ssh/projects")
    );
}

#[test]
fn mining_skips_denied_session_and_reports_its_count() {
    let home = Scratch::new("deny-session");
    let projects = session(&home.0);
    fs::write(projects.join("app/.env.jsonl"), "canary").unwrap();
    let mined = run(&projects, &home.0).unwrap();
    assert_eq!(mined.records.len(), 1);
    assert_eq!(mined.records[0]["text"], "Keep the reading list short.");
    assert_eq!(mined.denied, 1);
    assert_eq!(mined.unreadable, 0);
    assert_eq!(
        summary(&mined),
        "mined 0 questions (0 held out) and 1 messages, 2026-09-01 → 2026-09-01; held out from nothing\ndropped 0 excluded folders, 0 personal records, 0 secret records; left out 1 files on the never-read list\n"
    );
}

#[test]
fn summary_with_nothing_denied_is_byte_identical() {
    let home = Scratch::new("deny-summary");
    let projects = session(&home.0);
    let mined = run(&projects, &home.0).unwrap();
    assert_eq!(mined.denied, 0);
    assert_eq!(
        summary(&mined),
        "mined 0 questions (0 held out) and 1 messages, 2026-09-01 → 2026-09-01; held out from nothing\ndropped 0 excluded folders, 0 personal records, 0 secret records\n"
    );
}

#[test]
fn export_under_aws_is_refused_before_opening() {
    let home = Scratch::new("deny-export");
    fs::create_dir(home.0.join(".aws")).unwrap();
    let path = home.0.join(".aws/export.zip");
    // Leave it missing: a path refusal must precede even an open error.
    let deny = DenyList::new(&home.0, &[]).unwrap();
    assert_eq!(
        read_export(&path, &deny)
            .err()
            .expect("export refused")
            .to_string(),
        sentence("~/.aws/export.zip")
    );
}

#[test]
fn folder_export_refuses_conversations_link_into_ssh() {
    let home = Scratch::new("deny-export-link");
    fs::create_dir(home.0.join(".ssh")).unwrap();
    let target = home.0.join(".ssh/history.json");
    fs::write(&target, "canary").unwrap();
    let folder = home.0.join("export");
    fs::create_dir(&folder).unwrap();
    std::os::unix::fs::symlink(target, folder.join("conversations.json")).unwrap();
    let deny = DenyList::new(&home.0, &[]).unwrap();
    assert_eq!(
        read_export(&folder, &deny)
            .err()
            .expect("export refused")
            .to_string(),
        sentence("~/export/conversations.json")
    );
}
