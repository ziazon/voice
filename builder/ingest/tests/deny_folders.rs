mod common;

use common::{Scratch, repo};
use persona_builder::{deny::DenyList, member::read_member};
use persona_builder_ingest::{
    filters::Filters,
    mine::{Split, folders, mine},
};
use std::fs;

#[test]
fn denied_real_project_is_excluded_without_listing_its_sessions() {
    let home = Scratch::new("deny-project-folder");
    let projects = home.0.join("projects");
    fs::create_dir_all(projects.join("app")).unwrap();
    fs::write(projects.join("app/one.jsonl"), "canary").unwrap();
    fs::write(projects.join("app/two.jsonl"), "canary").unwrap();
    fs::create_dir(projects.join("other")).unwrap();
    fs::write(projects.join("other/good.jsonl"), serde_json::json!({"type":"user", "timestamp":"2026-09-01T10:00:00Z", "origin":{"kind":"human"}, "message":{"content":"Keep the reading list short."}}).to_string()).unwrap();
    let mut member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    let filters = Filters::compile(&member.filters).unwrap();
    let baseline = mine(&member, &filters, &projects, &home.0, &Split::Holdout(0.15)).unwrap();
    member.filters.denied_paths = vec!["~/projects/app".into()];
    let deny = DenyList::for_member(&member, &home.0).unwrap();
    let mined = mine(&member, &filters, &projects, &home.0, &Split::Holdout(0.15)).unwrap();
    assert_eq!(mined.excluded, baseline.excluded + 1);
    assert_eq!(mined.denied, 0);
    assert_eq!(mined.unreadable, 0);
    assert_eq!(mined.records.len(), 1);
    assert_eq!(mined.records[0]["text"], "Keep the reading list short.");
    let rows = folders(&filters, &projects, &home.0, &deny).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].folder, "other");
}

#[test]
fn denied_project_link_is_refused_with_the_same_error_as_an_allowed_link() {
    let home = Scratch::new("deny-project-link");
    let projects = home.0.join("projects");
    fs::create_dir(&projects).unwrap();
    let app = projects.join("app");
    let allowed = home.0.join("allowed");
    fs::create_dir(&allowed).unwrap();
    std::os::unix::fs::symlink(&allowed, &app).unwrap();
    let member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    let filters = Filters::compile(&member.filters).unwrap();
    let allowed_error = mine(&member, &filters, &projects, &home.0, &Split::Holdout(0.15))
        .unwrap_err()
        .to_string();
    fs::remove_file(&app).unwrap();
    let denied = home.0.join(".aws");
    fs::create_dir(&denied).unwrap();
    std::os::unix::fs::symlink(&denied, &app).unwrap();
    let denied_error = mine(&member, &filters, &projects, &home.0, &Split::Holdout(0.15))
        .unwrap_err()
        .to_string();
    assert_eq!(denied_error, allowed_error);
}

#[test]
fn folder_sessions_count_only_allowed_files() {
    let home = Scratch::new("deny-folder-sessions");
    let projects = home.0.join("projects");
    fs::create_dir_all(projects.join("app")).unwrap();
    fs::write(projects.join("app/good.jsonl"), "canary").unwrap();
    fs::write(projects.join("app/.env.jsonl"), "canary").unwrap();
    let member = read_member(&repo().join("crew/_template/member.json")).unwrap();
    let filters = Filters::compile(&member.filters).unwrap();
    let deny = DenyList::for_member(&member, &home.0).unwrap();
    let rows = folders(&filters, &projects, &home.0, &deny).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].folder, "app");
    assert_eq!(rows[0].sessions, 1);
    let mined = mine(&member, &filters, &projects, &home.0, &Split::Holdout(0.15)).unwrap();
    assert_eq!(mined.denied, 1);
}
