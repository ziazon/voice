use persona_builder::{
    error::{Error, Result},
    json::to_python_json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
pub enum SaveRecord {
    Local,
    Existing {
        repo: PathBuf,
        remote: String,
        branch: String,
    },
    Github {
        owner: String,
        name: String,
        branch: String,
    },
}
fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
fn check_state(state_dir: &Path) -> Result<bool> {
    match fs::symlink_metadata(state_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Error::Invalid("state folder is a link".into()))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io(state_dir, error)),
    }
}
pub fn read_record(state_dir: &Path) -> Result<Option<SaveRecord>> {
    if !check_state(state_dir)? {
        return Ok(None);
    }
    let path = state_dir.join("save.json");
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(&path, error)),
    };
    let invalid = || Error::Invalid("save.json: not a save record".into());
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| io(&path, e))?;
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    let object = value.as_object_mut().ok_or_else(invalid)?;
    if object.remove("version").and_then(|v| v.as_u64()) != Some(1) {
        return Err(invalid());
    }
    let keys: &[&str] = match object.get("mode").and_then(|v| v.as_str()) {
        Some("local") => &["mode"],
        Some("existing") => &["mode", "repo", "remote", "branch"],
        Some("github") => &["mode", "owner", "name", "branch"],
        _ => return Err(invalid()),
    };
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid());
    }
    let record: SaveRecord = serde_json::from_value(value).map_err(|_| invalid())?;
    let valid = match &record {
        SaveRecord::Local => true,
        SaveRecord::Existing {
            repo,
            remote,
            branch,
        } => {
            repo.is_absolute()
                && (1..=100).contains(&remote.len())
                && remote.starts_with(|c: char| c.is_ascii_alphanumeric())
                && remote
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && branch.strip_prefix("persona/").is_some_and(|slug| {
                    !slug.is_empty()
                        && slug
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                })
        }
        SaveRecord::Github {
            owner,
            name,
            branch,
        } => {
            crate::github::valid_owner(owner)
                && crate::github::valid_repo_name(name)
                && branch == "main"
        }
    };
    if !valid {
        return Err(invalid());
    }
    Ok(Some(record))
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn write_record(state_dir: &Path, record: &SaveRecord) -> Result<()> {
    create_state(state_dir)?;
    let path = state_dir.join("save.json");
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(io(&path, std::io::Error::from_raw_os_error(libc::ELOOP)));
    }
    #[derive(Serialize)]
    struct Versioned<'a> {
        version: u8,
        #[serde(flatten)]
        record: &'a SaveRecord,
    }
    let bytes = to_python_json(&Versioned { version: 1, record })?;
    let temporary = state_dir.join(format!("save.json.tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&temporary)
        .map_err(|e| io(&temporary, e))?;
    let temporary = Temporary(temporary);
    file.write_all(bytes.as_bytes())
        .map_err(|e| io(&temporary.0, e))?;
    file.sync_all().map_err(|e| io(&temporary.0, e))?;
    fs::rename(&temporary.0, &path).map_err(|e| io(&path, e))?;
    Ok(())
}

pub(crate) fn create_state(state_dir: &Path) -> Result<()> {
    if !check_state(state_dir)? {
        if let Some(parent) = state_dir.parent() {
            fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(state_dir)
            .map_err(|e| io(state_dir, e))?;
    }
    Ok(())
}
