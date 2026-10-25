mod common;

use common::Scratch;
use persona_builder_ingest::private::{list_private, write_private};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{PermissionsExt, symlink},
    },
};

#[test]
fn lists_only_regular_utf8_files_in_sorted_order() {
    let s = Scratch::new("private-list");
    let last = write_private(&s.0, "wren", "z.jsonl", b"last").unwrap();
    write_private(&s.0, "wren", "a.json", b"first").unwrap();
    let state = last.parent().unwrap();
    fs::create_dir(state.join("folder")).unwrap();
    if let Err(error) = fs::write(
        state.join(std::ffi::OsString::from_vec(b"bad\xff".to_vec())),
        b"non UTF-8",
    ) {
        eprintln!("non-UTF-8 fixture unavailable on this filesystem: {error}");
    }
    symlink(&last, state.join("link.jsonl")).unwrap();
    assert_eq!(list_private(&s.0, "wren").unwrap(), ["a.json", "z.jsonl"]);
}

#[test]
fn missing_state_folder_lists_nothing_and_creates_nothing() {
    let s = Scratch::new("private-list-missing");
    assert_eq!(list_private(&s.0, "wren").unwrap(), Vec::<String>::new());
    assert!(!s.0.join(".claude").exists());
}

#[test]
fn refuses_a_link_in_place_of_the_state_folder() {
    let s = Scratch::new("private-list-link");
    let outside = s.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("private.json"), b"do not read").unwrap();
    fs::create_dir_all(s.0.join(".claude/state")).unwrap();
    symlink(&outside, s.0.join(".claude/state/wren")).unwrap();
    assert!(list_private(&s.0, "wren").is_err());
}

#[test]
fn refuses_a_state_folder_others_can_change() {
    let s = Scratch::new("private-list-permissions");
    let path = write_private(&s.0, "wren", "x.jsonl", b"private").unwrap();
    fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o770)).unwrap();
    assert!(list_private(&s.0, "wren").is_err());
}
