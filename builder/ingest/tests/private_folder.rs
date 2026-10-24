mod common;

use common::Scratch;
use persona_builder_ingest::private::{
    lock_state, private_folder, read_private_optional, write_private,
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn private_folder_is_created_private_and_found_again() {
    let home = Scratch::new("private-folder");
    let folder = private_folder(&home.0, "wren", "agent").unwrap();
    assert_eq!(
        folder,
        home.0
            .canonicalize()
            .unwrap()
            .join(".claude/state/wren/agent")
    );
    assert_eq!(mode(&folder), 0o700);
    assert_eq!(mode(folder.parent().unwrap()), 0o700);
    assert_eq!(private_folder(&home.0, "wren", "agent").unwrap(), folder);
}

#[test]
fn private_folder_refuses_a_link_and_an_open_folder() {
    let home = Scratch::new("private-folder-link");
    let state = home.0.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    let target = home.0.join("elsewhere");
    fs::create_dir(&target).unwrap();
    symlink(&target, state.join("agent")).unwrap();
    let error = private_folder(&home.0, "wren", "agent").unwrap_err();
    assert!(
        error.to_string().contains("is a link or not a folder"),
        "{error}"
    );

    let home = Scratch::new("private-folder-open");
    let folder = home.0.join(".claude/state/wren/agent");
    fs::create_dir_all(&folder).unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    let error = private_folder(&home.0, "wren", "agent").unwrap_err();
    assert!(error.to_string().contains("only you can open"), "{error}");
}

#[test]
fn private_folder_takes_plain_names_only() {
    let home = Scratch::new("private-folder-names");
    for name in ["", ".", "..", "a/b", ".hidden", "a\0b"] {
        assert!(private_folder(&home.0, "wren", name).is_err(), "{name:?}");
    }
}

#[test]
fn optional_read_is_none_only_when_missing() {
    let home = Scratch::new("private-optional");
    assert!(
        read_private_optional(&home.0, "wren", "runs.jsonl")
            .unwrap()
            .is_none()
    );
    write_private(&home.0, "wren", "other.jsonl", b"x\n").unwrap();
    assert!(
        read_private_optional(&home.0, "wren", "runs.jsonl")
            .unwrap()
            .is_none()
    );
    write_private(&home.0, "wren", "runs.jsonl", b"line\n").unwrap();
    assert_eq!(
        read_private_optional(&home.0, "wren", "runs.jsonl")
            .unwrap()
            .unwrap(),
        b"line\n"
    );
    let state = home.0.join(".claude/state/wren");
    fs::remove_file(state.join("runs.jsonl")).unwrap();
    symlink(state.join("other.jsonl"), state.join("runs.jsonl")).unwrap();
    assert!(read_private_optional(&home.0, "wren", "runs.jsonl").is_err());
}

#[test]
fn the_state_lock_is_exclusive_until_dropped() {
    let home = Scratch::new("private-lock");
    let first = lock_state(&home.0, "wren").unwrap();
    assert_eq!(mode(&home.0.join(".claude/state/wren")), 0o700);
    let error = lock_state(&home.0, "wren").unwrap_err();
    assert!(error.to_string().contains("already building"), "{error}");
    let other = lock_state(&home.0, "robin").unwrap();
    drop(first);
    let again = lock_state(&home.0, "wren").unwrap();
    drop((again, other));
}

#[test]
fn optional_read_fails_when_the_folder_cannot_be_searched() {
    let home = Scratch::new("private-optional-denied");
    write_private(&home.0, "wren", "runs.jsonl", b"line\n").unwrap();
    let state = home.0.join(".claude/state/wren");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o400)).unwrap();
    let result = read_private_optional(&home.0, "wren", "runs.jsonl");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn private_folder_refuses_an_assistant_folder_others_can_write() {
    for mode in [0o770, 0o702, 0o777] {
        let home = Scratch::new("private-folder-parent");
        let state = home.0.join(".claude/state/wren");
        fs::create_dir_all(&state).unwrap();
        fs::set_permissions(&state, fs::Permissions::from_mode(mode)).unwrap();
        let error = private_folder(&home.0, "wren", "agent").unwrap_err();
        assert!(
            error.to_string().contains("can be changed by others")
                && error.to_string().contains("only you can change"),
            "{mode:o}: {error}"
        );
    }
    let home = Scratch::new("private-folder-parent-ok");
    let state = home.0.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o750)).unwrap();
    private_folder(&home.0, "wren", "agent").unwrap();
}
