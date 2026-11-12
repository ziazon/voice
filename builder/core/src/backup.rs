//! Verified, private backups of an older assistant's files.
use crate::{
    crew,
    error::{Error, Result},
    install_fs::io,
};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

pub struct Item<'a> {
    pub name: &'a str,
    pub source: &'a Path,
    pub follow: bool,
}

pub fn back_up(home: &Path, slug: &str, stamp: &str, items: &[Item<'_>]) -> Result<PathBuf> {
    if crew::slugify(slug).as_deref() != Some(slug) {
        return Err(Error::Invalid("invalid assistant slug".into()));
    }
    if stamp.len() != 17
        || !stamp.bytes().enumerate().all(|(n, b)| {
            if matches!(n, 4 | 7 | 10) {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return Err(Error::Invalid("invalid backup date".into()));
    }
    if items
        .iter()
        .any(|item| !matches!(item.name, "state" | "skill" | "CLAUDE.md" | "member"))
    {
        return Err(Error::Invalid("invalid backup item".into()));
    }
    let claude = home.join(".claude");
    io(&claude, fs::create_dir_all(&claude))?;
    let root = claude.join("voice-backups");
    private_parent(&root)?;
    let parent = root.join(slug);
    private_parent(&parent)?;
    let backup = new_backup(&parent, stamp)?;
    let result = (|| {
        io(
            &backup,
            fs::set_permissions(&backup, fs::Permissions::from_mode(0o700)),
        )?;
        for item in items {
            let source = item_source(item)?;
            copy_entry(&source, &backup.join(item.name))?;
        }
        // A successful copy alone does not prove the older assistant stayed unchanged.
        for item in items {
            let source = item_source(item)?;
            verify_entry(&source, &backup.join(item.name))?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        io(&backup, fs::remove_dir_all(&backup))?;
        return Err(error);
    }
    Ok(backup)
}

fn private_parent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(Error::Invalid(
            "Voice's backup folder ~/.claude/voice-backups isn't a plain folder, so it backed nothing up.".into(),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            io(path, fs::DirBuilder::new().mode(0o700).create(path))?;
            io(path, fs::set_permissions(path, fs::Permissions::from_mode(0o700)))
        }
        result => io(path, result).map(|_| ()),
    }
}

fn new_backup(parent: &Path, stamp: &str) -> Result<PathBuf> {
    for n in 1..=9 {
        let path = parent.join(if n == 1 {
            stamp.to_owned()
        } else {
            format!("{stamp}-{n}")
        });
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            result => io(&path, result)?,
        }
    }
    Err(Error::Invalid(
        "Voice couldn't make a new backup folder, so it backed nothing up. Try again in a moment."
            .into(),
    ))
}

fn not_plain(path: &Path) -> Error {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    Error::Invalid(format!(
        "{name} isn't a plain file, so Voice backed nothing up."
    ))
}

fn item_source(item: &Item<'_>) -> Result<PathBuf> {
    if !item.follow {
        return Ok(item.source.to_owned());
    }
    // Only the item root may be followed. Once resolved, regular-file opens still refuse links.
    let path = io(item.source, fs::canonicalize(item.source))?;
    if !io(&path, fs::symlink_metadata(&path))?.is_file() {
        return Err(not_plain(item.source));
    }
    Ok(path)
}

fn entries(path: &Path) -> Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in io(path, fs::read_dir(path))? {
        names.push(io(path, entry)?.file_name());
    }
    names.sort();
    Ok(names)
}

fn open_regular(path: &Path) -> Result<File> {
    let file = io(
        path,
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path),
    )?;
    if !io(path, file.metadata())?.is_file() {
        return Err(not_plain(path));
    }
    Ok(file)
}

fn copy_entry(source: &Path, destination: &Path) -> Result<()> {
    let kind = io(source, fs::symlink_metadata(source))?.file_type();
    if kind.is_dir() {
        io(
            destination,
            fs::DirBuilder::new().mode(0o700).create(destination),
        )?;
        io(
            destination,
            fs::set_permissions(destination, fs::Permissions::from_mode(0o700)),
        )?;
        for name in entries(source)? {
            copy_entry(&source.join(&name), &destination.join(&name))?;
        }
    } else if kind.is_file() {
        let mut input = open_regular(source)?;
        let mut output = io(
            destination,
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(destination),
        )?;
        io(
            destination,
            output.set_permissions(fs::Permissions::from_mode(0o600)),
        )?;
        io(destination, std::io::copy(&mut input, &mut output))?;
    } else if kind.is_symlink() {
        let target = io(source, fs::read_link(source))?;
        io(destination, symlink(target, destination))?;
    } else {
        return Err(not_plain(source));
    }
    Ok(())
}

fn different() -> Error {
    Error::Invalid("The older assistant's files changed while Voice backed them up, so it kept no backup. Try again.".into())
}

fn verify_entry(source: &Path, destination: &Path) -> Result<()> {
    let kind = io(source, fs::symlink_metadata(source))?.file_type();
    if kind != io(destination, fs::symlink_metadata(destination))?.file_type() {
        return Err(different());
    }
    if kind.is_dir() {
        let names = entries(source)?;
        if names != entries(destination)? {
            return Err(different());
        }
        for name in names {
            verify_entry(&source.join(&name), &destination.join(&name))?;
        }
    } else if kind.is_file() {
        let mut input = open_regular(source)?;
        let mut copied = open_regular(destination)?;
        let mut left = [0; 64 * 1024];
        let mut right = [0; 64 * 1024];
        loop {
            let count = io(source, input.read(&mut left))?;
            if count == 0 {
                if io(destination, copied.read(&mut right[..1]))? != 0 {
                    return Err(different());
                }
                break;
            }
            match copied.read_exact(&mut right[..count]) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(different());
                }
                result => io(destination, result)?,
            }
            if left[..count] != right[..count] {
                return Err(different());
            }
        }
    } else if kind.is_symlink() {
        if io(source, fs::read_link(source))? != io(destination, fs::read_link(destination))? {
            return Err(different());
        }
    } else {
        return Err(not_plain(source));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    const STAMP: &str = "2026-11-12-093000";

    fn item(source: &Path) -> Item<'_> {
        Item {
            name: "state",
            source,
            follow: false,
        }
    }

    #[test]
    fn copies_nested_tree_bytes_permissions_and_dangling_link() {
        let scratch = Scratch::new();
        let source = scratch.path().join("old");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("one"), b"first invented bytes\0").unwrap();
        fs::write(source.join("nested/two"), vec![42; 100_000]).unwrap();
        symlink("does-not-exist", source.join("dangling")).unwrap();
        let backup = back_up(scratch.path(), "ari", STAMP, &[item(&source)]).unwrap();
        assert_eq!(
            backup,
            scratch.path().join(".claude/voice-backups/ari").join(STAMP)
        );
        for path in [
            ".claude/voice-backups",
            ".claude/voice-backups/ari",
            ".claude/voice-backups/ari/2026-11-12-093000",
            ".claude/voice-backups/ari/2026-11-12-093000/state",
            ".claude/voice-backups/ari/2026-11-12-093000/state/nested",
        ] {
            assert_eq!(
                fs::metadata(scratch.path().join(path))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        for path in ["one", "nested/two"] {
            assert_eq!(
                fs::read(backup.join("state").join(path)).unwrap(),
                fs::read(source.join(path)).unwrap()
            );
            assert_eq!(
                fs::metadata(backup.join("state").join(path))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert!(
            fs::symlink_metadata(backup.join("state/dangling"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(
            fs::read_link(backup.join("state/dangling")).unwrap(),
            Path::new("does-not-exist")
        );
    }

    #[test]
    fn follows_only_requested_claude_file_root() {
        let scratch = Scratch::new();
        let target = scratch.path().join("dotfile");
        fs::write(&target, "invented instructions").unwrap();
        let source = scratch.path().join("CLAUDE.md");
        symlink(&target, &source).unwrap();
        let backup = back_up(
            scratch.path(),
            "ari",
            STAMP,
            &[Item {
                name: "CLAUDE.md",
                source: &source,
                follow: true,
            }],
        )
        .unwrap();
        let copied = backup.join("CLAUDE.md");
        assert!(fs::symlink_metadata(&copied).unwrap().is_file());
        assert_eq!(fs::read(copied).unwrap(), b"invented instructions");
    }

    #[test]
    fn fifo_refuses_without_waiting_and_removes_partial_backup() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let scratch = Scratch::new();
        let source = scratch.path().join("old");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("a-file"), "invented").unwrap();
        let fifo = CString::new(source.join("nested/pipe").as_os_str().as_bytes()).unwrap();
        // SAFETY: fifo is a live, NUL-terminated path and mode is a valid permission mask.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let start = std::time::Instant::now();
        let result = back_up(scratch.path(), "ari", STAMP, &[item(&source)]);
        assert!(
            matches!(result, Err(crate::error::Error::Invalid(message)) if message == "pipe isn't a plain file, so Voice backed nothing up.")
        );
        assert!(start.elapsed().as_secs() < 2);
        assert!(
            !scratch
                .path()
                .join(".claude/voice-backups/ari")
                .join(STAMP)
                .exists()
        );
    }

    #[test]
    fn linked_backup_parent_refuses_without_writing_through_it() {
        let scratch = Scratch::new();
        let outside = scratch.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::create_dir(scratch.path().join(".claude")).unwrap();
        symlink(&outside, scratch.path().join(".claude/voice-backups")).unwrap();
        let result = back_up(scratch.path(), "ari", STAMP, &[]);
        assert!(
            matches!(result, Err(crate::error::Error::Invalid(message)) if message == "Voice's backup folder ~/.claude/voice-backups isn't a plain folder, so it backed nothing up.")
        );
        assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    }

    #[test]
    fn repeated_stamp_uses_second_folder() {
        let scratch = Scratch::new();
        let first = back_up(scratch.path(), "ari", STAMP, &[]).unwrap();
        let second = back_up(scratch.path(), "ari", STAMP, &[]).unwrap();
        assert_eq!(first.file_name().unwrap(), STAMP);
        assert_eq!(second.file_name().unwrap(), format!("{STAMP}-2").as_str());
        assert!(first.is_dir());
        assert!(second.is_dir());
    }

    #[test]
    fn invalid_slug_refuses_before_writing() {
        let scratch = Scratch::new();
        for slug in ["", "../ari", "Ari", "ari/x"] {
            assert!(back_up(scratch.path(), slug, STAMP, &[]).is_err());
        }
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
    }
}
