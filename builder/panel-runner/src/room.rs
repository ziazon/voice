//! Private panel files and the exclusive run lock.
use crate::{run::Convened, text};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};
use voice_panels::DeliberationState;

const DIR_FLAGS: i32 = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
static CREATION: Mutex<()> = Mutex::new(());

fn with_private_mask<T>(create: impl FnOnce() -> T) -> T {
    let _lock = CREATION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // SAFETY: umask has no pointer arguments. The previous mask is restored before unlocking.
    let previous = unsafe { libc::umask(0o077) };
    let result = create();
    // SAFETY: previous is the mask returned by umask in this scope.
    unsafe { libc::umask(previous) };
    result
}

/// A panel folder with a validated id.
pub struct Room {
    pub id: String,
    pub path: PathBuf,
    directory: File,
}
/// An exclusive hold on a running panel.
pub struct RoomLock {
    _file: File,
}

fn cstring(name: &str) -> Result<CString, String> {
    CString::new(name).map_err(|_| text::STORAGE.into())
}

fn private(file: &File, directory: bool) -> Result<(), String> {
    let metadata = file.metadata().map_err(|_| text::STORAGE)?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(text::STORAGE.into());
    }
    Ok(())
}

fn directory_at(parent: &File, name: &str, create: bool, check: bool) -> Result<File, String> {
    let name = cstring(name)?;
    // SAFETY: parent and name remain live; flags refuse links.
    let mut fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), DIR_FLAGS) };
    if fd < 0 && create && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
        // SAFETY: parent and name remain live; mkdirat creates only this child.
        if with_private_mask(|| unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) })
            < 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(text::STORAGE.into());
        }
        // SAFETY: flags still refuse a link if the directory was exchanged.
        fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), DIR_FLAGS) };
    }
    if fd < 0 {
        return Err(text::STORAGE.into());
    }
    // SAFETY: openat returned a fresh descriptor, now owned once.
    let file = unsafe { File::from_raw_fd(fd) };
    if check {
        private(&file, true)?;
    }
    Ok(file)
}

fn random_hex() -> Result<String, String> {
    let mut bytes = [0; 3];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| text::RANDOM)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Validate the exact panel id shape before joining a path.
pub fn validate_id(id: &str) -> Result<(), String> {
    let bytes = id.as_bytes();
    let valid = bytes.len() == 26
        && bytes.starts_with(b"dl-")
        && bytes.get(11) == Some(&b'T')
        && bytes.get(18) == Some(&b'Z')
        && bytes.get(19) == Some(&b'-')
        && bytes
            .get(3..11)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
        && bytes
            .get(12..18)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
        && bytes.get(20..26).is_some_and(|part| {
            part.iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        });
    if valid {
        Ok(())
    } else {
        Err(text::BAD_ID.into())
    }
}
/// A UTC id with six random lowercase hex digits.
pub fn new_id() -> Result<String, String> {
    let stamp = persona_builder_agent::ledger::utc_stamp(SystemTime::now());
    let stamp: String = stamp.chars().filter(|c| !matches!(c, '-' | ':')).collect();
    let id = format!("dl-{stamp}-{}", random_hex()?);
    validate_id(&id)?;
    Ok(id)
}

impl Room {
    /// Open an existing room, or create a private room when requested.
    pub fn open(home: &Path, id: &str, create: bool) -> Result<Self, String> {
        validate_id(id)?;
        let home_dir = OpenOptions::new()
            .read(true)
            .custom_flags(DIR_FLAGS)
            .open(home)
            .map_err(|_| text::STORAGE)?;
        let claude = directory_at(&home_dir, ".claude", create, false)?;
        let voice = directory_at(&claude, "voice", create, true)?;
        let panels = directory_at(&voice, "panels", create, true)?;
        if create {
            let name = cstring(id)?;
            // SAFETY: panels and name are live; a new panel never reuses an existing folder.
            if with_private_mask(|| unsafe {
                libc::mkdirat(panels.as_raw_fd(), name.as_ptr(), 0o700)
            }) < 0
            {
                return Err(text::STORAGE.into());
            }
        }
        let directory = directory_at(&panels, id, false, true)?;
        Ok(Self {
            id: id.into(),
            path: home.join(".claude/voice/panels").join(id),
            directory,
        })
    }

    fn location(&self, name: &str, create: bool) -> Result<(File, CString), String> {
        let (folder, name) = match name.split_once('/') {
            Some(("withheld", name)) => (Some("withheld"), name),
            Some(_) => return Err(text::STORAGE.into()),
            None => (None, name),
        };
        if name.is_empty() || name.contains(['/', '\\', '\0']) || name == "." || name == ".." {
            return Err(text::STORAGE.into());
        }
        let directory = if let Some(folder) = folder {
            directory_at(&self.directory, folder, create, true)?
        } else {
            self.directory.try_clone().map_err(|_| text::STORAGE)?
        };
        Ok((directory, cstring(name)?))
    }

    /// Take the nonblocking run lock.
    pub fn lock(&self) -> Result<RoomLock, String> {
        let name = cstring(".lock")?;
        // SAFETY: the descriptor and name are live; links are refused.
        let fd = with_private_mask(|| unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
                0o600,
            )
        });
        if fd < 0 {
            return Err(text::STORAGE.into());
        }
        // SAFETY: openat returned a fresh descriptor, now owned once.
        let file = unsafe { File::from_raw_fd(fd) };
        private(&file, false)?;
        // SAFETY: file holds a live descriptor for the entire lock lifetime.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
            return Err(text::BUSY.into());
        }
        Ok(RoomLock { _file: file })
    }

    /// Atomically write a private file.
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let (directory, target) = self.location(name, true)?;
        let temporary = cstring(&format!(
            ".{}-{}.tmp",
            target.to_string_lossy(),
            random_hex()?
        ))?;
        // SAFETY: directory and names are live; exclusive creation never follows a link.
        let fd = with_private_mask(|| unsafe {
            libc::openat(
                directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        });
        if fd < 0 {
            return Err(text::SAVE.into());
        }
        // SAFETY: openat returned a fresh descriptor, now owned once.
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = file
            .write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| text::SAVE.to_owned())
            .and_then(|_| {
                // SAFETY: both names refer to children of this live directory descriptor.
                if unsafe {
                    libc::renameat(
                        directory.as_raw_fd(),
                        temporary.as_ptr(),
                        directory.as_raw_fd(),
                        target.as_ptr(),
                    )
                } < 0
                {
                    Err(text::SAVE.into())
                } else {
                    Ok(())
                }
            });
        if result.is_err() {
            // SAFETY: only this exclusive temporary file is removed.
            unsafe { libc::unlinkat(directory.as_raw_fd(), temporary.as_ptr(), 0) };
        }
        result
    }

    fn read_optional(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let (directory, target) = self.location(name, false)?;
        // SAFETY: directory and target are live; links are refused, and a pipe cannot block.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                target.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(text::STORAGE.into())
            };
        }
        // SAFETY: openat returned a fresh descriptor, now owned once.
        let mut file = unsafe { File::from_raw_fd(fd) };
        private(&file, false)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|_| text::STORAGE)?;
        Ok(Some(bytes))
    }

    /// Read a checked private file without following links.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        self.read_optional(name)?
            .ok_or_else(|| text::STORAGE.into())
    }

    pub(crate) fn exists(&self, name: &str) -> Result<bool, String> {
        Ok(self.read_optional(name)?.is_some())
    }

    pub(crate) fn delete(&self, name: &str) -> Result<(), String> {
        if self.read_optional(name)?.is_none() {
            return Ok(());
        }
        let (directory, target) = self.location(name, false)?;
        // SAFETY: target names a checked file in this live directory; unlink never follows links.
        if unsafe { libc::unlinkat(directory.as_raw_fd(), target.as_ptr(), 0) } < 0 {
            return Err(text::SAVE.into());
        }
        Ok(())
    }

    pub(crate) fn read_json<T: DeserializeOwned>(&self, name: &str) -> Result<T, String> {
        serde_json::from_slice(&self.read(name)?).map_err(|_| text::READ_STATE.into())
    }

    pub(crate) fn write_json<T: Serialize>(&self, name: &str, value: &T) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(value).map_err(|_| text::SAVE)?;
        bytes.push(b'\n');
        self.write(name, &bytes)
    }

    pub(crate) fn lines<T: DeserializeOwned>(&self, name: &str) -> Result<Vec<T>, String> {
        let bytes = self.read_optional(name)?.unwrap_or_default();
        let text = std::str::from_utf8(&bytes).map_err(|_| text::READ_STATE)?;
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(|_| text::READ_STATE.into()))
            .collect()
    }

    pub(crate) fn write_lines<T: Serialize>(&self, name: &str, rows: &[T]) -> Result<(), String> {
        let mut bytes = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut bytes, row).map_err(|_| text::SAVE)?;
            bytes.push(b'\n');
        }
        self.write(name, &bytes)
    }

    /// Read and validate state against the original assistant list.
    pub fn load_state(&self, convene: &Convened) -> Result<DeliberationState, String> {
        let value = self.read_json("state.json")?;
        let state = DeliberationState::from_json(&value).map_err(|_| text::READ_STATE)?;
        let valid_name =
            |name: &String| persona_builder::crew::slugify(name).as_ref() == Some(name);
        if state.crew != convene.input.seats
            || !state.crew.iter().all(valid_name)
            || !state
                .weight
                .values()
                .chain(state.prev_own.values())
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || !state
                .prev_d
                .iter()
                .chain(state.d_history.iter())
                .all(|v| v.is_finite())
        {
            return Err(text::READ_STATE.into());
        }
        Ok(state)
    }
}
