mod common;

use common::Scratch;
use persona_builder::{deny::DenyList, error::Error};
use std::{fs, path::Path};

fn canary(home: &Path, relative: &str) -> std::path::PathBuf {
    let path = home.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "canary").unwrap();
    path
}

#[test]
fn builtin_entries_deny_canaries() {
    let home = Scratch::new("deny-builtins");
    let deny = DenyList::builtin();
    for name in [
        ".env",
        ".env.local",
        ".envrc",
        ".npmrc",
        ".netrc",
        ".pgpass",
        ".ssh/id_ed25519",
        ".aws/credentials",
        ".gnupg/pubring.kbx",
        ".config/gh/hosts.yml",
        ".docker/config.json",
        "certs/site.pem",
        "certs/site.KEY",
        "certs/id.p12",
        "login.keychain-db",
        "Library/Keychains/x.db",
        ".claude/.credentials.json",
    ] {
        assert!(deny.denied(&canary(&home.0, name)), "{name}");
    }
}

#[test]
fn allowed_controls_are_not_denied() {
    let home = Scratch::new("deny-controls");
    let deny = DenyList::builtin();
    for name in [
        "notes.md",
        "monkey.txt",
        "keys.md",
        "docker/config.json",
        ".config/other/x",
        ".sshrc",
    ] {
        assert!(!deny.denied(&canary(&home.0, name)), "{name}");
    }
}

#[test]
fn matching_ignores_letter_case() {
    let home = Scratch::new("deny-case");
    for name in [".SSH/ID_RSA", "Certs/Site.PEM"] {
        assert!(DenyList::builtin().denied(&canary(&home.0, name)), "{name}");
    }
}

#[test]
fn file_link_to_denied_target_is_denied() {
    let home = Scratch::new("deny-file-link");
    let target = canary(&home.0, ".ssh/id_rsa");
    let link = home.0.join("notes.md");
    std::os::unix::fs::symlink(target, &link).unwrap();
    assert!(DenyList::builtin().denied(&link));
}

#[test]
fn folder_link_to_denied_target_is_denied() {
    let home = Scratch::new("deny-folder-link");
    canary(&home.0, ".aws/credentials");
    std::os::unix::fs::symlink(home.0.join(".aws"), home.0.join("docs")).unwrap();
    assert!(DenyList::builtin().denied(&home.0.join("docs/credentials")));
}

#[test]
fn dangling_link_uses_the_given_name() {
    let home = Scratch::new("deny-dangling");
    let link = home.0.join("notes.md");
    std::os::unix::fs::symlink(home.0.join("missing"), &link).unwrap();
    assert!(!DenyList::builtin().denied(&link));
}

#[test]
fn as_given_path_keeps_denied_components_before_parent_parts() {
    let home = Scratch::new("deny-parent");
    fs::create_dir(home.0.join(".ssh")).unwrap();
    canary(&home.0, "notes.md");
    assert!(DenyList::builtin().denied(&home.0.join(".ssh/../notes.md")));
}

#[test]
fn extra_name_pattern_matches_nested_components() {
    let home = Scratch::new("deny-name");
    let deny = DenyList::new(&home.0, &["diary*".into()]).unwrap();
    assert!(deny.denied(&canary(&home.0, "journal/Diary-2026.md")));
}

#[test]
fn home_path_entry_matches_descendants_but_not_name_prefixes() {
    let home = Scratch::new("deny-home-path");
    let deny = DenyList::new(&home.0, &["~/Private/".into()]).unwrap();
    assert!(deny.denied(&canary(&home.0, "Private/a/b.txt")));
    assert!(!deny.denied(&canary(&home.0, "Privateer.txt")));
}

#[test]
fn absolute_path_entry_matches_descendants() {
    let home = Scratch::new("deny-absolute");
    let entry = home.0.join("private").display().to_string();
    let deny = DenyList::new(&home.0, &[entry]).unwrap();
    assert!(deny.denied(&canary(&home.0, "private/a.txt")));
    assert!(!deny.denied(&canary(&home.0, "public/a.txt")));
}

#[test]
fn entry_naming_a_link_also_denies_its_target() {
    let home = Scratch::new("deny-entry-link");
    let target = canary(&home.0, "journal/a.txt");
    let link = home.0.join("private");
    std::os::unix::fs::symlink(home.0.join("journal"), &link).unwrap();
    let deny = DenyList::new(&home.0, &[link.display().to_string()]).unwrap();
    assert!(deny.denied(&target));
}

#[test]
fn malformed_entries_are_refused_with_the_sentence() {
    let home = Scratch::new("deny-bad-entry");
    for entry in [
        "".to_owned(),
        "../x".into(),
        "a/b".into(),
        "~/./x".into(),
        "x".repeat(256),
        "bad\nname".into(),
    ] {
        let error = match DenyList::new(&home.0, std::slice::from_ref(&entry)) {
            Ok(_) => panic!("accepted {entry:?}"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::Invalid(_)));
        assert_eq!(
            error.to_string(),
            format!(
                "member config: filters.denied_paths: {entry:?} isn't a name or a path Persona Builder can use. Write a name like diary*, or a path starting with ~/ or /, with no . or .. parts, at most 255 characters."
            )
        );
    }
}

#[test]
fn more_than_200_entries_are_refused_with_the_sentence() {
    let home = Scratch::new("deny-too-many");
    let entries = (0..201).map(|i| format!("diary-{i}")).collect::<Vec<_>>();
    let error = match DenyList::new(&home.0, &entries) {
        Ok(_) => panic!("accepted 201 entries"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Invalid(_)));
    assert_eq!(
        error.to_string(),
        "member config: filters.denied_paths holds more than 200 entries."
    );
}

#[test]
fn relative_matching_checks_only_relative_components() {
    let deny = DenyList::builtin();
    assert!(deny.denied_relative(Path::new(".env")));
    assert!(deny.denied_relative(Path::new("sub/.ssh/x")));
    assert!(!deny.denied_relative(Path::new("notes.md")));
}
