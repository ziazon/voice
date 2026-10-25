mod common;
use common::Scratch;
use persona_builder_ingest::private::{remove_private, write_private};
use std::{fs, os::unix::fs::symlink};

#[test]
fn removes_private_file_and_reports_missing_without_creating_folders() {
    let home = Scratch::new("private-remove");
    assert!(!remove_private(&home.0, "ari", "consent.json").unwrap());
    assert!(!home.0.join(".claude").exists());
    let path = write_private(&home.0, "ari", "consent.json", b"record").unwrap();
    assert!(!remove_private(&home.0, "ari", "missing.json").unwrap());
    assert!(remove_private(&home.0, "ari", "consent.json").unwrap());
    assert!(!path.exists());
    assert!(!remove_private(&home.0, "ari", "consent.json").unwrap());
    for name in [
        "../consent.json",
        "a/b.json",
        ".consent.json",
        "consent.txt",
    ] {
        assert!(remove_private(&home.0, "ari", name).is_err());
    }
}

#[test]
fn refuses_linked_state_folder_and_unlinks_file_links_without_following() {
    let home = Scratch::new("private-remove-links");
    let state = home.0.join(".claude/state");
    let elsewhere = home.0.join("elsewhere");
    fs::create_dir_all(&state).unwrap();
    fs::create_dir(&elsewhere).unwrap();
    let target = elsewhere.join("consent.json");
    fs::write(&target, b"keep").unwrap();
    symlink(&elsewhere, state.join("ari")).unwrap();
    assert!(remove_private(&home.0, "ari", "consent.json").is_err());
    fs::remove_file(state.join("ari")).unwrap();
    fs::create_dir(state.join("ari")).unwrap();
    symlink(&target, state.join("ari/consent.json")).unwrap();
    assert!(remove_private(&home.0, "ari", "consent.json").unwrap());
    assert_eq!(fs::read(target).unwrap(), b"keep");
}
