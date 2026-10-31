//! Descriptor-anchored walks: a renamed folder cannot redirect a skill's reads.
use super::{SKILL_TOO_LARGE, Skill, Source, invalid, io};
use crate::anonymize::{self, Rules};
use persona_builder::error::Result;
use std::{
    ffi::{CStr, OsString},
    fs::{File, OpenOptions},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{ffi::OsStringExt, fs::OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

pub fn read(skill: &Skill) -> Result<Vec<(PathBuf, Source)>> {
    read_with_rules(skill, &Rules::builtin())
}
/// Apply exclusions before opening files; empty sources retain their report paths.
pub fn read_with_rules(skill: &Skill, rules: &Rules) -> Result<Vec<(PathBuf, Source)>> {
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&skill.folder)
        .map_err(|e| io(&skill.folder, e))?;
    let mut files = vec![];
    walk(root, Path::new(""), 0, &mut files, &mut 0, rules)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}
struct DirectoryStream(*mut libc::DIR);
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns the stream and its directory descriptor.
        unsafe { libc::closedir(self.0) };
    }
}
#[cfg(target_os = "macos")]
fn errno() -> *mut libc::c_int {
    // SAFETY: __error returns this thread's live errno pointer.
    unsafe { libc::__error() }
}
#[cfg(not(target_os = "macos"))]
fn errno() -> *mut libc::c_int {
    // SAFETY: __errno_location returns this thread's live errno pointer.
    unsafe { libc::__errno_location() }
}
fn open_at(fd: libc::c_int, name: &CStr, flags: libc::c_int, path: &Path) -> Result<File> {
    // SAFETY: fd is retained by the caller; name is terminated and one directory entry.
    let child = unsafe {
        libc::openat(
            fd,
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if child < 0 {
        return Err(io(path, std::io::Error::last_os_error()));
    }
    // SAFETY: successful openat returns a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(child) })
}
fn walk(
    directory: File,
    relative: &Path,
    depth: usize,
    files: &mut Vec<(PathBuf, Source)>,
    total: &mut u64,
    rules: &Rules,
) -> Result<()> {
    let fd = directory.as_raw_fd();
    // SAFETY: directory owns a live descriptor; success transfers ownership to the stream.
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        return Err(io(relative, std::io::Error::last_os_error()));
    }
    let _ = directory.into_raw_fd();
    let stream = DirectoryStream(stream);
    loop {
        // SAFETY: errno's thread-local pointer is writable on this thread.
        unsafe { *errno() = 0 };
        // SAFETY: stream remains live and is used by this thread only.
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(io(relative, error));
            }
            break;
        }
        // SAFETY: readdir returned a terminated name valid until the next readdir.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_owned();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        let relative = relative.join(OsString::from_vec(name.to_bytes().to_vec()));
        if relative.to_str().is_none() {
            if files.len() >= 500 {
                return Err(invalid(SKILL_TOO_LARGE));
            }
            files.push((relative, Source::Bytes(vec![])));
            continue;
        }
        // SAFETY: stat is plain data initialized before fstatat overwrites it.
        let mut metadata: libc::stat = unsafe { std::mem::zeroed() };
        // SAFETY: fd remains owned by stream; name and writable stat are live.
        if unsafe { libc::fstatat(fd, name.as_ptr(), &mut metadata, libc::AT_SYMLINK_NOFOLLOW) } < 0
        {
            return Err(io(&relative, std::io::Error::last_os_error()));
        }
        let mode = metadata.st_mode & libc::S_IFMT;
        if mode == libc::S_IFDIR {
            if name.to_bytes() == b".git" {
                continue;
            }
            if depth >= 8 {
                return Err(invalid(SKILL_TOO_LARGE));
            }
            walk(
                open_at(fd, &name, libc::O_RDONLY | libc::O_DIRECTORY, &relative)?,
                &relative,
                depth + 1,
                files,
                total,
                rules,
            )?;
        } else if matches!(mode, libc::S_IFREG | libc::S_IFLNK) {
            if files.len() >= 500 {
                return Err(invalid(SKILL_TOO_LARGE));
            }
            let source = if mode == libc::S_IFLNK {
                Source::Link
            } else {
                let size = u64::try_from(metadata.st_size).map_err(|_| invalid(SKILL_TOO_LARGE))?;
                if anonymize::exclusion(&relative, 0, rules).is_some() {
                    Source::Bytes(vec![])
                } else if size > 1 << 20 {
                    Source::TooLarge
                } else {
                    let file = open_at(fd, &name, libc::O_RDONLY | libc::O_NONBLOCK, &relative)?;
                    if !file.metadata().map_err(|e| io(&relative, e))?.is_file() {
                        return Err(invalid(
                            "A skill file changed while it was being read. Run anonymize again.",
                        ));
                    }
                    let mut bytes = Vec::new();
                    file.take((1 << 20) + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| io(&relative, e))?;
                    *total += bytes.len() as u64;
                    if *total > 20 << 20 {
                        return Err(invalid(SKILL_TOO_LARGE));
                    }
                    if bytes.len() > 1 << 20 {
                        Source::TooLarge
                    } else {
                        Source::Bytes(bytes)
                    }
                }
            };
            files.push((relative, source));
        }
    }
    Ok(())
}
