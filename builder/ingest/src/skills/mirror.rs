//! Fresh private skill copies and hashes of their written bytes.
use super::{
    Copied, copy::load_agreed, copy_agreed, display_path, prepare_output_with_projects,
    remove_copy, write_manifest,
};
use crate::{
    anonymize::{self, Rules},
    private,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use voice_core::{
    error::{Error, Result},
    member::Member,
};

#[derive(Debug, Default)]
pub struct Manifest {
    pub skills: BTreeMap<String, BTreeMap<String, String>>,
}
impl Manifest {
    /// Unreadable, oversized or malformed manifests count as having no earlier copies.
    pub fn read(path: &Path) -> Self {
        Self::read_checked(path).unwrap_or_default()
    }
    fn read_checked(path: &Path) -> Option<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.is_file() || metadata.len() > 4 << 20 {
            return None;
        }
        let mut bytes = vec![];
        (&mut file)
            .take((4 << 20) + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 4 << 20 {
            return None;
        }
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        if value.get("version") != Some(&json!(1)) {
            return None;
        }
        let mut skills = BTreeMap::new();
        for (name, paths) in value.get("skills")?.as_object()? {
            let mut files = BTreeMap::new();
            for (path, hash) in paths.as_object()? {
                let hash = hash.as_str()?;
                if hash.len() != 64
                    || !hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return None;
                }
                files.insert(path.clone(), hash.into());
            }
            skills.insert(name.clone(), files);
        }
        Some(Self { skills })
    }
    pub fn to_json(&self, date: &str) -> Vec<u8> {
        let mut bytes =
            serde_json::to_vec(&json!({"version":1,"made_on":date,"skills":self.skills}))
                .expect("JSON values");
        bytes.push(b'\n');
        bytes
    }
}

pub struct Refreshed {
    pub copied: Copied,
    pub rules: Rules,
    pub mirror: PathBuf,
    pub new: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
}
pub const MIRROR_REMOVED: &str = "Voice removed your assistant's copy of your skills, so it can't be out of date. Fix this, then run voice skills refresh again.";

pub fn mirror_not_removed(path: &str) -> String {
    format!(
        "Voice couldn't remove your assistant's copy of your skills, so it may be out of date. Move anything you want to keep out of the folder {path}, delete the folder, then run voice skills refresh again."
    )
}

#[derive(Debug)]
enum Failure {
    Foreign,
    Other(Error),
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Other(error)
    }
}

fn output_error(error: Error, mirror: &Path) -> Failure {
    match error {
        Error::Invalid(ref message) if matches!(super::output::output_bad(), Error::Invalid(own) if *message == own) => {
            if matches!(fs::symlink_metadata(mirror.join(".anonymized-copy")), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
                && fs::read_dir(mirror)
                    .is_ok_and(|mut entries| matches!(entries.next(), Some(Ok(_))))
            {
                Failure::Foreign
            } else {
                Failure::Other(error)
            }
        }
        other => Failure::Other(other),
    }
}

fn masked(error: Error, raw: &Path, shown: &str, home: &Path) -> Error {
    let mut message = match error {
        Error::Io { source, .. } => {
            return Error::Invalid(format!("Voice couldn't change {shown}: {source}"));
        }
        Error::Invalid(message) => message,
        other => other.to_string(),
    };
    if let (Ok(home), Ok(relative)) = (fs::canonicalize(home), raw.strip_prefix(home)) {
        message = message.replace(&home.join(relative).display().to_string(), shown);
    }
    Error::Invalid(
        message
            .replace(&raw.display().to_string(), shown)
            .replace(&display_path(raw, home), shown),
    )
}

/// The caller holds `private::lock_state(home, &member.slug)`.
pub fn refresh(home: &Path, member: &Member, date: &str) -> Result<Refreshed> {
    let mirror = home.join(".claude/state").join(&member.slug).join("mirror");
    let rules = Rules::for_member(member, home);
    let path = PathBuf::from(display_path(&mirror, home));
    let shown = match &rules {
        Ok(rules) => anonymize::report_path(&path, rules).display().to_string(),
        Err(_) => path.display().to_string(),
    };
    let mirror = private::private_folder(home, &member.slug, "mirror")
        .map_err(|error| masked(error, &mirror, &shown, home))?;
    let result = (|| {
        let rules = rules?;
        let (found, agreements, mut mapping) = load_agreed(home, member, &rules)?;
        let old = Manifest::read(&mirror.join(".manifest.json"));
        let out = prepare_output_with_projects(&mirror, home, &found.skills, rules.projects())
            .map_err(|error| output_error(error, &mirror))?;
        let copied = copy_agreed(
            home,
            &member.slug,
            &rules,
            &found,
            &agreements,
            &mut mapping,
            &out,
        )?;
        write_manifest(&out, &copied.manifest.to_json(date))?;
        let mut new = 0;
        let mut updated = 0;
        let mut unchanged = 0;
        for (name, files) in &copied.manifest.skills {
            match old.skills.get(name) {
                None => new += 1,
                Some(before) if before == files => unchanged += 1,
                Some(_) => updated += 1,
            }
        }
        let removed = old
            .skills
            .keys()
            .filter(|name| !copied.manifest.skills.contains_key(*name))
            .count();
        Ok(Refreshed {
            copied,
            rules,
            mirror: mirror.clone(),
            new,
            updated,
            unchanged,
            removed,
        })
    })();
    result.map_err(|failure| {
        let error = match failure {
            Failure::Foreign => {
                return Error::Invalid(format!(
                    "{shown} holds files Voice didn't put there. Move them away, then run voice skills refresh again."
                ));
            }
            Failure::Other(error) => masked(error, &mirror, &shown, home),
        };
        let removed = matches!(remove_copy(&mirror), Ok(true))
            || matches!(std::fs::symlink_metadata(&mirror), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
        if !removed
            && fs::symlink_metadata(&mirror).is_ok_and(|meta| meta.is_dir())
            && fs::symlink_metadata(mirror.join(".anonymized-copy")).is_ok()
        {
            let manifest = mirror.join(".manifest.json");
            if fs::symlink_metadata(&manifest).is_ok_and(|meta| meta.is_file()) {
                let _ = fs::remove_file(manifest);
            }
        }
        let outcome = if removed {
            MIRROR_REMOVED.to_owned()
        } else {
            mirror_not_removed(&shown)
        };
        let message = match error {
            Error::Invalid(message) => message.strip_suffix('.').unwrap_or(&message).to_owned(),
            other => other.to_string(),
        };
        Error::Invalid(format!("{message}. {outcome}"))
    })
}

#[cfg(test)]
#[path = "../../../core/tests/common/mod.rs"]
mod common;

#[cfg(test)]
mod tests {
    use super::Manifest;
    use super::common::Scratch;
    use crate::skills::remove_copy;
    use serde_json::json;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn only_output_folder_refusals_are_reworded() {
        use voice_core::error::Error;
        let home = Scratch::new("output-refusals");
        let mirror = home.0.join("mirror");
        fs::create_dir(&mirror).unwrap();
        fs::write(mirror.join("notes.md"), "Keep this.").unwrap();
        let other = Error::Invalid("The stamp is a link".into());
        assert!(matches!(
            super::output_error(other, &mirror),
            super::Failure::Other(Error::Invalid(message)) if message == "The stamp is a link"
        ));
        let own = super::super::output::output_bad();
        assert!(matches!(
            super::output_error(own, &mirror),
            super::Failure::Foreign
        ));
        let error = Error::Io {
            path: "mirror".into(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        let message = error.to_string();
        assert!(matches!(
            super::output_error(error, &mirror),
            super::Failure::Other(error) if error.to_string() == message
        ));
    }

    #[test]
    fn bad_manifest_shapes_are_empty() {
        let home = Scratch::new("manifest-shapes");
        let path = home.0.join("manifest");
        let hash = "a".repeat(64);
        for value in [
            json!(null),
            json!([]),
            json!({}),
            json!({"version":2,"skills":{}}),
            json!({"version":1}),
            json!({"version":1,"skills":[]}),
            json!({"version":1,"skills":{"notes":null}}),
            json!({"version":1,"skills":{"notes":{"SKILL.md":1}}}),
            json!({"version":1,"skills":{"notes":{"SKILL.md":"A".repeat(64)}}}),
            json!({"version":1,"skills":{"notes":{"SKILL.md":"g".repeat(64)}}}),
            json!({"version":1,"skills":{"notes":{"SKILL.md":"a".repeat(63)}}}),
            json!({"version":1,"skills":{"notes":{"SKILL.md":hash},"bad":false}}),
        ] {
            fs::write(&path, value.to_string()).unwrap();
            assert!(Manifest::read(&path).skills.is_empty(), "{value}");
        }
        for bytes in [b"invalid".to_vec(), vec![b'x'; 5 << 20]] {
            fs::write(&path, bytes).unwrap();
            assert!(Manifest::read(&path).skills.is_empty());
        }
        fs::remove_file(&path).unwrap();
        assert!(Manifest::read(&path).skills.is_empty());
        assert!(Manifest::read(&home.0).skills.is_empty());
    }
    #[test]
    fn manifest_roundtrip_is_sorted_compact_and_no_follow() {
        let home = Scratch::new("manifest-roundtrip");
        let path = home.0.join("manifest");
        let manifest = Manifest {
            skills: [
                (
                    "writing".into(),
                    [
                        ("z.md".into(), "b".repeat(64)),
                        ("SKILL.md".into(), "a".repeat(64)),
                    ]
                    .into(),
                ),
                ("notes".into(), [("SKILL.md".into(), "c".repeat(64))].into()),
            ]
            .into(),
        };
        let bytes = manifest.to_json("2026-11-01");
        assert_eq!(
            String::from_utf8(bytes.clone()).unwrap(),
            format!(
                "{{\"version\":1,\"made_on\":\"2026-11-01\",\"skills\":{{\"notes\":{{\"SKILL.md\":\"{}\"}},\"writing\":{{\"SKILL.md\":\"{}\",\"z.md\":\"{}\"}}}}}}\n",
                "c".repeat(64),
                "a".repeat(64),
                "b".repeat(64)
            )
        );
        fs::write(&path, bytes).unwrap();
        assert_eq!(Manifest::read(&path).skills, manifest.skills);
        let link = home.0.join("link");
        symlink(&path, &link).unwrap();
        assert!(Manifest::read(&link).skills.is_empty());
        // A FIFO must return without blocking.
        use std::os::unix::ffi::OsStrExt;
        let fifo = home.0.join("fifo");
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a valid terminated path; mkfifo retains no pointer.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(Manifest::read(&fifo).skills.is_empty());
    }
    #[test]
    fn unreadable_stamp_is_not_voices_copy() {
        use std::os::unix::fs::PermissionsExt;
        // SAFETY: geteuid reads the effective user id and has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let home = Scratch::new("remove-unreadable-stamp");
        let path = crate::skills::prepare_output(&home.0.join("copy"), &home.0, &[]).unwrap();
        let stamp = path.join(".anonymized-copy");
        fs::set_permissions(&stamp, fs::Permissions::from_mode(0o000)).unwrap();
        let result = remove_copy(&path);
        fs::set_permissions(&stamp, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(result, Ok(false)), "{result:?}");
        assert!(path.exists());
    }

    #[test]
    fn remove_only_empty_or_stamped_real_folders() {
        let home = Scratch::new("remove-copy");
        let path = home.0.join("copy");
        assert!(!remove_copy(&path).unwrap());
        fs::create_dir(&path).unwrap();
        assert!(remove_copy(&path).unwrap());
        fs::create_dir(&path).unwrap();
        fs::write(path.join("notes"), "Keep").unwrap();
        assert!(!remove_copy(&path).unwrap());
        fs::write(path.join(".anonymized-copy"), "wrong stamp").unwrap();
        assert!(!remove_copy(&path).unwrap());
        fs::write(
            path.join(".anonymized-copy"),
            "Made by voice anonymize. Running it again replaces this folder.\n",
        )
        .unwrap();
        let link = home.0.join("link");
        symlink(&path, &link).unwrap();
        assert!(!remove_copy(&link).unwrap());
        assert!(path.join("notes").exists());
        assert!(remove_copy(&path).unwrap());
        assert!(!path.exists());
        fs::write(&path, "file").unwrap();
        assert!(!remove_copy(&path).unwrap());
    }
}
