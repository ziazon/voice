pub mod common;

use common::Scratch;
use persona_builder_save::{git::Git, text};
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn missing_git_needs_installation() {
    let scratch = Scratch::new("git-version-missing");
    let error = Git::at(scratch.0.join("no-such-git"))
        .check_version()
        .unwrap_err();
    assert_eq!(error.to_string(), text::NEEDS_GIT);
}

fn check_version(line: &str, expected_error: Option<&str>) {
    let scratch = Scratch::new("git-version");
    let program = scratch.0.join("git-wrapper");
    fs::write(
        &program,
        format!("#!/bin/sh\nprintf '%s\\n' '{line}'\nexit 0\n"),
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let result = Git::at(program).check_version();
    match expected_error {
        Some(expected) => assert_eq!(result.unwrap_err().to_string(), expected),
        None => result.unwrap(),
    }
}

macro_rules! version_case {
    ($name:ident, $line:literal, $error:expr) => {
        #[test]
        fn $name() {
            check_version($line, $error);
        }
    };
}

version_case!(
    git_2_31_9_is_too_old,
    "git version 2.31.9",
    Some(text::GIT_TOO_OLD)
);
version_case!(git_2_32_0_is_supported, "git version 2.32.0", None);
version_case!(git_2_56_0_is_supported, "git version 2.56.0", None);
version_case!(
    apple_git_is_supported,
    "git version 2.39.5 (Apple Git-154)",
    None
);
version_case!(
    git_1_99_0_is_too_old,
    "git version 1.99.0",
    Some(text::GIT_TOO_OLD)
);
version_case!(git_3_0_0_is_supported, "git version 3.0.0", None);
version_case!(garbage_is_too_old, "hello", Some(text::GIT_TOO_OLD));
