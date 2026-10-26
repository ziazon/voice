mod common;
use common::Scratch;
use persona_builder_ingest::private::{
    place, read_private_at, validate_json_name, write_private, write_private_at,
};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

fn mode(path: &std::path::Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn writes_and_reads_inside_a_private_folder() {
    let s = Scratch::new("private-at");
    let path = write_private_at(&s.0, "wren", Some("chunks"), "train-01.jsonl", b"one\n").unwrap();
    assert_eq!(path.file_name().unwrap(), "train-01.jsonl");
    assert_eq!(path.parent().unwrap().file_name().unwrap(), "chunks");
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(path.parent().unwrap()), 0o700);
    assert_eq!(
        read_private_at(&s.0, "wren", Some("chunks"), "train-01.jsonl").unwrap(),
        b"one\n"
    );
    write_private_at(&s.0, "wren", Some("chunks"), "train-01.jsonl", b"two\n").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"two\n");
    assert_eq!(
        fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1,
        "the temporary file is gone"
    );
    write_private_at(&s.0, "wren", None, "labels.json", b"{}").unwrap();
    assert_eq!(
        read_private_at(&s.0, "wren", None, "labels.json").unwrap(),
        b"{}"
    );
}

#[test]
fn a_failed_read_creates_no_folder() {
    let s = Scratch::new("private-at-missing");
    write_private(&s.0, "wren", "x.jsonl", b"").unwrap();
    let state = fs::canonicalize(&s.0).unwrap().join(".claude/state/wren");
    assert_eq!(
        read_private_at(&s.0, "wren", Some("chunks"), "train-01.jsonl")
            .unwrap_err()
            .to_string(),
        format!("{} doesn't exist", state.join("chunks").display())
    );
    assert!(!state.join("chunks").exists());
}

#[test]
fn links_at_the_folder_and_the_file_are_refused() {
    let s = Scratch::new("private-at-links");
    write_private(&s.0, "wren", "x.jsonl", b"").unwrap();
    let state = s.0.join(".claude/state/wren");
    let elsewhere = s.0.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&elsewhere, state.join("chunks")).unwrap();
    for result in [
        write_private_at(&s.0, "wren", Some("chunks"), "a.jsonl", b"x").map(|_| ()),
        read_private_at(&s.0, "wren", Some("chunks"), "a.jsonl").map(|_| ()),
    ] {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("is a link or not a folder")
        );
    }
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    fs::remove_file(state.join("chunks")).unwrap();
    write_private_at(&s.0, "wren", Some("chunks"), "a.jsonl", b"x").unwrap();
    fs::write(elsewhere.join("b.jsonl"), b"outside").unwrap();
    symlink(elsewhere.join("b.jsonl"), state.join("chunks/b.jsonl")).unwrap();
    assert!(read_private_at(&s.0, "wren", Some("chunks"), "b.jsonl").is_err());
    assert!(write_private_at(&s.0, "wren", Some("chunks"), "b.jsonl", b"in").is_ok());
    assert_eq!(fs::read(elsewhere.join("b.jsonl")).unwrap(), b"outside");
}

#[test]
fn a_folder_others_can_open_is_refused() {
    let s = Scratch::new("private-at-open");
    write_private_at(&s.0, "wren", Some("chunks"), "a.jsonl", b"x").unwrap();
    let folder = s.0.join(".claude/state/wren/chunks");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    let error = read_private_at(&s.0, "wren", Some("chunks"), "a.jsonl")
        .unwrap_err()
        .to_string();
    assert!(error.contains("can be opened by others"), "{error}");
    assert!(error.contains("by others on this computer"), "{error}");
}

#[test]
fn places_are_a_name_or_one_folder_and_a_name() {
    assert_eq!(place("labels.json").unwrap(), (None, "labels.json"));
    assert_eq!(
        place("chunks/train-01.jsonl").unwrap(),
        (Some("chunks"), "train-01.jsonl")
    );
    for bad in [
        "a/b/c.jsonl",
        "/abs.jsonl",
        "../x.jsonl",
        ".hidden/x.jsonl",
        "a\0/x.jsonl",
    ] {
        assert_eq!(
            place(bad).unwrap_err().to_string(),
            format!(
                "{bad}: a file here is NAME or FOLDER/NAME inside the assistant's private folder"
            )
        );
    }
    assert!(validate_json_name("labels.json").is_ok());
    for bad in ["labels.jsonl", ".labels.json", "a/labels.json"] {
        assert_eq!(
            validate_json_name(bad).unwrap_err().to_string(),
            "the file name must be a plain name ending .json"
        );
    }
}

#[test]
fn a_state_folder_others_can_change_is_refused() {
    let s = Scratch::new("private-at-state");
    write_private(&s.0, "wren", "x.jsonl", b"x").unwrap();
    let state = s.0.join(".claude/state/wren");
    fs::set_permissions(&state, fs::Permissions::from_mode(0o777)).unwrap();
    for result in [
        write_private(&s.0, "wren", "y.jsonl", b"y").map(|_| ()),
        read_private_at(&s.0, "wren", None, "x.jsonl").map(|_| ()),
    ] {
        let error = result.unwrap_err().to_string();
        assert!(error.contains("can be changed by others"), "{error}");
    }
    assert!(!state.join("y.jsonl").exists());
    fs::set_permissions(&state, fs::Permissions::from_mode(0o750)).unwrap();
    assert_eq!(
        read_private_at(&s.0, "wren", None, "x.jsonl").unwrap(),
        b"x"
    );
}

#[test]
fn bounded_private_reads_refuse_oversized_files_before_reading() {
    use persona_builder_ingest::private::read_private_at_max;
    let s = Scratch::new("private-at-max");
    let path = write_private_at(&s.0, "wren", Some("growth"), "signals.jsonl", b"one").unwrap();
    assert_eq!(
        read_private_at_max(&s.0, "wren", Some("growth"), "signals.jsonl", 3).unwrap(),
        b"one"
    );
    let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    let result = read_private_at_max(
        &s.0,
        "wren",
        Some("growth"),
        "signals.jsonl",
        64 * 1024 * 1024,
    );
    assert!(result.is_err(), "oversized opened files must be refused");
    let error = result.unwrap_err();
    assert_eq!(
        error.to_string(),
        "signals.jsonl is too big to read (over 65536 KiB)"
    );
    assert!(
        persona_builder_ingest::private::read_private_optional_at_max(
            &s.0,
            "wren",
            Some("growth"),
            "signals.jsonl",
            64 * 1024 * 1024
        )
        .is_err()
    );
    assert!(
        persona_builder_ingest::private::read_private_optional_at_max(
            &s.0,
            "wren",
            Some("growth"),
            "missing.json",
            1
        )
        .unwrap()
        .is_none()
    );
    assert!(read_private_at_max(&s.0, "wren", Some("growth"), "missing.json", 1).is_err());
    symlink(&path, path.parent().unwrap().join("linked.json")).unwrap();
    assert!(read_private_at_max(&s.0, "wren", Some("growth"), "linked.json", u64::MAX).is_err());
}
