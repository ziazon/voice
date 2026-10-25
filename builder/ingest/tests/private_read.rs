mod common;
use common::Scratch;
use persona_builder_ingest::private::{read_private, write_private};
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{PermissionsExt, symlink},
    },
    process::Command,
};
fn listing(path: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    names
}
#[test]
fn reads_private_bytes_and_creates_nothing() {
    let s = Scratch::new("private-read");
    let path = write_private(&s.0, "wren", "x.jsonl", b"private bytes").unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        read_private(&s.0, "wren", "x.jsonl").unwrap(),
        b"private bytes"
    );
    let before = listing(path.parent().unwrap());
    assert_eq!(
        read_private(&s.0, "wren", "missing.jsonl")
            .unwrap_err()
            .to_string(),
        format!(
            "{} doesn't exist",
            path.with_file_name("missing.jsonl").display()
        )
    );
    assert_eq!(listing(path.parent().unwrap()), before);
}
#[test]
fn missing_folders_and_bad_names_create_nothing() {
    let s = Scratch::new("missing-private-read");
    let canonical = fs::canonicalize(&s.0).unwrap();
    assert_eq!(
        read_private(&s.0, "wren", "x.jsonl")
            .unwrap_err()
            .to_string(),
        format!("{} doesn't exist", canonical.join(".claude").display())
    );
    for name in [
        "../x.jsonl",
        "a/b.jsonl",
        ".x.jsonl",
        "x.jso",
        "x\0.jsonl",
        "x.txt",
        "x.tsv/",
        ".x.tsv",
    ] {
        assert_eq!(
            read_private(&s.0, "wren", name).unwrap_err().to_string(),
            "the file name must be a plain name ending .jsonl, .json or .tsv"
        );
    }
    assert!(listing(&s.0).is_empty());
}
#[test]
fn links_at_every_level_are_refused() {
    for depth in 0..4 {
        let s = Scratch::new("private-read-links");
        let target = s.0.join("elsewhere");
        fs::create_dir(&target).unwrap();
        let parts = [".claude", "state", "wren", "x.jsonl"];
        let mut path = s.0.clone();
        for part in &parts[..depth] {
            path.push(part);
            fs::create_dir(&path).unwrap();
        }
        path.push(parts[depth]);
        symlink(&target, &path).unwrap();
        let shown = fs::canonicalize(path.parent().unwrap())
            .unwrap()
            .join(parts[depth]);
        let err = read_private(&s.0, "wren", "x.jsonl")
            .unwrap_err()
            .to_string();
        let expected = if depth == 3 {
            format!(
                "{} is a link: Persona Builder reads only real files, never links. Move it away and try again",
                shown.display()
            )
        } else {
            format!(
                "{} is a link or not a folder: Persona Builder writes your history only into a real folder. Move it away and try again",
                shown.display()
            )
        };
        assert_eq!(err, expected);
        assert!(listing(&target).is_empty());
    }
}
#[test]
fn folders_and_pipes_are_refused_without_blocking() {
    for pipe in [false, true] {
        let s = Scratch::new("private-read-special");
        let folder = s.0.join(".claude/state/wren");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("x.jsonl");
        if pipe {
            assert!(
                Command::new("mkfifo")
                    .arg(&path)
                    .status()
                    .unwrap()
                    .success()
            );
        } else {
            fs::create_dir(&path).unwrap();
        }
        assert_eq!(
            read_private(&s.0, "wren", "x.jsonl")
                .unwrap_err()
                .to_string(),
            format!(
                "{} is not a file",
                fs::canonicalize(&s.0)
                    .unwrap()
                    .join(".claude/state/wren/x.jsonl")
                    .display()
            )
        );
        assert_eq!(listing(&folder)[0].as_bytes(), b"x.jsonl");
    }
}
#[test]
fn review_files_are_private_too() {
    let s = Scratch::new("private-review");
    let path = write_private(&s.0, "wren", "claudeai-review.tsv", b"mine\n").unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        read_private(&s.0, "wren", "claudeai-review.tsv").unwrap(),
        b"mine\n"
    );
}
