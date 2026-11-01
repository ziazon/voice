//! Fresh private skill copies and hashes of their written bytes.
use super::{
    Copied, check_output, copy::load_agreed, copy_agreed, display_path, finish_output, remove_copy,
    write_manifest,
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
pub const MIRROR_STALE: &str = "Voice marked your assistant's copy of your skills as out of date and left its files alone. Fix this, then run voice skills refresh again.";
pub const MIRROR_REMOVED: &str = "Voice removed your assistant's copy of your skills, so it can't be out of date. Fix this, then run voice skills refresh again.";

pub fn mirror_not_removed(path: &str) -> String {
    format!(
        "Voice couldn't remove your assistant's copy of your skills, so it may be out of date. Move anything you want to keep out of the folder {path}, delete the folder, then run voice skills refresh again."
    )
}

pub fn mirror_not_marked(path: &str) -> String {
    format!(
        "Voice couldn't mark your assistant's copy of your skills as out of date, so it may be out of date. Make sure you can change the folder {path}, fix this, then run voice skills refresh again."
    )
}

#[derive(Debug)]
enum Failure {
    Foreign,
    Overlap,
    Unchecked(Error),
    Other(Error),
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Other(error)
    }
}

fn output_error(error: Error, mirror: &Path) -> Failure {
    match error {
        Error::Invalid(_) => {
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

/// Invalidate only a stamped, real folder's regular manifest; never follow a link.
pub fn drop_manifest(mirror: &Path) -> bool {
    if fs::symlink_metadata(mirror).is_ok_and(|meta| meta.is_dir())
        && fs::symlink_metadata(mirror.join(".anonymized-copy")).is_ok()
    {
        let manifest = mirror.join(".manifest.json");
        if fs::symlink_metadata(&manifest).is_ok_and(|meta| meta.is_file()) {
            let _ = fs::remove_file(manifest);
        }
    }
    matches!(fs::symlink_metadata(mirror.join(".manifest.json")), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}

fn masked(error: Error, raw: &Path, shown: &str, home: &Path) -> Error {
    let state = raw.parent().unwrap_or(raw);
    let shown_state = shown.strip_suffix("/mirror").unwrap_or(shown);
    let home_path = PathBuf::from(display_path(state, home));
    let mut forms = vec![state.to_path_buf(), home_path.clone()];
    if let Ok(relative) = home_path.strip_prefix("~") {
        forms.push(home.join(relative));
        if let Ok(home) = fs::canonicalize(home) {
            forms.push(home.join(relative));
        }
    }
    forms.sort_by_key(|path| std::cmp::Reverse(path.as_os_str().len()));
    let mut message = match error {
        Error::Io { path, source } => {
            let path = forms.iter().find_map(|state| path.strip_prefix(state).ok());
            let message = match path {
                Some(path) if path.starts_with("mirror") => {
                    format!("Voice couldn't change {shown}: {source}")
                }
                Some(path) => {
                    let path = Path::new(shown_state).join(path);
                    format!("Voice couldn't change {}: {source}", path.display())
                }
                None => format!("Voice couldn't read or change a file: {source}"),
            };
            return Error::Invalid(message);
        }
        Error::Invalid(message) => message,
        other => other.to_string(),
    };
    for form in forms {
        message = message.replace(&form.display().to_string(), shown_state);
    }
    Error::Invalid(message)
}

/// The caller holds `private::lock_state(home, &member.slug)`.
pub fn refresh(home: &Path, member: &Member, date: &str) -> Result<Refreshed> {
    let mirror = home.join(".claude/state").join(&member.slug).join("mirror");
    let rules = Rules::for_member(member, home);
    let path = PathBuf::from(display_path(&mirror, home));
    let shown = match &rules {
        Ok(rules) => anonymize::report_path(&path, rules).display().to_string(),
        Err(_) => "~/.claude/state/*/mirror".to_owned(),
    };
    let mirror = private::private_folder(home, &member.slug, "mirror").map_err(|error| {
        drop_manifest(&mirror);
        masked(error, &mirror, &shown, home)
    })?;
    let result = (|| {
        let rules = rules.map_err(Failure::Unchecked)?;
        let (found, agreements, mut mapping) =
            load_agreed(home, member, &rules).map_err(Failure::Unchecked)?;
        let old = Manifest::read(&mirror.join(".manifest.json"));
        let out = check_output(
            &mirror,
            home,
            &found.skills,
            &found.protected,
            rules.projects(),
        )
        .map_err(|_| Failure::Overlap)?;
        let out = finish_output(&out).map_err(|error| output_error(error, &mirror))?;
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
        let (error, outcome) = match failure {
            Failure::Foreign => {
                return Error::Invalid(format!(
                    "{shown} holds files Voice didn't put there. Move them away, then run voice skills refresh again."
                ));
            }
            Failure::Overlap => {
                let mut message = format!(
                    "{shown} overlaps one of your skill or project folders, so Voice left it as it is. Move that folder, or the link to it, so neither is inside the other, then run voice skills refresh again."
                );
                if !drop_manifest(&mirror) {
                    message.push(' ');
                    message.push_str(&mirror_not_marked(&shown));
                }
                return Error::Invalid(message);
            }
            Failure::Unchecked(error) => {
                let outcome = if drop_manifest(&mirror) {
                    MIRROR_STALE.to_owned()
                } else {
                    mirror_not_marked(&shown)
                };
                (masked(error, &mirror, &shown, home), outcome)
            }
            Failure::Other(error) => {
                let error = masked(error, &mirror, &shown, home);
                let removed = matches!(remove_copy(&mirror), Ok(true))
                    || matches!(std::fs::symlink_metadata(&mirror), Err(e) if e.kind() == std::io::ErrorKind::NotFound);
                drop_manifest(&mirror);
                let outcome = if removed {
                    MIRROR_REMOVED.to_owned()
                } else {
                    mirror_not_removed(&shown)
                };
                (error, outcome)
            }
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
pub(super) mod common;

#[cfg(test)]
mod tests {
    use super::Manifest;
    use super::common::Scratch;
    use crate::skills::remove_copy;
    use serde_json::json;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn drop_manifest_reports_absence_without_following_links() {
        let home = Scratch::new("drop-manifest-result");
        let mirror = crate::skills::prepare_output(&home.0.join("mirror"), &home.0, &[]).unwrap();
        fs::write(mirror.join(".manifest.json"), "{}").unwrap();
        let link = home.0.join("link");
        symlink(&mirror, &link).unwrap();
        assert!(!super::drop_manifest(&link));
        assert!(mirror.join(".manifest.json").exists());
        assert!(super::drop_manifest(&mirror));
        assert!(!mirror.join(".manifest.json").exists());
        assert!(super::drop_manifest(&mirror));
        fs::remove_file(mirror.join(".anonymized-copy")).unwrap();
        fs::write(mirror.join(".manifest.json"), "{}").unwrap();
        assert!(!super::drop_manifest(&mirror));
        assert!(mirror.join(".manifest.json").exists());
    }

    #[test]
    fn finish_output_refusals_are_reworded() {
        use voice_core::error::Error;
        let home = Scratch::new("output-refusals");
        let mirror = home.0.join("mirror");
        fs::create_dir(&mirror).unwrap();
        fs::write(mirror.join("notes.md"), "Keep this.").unwrap();
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
    fn io_errors_mask_all_state_path_forms_and_hide_outside_files() {
        use voice_core::error::Error;
        let scratch = Scratch::new("masked-state-forms");
        let home = scratch.0.join("home");
        fs::create_dir(&home).unwrap();
        let alias = scratch.0.join("home-link");
        symlink(&home, &alias).unwrap();
        let state = ".claude/state/pebble-freight";
        let mirror = home.canonicalize().unwrap().join(state).join("mirror");
        let reason = std::io::Error::from(std::io::ErrorKind::PermissionDenied).to_string();
        for root in [
            alias.join(state),
            home.canonicalize().unwrap().join(state),
            std::path::PathBuf::from("~").join(state),
        ] {
            for (relative, shown) in [
                (
                    "anonymizer/placeholders.json",
                    "~/.claude/state/*/anonymizer/placeholders.json",
                ),
                ("mirror/notes/SKILL.md", "~/.claude/state/*/mirror"),
            ] {
                let error = Error::Io {
                    path: root.join(relative),
                    source: std::io::ErrorKind::PermissionDenied.into(),
                };
                assert_eq!(
                    super::masked(error, &mirror, "~/.claude/state/*/mirror", &alias).to_string(),
                    format!("Voice couldn't change {shown}: {reason}")
                );
            }
        }
        let error = Error::Io {
            path: scratch.0.join("outside.txt"),
            source: std::io::ErrorKind::PermissionDenied.into(),
        };
        assert_eq!(
            super::masked(error, &mirror, "~/.claude/state/*/mirror", &alias).to_string(),
            format!("Voice couldn't read or change a file: {reason}")
        );
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
