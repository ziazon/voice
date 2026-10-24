//! Descriptor-relative private reads and writes: directory links are never followed.
use persona_builder::error::{Error, Result};
use std::{
    ffi::{CStr, CString},
    fs::{self, File, OpenOptions, Permissions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            ffi::OsStrExt,
            fs::{OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const DIRECTORY_FLAGS: libc::c_int =
    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

fn directory_error(path: &Path, source: std::io::Error) -> Error {
    if matches!(source.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
        Error::Invalid(format!(
            "{} is a link or not a folder: Persona Builder writes your history only into a real folder. Move it away and try again",
            path.display()
        ))
    } else {
        io(path, source)
    }
}

fn open_directory(parent: &OwnedFd, part: &CStr, path: &Path, create: bool) -> Result<OwnedFd> {
    // SAFETY: parent is live and part is a terminated C string; no ownership is transferred.
    let mut fd = unsafe { libc::openat(parent.as_raw_fd(), part.as_ptr(), DIRECTORY_FLAGS) };
    if fd < 0 {
        let source = std::io::Error::last_os_error();
        if !create || source.kind() != std::io::ErrorKind::NotFound {
            return Err(directory_error(path, source));
        }
        // SAFETY: the directory descriptor and C string remain live for this call.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), part.as_ptr(), 0o700) } < 0 {
            let source = std::io::Error::last_os_error();
            if source.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(directory_error(path, source));
            }
        }
        // SAFETY: parent and part remain live; flags refuse links even after mkdir races.
        fd = unsafe { libc::openat(parent.as_raw_fd(), part.as_ptr(), DIRECTORY_FLAGS) };
        if fd < 0 {
            return Err(directory_error(path, std::io::Error::last_os_error()));
        }
    }
    // SAFETY: openat returned a fresh valid descriptor, now owned exactly once.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

struct Temporary<'a> {
    directory: &'a OwnedFd,
    name: CString,
    armed: bool,
}

impl Temporary<'_> {
    fn rename(&mut self, target: &CStr) -> std::io::Result<()> {
        // SAFETY: both names are valid C strings relative to the same live directory descriptor.
        if unsafe {
            libc::renameat(
                self.directory.as_raw_fd(),
                self.name.as_ptr(),
                self.directory.as_raw_fd(),
                target.as_ptr(),
            )
        } < 0
        {
            return Err(std::io::Error::last_os_error());
        }
        self.armed = false;
        Ok(())
    }
}

impl Drop for Temporary<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        // SAFETY: directory is borrowed for this guard's lifetime and name is a C string.
        unsafe { libc::unlinkat(self.directory.as_raw_fd(), self.name.as_ptr(), 0) };
    }
}

fn plain(name: &str, endings: &[&str]) -> bool {
    !name.contains(['/', '\0'])
        && !name.starts_with('.')
        && endings.iter().any(|ending| name.ends_with(ending))
}
pub fn validate_name(name: &str) -> Result<()> {
    if !plain(name, &[".jsonl"]) {
        return Err(Error::Invalid(
            "the file name must be a plain name ending .jsonl".into(),
        ));
    }
    Ok(())
}
pub fn validate_review_name(name: &str) -> Result<()> {
    if !plain(name, &[".tsv"]) {
        return Err(Error::Invalid(
            "the review file name must be a plain name ending .tsv".into(),
        ));
    }
    Ok(())
}
fn validate_private_name(name: &str) -> Result<()> {
    if !plain(name, &[".jsonl", ".tsv"]) {
        return Err(Error::Invalid(
            "the file name must be a plain name ending .jsonl or .tsv".into(),
        ));
    }
    Ok(())
}

fn state_directory(home: &Path, slug: &str, create: bool) -> Result<(OwnedFd, PathBuf)> {
    if slug.is_empty() || slug.contains(['/', '\0']) || matches!(slug, "." | "..") {
        return Err(Error::Invalid(
            "the assistant slug must be a plain folder name".into(),
        ));
    }
    let home = fs::canonicalize(home).map_err(|e| io(home, e))?;
    let root = Path::new("/");
    let mut directory: OwnedFd = OpenOptions::new()
        .read(true)
        .custom_flags(DIRECTORY_FLAGS)
        .open(root)
        .map_err(|e| io(root, e))?
        .into();
    let mut path = PathBuf::from(root);
    for component in home.components() {
        if let Component::Normal(part) = component {
            path.push(part);
            let part = CString::new(part.as_bytes())
                .map_err(|_| Error::Invalid("invalid home folder".into()))?;
            directory = open_directory(&directory, &part, &path, false)?;
        }
    }
    for part in [".claude", "state", slug] {
        path.push(part);
        let part = CString::new(part).expect("validated folder name");
        directory = open_directory(&directory, &part, &path, create)?;
    }
    Ok((directory, path))
}

/// Atomically replace a JSONL or review file in the member's private state folder.
pub fn write_private(home: &Path, slug: &str, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    validate_private_name(name)?;
    let (directory, path) = state_directory(home, slug, true)?;
    let destination = path.join(name);
    let target = CString::new(name).expect("validated file name");
    let (mut temporary, mut file) = loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let count = NEXT.fetch_add(1, Ordering::Relaxed);
        let temporary = CString::new(format!(
            ".{name}.{}-{nanos}-{count}.tmp",
            std::process::id()
        ))
        .expect("validated file name");
        // SAFETY: directory is owned and live; the temporary C string is valid and O_EXCL creates a fresh file.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd >= 0 {
            // SAFETY: openat returned a fresh descriptor; OwnedFd takes sole ownership.
            let owned = unsafe { OwnedFd::from_raw_fd(fd) };
            break (
                Temporary {
                    directory: &directory,
                    name: temporary,
                    armed: true,
                },
                File::from(owned),
            );
        }
        let source = std::io::Error::last_os_error();
        if source.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(io(&destination, source));
        }
    };
    file.set_permissions(Permissions::from_mode(0o600))
        .map_err(|e| io(&destination, e))?;
    file.write_all(bytes).map_err(|e| io(&destination, e))?;
    file.sync_all().map_err(|e| io(&destination, e))?;
    temporary.rename(&target).map_err(|e| io(&destination, e))?;
    Ok(destination)
}

/// Read a corpus or review file from the private state folder.
pub fn read_private(home: &Path, slug: &str, name: &str) -> Result<Vec<u8>> {
    validate_private_name(name)?;
    let missing = |error| match error {
        Error::Io { path, source } if source.kind() == std::io::ErrorKind::NotFound => {
            Error::Invalid(format!("{} doesn't exist", path.display()))
        }
        other => other,
    };
    let (directory, path) = state_directory(home, slug, false).map_err(missing)?;
    let path = path.join(name);
    let name = CString::new(name).expect("validated file name");
    // SAFETY: directory is live and name is terminated; flags refuse links and avoid blocking on pipes.
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(missing(directory_error(
            &path,
            std::io::Error::last_os_error(),
        )));
    }
    // SAFETY: openat returned a fresh valid descriptor, owned exactly once from here.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut file = File::from(owned);
    if !file.metadata().map_err(|e| io(&path, e))?.is_file() {
        return Err(Error::Invalid(format!("{} is not a file", path.display())));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| io(&path, e))?;
    Ok(bytes)
}

/// An exclusive `flock` on the member's state folder, held until dropped: one build per assistant.
#[derive(Debug)]
pub struct StateLock(#[allow(dead_code)] OwnedFd);

/// Lock the member's state folder (created 0700 when missing), or refuse when another build holds it.
/// `flock` locks belong to the open file description, so a second lock in the same process conflicts too.
pub fn lock_state(home: &Path, slug: &str) -> Result<StateLock> {
    let (directory, path) = state_directory(home, slug, true)?;
    // SAFETY: directory is a live descriptor owned by this function.
    if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        let source = std::io::Error::last_os_error();
        if source.raw_os_error() == Some(libc::EWOULDBLOCK) {
            return Err(Error::Invalid(
                "Persona Builder is already building this assistant. Wait for that to finish, then try again"
                    .into(),
            ));
        }
        return Err(io(&path, source));
    }
    Ok(StateLock(directory))
}

/// A plain-named folder inside the member's state folder, created 0700, never through a link, and
/// refused unless only this user can open it.
pub fn private_folder(home: &Path, slug: &str, name: &str) -> Result<PathBuf> {
    if !plain(name, &[""]) || name.is_empty() {
        return Err(Error::Invalid(
            "the folder name must be a plain name".into(),
        ));
    }
    let (directory, path) = state_directory(home, slug, true)?;
    let path = path.join(name);
    let part = CString::new(name).expect("validated folder name");
    let folder = File::from(open_directory(&directory, &part, &path, true)?);
    let metadata = folder.metadata().map_err(|e| io(&path, e))?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    if metadata.permissions().mode() & 0o077 != 0
        || std::os::unix::fs::MetadataExt::uid(&metadata) != me
    {
        return Err(Error::Invalid(format!(
            "{} can be opened by others on this Mac, so Persona Builder stopped. Make it a folder only you can open, then try again",
            path.display()
        )));
    }
    Ok(path)
}

/// [`read_private`], or `None` when the state folder or the file does not exist.
pub fn read_private_optional(home: &Path, slug: &str, name: &str) -> Result<Option<Vec<u8>>> {
    validate_private_name(name)?;
    let (directory, path) = match state_directory(home, slug, false) {
        Ok(found) => found,
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(other) => return Err(other),
    };
    let target = CString::new(name).expect("validated file name");
    // SAFETY: stat is plain data; an all-zero value is valid and is overwritten by fstatat.
    let mut status: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: directory is live, target is terminated and status is a valid out pointer.
    let found = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            target.as_ptr(),
            &mut status,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if found < 0 {
        let source = std::io::Error::last_os_error();
        if source.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(io(&path.join(name), source));
    }
    read_private(home, slug, name).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renamed_temporary_does_not_unlink_a_reused_name() {
        let path =
            std::env::temp_dir().join(format!("persona-builder-guard-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let directory: OwnedFd = File::open(&path).unwrap().into();
        let source = path.join("source.tmp");
        let target = path.join("target.jsonl");
        fs::write(&source, b"original").unwrap();
        let mut temporary = Temporary {
            directory: &directory,
            name: CString::new("source.tmp").unwrap(),
            armed: true,
        };
        temporary
            .rename(&CString::new("target.jsonl").unwrap())
            .unwrap();
        fs::write(&source, b"new owner").unwrap();
        drop(temporary);
        let reused = fs::read(&source);
        let renamed = fs::read(&target).unwrap();
        fs::remove_dir_all(&path).unwrap();
        assert_eq!(reused.unwrap(), b"new owner");
        assert_eq!(renamed, b"original");
    }
}
