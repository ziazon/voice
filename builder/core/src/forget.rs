use crate::{
    error::{Error, Result},
    install_fs::io,
    member::{read_member, refuse_symlink},
};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub struct Forgotten {
    pub path: PathBuf,
    pub files: usize,
    pub existed: bool,
}

fn count_files(path: &Path) -> Result<usize> {
    let mut files = 0;
    for entry in io(path, fs::read_dir(path))? {
        let path = io(path, entry)?.path();
        if io(&path, fs::symlink_metadata(&path))?.is_dir() {
            files += count_files(&path)?;
        } else {
            files += 1;
        }
    }
    Ok(files)
}

pub fn forget(member_dir: &Path, home: &Path, delete: bool) -> Result<Forgotten> {
    let member = read_member(&member_dir.join("member.json"))?;
    let path = match member.state_dir.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(&member.state_dir),
    };
    let expected = home.join(".claude/state").join(&member.slug);
    // Path::components normalizes interior dots, so reject them in the written path too.
    let has_dots = path
        .as_os_str()
        .as_encoded_bytes()
        .split(|byte| *byte == b'/')
        .any(|part| matches!(part, b"." | b".."));
    let mut slug = Path::new(&member.slug).components();
    let single_slug = matches!(slug.next(), Some(Component::Normal(_))) && slug.next().is_none();
    if !single_slug
        || !path.is_absolute()
        || has_dots
        || !path.components().eq(expected.components())
    {
        return Err(Error::Invalid(format!(
            "state_dir {} is not ~/.claude/state/{}, so Persona Builder will not delete it. Delete that folder yourself if you mean to",
            member.state_dir, member.slug
        )));
    }
    refuse_symlink(&home.join(".claude/state"))?;
    refuse_symlink(&path)?;
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Forgotten {
                path,
                files: 0,
                existed: false,
            });
        }
        metadata => {
            if !io(&path, metadata)?.is_dir() {
                return Err(Error::Invalid(format!(
                    "{} is not a folder. Delete it yourself if you mean to",
                    path.display()
                )));
            }
        }
    }
    let files = count_files(&path)?;
    if delete {
        io(&path, fs::remove_dir_all(&path))?;
    }
    Ok(Forgotten {
        path,
        files,
        existed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crew::{NewMember, new_member},
        test_support::{Scratch, repo},
    };
    use std::fs;

    fn fixture(scratch: &Scratch) -> (PathBuf, PathBuf, PathBuf) {
        let member = scratch.path().join("ari");
        new_member(
            &repo().join("crew/_template"),
            &member,
            &NewMember {
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
                repo_shown: "~/voice",
                date: "2026-10-24",
            },
        )
        .unwrap();
        let home = scratch.path().join("home");
        let state = home.join(".claude/state/ari");
        fs::create_dir_all(state.join("drafts/empty")).unwrap();
        fs::write(state.join("history.json"), "invented history").unwrap();
        fs::write(state.join("drafts/notes.md"), "invented draft").unwrap();
        (member, home, state)
    }

    #[test]
    fn preview_counts_nested_files_without_deleting() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        let result = forget(&member, &home, false).unwrap();
        assert_eq!(result.files, 2);
        assert!(result.existed);
        assert_eq!(result.path, state);
        assert_eq!(
            fs::read_to_string(state.join("history.json")).unwrap(),
            "invented history"
        );
        assert_eq!(
            fs::read_to_string(state.join("drafts/notes.md")).unwrap(),
            "invented draft"
        );
        assert!(state.join("drafts/empty").is_dir());
    }
    fn snapshot(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            let name = PathBuf::from(path.file_name().unwrap());
            if path.is_dir() {
                entries.push((name.clone(), Vec::new()));
                entries.extend(
                    snapshot(&path)
                        .into_iter()
                        .map(|(child, bytes)| (name.join(child), bytes)),
                );
            } else {
                entries.push((name, fs::read(path).unwrap()));
            }
        }
        entries.sort();
        entries
    }

    #[test]
    fn delete_preserves_member_bytes_and_second_run_is_missing() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        let before = snapshot(&member);
        let result = forget(&member, &home, true).unwrap();
        assert!(!state.exists());
        assert_eq!(result.path, state);
        assert_eq!(result.files, 2);
        assert!(result.existed);
        assert_eq!(snapshot(&member), before);
        let again = forget(&member, &home, true).unwrap();
        assert!(!again.existed);
        assert_eq!(again.files, 0);
        assert_eq!(again.path, state);
    }

    fn set_state(member: &Path, state: &str) {
        let path = member.join("member.json");
        let mut config = read_member(&path).unwrap();
        config.state_dir = state.into();
        crate::member::write_member(&path, &config).unwrap();
    }

    #[test]
    fn refuses_every_state_path_except_exact_member_folder() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        for written in [
            "~",
            "~/.claude/state",
            "~/.claude/state/other",
            "~/.claude/state/ari/..",
            "state/ari",
            "/tmp/x",
            "~/.claude/state/./ari",
            "~/.claude/./state/ari",
            "~/.claude/state/ari/.",
            "~/.claude/state/ari/drafts",
        ] {
            set_state(&member, written);
            let result = forget(&member, &home, true);
            assert!(
                matches!(result, Err(Error::Invalid(ref message)) if message == &format!(
                    "state_dir {written} is not ~/.claude/state/ari, so Persona Builder will not delete it. Delete that folder yourself if you mean to"
                )),
                "{written}: {result:?}"
            );
            assert_eq!(
                fs::read_to_string(state.join("history.json")).unwrap(),
                "invented history"
            );
            assert_eq!(
                fs::read_to_string(state.join("drafts/notes.md")).unwrap(),
                "invented draft"
            );
        }
        set_state(&member, state.to_str().unwrap());
        assert_eq!(forget(&member, &home, false).unwrap().files, 2);
    }

    #[test]
    fn private_state_override_is_checked() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        fs::write(member.join("private.json"), r#"{"state_dir":"~"}"#).unwrap();
        let before = snapshot(&member);
        let result = forget(&member, &home, true);
        assert!(
            matches!(result, Err(Error::Invalid(ref message)) if message.starts_with("state_dir ~ is not")),
            "{result:?}"
        );
        assert!(state.join("history.json").is_file());
        assert_eq!(snapshot(&member), before);
    }

    #[test]
    fn refuses_state_symlink_without_touching_target() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        let target = scratch.path().join("outside");
        fs::rename(&state, &target).unwrap();
        std::os::unix::fs::symlink(&target, &state).unwrap();
        let before = snapshot(&target);
        let result = forget(&member, &home, true);
        assert!(
            matches!(result, Err(Error::Invalid(ref message)) if message == &format!("{}: symlink refused", state.display())),
            "{result:?}"
        );
        assert!(fs::symlink_metadata(&state).unwrap().is_symlink());
        assert_eq!(snapshot(&target), before);
    }

    #[test]
    fn refuses_state_that_is_a_regular_file() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        fs::remove_dir_all(&state).unwrap();
        fs::write(&state, "keep this file").unwrap();
        let result = forget(&member, &home, true);
        assert!(
            matches!(result, Err(Error::Invalid(ref message)) if message == &format!("{} is not a folder. Delete it yourself if you mean to", state.display())),
            "{result:?}"
        );
        assert_eq!(fs::read_to_string(&state).unwrap(), "keep this file");
    }

    #[test]
    fn counts_and_removes_inner_symlinks_without_following_them() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        let outside = scratch.path().join("outside.md");
        fs::write(&outside, "keep outside").unwrap();
        std::os::unix::fs::symlink(&outside, state.join("linked-file")).unwrap();
        std::os::unix::fs::symlink(scratch.path().join("missing"), state.join("dangling")).unwrap();
        let outside_dir = scratch.path().join("outside-dir");
        fs::create_dir(&outside_dir).unwrap();
        fs::write(outside_dir.join("keep.md"), "keep directory").unwrap();
        std::os::unix::fs::symlink(&outside_dir, state.join("linked-dir")).unwrap();
        assert_eq!(forget(&member, &home, false).unwrap().files, 5);
        assert!(state.join("linked-file").is_symlink());
        assert_eq!(forget(&member, &home, true).unwrap().files, 5);
        assert!(!state.exists());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "keep outside");
        assert_eq!(
            fs::read_to_string(outside_dir.join("keep.md")).unwrap(),
            "keep directory"
        );
    }

    #[test]
    fn slug_cannot_replace_the_expected_state_root() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        let path = member.join("member.json");
        let mut config = read_member(&path).unwrap();
        config.slug = state.to_str().unwrap().into();
        config.state_dir = state.to_str().unwrap().into();
        crate::member::write_member(&path, &config).unwrap();
        let result = forget(&member, &home, true);
        assert!(matches!(result, Err(Error::Invalid(_))), "{result:?}");
        assert!(state.join("history.json").is_file());
    }

    #[test]
    fn refuses_symlink_at_state_parent() {
        let scratch = Scratch::new();
        let (member, home, state) = fixture(&scratch);
        let parent = state.parent().unwrap();
        let target = scratch.path().join("outside-state");
        fs::rename(parent, &target).unwrap();
        std::os::unix::fs::symlink(&target, parent).unwrap();
        let result = forget(&member, &home, true);
        assert!(
            matches!(result, Err(Error::Invalid(ref message)) if message == &format!("{}: symlink refused", parent.display())),
            "{result:?}"
        );
        assert_eq!(
            fs::read_to_string(target.join("ari/history.json")).unwrap(),
            "invented history"
        );
    }
}
