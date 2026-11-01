use super::install_fs::{exists, io};
use crate::{
    block,
    error::{Error, Result},
    member::refuse_symlink,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Record {
    pub slug: String,
    pub name: String,
    pub member_dir: PathBuf,
    pub claude_md_created: bool,
    pub added: Vec<Added>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum Added {
    Dir { path: PathBuf },
    File { path: PathBuf, text: String },
}
impl Added {
    pub fn path(&self) -> &Path {
        match self {
            Self::Dir { path } | Self::File { path, .. } => path,
        }
    }
}

pub(super) fn record_path(claude_home: &Path) -> PathBuf {
    claude_home.join("voice/installed.json")
}

pub(super) fn check_path(path: &Path, claude_home: &Path) -> Result<()> {
    let invalid = || {
        Error::Invalid(format!(
            "{} is outside the Claude Code folder: fix or remove installed.json",
            path.display()
        ))
    };
    if !path.starts_with(claude_home)
        || path == claude_home
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid());
    }
    // Validate each ancestor too: a lexically contained path can escape through a link.
    let relative = path.strip_prefix(claude_home).map_err(|_| invalid())?;
    let mut parent = claude_home.to_owned();
    let mut canonical_root = claude_home.to_owned();
    let parts: Vec<_> = relative.components().collect();
    for part in &parts[..parts.len() - 1] {
        parent.push(part.as_os_str());
        if parent == claude_home.join("skills") {
            if exists(&parent)? {
                canonical_root = io(&parent, fs::canonicalize(&parent))?;
                if io(&parent, fs::symlink_metadata(&parent))?.is_symlink()
                    && (claude_home.starts_with(&canonical_root)
                        || canonical_root.starts_with(claude_home))
                {
                    return Err(Error::Invalid(format!(
                        "{} points back into the Claude Code folder. Point it at a folder outside {}, then try again",
                        parent.display(),
                        claude_home.display()
                    )));
                }
            }
            continue;
        }
        refuse_symlink(&parent)?;
        if exists(&parent)? && !io(&parent, fs::canonicalize(&parent))?.starts_with(&canonical_root)
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn private_root(path: &Path) -> Result<()> {
    let metadata = io(path, fs::metadata(path))?;
    // SAFETY: geteuid has no preconditions.
    let uid = unsafe { libc::geteuid() };
    if metadata.uid() != uid || metadata.permissions().mode() & 0o077 != 0 {
        return Err(Error::Invalid(format!(
            "{0} can be opened by other people on this computer, so Voice won't use it. Make it yours alone (chmod 700 {0}), then try again",
            path.display()
        )));
    }
    Ok(())
}

pub(super) fn read(claude_home: &Path) -> Result<Option<Record>> {
    let folder = claude_home.join("voice");
    refuse_symlink(&folder)?;
    if !exists(&folder)? {
        return Ok(None);
    }
    if !folder.is_dir() {
        return Err(Error::Invalid(format!(
            "{} is not a folder: move it away and try again",
            folder.display()
        )));
    }
    private_root(&folder)?;
    let path = record_path(claude_home);
    refuse_symlink(&path)?;
    if !exists(&path)? {
        return Ok(None);
    }
    let text = io(&path, fs::read_to_string(&path))?;
    let record: Record = serde_json::from_str(&text).map_err(|error| {
        Error::Invalid(format!(
            "{} is not a valid installation record ({error}): fix or remove it",
            path.display()
        ))
    })?;
    block::validate_id(&record.slug)
        .map_err(|error| Error::Invalid(format!("{}: {error}", path.display())))?;
    for added in &record.added {
        let relative = added.path().strip_prefix(claude_home).ok();
        let parts: Vec<_> = relative.iter().flat_map(|path| path.components()).collect();
        let matches = |index: usize, expected: &[u8]| {
            parts.get(index).is_some_and(|part| {
                part.as_os_str()
                    .as_encoded_bytes()
                    .eq_ignore_ascii_case(expected)
            })
        };
        if (parts.len() == 1 && matches(0, b"CLAUDE.md"))
            || (parts.len() == 2 && matches(0, b"voice") && matches(1, b"installed.json"))
            || (matches(0, b"voice") && !matches(1, b"assistants"))
        {
            return Err(Error::Invalid(format!(
                "{} lists {} as added: fix or remove installed.json",
                path.display(),
                added.path().display()
            )));
        }
        check_path(added.path(), claude_home)
            .map_err(|error| Error::Invalid(format!("{}: {error}", path.display())))?;
    }
    Ok(Some(record))
}
