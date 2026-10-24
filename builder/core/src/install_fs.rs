//! Private filesystem writes with an undo journal for an entire on/off call.
use crate::{
    error::{Error, Result},
    member::refuse_symlink,
};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub(super) fn io<T>(path: &Path, result: std::io::Result<T>) -> Result<T> {
    result.map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

pub(super) fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

pub(super) struct ClaudeFile {
    pub path: PathBuf,
    pub text: String,
    pub mode: u32,
    pub existed: bool,
    pub symlink: bool,
}
impl ClaudeFile {
    pub fn read(path: &Path) -> Result<Self> {
        let existed = exists(path)?;
        let symlink = existed && io(path, fs::symlink_metadata(path))?.is_symlink();
        let resolved = if existed {
            io(path, fs::canonicalize(path))?
        } else {
            path.to_owned()
        };
        if existed && !resolved.is_file() {
            return Err(Error::Invalid(format!(
                "{} is not a file: move it away and try again",
                path.display()
            )));
        }
        let text = if existed {
            io(path, fs::read_to_string(&resolved))?
        } else {
            String::new()
        };
        let mode = if existed {
            io(path, fs::metadata(&resolved))?.permissions().mode()
        } else {
            0o600
        };
        Ok(Self {
            path: resolved,
            text,
            mode,
            existed,
            symlink,
        })
    }
}

fn temporary_path(path: &Path, record: bool) -> PathBuf {
    path.with_file_name(if record {
        format!(".installed.json.tmp-{}", std::process::id())
    } else {
        format!(".CLAUDE.md.persona-builder-{}", std::process::id())
    })
}

pub(super) fn check_temporary(path: &Path, record: bool) -> Result<()> {
    let temporary = temporary_path(path, record);
    refuse_symlink(&temporary)?;
    if exists(&temporary)? {
        return Err(Error::Invalid(format!(
            "{} exists: move it away and try again",
            temporary.display()
        )));
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8], mode: u32, record: bool) -> Result<()> {
    refuse_symlink(path)?;
    let temporary = temporary_path(path, record);
    refuse_symlink(&temporary)?;
    let mut file = io(
        &temporary,
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary),
    )?;
    let result = (|| {
        io(&temporary, file.write_all(bytes))?;
        io(
            &temporary,
            file.set_permissions(fs::Permissions::from_mode(mode)),
        )?;
        io(path, fs::rename(&temporary, path))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

enum Undo {
    RemoveFile(PathBuf),
    RemoveDir(PathBuf),
    RestoreFile {
        path: PathBuf,
        bytes: Vec<u8>,
        mode: u32,
        record: bool,
    },
    RecreateFile {
        path: PathBuf,
        bytes: Vec<u8>,
        mode: u32,
    },
    RestoreDir {
        path: PathBuf,
        mode: u32,
    },
}
#[derive(Default)]
pub(super) struct Journal {
    undo: Vec<Undo>,
}
impl Journal {
    pub fn commit(mut self) {
        self.undo.clear();
    }
    pub fn directory(&mut self, path: &Path, follow: bool) -> Result<bool> {
        if !follow {
            refuse_symlink(path)?;
        }
        if exists(path)? {
            if path.is_dir() {
                return Ok(false);
            }
            return Err(Error::Invalid(format!(
                "{} is not a folder: move it away and try again",
                path.display()
            )));
        }
        io(path, fs::DirBuilder::new().mode(0o700).create(path))?;
        self.undo.push(Undo::RemoveDir(path.to_owned()));
        Ok(true)
    }
    pub fn new_file(&mut self, path: &Path, text: &str) -> Result<()> {
        refuse_symlink(path)?;
        let mut file = io(
            path,
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path),
        )?;
        self.undo.push(Undo::RemoveFile(path.to_owned()));
        io(path, file.write_all(text.as_bytes()))
    }
    pub fn replace(&mut self, path: &Path, text: &str, mode: u32, record: bool) -> Result<()> {
        refuse_symlink(path)?;
        let undo = if exists(path)? {
            Undo::RestoreFile {
                path: path.to_owned(),
                bytes: io(path, fs::read(path))?,
                mode: io(path, fs::metadata(path))?.permissions().mode(),
                record,
            }
        } else {
            Undo::RemoveFile(path.to_owned())
        };
        atomic_write(path, text.as_bytes(), mode, record)?;
        self.undo.push(undo);
        Ok(())
    }
    pub fn remove_file(&mut self, path: &Path, claude: bool) -> Result<()> {
        refuse_symlink(path)?;
        if claude {
            check_temporary(path, false)?;
        }
        let bytes = io(path, fs::read(path))?;
        let mode = io(path, fs::metadata(path))?.permissions().mode();
        io(path, fs::remove_file(path))?;
        let undo = if claude {
            Undo::RestoreFile {
                path: path.to_owned(),
                bytes,
                mode,
                record: false,
            }
        } else {
            Undo::RecreateFile {
                path: path.to_owned(),
                bytes,
                mode,
            }
        };
        self.undo.push(undo);
        Ok(())
    }
    pub fn remove_dir(&mut self, path: &Path) -> Result<bool> {
        refuse_symlink(path)?;
        let mode = io(path, fs::metadata(path))?.permissions().mode();
        match fs::remove_dir(path) {
            Ok(()) => {
                self.undo.push(Undo::RestoreDir {
                    path: path.to_owned(),
                    mode,
                });
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(false),
            Err(source) => Err(Error::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }
}
impl Drop for Journal {
    fn drop(&mut self) {
        for undo in self.undo.iter().rev() {
            match undo {
                Undo::RemoveFile(path) => {
                    let _ = fs::remove_file(path);
                }
                Undo::RemoveDir(path) => {
                    let _ = fs::remove_dir(path);
                }
                Undo::RestoreFile {
                    path,
                    bytes,
                    mode,
                    record,
                } => {
                    let _ = atomic_write(path, bytes, *mode, *record);
                }
                Undo::RecreateFile { path, bytes, mode } => {
                    // A removed file has no destination to replace. Recreate it exclusively,
                    // without claiming any unrelated temporary filename in its folder.
                    if let Ok(mut file) = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(path)
                    {
                        let _ = file.write_all(bytes);
                        let _ = file.set_permissions(fs::Permissions::from_mode(*mode));
                    }
                }
                Undo::RestoreDir { path, mode } => {
                    let _ = fs::DirBuilder::new().mode(*mode).create(path);
                }
            }
        }
    }
}
