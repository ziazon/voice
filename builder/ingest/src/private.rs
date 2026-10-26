//! Descriptor-relative private reads and writes: directory links are never followed.
use persona_builder::error::{Error, Result};
use std::{
    ffi::{CStr, CString},
    fs::{self, File, OpenOptions, Permissions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const BUSY: &str = "Persona Builder is already working on this assistant in another window. Wait for it to finish, then try again";

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
pub fn validate_json_name(name: &str) -> Result<()> {
    if !plain(name, &[".json"]) {
        return Err(Error::Invalid(
            "the file name must be a plain name ending .json".into(),
        ));
    }
    Ok(())
}
fn validate_private_name(name: &str) -> Result<()> {
    if !plain(name, &[".jsonl", ".tsv", ".json"]) {
        return Err(Error::Invalid(
            "the file name must be a plain name ending .jsonl, .json or .tsv".into(),
        ));
    }
    Ok(())
}
fn validate_folder_name(name: &str) -> Result<()> {
    if !plain(name, &[""]) || name.is_empty() {
        return Err(Error::Invalid(
            "the folder name must be a plain name".into(),
        ));
    }
    Ok(())
}
/// Split a file argument into its folder, if any, and its name: `NAME` or `FOLDER/NAME` inside the
/// member's state folder, never deeper and never a path of its own.
pub fn place(argument: &str) -> Result<(Option<&str>, &str)> {
    let wrong = || {
        Error::Invalid(format!(
            "{argument}: a file here is NAME or FOLDER/NAME inside the assistant's private folder"
        ))
    };
    match argument.split_once('/') {
        None => Ok((None, argument)),
        Some((folder, name)) => {
            validate_folder_name(folder).map_err(|_| wrong())?;
            if name.contains('/') {
                return Err(wrong());
            }
            Ok((Some(folder), name))
        }
    }
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

/// The member's state folder, or a plain folder inside it: made 0700 when `create`, never through a
/// link. The state folder is refused when others can change it, a folder inside it when others can
/// open it.
fn place_directory(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    create: bool,
) -> Result<(OwnedFd, PathBuf)> {
    let (directory, state) = state_directory(home, slug, create)?;
    // The assistant's folder may be readable by others (an older build left 0750), never writable.
    let parent = File::from(directory.try_clone().map_err(|e| io(&state, e))?);
    only_mine(&parent, &state, 0o022, "changed")?;
    let Some(folder) = folder else {
        return Ok((directory, state));
    };
    validate_folder_name(folder)?;
    let path = state.join(folder);
    let part = CString::new(folder).expect("validated folder name");
    let inner = File::from(open_directory(&directory, &part, &path, create)?);
    only_mine(&inner, &path, 0o077, "opened")?;
    Ok((OwnedFd::from(inner), path))
}

/// Atomically replace a JSONL or review file in the member's private state folder.
pub fn write_private(home: &Path, slug: &str, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    write_private_at(home, slug, None, name, bytes)
}

/// [`write_private`], into `folder` inside the state folder when one is given.
pub fn write_private_at(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
    bytes: &[u8],
) -> Result<PathBuf> {
    validate_private_name(name)?;
    let (directory, path) = place_directory(home, slug, folder, true)?;
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

/// Remove a private file relative to its state folder, without following directory links.
pub fn remove_private(home: &Path, slug: &str, name: &str) -> Result<bool> {
    validate_private_name(name)?;
    let (directory, path) = match place_directory(home, slug, None, false) {
        Ok(value) => value,
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(other) => return Err(other),
    };
    let target = CString::new(name).expect("validated file name");
    // SAFETY: directory is live and target is a valid C string relative to that descriptor.
    if unsafe { libc::unlinkat(directory.as_raw_fd(), target.as_ptr(), 0) } == 0 {
        return Ok(true);
    }
    let source = std::io::Error::last_os_error();
    if source.kind() == std::io::ErrorKind::NotFound {
        Ok(false)
    } else {
        Err(io(&path.join(name), source))
    }
}

/// Read a corpus or review file from the private state folder.
pub fn read_private(home: &Path, slug: &str, name: &str) -> Result<Vec<u8>> {
    read_private_at(home, slug, None, name)
}

/// [`read_private`], from `folder` inside the state folder when one is given; creates nothing.
pub fn read_private_at(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
) -> Result<Vec<u8>> {
    read_private_at_limit(home, slug, folder, name, None)
}

/// Read a private file, refusing an opened file larger than `max` before reading its bytes.
pub fn read_private_at_max(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
    max: u64,
) -> Result<Vec<u8>> {
    read_private_at_limit(home, slug, folder, name, Some(max))
}

fn read_private_at_limit(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
    max: Option<u64>,
) -> Result<Vec<u8>> {
    validate_private_name(name)?;
    let missing = |error| match error {
        Error::Io { path, source } if source.kind() == std::io::ErrorKind::NotFound => {
            Error::Invalid(format!("{} doesn't exist", path.display()))
        }
        other => other,
    };
    let (directory, path) = place_directory(home, slug, folder, false).map_err(missing)?;
    let path = path.join(name);
    let file_name = name;
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
        let source = std::io::Error::last_os_error();
        if source.raw_os_error() == Some(libc::ELOOP) {
            return Err(Error::Invalid(format!(
                "{} is a link: Persona Builder reads only real files, never links. Move it away and try again",
                path.display()
            )));
        }
        return Err(missing(directory_error(&path, source)));
    }
    // SAFETY: openat returned a fresh valid descriptor, owned exactly once from here.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut file = File::from(owned);
    let metadata = file.metadata().map_err(|e| io(&path, e))?;
    if !metadata.is_file() {
        return Err(Error::Invalid(format!("{} is not a file", path.display())));
    }
    if let Some(max) = max
        && metadata.len() > max
    {
        return Err(Error::Invalid(format!(
            "{file_name} is too big to read (over {} KiB)",
            max / 1024
        )));
    }
    read_private_bytes(&mut file, &path, file_name, max)
}

fn read_private_bytes(
    file: &mut impl Read,
    path: &Path,
    name: &str,
    max: Option<u64>,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    if let Some(max) = max {
        file.take(max.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| io(path, e))?;
        if bytes.len() as u64 > max {
            return Err(Error::Invalid(format!(
                "{name} is too big to read (over {} KiB)",
                max / 1024
            )));
        }
    } else {
        file.read_to_end(&mut bytes).map_err(|e| io(path, e))?;
    }
    Ok(bytes)
}

/// An exclusive `flock` on the member's state folder, held until dropped: one build per assistant.
#[derive(Debug)]
pub struct StateLock(#[allow(dead_code)] OwnedFd);

/// Lock the member's state folder (created 0700 when missing), or refuse when another build holds it.
/// `flock` locks belong to the open file description, so a second lock in the same process conflicts too.
pub fn lock_state(home: &Path, slug: &str) -> Result<StateLock> {
    lock_state_folder(home, slug, true)
}

/// Lock the member's state folder only if it already exists, or refuse when another build holds it.
pub fn lock_existing_state(home: &Path, slug: &str) -> Result<StateLock> {
    lock_state_folder(home, slug, false)
}

fn lock_state_folder(home: &Path, slug: &str, create: bool) -> Result<StateLock> {
    lock_state_folder_after(home, slug, create, &mut || {})
}

fn lock_state_folder_after(
    home: &Path,
    slug: &str,
    create: bool,
    between: &mut dyn FnMut(),
) -> Result<StateLock> {
    let (directory, path) = state_directory(home, slug, create)?;
    between();
    // SAFETY: directory is a live descriptor owned by this function.
    if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        let source = std::io::Error::last_os_error();
        if source.raw_os_error() == Some(libc::EWOULDBLOCK) {
            return Err(Error::Invalid(BUSY.into()));
        }
        return Err(io(&path, source));
    }
    // The lock must hold the folder at the path, not one a forget removed between the open and the lock.
    let same_folder = match state_directory(home, slug, false) {
        Ok((current, _)) => {
            let locked = File::from(directory.try_clone().map_err(|e| io(&path, e))?)
                .metadata()
                .map_err(|e| io(&path, e))?;
            let at_path = File::from(current.try_clone().map_err(|e| io(&path, e))?)
                .metadata()
                .map_err(|e| io(&path, e))?;
            drop(current);
            locked.dev() == at_path.dev() && locked.ino() == at_path.ino()
        }
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => false,
        Err(other) => return Err(other),
    };
    if !same_folder {
        return Err(if create {
            Error::Invalid(BUSY.into())
        } else {
            io(&path, std::io::Error::from(std::io::ErrorKind::NotFound))
        });
    }
    Ok(StateLock(directory))
}

/// A plain-named folder inside the member's state folder, created 0700, never through a link, and
/// refused unless only this user can open it.
pub fn private_folder(home: &Path, slug: &str, name: &str) -> Result<PathBuf> {
    validate_folder_name(name)?;
    place_directory(home, slug, Some(name), true).map(|(_, path)| path)
}

/// Whether a private subfolder exists; creates nothing and refuses links.
pub fn private_folder_exists(home: &Path, slug: &str, folder: &str) -> Result<bool> {
    validate_folder_name(folder)?;
    match place_directory(home, slug, Some(folder), false) {
        Ok(_) => Ok(true),
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(other) => Err(other),
    }
}

/// Refuse a folder another user owns, or whose mode has any bit of `others` set; `how` says what
/// others could do with it ("opened", "changed").
fn only_mine(folder: &File, path: &Path, others: u32, how: &str) -> Result<()> {
    let metadata = folder.metadata().map_err(|e| io(path, e))?;
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    if metadata.permissions().mode() & others != 0
        || std::os::unix::fs::MetadataExt::uid(&metadata) != me
    {
        return Err(Error::Invalid(format!(
            "{} can be {how} by others on this Mac, so Persona Builder stopped. Make it a folder only you can {}, then try again",
            path.display(),
            if how == "changed" { "change" } else { "open" }
        )));
    }
    Ok(())
}

/// [`read_private`], or `None` when the state folder or the file does not exist.
pub fn read_private_optional(home: &Path, slug: &str, name: &str) -> Result<Option<Vec<u8>>> {
    read_private_optional_at(home, slug, None, name)
}

/// [`read_private_optional`], from `folder` inside the state folder when one is given.
pub fn read_private_optional_at(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
) -> Result<Option<Vec<u8>>> {
    read_private_optional_at_limit(home, slug, folder, name, None)
}

/// An optional private read with the same opened-file limit as [`read_private_at_max`].
pub fn read_private_optional_at_max(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
    max: u64,
) -> Result<Option<Vec<u8>>> {
    read_private_optional_at_limit(home, slug, folder, name, Some(max))
}

fn read_private_optional_at_limit(
    home: &Path,
    slug: &str,
    folder: Option<&str>,
    name: &str,
    max: Option<u64>,
) -> Result<Option<Vec<u8>>> {
    validate_private_name(name)?;
    let (directory, path) = match place_directory(home, slug, folder, false) {
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
    read_private_at_limit(home, slug, folder, name, max).map(Some)
}

/// List regular UTF-8 file names without reopening the checked private state folder by path.
pub fn list_private(home: &Path, slug: &str) -> Result<Vec<String>> {
    let (directory, path) = match place_directory(home, slug, None, false) {
        Ok(found) => found,
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(other) => return Err(other),
    };
    // SAFETY: directory owns a live descriptor. On success fdopendir takes ownership.
    let stream = unsafe { libc::fdopendir(directory.as_raw_fd()) };
    if stream.is_null() {
        return Err(io(&path, std::io::Error::last_os_error()));
    }
    let descriptor = directory.into_raw_fd();
    let stream = DirectoryStream(stream);
    let mut names = vec![];
    loop {
        // SAFETY: errno is thread-local and the returned pointer is valid on this thread.
        unsafe { *directory_errno() = 0 };
        // SAFETY: stream owns a live directory stream, used only on this thread.
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(io(&path, error));
            }
            break;
        }
        // SAFETY: readdir returned a live entry with a terminated name, valid until next readdir.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let Ok(text) = name.to_str() else {
            continue;
        };
        // SAFETY: stat is plain data, overwritten by fstatat before inspection.
        let mut status: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: descriptor stays live through stream; name is terminated; status is writable.
        if unsafe {
            libc::fstatat(
                descriptor,
                name.as_ptr(),
                &mut status,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } < 0
        {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                continue;
            }
            return Err(io(&path.join(text), error));
        }
        if status.st_mode & libc::S_IFMT == libc::S_IFREG {
            names.push(text.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

struct DirectoryStream(*mut libc::DIR);

impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // SAFETY: this guard is the sole owner of the stream and its directory descriptor.
        unsafe { libc::closedir(self.0) };
    }
}

#[cfg(target_os = "macos")]
fn directory_errno() -> *mut libc::c_int {
    // SAFETY: __error returns this thread's errno pointer.
    unsafe { libc::__error() }
}

#[cfg(not(target_os = "macos"))]
fn directory_errno() -> *mut libc::c_int {
    // SAFETY: __errno_location returns this thread's errno pointer.
    unsafe { libc::__errno_location() }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Home(
        PathBuf,
        #[allow(dead_code)] std::sync::MutexGuard<'static, ()>,
    );

    impl Home {
        fn new() -> Self {
            let guard = crate::serial_test();
            let path = std::env::temp_dir().join(format!(
                "pb-state-lock-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let home = Self(path, guard);
            fs::create_dir(&home.0).unwrap();
            drop(lock_state(&home.0, "wren").unwrap());
            home
        }

        fn state(&self) -> PathBuf {
            self.0.join(".claude/state/wren")
        }

        fn replace_state(&self) {
            fs::remove_dir_all(self.state()).unwrap();
            fs::create_dir(self.state()).unwrap();
            fs::set_permissions(self.state(), Permissions::from_mode(0o700)).unwrap();
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn existing_lock_refuses_a_folder_forgotten_before_the_lock() {
        let home = Home::new();
        let result = lock_state_folder_after(&home.0, "wren", false, &mut || {
            fs::remove_dir_all(home.state()).unwrap();
        });
        assert!(matches!(
            result,
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(!home.state().exists());
    }

    #[test]
    fn existing_lock_refuses_a_folder_replaced_before_the_lock() {
        let home = Home::new();
        let result = lock_state_folder_after(&home.0, "wren", false, &mut || {
            home.replace_state();
        });
        assert!(matches!(
            result,
            Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(home.state().is_dir());
        assert_eq!(fs::read_dir(home.state()).unwrap().count(), 0);
    }

    #[test]
    fn creating_lock_refuses_a_folder_forgotten_before_the_lock() {
        let home = Home::new();
        let result = lock_state_folder_after(&home.0, "wren", true, &mut || {
            fs::remove_dir_all(home.state()).unwrap();
        });
        assert!(matches!(result, Err(Error::Invalid(text)) if text == BUSY));
        assert!(!home.state().exists());
    }

    #[test]
    fn creating_lock_refuses_a_folder_replaced_before_the_lock() {
        let home = Home::new();
        let result = lock_state_folder_after(&home.0, "wren", true, &mut || {
            home.replace_state();
        });
        assert!(matches!(result, Err(Error::Invalid(text)) if text == BUSY));
    }

    #[test]
    fn lock_with_nothing_between_still_locks() {
        let home = Home::new();
        for create in [false, true] {
            let lock = lock_state_folder_after(&home.0, "wren", create, &mut || {}).unwrap();
            assert!(matches!(
                lock_existing_state(&home.0, "wren"),
                Err(Error::Invalid(text)) if text == BUSY
            ));
            drop(lock);
        }
    }

    #[test]
    fn a_started_child_keeps_no_lock_once_it_runs() {
        let home = Home::new();
        let lock = lock_state(&home.0, "wren").unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("2")
            .spawn()
            .unwrap();
        // Give the child time to exec, which closes every descriptor opened with O_CLOEXEC.
        std::thread::sleep(std::time::Duration::from_millis(200));
        drop(lock);
        let again = lock_existing_state(&home.0, "wren");
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(
            again.is_ok(),
            "a started child kept the state lock: {again:?}"
        );
    }

    #[test]
    fn private_read_caps_bytes_after_the_metadata_check() {
        let path = Path::new("growing.jsonl");
        // The byte stream can outgrow the size checked before the first read.
        let mut file = std::io::Cursor::new(vec![b'x'; 4096]);
        let result = read_private_bytes(&mut file, path, "growing.jsonl", Some(1024));
        assert!(result.is_err(), "bytes beyond the limit must be refused");
        assert_eq!(
            result.unwrap_err().to_string(),
            "growing.jsonl is too big to read (over 1 KiB)",
        );
        assert_eq!(file.position(), 1025, "read at most max + 1 bytes");
        for (length, max) in [(0, Some(1024)), (1024, Some(1024)), (4096, None)] {
            let bytes = vec![b'x'; length];
            assert_eq!(
                read_private_bytes(
                    &mut std::io::Cursor::new(&bytes),
                    path,
                    "growing.jsonl",
                    max,
                )
                .unwrap(),
                bytes,
            );
        }
    }

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
