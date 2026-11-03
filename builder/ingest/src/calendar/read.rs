//! Read the chosen export by opened descriptor, with bounded memory and fixed refusals.
use super::{BAD, TOO_BIG, TOO_MANY, invalid};
use crate::{claudeai, folder::Folder};
use std::{
    ffi::OsStr,
    fs::File,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
};
use voice_core::{
    deny::DenyList,
    error::{Error, Result},
};

const FILE_CAP: u64 = 64 * 1024 * 1024;
const TOTAL_CAP: u64 = 128 * 1024 * 1024;
const HARD_LINK: &str = "Voice won't read this calendar: the same file has another name somewhere else on this computer (a hard link). Make a copy of it and choose the copy.";
fn ics(name: &[u8]) -> bool {
    name.len() >= 4 && name[name.len() - 4..].eq_ignore_ascii_case(b".ics")
}

pub fn read_export(path: &Path, deny: &DenyList, folder: bool) -> Result<Vec<Vec<u8>>> {
    with_caps(path, deny, folder, FILE_CAP, TOTAL_CAP)
}
fn read_file(path: &Path, file: File, cap: u64) -> Result<Vec<u8>> {
    let metadata = file.metadata().map_err(|_| invalid(BAD))?;
    if !metadata.is_file() {
        return Err(invalid(BAD));
    }
    if metadata.nlink() > 1 {
        return Err(invalid(HARD_LINK));
    }
    claudeai::read_capped(path, file, metadata.len(), cap).map_err(|error| match error {
        Error::Invalid(_) => invalid(TOO_BIG),
        _ => invalid(BAD),
    })
}
fn with_caps(
    path: &Path,
    deny: &DenyList,
    folder: bool,
    single: u64,
    total: u64,
) -> Result<Vec<Vec<u8>>> {
    deny.check(path).map_err(|_| invalid(BAD))?;
    if !folder {
        if !path.file_name().is_some_and(|name| ics(name.as_bytes())) {
            return Err(invalid(BAD));
        }
        return Ok(vec![read_file(
            path,
            claudeai::open(path).map_err(|_| invalid(BAD))?,
            single.min(total),
        )?]);
    }
    let folder = Folder::open(path, false).map_err(|_| invalid(BAD))?;
    let mut names = Vec::new();
    for name in folder.names().map_err(|_| invalid(BAD))? {
        if !ics(name.to_bytes()) {
            continue;
        }
        let metadata = folder.stat(&name, false).map_err(|_| invalid(BAD))?;
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
            continue;
        }
        names.push(name);
        if names.len() > 64 {
            return Err(invalid(TOO_MANY));
        }
    }
    if names.is_empty() {
        return Err(invalid(BAD));
    }
    names.sort();
    let mut remaining = total;
    let mut files = Vec::new();
    for name in names {
        let path = path.join(OsStr::from_bytes(name.to_bytes()));
        deny.check(&path).map_err(|_| invalid(BAD))?;
        let file = folder
            .file(&name, libc::O_NONBLOCK)
            .map_err(|_| invalid(BAD))?;
        let bytes = read_file(&path, file, single.min(remaining))?;
        remaining -= bytes.len() as u64;
        files.push(bytes);
    }
    Ok(files)
}
#[cfg(test)]
#[path = "../../tests/common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod test_common;
#[cfg(test)]
mod tests {
    use super::{test_common::Scratch, *};
    use std::fs;
    #[test]
    fn size_caps_apply_to_each_file_and_the_total_including_exact_limits() {
        let scratch = Scratch::new("calendar-caps");
        let one = scratch.0.join("a.ics");
        fs::write(&one, b"123").unwrap();
        fs::write(scratch.0.join("b.ics"), b"456").unwrap();
        for (path, folder, single, total) in [
            (&one, false, 2, 6),
            (&one, false, 3, 2),
            (&scratch.0, true, 2, 6),
            (&scratch.0, true, 3, 5),
        ] {
            let result = with_caps(path, &DenyList::builtin(), folder, single, total);
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().to_string(),
                "This calendar is bigger than Voice reads (128 MB). Export a shorter stretch of time, then choose it again."
            );
        }
        assert_eq!(
            with_caps(&scratch.0, &DenyList::builtin(), true, 3, 6).unwrap(),
            [b"123".to_vec(), b"456".to_vec()]
        );
        assert_eq!(
            with_caps(&one, &DenyList::builtin(), false, 3, 3).unwrap(),
            [b"123".to_vec()]
        );
    }
}
