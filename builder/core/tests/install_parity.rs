mod common;

use common::{Scratch, checked, copy_tree, parity_tools_available, repo};
use persona_builder::crew::{NewMember, new_member, new_member_embedded};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn isolated<'a>(command: &'a mut Command, home: &Path) -> &'a mut Command {
    command
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_0", "/dev/null")
        .env("GIT_CONFIG_KEY_1", "protocol.allow")
        .env("GIT_CONFIG_VALUE_1", "never")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_CONFIG_PARAMETERS")
}

fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(base: &Path, dir: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(base, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(base).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    collect(dir, dir, &mut result);
    result
}

fn date() -> String {
    String::from_utf8(checked(Command::new("date").arg("+%F")).stdout)
        .unwrap()
        .trim()
        .to_owned()
}

fn parity_case(name: &str, slug: &str, pronouns: &str, principal: &str, principal_pronouns: &str) {
    if !parity_tools_available() {
        return;
    }
    for attempt in 0..2 {
        let scratch = Scratch::new("install");
        let home = scratch.0.join("home");
        let fork = home.join("projects/voice");
        fs::create_dir_all(fork.join("crew")).unwrap();
        for tree in ["scripts", "bootstrap", "crew/_template"] {
            copy_tree(&repo().join(tree), &fork.join(tree));
        }
        fs::copy(repo().join(".gitignore"), fork.join(".gitignore")).unwrap();
        checked(isolated(
            Command::new("git")
                .arg("-C")
                .arg(&fork)
                .args(["init", "--quiet", "-b", "main"]),
            &home,
        ));
        checked(isolated(
            Command::new("git").arg("-C").arg(&fork).args(["add", "-A"]),
            &home,
        ));
        checked(isolated(
            Command::new("git").arg("-C").arg(&fork).args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ]),
            &home,
        ));
        let bin = scratch.0.join("bin");
        fs::create_dir(&bin).unwrap();
        let claude = bin.join("claude");
        fs::write(&claude, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let before = date();
        checked(isolated(
            Command::new("bash")
                .arg(fork.join("bootstrap/install.sh"))
                .args([
                    "--name",
                    name,
                    "--principal",
                    principal,
                    "--pronouns",
                    pronouns,
                    "--principal-pronouns",
                    principal_pronouns,
                    "--no-history",
                    "--repo-dir",
                ])
                .arg(&fork)
                .env("PATH", std::env::join_paths(paths).unwrap()),
            &home,
        ));
        if date() != before {
            assert_eq!(attempt, 0, "date changed during both installation attempts");
            continue;
        }
        let target = scratch.0.join("rust").join(slug);
        let filled = new_member(
            &fork.join("crew/_template"),
            &target,
            &NewMember {
                name,
                pronouns,
                principal,
                principal_pronouns,
                repo_shown: "~/projects/voice",
                date: &before,
            },
        );
        assert!(
            filled.is_ok(),
            "new_member must accept installer inputs: {filled:?}"
        );
        let python_files = files(&fork.join("crew").join(slug));
        let embedded_target = scratch.0.join("embedded").join(slug);
        new_member_embedded(
            &embedded_target,
            &NewMember {
                name,
                pronouns,
                principal,
                principal_pronouns,
                repo_shown: "~/projects/voice",
                date: &before,
            },
        )
        .unwrap();
        compare_files(&python_files, &target);
        compare_files(&python_files, &embedded_target);
        return;
    }
}

fn compare_files(expected: &BTreeMap<PathBuf, Vec<u8>>, target: &Path) {
    let actual = files(target);
    assert_eq!(
        actual.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "Rust and install.sh must create exactly the same file set"
    );
    for (path, bytes) in expected {
        assert_eq!(
            &actual[path],
            bytes,
            "file bytes differ: {}",
            path.display()
        );
    }
}

#[test]
fn ari_robin_matches_installer_files_and_bytes() {
    parity_case("Ari", "ari", "they/them", "Robin", "they/them");
}

#[test]
fn mo_lee_sam_oneil_matches_installer_files_and_bytes() {
    parity_case("Mo Lee", "mo-lee", "she/her", "Sam O'Neil", "he/him");
}

#[test]
fn unicode_name_matches_installer_files_and_bytes() {
    parity_case("Zoë", "zo", "they/them", "Kit", "they/them");
}

#[test]
fn nested_principal_placeholder_matches_installer_order() {
    parity_case("Ari", "ari", "they/them", "{{PRINCIPAL_SLUG}}", "they/them");
}
