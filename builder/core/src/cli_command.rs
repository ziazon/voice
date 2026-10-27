use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub const CLI_FOLDER: &str = "Persona Builder can't be used from a folder whose name has \" $ ` \\ ! { or } in it. Move Persona Builder to the Applications folder in your home folder, then try again.";

pub const CLI_PLACE: &str = "Persona Builder needs to be installed before it can make an assistant. Install it with install.sh, as \"How to install.txt\" shows (both are in the Persona Builder folder you were sent), then open it from the Applications folder in your home folder.";

/// The `persona-builder` command that ships with this program: this program itself when it is
/// that command, else the `persona-builder` beside it (the app's `Contents/MacOS`).
pub fn shipped_cli() -> Result<PathBuf> {
    let executable = std::env::current_exe().map_err(|source| Error::Io {
        path: PathBuf::from("persona-builder"),
        source,
    })?;
    let path = executable.canonicalize().map_err(|source| Error::Io {
        path: executable,
        source,
    })?;
    if path.file_name() == Some(std::ffi::OsStr::new("persona-builder")) {
        Ok(path)
    } else {
        Ok(path
            .parent()
            .expect("executable has parent")
            .join("persona-builder"))
    }
}

/// `path` as one shell word for a SKILL.md: `"$HOME/<rest>"` when under `home`, else `"<path>"`.
pub fn cli_word(path: &Path, home: &Path) -> Result<String> {
    let home = home.canonicalize().map_err(|source| Error::Io {
        path: home.to_path_buf(),
        source,
    })?;
    let text = path
        .to_str()
        .ok_or_else(|| Error::Invalid(CLI_FOLDER.into()))?;
    if text.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '"' | '$' | '`' | '\\' | '!' | '{' | '}' | '\u{2028}' | '\u{2029}'
            )
    }) {
        return Err(Error::Invalid(CLI_FOLDER.into()));
    }
    match path.strip_prefix(home) {
        Ok(rest) => Ok(format!(
            "\"$HOME/{}\"",
            rest.to_str().expect("UTF-8 path checked")
        )),
        Err(_) => Ok(format!("\"{text}\"")),
    }
}

/// `shipped_cli()` as one shell word for a SKILL.md (see `cli_word`). A release build refuses
/// with `CLI_PLACE` unless the command is in a place that lasts (see `lasting`); a development
/// build runs from its own build folder, wherever that is.
pub fn shipped_cli_word(home: &Path) -> Result<String> {
    word_from(&shipped_cli()?, home, !cfg!(debug_assertions))
}

fn word_from(path: &Path, home: &Path, lasting_only: bool) -> Result<String> {
    if lasting_only {
        lasting(
            path,
            &[&home.join("Applications"), Path::new("/Applications")],
        )?;
    }
    cli_word(path, home)
}

fn lasting(path: &Path, roots: &[&Path]) -> Result<()> {
    let canonical = path.canonicalize().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if on_a_passing_disk(&canonical) {
        return Err(Error::Invalid(CLI_PLACE.into()));
    }
    if !roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| canonical.starts_with(root))
    {
        return Err(Error::Invalid(CLI_PLACE.into()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid takes no arguments and cannot fail.
        let euid = unsafe { libc::geteuid() };
        for folder in canonical.ancestors() {
            let metadata = std::fs::symlink_metadata(folder).map_err(|source| Error::Io {
                path: folder.to_path_buf(),
                source,
            })?;
            if (metadata.uid() != euid && metadata.uid() != 0) || metadata.mode() & 0o002 != 0 {
                return Err(Error::Invalid(CLI_PLACE.into()));
            }
        }
    }
    Ok(())
}

fn on_a_passing_disk(canonical: &Path) -> bool {
    canonical.starts_with("/Volumes")
        || canonical
            .components()
            .any(|c| c.as_os_str() == "AppTranslocation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use std::fs;

    #[test]
    fn a_missing_command_is_an_io_error() {
        let root = Scratch::new();
        assert!(matches!(
            lasting(&root.path().join("absent"), &[root.path()]),
            Err(Error::Io { .. })
        ));
    }

    #[test]
    fn outside_every_root_is_refused() {
        let place = Scratch::new();
        let root = Scratch::new();
        let command = command_at(place.path(), "persona-builder");
        assert!(matches!(
            lasting(&command, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[test]
    fn a_translocated_copy_under_a_root_is_refused() {
        let root = Scratch::new();
        let command = command_at(
            root.path(),
            "AppTranslocation/1234/d/Persona Builder.app/Contents/MacOS/persona-builder",
        );
        assert!(matches!(
            lasting(&command, &[root.path()]),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_others_can_write_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let root = Scratch::new();
        let command = command_at(root.path(), "open/persona-builder");
        let open = root.path().join("open");
        fs::set_permissions(&open, fs::Permissions::from_mode(0o757)).unwrap();
        let folder_result = lasting(&command, &[root.path()]);
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o646)).unwrap();
        let file_result = lasting(&command, &[root.path()]);
        fs::set_permissions(&command, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(folder_result, Err(Error::Invalid(message)) if message == CLI_PLACE));
        assert!(matches!(file_result, Err(Error::Invalid(message)) if message == CLI_PLACE));
    }

    #[test]
    fn the_place_rule_runs_only_when_asked() {
        let place = Scratch::new();
        let home = Scratch::new();
        let command = command_at(place.path(), "persona-builder");
        assert_eq!(
            word_from(&command, home.path(), false).unwrap(),
            cli_word(&command, home.path()).unwrap()
        );
        assert!(matches!(
            word_from(&command, home.path(), true),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_development_build_skips_the_place_rule() {
        let home = Scratch::new();
        assert_eq!(
            shipped_cli_word(home.path()).ok(),
            Some(cli_word(&shipped_cli().unwrap(), home.path()).unwrap())
        );
    }

    fn command_at(root: &Path, relative: &str) -> PathBuf {
        let command = root.join(relative);
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::write(&command, "command").unwrap();
        command
    }

    // macOS has a per-user temp folder; Linux's /tmp is writable by others and refused.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_root_folder_owned_by_the_person_is_lasting() {
        use std::os::unix::fs::PermissionsExt;
        let root = Scratch::new();
        let command = command_at(
            root.path(),
            "Applications/Persona Builder.app/Contents/MacOS/persona-builder",
        );
        assert!(lasting(&command, &[root.path()]).is_ok());
        assert_eq!(
            word_from(&command, root.path(), true).unwrap(),
            cli_word(&command, root.path()).unwrap()
        );
        let applications = root.path().join("Applications");
        fs::set_permissions(&applications, fs::Permissions::from_mode(0o775)).unwrap();
        assert!(lasting(&command, &[root.path()]).is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn only_the_applications_folder_in_home_is_lasting() {
        let home = Scratch::new();
        let command = command_at(
            home.path(),
            "Applications/Persona Builder.app/Contents/MacOS/persona-builder",
        );
        assert_eq!(
            word_from(&command, home.path(), true).unwrap(),
            cli_word(&command, home.path()).unwrap()
        );
        let download = command_at(
            home.path(),
            "Downloads/Persona Builder/Persona Builder.app/Contents/MacOS/persona-builder",
        );
        assert!(matches!(
            word_from(&download, home.path(), true),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
        let applications_old = command_at(
            home.path(),
            "ApplicationsOld/Persona Builder.app/Contents/MacOS/persona-builder",
        );
        assert!(matches!(
            word_from(&applications_old, home.path(), true),
            Err(Error::Invalid(message)) if message == CLI_PLACE
        ));
    }

    #[test]
    fn a_passing_disk_is_the_volumes_folder_or_a_translocated_copy() {
        for path in [
            "/Volumes/Persona Builder/Persona Builder.app/Contents/MacOS/persona-builder",
            "/Volumes",
            "/private/var/folders/ab/cd/T/AppTranslocation/1234-ABCD/d/Persona Builder.app/Contents/MacOS/persona-builder",
        ] {
            assert!(on_a_passing_disk(Path::new(path)), "accepted {path}");
        }
        for path in [
            "/VolumesX/persona-builder",
            "/Applications/Persona Builder.app/Contents/MacOS/persona-builder",
            "/Users/robin/AppTranslocationNotes/persona-builder",
        ] {
            assert!(!on_a_passing_disk(Path::new(path)), "refused {path}");
        }
    }

    #[test]
    fn shipped_cli_is_beside_the_canonical_executable() {
        let executable = std::env::current_exe().unwrap().canonicalize().unwrap();
        let expected = executable.parent().unwrap().join("persona-builder");
        assert_eq!(shipped_cli().unwrap(), expected);
    }
    #[test]
    fn under_home_is_one_word_with_home_expansion_and_spaces() {
        let home = Scratch::new();
        let path = home
            .path()
            .canonicalize()
            .unwrap()
            .join("Applications/Persona Builder.app/Contents/MacOS/persona-builder");
        assert_eq!(
            cli_word(&path, home.path()).unwrap(),
            "\"$HOME/Applications/Persona Builder.app/Contents/MacOS/persona-builder\""
        );
    }
    #[test]
    fn outside_home_is_one_quoted_word() {
        let home = Scratch::new();
        let other = Scratch::new();
        let path = other.path().join("Persona Builder.app/persona-builder");
        assert_eq!(
            cli_word(&path, home.path()).unwrap(),
            format!("\"{}\"", path.display())
        );
    }
    #[test]
    fn refuses_shell_characters_and_controls_in_the_whole_path() {
        let home = Scratch::new();
        for c in [
            '"', '$', '`', '\\', '!', '{', '}', '\0', '\n', '\r', '\t', '\u{7f}', '\u{85}',
            '\u{2028}', '\u{2029}',
        ] {
            for path in [
                home.path().join(format!("bad{c}/persona-builder")),
                PathBuf::from(format!("/bad{c}/persona-builder")),
            ] {
                assert!(
                    matches!(cli_word(&path, home.path()), Err(Error::Invalid(message)) if message == CLI_FOLDER),
                    "accepted {path:?}"
                );
            }
            if !c.is_control() && c != '\\' {
                let unsafe_home = home.path().join(format!("home{c}"));
                std::fs::create_dir(&unsafe_home).unwrap();
                let path = unsafe_home.canonicalize().unwrap().join("persona-builder");
                assert!(
                    matches!(cli_word(&path, &unsafe_home), Err(Error::Invalid(message)) if message == CLI_FOLDER),
                    "accepted unsafe home: {path:?}"
                );
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn refuses_non_utf8_paths() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        let home = Scratch::new();
        assert!(
            matches!(cli_word(Path::new(OsStr::from_bytes(b"/bad\xff/persona-builder")), home.path()), Err(Error::Invalid(message)) if message == CLI_FOLDER)
        );
    }
    #[test]
    fn missing_home_is_an_io_error() {
        let home = Scratch::new();
        assert!(matches!(
            cli_word(
                Path::new("/Applications/persona-builder"),
                &home.path().join("absent")
            ),
            Err(Error::Io { .. })
        ));
    }
}
