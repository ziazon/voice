//! Descriptor-anchored folders: a folder renamed or swapped for a link after it is opened can't redirect a read.
use std::{
    ffi::{CStr, CString},
    fs::{File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::fs::OpenOptionsExt,
    },
    path::Path,
};

pub(crate) struct Folder(File);
impl Folder {
    /// O_RDONLY | O_DIRECTORY | O_CLOEXEC, plus O_NOFOLLOW unless `follow`.
    pub(crate) fn open(path: &Path, follow: bool) -> std::io::Result<Folder> {
        OpenOptions::new()
            .read(true)
            .custom_flags(
                libc::O_DIRECTORY | libc::O_CLOEXEC | if follow { 0 } else { libc::O_NOFOLLOW },
            )
            .open(path)
            .map(Self)
    }
    /// A descriptor the caller already opened and checked is a folder.
    pub(crate) fn from_file(file: File) -> Folder {
        Self(file)
    }
    /// The folder's entry names, without "." and "..", in the order readdir gives them.
    pub(crate) fn names(&self) -> std::io::Result<Vec<CString>> {
        let directory = open_at(self.0.as_raw_fd(), c".", libc::O_RDONLY | libc::O_DIRECTORY)?;
        // SAFETY: directory owns a live descriptor; success transfers ownership to the stream.
        let stream = unsafe { libc::fdopendir(directory.as_raw_fd()) };
        if stream.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let _ = directory.into_raw_fd();
        let stream = DirectoryStream(stream);
        let mut names = vec![];
        loop {
            // SAFETY: errno's thread-local pointer is writable on this thread.
            unsafe { *errno() = 0 };
            // SAFETY: stream remains live and is used by this thread only.
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(0) {
                    return Err(error);
                }
                break;
            }
            // SAFETY: readdir returned a terminated name valid until the next readdir.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_owned();
            if !matches!(name.to_bytes(), b"." | b"..") {
                names.push(name);
            }
        }
        Ok(names)
    }
    /// fstatat on the descriptor; AT_SYMLINK_NOFOLLOW unless `follow`.
    pub(crate) fn stat(&self, name: &CStr, follow: bool) -> std::io::Result<libc::stat> {
        // SAFETY: stat is plain data initialized before fstatat overwrites it.
        let mut metadata: libc::stat = unsafe { std::mem::zeroed() };
        let flags = if follow { 0 } else { libc::AT_SYMLINK_NOFOLLOW };
        // SAFETY: the descriptor, terminated name and writable stat are live.
        if unsafe { libc::fstatat(self.0.as_raw_fd(), name.as_ptr(), &mut metadata, flags) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(metadata)
    }
    /// open_at(fd, name, O_RDONLY | O_DIRECTORY): never through a link.
    pub(crate) fn folder(&self, name: &CStr) -> std::io::Result<Folder> {
        open_at(self.0.as_raw_fd(), name, libc::O_RDONLY | libc::O_DIRECTORY)
            .map(Self)
            // macOS answers O_DIRECTORY | O_NOFOLLOW on a link with ENOTDIR; call it a link.
            .map_err(|error| {
                if error.raw_os_error() == Some(libc::ENOTDIR)
                    && self
                        .stat(name, false)
                        .is_ok_and(|m| m.st_mode & libc::S_IFMT == libc::S_IFLNK)
                {
                    std::io::Error::from_raw_os_error(libc::ELOOP)
                } else {
                    error
                }
            })
    }
    /// open_at(fd, name, O_RDONLY | flags): never through a link.
    pub(crate) fn file(&self, name: &CStr, flags: libc::c_int) -> std::io::Result<File> {
        open_at(self.0.as_raw_fd(), name, libc::O_RDONLY | flags)
    }
}

pub(crate) struct DirectoryStream(pub(crate) *mut libc::DIR);
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns the stream and its directory descriptor.
        unsafe { libc::closedir(self.0) };
    }
}
#[cfg(target_os = "macos")]
pub(crate) fn errno() -> *mut libc::c_int {
    // SAFETY: __error returns this thread's live errno pointer.
    unsafe { libc::__error() }
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn errno() -> *mut libc::c_int {
    // SAFETY: __errno_location returns this thread's live errno pointer.
    unsafe { libc::__errno_location() }
}
pub(crate) fn open_at(fd: libc::c_int, name: &CStr, flags: libc::c_int) -> std::io::Result<File> {
    // SAFETY: fd is retained by the caller; name is terminated and one directory entry.
    let child = unsafe {
        libc::openat(
            fd,
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if child < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful openat returns a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(child) })
}

#[cfg(test)]
#[path = "../tests/common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod test_common;

#[cfg(test)]
mod tests {
    use super::test_common::Scratch;
    use super::*;
    use std::{fs, io::Read, os::unix::fs::symlink};

    #[test]
    fn names_list_files_folders_and_links_twice_without_dot_entries() {
        let scratch = Scratch::new("folder-names");
        fs::write(scratch.0.join("file"), "text").unwrap();
        fs::create_dir(scratch.0.join("subfolder")).unwrap();
        symlink("file", scratch.0.join("link")).unwrap();
        let folder = Folder::open(&scratch.0, false).unwrap();
        for _ in 0..2 {
            let mut names = folder.names().unwrap();
            names.sort();
            assert_eq!(
                names,
                [
                    c"file".to_owned(),
                    c"link".to_owned(),
                    c"subfolder".to_owned()
                ]
            );
        }
    }

    #[test]
    fn stat_follows_only_when_asked_and_reports_hard_links() {
        let scratch = Scratch::new("folder-stat");
        fs::write(scratch.0.join("file"), "text").unwrap();
        symlink("file", scratch.0.join("link")).unwrap();
        fs::hard_link(scratch.0.join("file"), scratch.0.join("hard")).unwrap();
        let folder = Folder::open(&scratch.0, false).unwrap();
        assert_eq!(
            folder.stat(c"link", false).unwrap().st_mode & libc::S_IFMT,
            libc::S_IFLNK
        );
        assert_eq!(
            folder.stat(c"link", true).unwrap().st_mode & libc::S_IFMT,
            libc::S_IFREG
        );
        assert_eq!(folder.stat(c"file", false).unwrap().st_nlink, 2);
    }

    #[test]
    fn opens_refuse_links_and_a_file_is_not_a_folder() {
        let scratch = Scratch::new("folder-open");
        fs::write(scratch.0.join("file"), "text").unwrap();
        fs::create_dir(scratch.0.join("subfolder")).unwrap();
        symlink("file", scratch.0.join("file-link")).unwrap();
        symlink("subfolder", scratch.0.join("folder-link")).unwrap();
        let folder = Folder::open(&scratch.0, false).unwrap();
        assert_eq!(
            folder.file(c"file-link", 0).unwrap_err().raw_os_error(),
            Some(libc::ELOOP)
        );
        assert_eq!(
            folder.folder(c"folder-link").err().unwrap().raw_os_error(),
            Some(libc::ELOOP)
        );
        assert_eq!(
            folder.folder(c"file").err().unwrap().raw_os_error(),
            Some(libc::ENOTDIR)
        );
    }

    #[test]
    fn a_renamed_folder_cannot_redirect_a_read() {
        let scratch = Scratch::new("folder-anchor");
        let a = scratch.0.join("a");
        let b = scratch.0.join("b");
        fs::create_dir(&a).unwrap();
        fs::create_dir(&b).unwrap();
        fs::write(a.join("x"), "first").unwrap();
        fs::write(b.join("x"), "second").unwrap();
        let folder = Folder::open(&a, false).unwrap();
        fs::rename(&a, scratch.0.join("a-moved")).unwrap();
        symlink(&b, &a).unwrap();
        let mut text = String::new();
        folder
            .file(c"x", 0)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "first");
    }
}
