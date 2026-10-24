mod common;

use common::Scratch;
use persona_builder_ingest::private::write_private;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

#[test]
fn private_files_replace_atomically_and_keep_existing_directory_modes() {
    let home = Scratch::new("private-write");
    let file = write_private(&home.0, "wren", "history.jsonl", b"first\n").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"first\n");
    for suffix in [".claude", ".claude/state", ".claude/state/wren"] {
        assert_eq!(
            fs::metadata(home.0.join(suffix))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let folder = file.parent().unwrap();
    fs::set_permissions(folder, fs::Permissions::from_mode(0o750)).unwrap();
    write_private(&home.0, "wren", "history.jsonl", b"second\n").unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"second\n");
    assert_eq!(
        fs::metadata(folder).unwrap().permissions().mode() & 0o777,
        0o750
    );
    assert_eq!(fs::read_dir(folder).unwrap().count(), 1);
}

#[test]
fn refuses_each_linked_directory_without_writing_to_its_target() {
    for suffix in [".claude", ".claude/state", ".claude/state/wren"] {
        let home = Scratch::new("private-links");
        let target = home.0.join("target");
        fs::create_dir(&target).unwrap();
        let link = home.0.join(suffix);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(&target, &link).unwrap();
        let error = write_private(&home.0, "wren", "history.jsonl", b"private\n").unwrap_err();
        assert_eq!(
            error.to_string(),
            format!(
                "{} is a link or not a folder: Persona Builder writes your history only into a real folder. Move it away and try again",
                fs::canonicalize(&home.0).unwrap().join(suffix).display()
            )
        );
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    }
}

#[test]
fn replaces_file_link_without_touching_target_and_accepts_linked_home() {
    let scratch = Scratch::new("private-target");
    let home = scratch.0.join("home");
    fs::create_dir(&home).unwrap();
    let alias = scratch.0.join("alias");
    symlink(&home, &alias).unwrap();
    let file = write_private(&alias, "wren", "history.jsonl", b"first\n").unwrap();
    let target = scratch.0.join("outside");
    fs::write(&target, b"unchanged").unwrap();
    fs::remove_file(&file).unwrap();
    symlink(&target, &file).unwrap();
    write_private(&alias, "wren", "history.jsonl", b"new\n").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"unchanged");
    assert_eq!(fs::read(&file).unwrap(), b"new\n");
    assert!(!fs::symlink_metadata(file).unwrap().is_symlink());
}

#[test]
fn rejects_unsafe_names_and_slugs_before_creating_folders() {
    let home = Scratch::new("private-names");
    for name in ["../x.jsonl", "a/b.jsonl", ".x.jsonl", "x.json", "x\0.jsonl"] {
        assert_eq!(
            write_private(&home.0, "wren", name, b"private")
                .unwrap_err()
                .to_string(),
            "the file name must be a plain name ending .jsonl"
        );
    }
    for slug in [
        "",
        ".",
        "..",
        "../outside",
        "wren/../outside",
        "/outside",
        "wren/",
        "wren\0",
    ] {
        assert!(
            write_private(&home.0, slug, "history.jsonl", b"private").is_err(),
            "{slug:?}"
        );
    }
    assert!(!home.0.join(".claude").exists());
}

#[test]
fn refuses_regular_directory_components_and_cleans_failed_replace() {
    let home = Scratch::new("private-failure");
    fs::write(home.0.join(".claude"), b"keep").unwrap();
    assert!(
        write_private(&home.0, "wren", "history.jsonl", b"private")
            .unwrap_err()
            .to_string()
            .contains("is a link or not a folder")
    );
    fs::remove_file(home.0.join(".claude")).unwrap();
    let folder = home.0.join(".claude/state/wren");
    fs::create_dir_all(folder.join("history.jsonl")).unwrap();
    assert!(write_private(&home.0, "wren", "history.jsonl", b"private").is_err());
    assert_eq!(fs::read_dir(folder).unwrap().count(), 1);
}
