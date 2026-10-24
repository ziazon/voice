use super::install_fs::{exists, io};
use crate::{
    block,
    error::{Error, Result},
    member::refuse_symlink,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
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
    claude_home.join("persona-builder/installed.json")
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
                    return Err(invalid());
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

pub(super) fn read(claude_home: &Path) -> Result<Option<Record>> {
    let folder = claude_home.join("persona-builder");
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
        if added.path() == claude_home.join("CLAUDE.md") || added.path() == path {
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
