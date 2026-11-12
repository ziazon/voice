//! The older assistant's own files, checked before they replace template files.
use crate::{
    autonomy::Autonomy,
    crew::MemberDir,
    error::{Error, Result},
    existing::read_capped,
    install_fs::io,
    member,
};
use std::{
    collections::HashSet,
    ffi::{CString, OsStr},
    fs::{self, OpenOptions},
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

pub const NAMES: [&str; 8] = [
    "IDENTITY.md",
    "autonomy.md",
    "interview.md",
    "lens.md",
    "decisions.md",
    "measurements.md",
    "briefs",
    "private.json",
];
pub struct Offered {
    pub name: &'static str,
    pub problem: Option<&'static str>,
}
pub const CANT_BRING: &str = "Voice couldn't bring in the older assistant's own files, so it changed nothing. Leave out private.json and autonomy.md, then try again.";
const LINK: &str = "It's a link, so Voice leaves it.";
const SHARED: &str = "It shares its contents with another file, so Voice leaves it.";
const READ: &str = "Voice can't read it, so Voice leaves it.";
const FILE: &str = "It isn't a plain file, so Voice leaves it.";
const FOLDER: &str = "It isn't a plain folder of files, so Voice leaves it.";
const BIG: &str = "It's too big for Voice to read, so Voice leaves it.";
const TEXT: &str = "It isn't plain text, so Voice leaves it.";
const AUTONOMY: &str = "Voice can't read its list of who decides what, so Voice leaves it.";
const PRIVATE: &str = "It changes more than the privacy settings, so Voice leaves it.";
const REFUSE_START: &str = "Voice can't bring in ";
const REFUSE_END: &str = ", so it changed nothing. Leave it out and try again.";

pub fn says(message: &str) -> bool {
    matches!(
        message,
        CANT_BRING | SHARED | READ | LINK | FILE | FOLDER | BIG | TEXT | AUTONOMY | PRIVATE
    ) || message
        .strip_prefix(REFUSE_START)
        .and_then(|rest| rest.strip_suffix(REFUSE_END))
        .is_some_and(|name| !name.is_empty())
}

pub fn refusal(name: &str) -> Error {
    Error::Invalid(format!("{REFUSE_START}{name}{REFUSE_END}"))
}

struct Checked {
    problem: Option<&'static str>,
    files: Vec<(PathBuf, Vec<u8>)>,
}
impl Checked {
    fn problem(problem: &'static str) -> Self {
        Self {
            problem: Some(problem),
            files: Vec::new(),
        }
    }
}

/// The old folder's files Voice can offer, in NAMES order; absent names are left out.
pub fn offer(
    folder: &Path,
    private_ok: &dyn Fn(&serde_json::Value) -> bool,
) -> Result<Vec<Offered>> {
    match fs::symlink_metadata(folder) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        metadata => {
            if !io(folder, metadata)?.is_dir() {
                return Err(Error::Invalid(CANT_BRING.into()));
            }
        }
    }
    let mut offered = Vec::new();
    for name in NAMES {
        if let Some(checked) = check(folder, name, private_ok)? {
            offered.push(Offered {
                name,
                problem: checked.problem,
            });
        }
    }
    Ok(offered)
}

fn check(
    folder: &Path,
    name: &str,
    private_ok: &dyn Fn(&serde_json::Value) -> bool,
) -> Result<Option<Checked>> {
    let path = folder.join(name);
    let metadata = match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        metadata => io(&path, metadata)?,
    };
    if metadata.is_symlink() {
        return Ok(Some(Checked::problem(LINK)));
    }
    if name != "briefs" {
        return Ok(Some(check_file(
            &path,
            Path::new(name),
            file_cap(name),
            private_ok,
        )?));
    }
    if !metadata.is_dir() {
        return Ok(Some(Checked::problem(FOLDER)));
    }
    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(_) => return Ok(Some(Checked::problem(READ))),
    };
    let entries = entries
        .filter(|entry| {
            !entry
                .as_ref()
                .is_ok_and(|entry| entry.file_name() == ".DS_Store")
        })
        .take(65)
        .map(|entry| io(&path, entry))
        .collect::<Result<Vec<_>>>();
    let mut entries = match entries {
        Ok(entries) => entries,
        Err(_) => return Ok(Some(Checked::problem(READ))),
    };
    if entries.len() > 64 {
        return Ok(Some(Checked::problem(FOLDER)));
    }
    entries.sort_by_key(|entry| entry.file_name());
    // Shape errors take precedence over the contents of any brief.
    for entry in &entries {
        match fs::symlink_metadata(entry.path()) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Ok(Some(Checked::problem(FOLDER))),
            Err(_) => return Ok(Some(Checked::problem(READ))),
        }
    }
    let mut files = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        let cap = if name.as_encoded_bytes().eq_ignore_ascii_case(b"codebook.md") {
            64 * 1024 // voice_build::draft::CODEBOOK_MOST; no dependency on the build crate.
        } else {
            1024 * 1024
        };
        let checked = check_file(
            &entry.path(),
            &Path::new("briefs").join(name),
            cap,
            private_ok,
        )?;
        if checked.problem.is_some() {
            return Ok(Some(checked));
        }
        files.extend(checked.files);
    }
    Ok(Some(Checked {
        problem: None,
        files,
    }))
}

fn file_cap(name: &str) -> u64 {
    if name == "decisions.md" {
        256 * 1024 // voice_grow::version::DECISIONS_MOST; no dependency on the grow crate.
    } else {
        1024 * 1024
    }
}

fn check_file(
    path: &Path,
    relative: &Path,
    cap: u64,
    private_ok: &dyn Fn(&serde_json::Value) -> bool,
) -> Result<Checked> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(Checked::problem(READ)),
    };
    let problem = if metadata.is_symlink() {
        Some(LINK)
    } else if !metadata.is_file() {
        Some(FILE)
    } else if metadata.nlink() > 1 {
        Some(SHARED)
    } else if metadata.len() > cap {
        Some(BIG)
    } else {
        None
    };
    if let Some(problem) = problem {
        return Ok(Checked::problem(problem));
    }
    let bytes = match read_capped(path, cap, false) {
        Ok(Some(bytes)) => bytes,
        // A file that changed after its metadata was checked is never offered.
        Ok(None) => return Ok(Checked::problem(FILE)),
        Err(Error::Invalid(message)) => {
            return Ok(Checked::problem(if message.contains("too big") {
                BIG
            } else {
                FILE
            }));
        }
        Err(Error::Io { .. }) => return Ok(Checked::problem(READ)),
        Err(error) => return Err(error),
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) if !text.contains('\0') => text,
        _ => return Ok(Checked::problem(TEXT)),
    };
    if relative == Path::new("autonomy.md") && Autonomy::parse(text).is_err() {
        return Ok(Checked::problem(AUTONOMY));
    }
    if relative == Path::new("private.json") {
        let valid = serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .is_some_and(|value| {
                value.as_object().is_some_and(|object| {
                    object
                        .keys()
                        .all(|key| matches!(key.as_str(), "filters" | "anonymizer"))
                }) && private_ok(&value)
            });
        if !valid {
            return Ok(Checked::problem(PRIVATE));
        }
    }
    Ok(Checked {
        problem: None,
        files: vec![(relative.to_owned(), bytes)],
    })
}

/// Copy checked selections from the backup's member copy, then check the new crew folder.
/// The caller removes the new crew folder and undoes its backup if this returns an error.
pub fn bring(
    from: &Path,
    to: &Path,
    names: &[String],
    private_ok: &dyn Fn(&serde_json::Value) -> bool,
) -> Result<()> {
    let mut seen = HashSet::new();
    let mut selections = Vec::new();
    for name in names {
        if !NAMES.contains(&name.as_str()) || !seen.insert(name) {
            return Err(refusal(name));
        }
        if !fs::symlink_metadata(from).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(refusal(name));
        }
        match check(from, name, private_ok).map_err(|_| refusal(name))? {
            Some(checked) if checked.problem.is_none() => selections.extend(checked.files),
            _ => return Err(refusal(name)),
        }
    }
    copy_and_check(to, &selections).map_err(|_| Error::Invalid(CANT_BRING.into()))
}

fn copy_and_check(to: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    let directory_flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let folder = io(
        to,
        OpenOptions::new()
            .read(true)
            .custom_flags(directory_flags)
            .open(to),
    )?;
    let expected = member::read_member(&to.join("member.json"))?;
    let mut briefs_folder = None;
    for (relative, bytes) in files {
        let directory = if relative.starts_with("briefs") {
            if briefs_folder.is_none() {
                let briefs = to.join("briefs");
                let name = CString::new("briefs").expect("literal has no NUL");
                // SAFETY: folder is an open directory and name is a valid C string.
                let created = unsafe { libc::mkdirat(folder.as_raw_fd(), name.as_ptr(), 0o700) };
                if created < 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
                {
                    return io(&briefs, Err(std::io::Error::last_os_error()));
                }
                briefs_folder = Some(open_at(
                    &folder,
                    OsStr::new("briefs"),
                    directory_flags,
                    &briefs,
                )?);
            }
            briefs_folder.as_ref().expect("briefs opened above")
        } else {
            &folder
        };
        let target = to.join(relative);
        let name = relative
            .file_name()
            .ok_or_else(|| Error::Invalid(CANT_BRING.into()))?;
        let mut file = open_at(
            directory,
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            &target,
        )?;
        io(
            &target,
            file.set_permissions(fs::Permissions::from_mode(0o600)),
        )?;
        io(&target, file.write_all(bytes))?;
    }
    let actual = member::read_member(&to.join("member.json"))?;
    if (actual.slug, actual.name, actual.principal, actual.state_dir)
        != (
            expected.slug,
            expected.name,
            expected.principal,
            expected.state_dir,
        )
    {
        return Err(Error::Invalid(CANT_BRING.into()));
    }
    MemberDir::open(to)?.autonomy()?;
    Ok(())
}

fn open_at(
    directory: &fs::File,
    name: &OsStr,
    flags: libc::c_int,
    path: &Path,
) -> Result<fs::File> {
    let bytes = name.as_encoded_bytes();
    if bytes.is_empty() || bytes.contains(&b'/') || bytes == b"." || bytes == b".." {
        return Err(Error::Invalid(CANT_BRING.into()));
    }
    let name = CString::new(bytes).map_err(|_| Error::Invalid(CANT_BRING.into()))?;
    if flags & libc::O_CREAT != 0 {
        // Replace the directory entry so a planted hard link is never written through.
        // SAFETY: directory is open and name is a single NUL-terminated path part.
        if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::NotFound {
                return io(path, Err(error));
            }
        }
    }
    // SAFETY: directory is open, name is a single NUL-terminated path part, and mode
    // is supplied for O_CREAT. The returned descriptor is owned exactly once below.
    let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags, 0o600) };
    if fd < 0 {
        return io(path, Err(std::io::Error::last_os_error()));
    }
    // SAFETY: openat returned a new, valid descriptor; OwnedFd closes it on every path.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    Ok(fs::File::from(owned))
}

#[cfg(test)]
mod tests;
