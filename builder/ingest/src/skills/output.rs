//! Private, no-follow output folders for anonymized skill copies.
use super::{Skill, invalid, io};
use crate::check;
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Write,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
};
use voice_core::error::{Error, Result};

const STAMP: &str = "Made by voice anonymize. Running it again replaces this folder.\n";
fn output_bad() -> Error {
    invalid(
        "Choose an output folder that is empty or was made by voice anonymize, outside your skill folders.",
    )
}
fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}
fn future_canonical(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(output_bad());
    }
    let absolute = if path.is_absolute() {
        path.into()
    } else {
        std::env::current_dir()
            .map_err(|_| output_bad())?
            .join(path)
    };
    if absolute
        .components()
        .any(|p| matches!(p, Component::ParentDir | Component::CurDir))
    {
        return Err(output_bad());
    }
    let mut ancestor = absolute.as_path();
    let mut tail = vec![];
    while !ancestor.exists() {
        tail.push(ancestor.file_name().ok_or_else(output_bad)?.to_owned());
        ancestor = ancestor.parent().ok_or_else(output_bad)?;
    }
    let mut canonical = ancestor.canonicalize().map_err(|_| output_bad())?;
    for part in tail.into_iter().rev() {
        canonical.push(part);
    }
    Ok(canonical)
}
// Also protect not-yet-created roots on disks that ignore letter case.
// Rejecting a case variant on a case-sensitive disk is deliberately conservative.
fn within(path: &Path, root: &Path) -> bool {
    let mut parts = path.components();
    root.components().all(|component| {
        parts.next().is_some_and(|part| {
            part.as_os_str().to_string_lossy().to_lowercase()
                == component.as_os_str().to_string_lossy().to_lowercase()
        })
    })
}
fn make_dirs(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(output_bad()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                make_dirs(parent)?;
            }
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(path).map_err(|e| io(path, e))
        }
        Err(e) => Err(io(path, e)),
    }
}
pub fn prepare_output(out: &Path, home: &Path, skills: &[Skill]) -> Result<PathBuf> {
    prepare_output_with_projects(out, home, skills, &[])
}
pub fn prepare_output_with_projects(
    out: &Path,
    home: &Path,
    skills: &[Skill],
    projects: &[PathBuf],
) -> Result<PathBuf> {
    if let Ok(meta) = fs::symlink_metadata(out)
        && (meta.file_type().is_symlink() || !meta.is_dir())
    {
        return Err(output_bad());
    }
    let out = future_canonical(out)?;
    let mut roots = vec![
        home.join(".claude/skills"),
        home.join(".codex/skills"),
        home.join(".claude/plugins"),
    ];
    roots.extend(projects.iter().map(|p| p.join(".claude/skills")));
    for root in roots {
        if within(&out, &future_canonical(&root)?) {
            return Err(output_bad());
        }
    }
    for skill in skills {
        let folder = future_canonical(&skill.folder)?;
        if within(&out, &folder) || within(&folder, &out) {
            return Err(output_bad());
        }
    }
    for project in projects {
        if within(&future_canonical(project)?, &out) {
            return Err(output_bad());
        }
    }
    if out.exists() {
        let empty = fs::read_dir(&out)
            .map_err(|e| io(&out, e))?
            .next()
            .is_none();
        if !empty {
            let stamp = out.join(".anonymized-copy");
            let valid = fs::symlink_metadata(&stamp).is_ok_and(|m| {
                m.is_file() && !m.file_type().is_symlink() && m.len() == STAMP.len() as u64
            }) && check::read(&stamp).is_ok_and(|b| b == STAMP.as_bytes());
            if !valid {
                return Err(output_bad());
            }
            let metadata = fs::symlink_metadata(&out).map_err(|e| io(&out, e))?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(output_bad());
            }
            fs::remove_dir_all(&out).map_err(|e| io(&out, e))?;
        } else {
            fs::remove_dir(&out).map_err(|e| io(&out, e))?;
        }
    }
    make_dirs(&out)?;
    write_file(&open_folder(&out)?, ".anonymized-copy", STAMP.as_bytes())?;
    Ok(out)
}
fn open_folder(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| io(path, e))
}
fn child_folder(parent: &File, name: &str) -> Result<File> {
    let name = CString::new(name).map_err(|_| output_bad())?;
    // SAFETY: the parent descriptor remains live and the name is a single checked component.
    let made = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
    if made < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists {
        return Err(output_bad());
    }
    // SAFETY: the live directory descriptor anchors this no-follow directory open.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(output_bad());
    }
    // SAFETY: openat returned a fresh owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn write_file(parent: &File, name: &str, bytes: &[u8]) -> Result<()> {
    let name = CString::new(name).map_err(|_| output_bad())?;
    // SAFETY: a live parent descriptor and one checked component anchor this exclusive creation.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(output_bad());
    }
    // SAFETY: openat returned a fresh owned descriptor.
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).map_err(|_| output_bad())
}
pub fn write_skill(out: &Path, name: &str, kept: &[(PathBuf, String)]) -> Result<()> {
    if !safe_relative(Path::new(name))
        || Path::new(name).components().count() != 1
        || kept.iter().any(|(p, _)| !safe_relative(p))
    {
        return Err(output_bad());
    }
    let failed = |path: &Path| {
        invalid(&format!(
            "Couldn't write {name}/{} in the copy. Check there's space, then run anonymize again.",
            path.display()
        ))
    };
    let first = kept
        .first()
        .map(|(p, _)| p.as_path())
        .unwrap_or(Path::new("SKILL.md"));
    let root = child_folder(&open_folder(out).map_err(|_| failed(first))?, name)
        .map_err(|_| failed(first))?;
    for (path, text) in kept {
        let write = || -> Result<()> {
            let mut directory = root.try_clone().map_err(|_| failed(path))?;
            let mut parts = path.iter().peekable();
            while let Some(part) = parts.next() {
                let part = part.to_str().ok_or_else(|| failed(path))?;
                if parts.peek().is_some() {
                    directory = child_folder(&directory, part)?;
                } else {
                    write_file(&directory, part, text.as_bytes())?;
                }
            }
            Ok(())
        };
        write().map_err(|_| failed(path))?;
    }
    Ok(())
}
