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
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
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
pub const CANT_BRING: &str = "Voice couldn't bring in the older assistant's own files, so it changed nothing. Untick private.json and autonomy.md, then try again.";
const LINK: &str = "It's a link, so Voice leaves it.";
const FILE: &str = "It isn't a plain file, so Voice leaves it.";
const FOLDER: &str = "It isn't a plain folder of files, so Voice leaves it.";
const BIG: &str = "It's too big for Voice to read, so Voice leaves it.";
const TEXT: &str = "It isn't plain text, so Voice leaves it.";
const AUTONOMY: &str = "Voice can't read its list of who decides what, so Voice leaves it.";
const PRIVATE: &str = "It changes more than the privacy settings, so Voice leaves it.";
const REFUSE_START: &str = "Voice can't bring in ";
const REFUSE_END: &str = ", so it changed nothing. Untick it and try again.";

pub fn says(message: &str) -> bool {
    matches!(
        message,
        CANT_BRING | LINK | FILE | FOLDER | BIG | TEXT | AUTONOMY | PRIVATE
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
pub fn offer(folder: &Path) -> Result<Vec<Offered>> {
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
        if let Some(checked) = check(folder, name)? {
            offered.push(Offered {
                name,
                problem: checked.problem,
            });
        }
    }
    Ok(offered)
}

fn check(folder: &Path, name: &str) -> Result<Option<Checked>> {
    let path = folder.join(name);
    let metadata = match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        metadata => io(&path, metadata)?,
    };
    if metadata.is_symlink() {
        return Ok(Some(Checked::problem(LINK)));
    }
    if name != "briefs" {
        return Ok(Some(check_file(&path, Path::new(name), file_cap(name))?));
    }
    if !metadata.is_dir() {
        return Ok(Some(Checked::problem(FOLDER)));
    }
    let mut entries = io(&path, fs::read_dir(&path))?
        .take(65)
        .map(|entry| io(&path, entry))
        .collect::<Result<Vec<_>>>()?;
    if entries.len() > 64 {
        return Ok(Some(Checked::problem(FOLDER)));
    }
    entries.sort_by_key(|entry| entry.file_name());
    // Shape errors take precedence over the contents of any brief.
    for entry in &entries {
        if !io(&entry.path(), fs::symlink_metadata(entry.path()))?.is_file() {
            return Ok(Some(Checked::problem(FOLDER)));
        }
    }
    let mut files = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        let cap = if name == "codebook.md" {
            64 * 1024 // voice_build::draft::CODEBOOK_MOST; no dependency on the build crate.
        } else {
            1024 * 1024
        };
        let checked = check_file(&entry.path(), &Path::new("briefs").join(name), cap)?;
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

fn check_file(path: &Path, relative: &Path, cap: u64) -> Result<Checked> {
    let metadata = io(path, fs::symlink_metadata(path))?;
    let problem = if metadata.is_symlink() {
        Some(LINK)
    } else if !metadata.is_file() {
        Some(FILE)
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
        let private = serde_json::from_str::<serde_json::Value>(text);
        if !private
            .ok()
            .and_then(|value| value.as_object().cloned())
            .is_some_and(|object| {
                object
                    .keys()
                    .all(|key| matches!(key.as_str(), "filters" | "anonymizer"))
            })
        {
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
pub fn bring(from: &Path, to: &Path, names: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    let mut selections = Vec::new();
    for name in names {
        if !NAMES.contains(&name.as_str()) || !seen.insert(name) {
            return Err(refusal(name));
        }
        if !fs::symlink_metadata(from).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(refusal(name));
        }
        match check(from, name).map_err(|_| refusal(name))? {
            Some(checked) if checked.problem.is_none() => selections.extend(checked.files),
            _ => return Err(refusal(name)),
        }
    }
    copy_and_check(to, &selections).map_err(|_| Error::Invalid(CANT_BRING.into()))
}

fn copy_and_check(to: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    if !io(to, fs::symlink_metadata(to))?.is_dir() {
        return Err(Error::Invalid(CANT_BRING.into()));
    }
    let expected = member::read_member(&to.join("member.json"))?;
    for (relative, bytes) in files {
        if relative.starts_with("briefs") {
            let briefs = to.join("briefs");
            match fs::symlink_metadata(&briefs) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    io(&briefs, fs::create_dir(&briefs))?
                }
                metadata => {
                    if !io(&briefs, metadata)?.is_dir() {
                        return Err(Error::Invalid(CANT_BRING.into()));
                    }
                }
            }
        }
        let target = to.join(relative);
        let mut file = io(
            &target,
            OpenOptions::new()
                .custom_flags(libc::O_NOFOLLOW)
                .write(true)
                .truncate(true)
                .create(true)
                .mode(0o600)
                .open(&target),
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

#[cfg(test)]
mod tests;
