use crate::error::{Error, Result};
use std::path::{Path, PathBuf};

pub const CLI_FOLDER: &str = "Persona Builder can't be used from a folder whose name has \" $ ` \\ ! { or } in it. Move Persona Builder to the Applications folder in your home folder, then try again.";

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
    if text
        .chars()
        .any(|c| c.is_control() || matches!(c, '"' | '$' | '`' | '\\' | '!' | '{' | '}'))
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
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
