use super::*;
use crate::{assistants, test_support::Scratch};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
const STAMP: &str = "2026-11-12-093000";
const OLD: &str = "keep\r\nAsk Ari (/ari) before choosing.\r\n/aria\r\n<!-- voice:begin other -->\r\n/ari\r\n<!-- voice:end other -->\r\n/ARI\r\nlast";
const KEPT: &str =
    "keep\r\n/aria\r\n<!-- voice:begin other -->\r\n/ari\r\n<!-- voice:end other -->\r\nlast";
struct Fixture {
    _scratch: Scratch,
    places: install::Places,
    member: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let home = scratch.path().canonicalize().unwrap();
        let claude_home = home.join(".claude");
        fs::create_dir(&claude_home).unwrap();
        let made = assistants::make(
            &home,
            &assistants::New {
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
            },
            "2026-11-12",
            "voice",
        )
        .unwrap();
        Self {
            _scratch: scratch,
            places: install::Places { home, claude_home },
            member: made.dir,
        }
    }
    fn skill(&self) -> PathBuf {
        self.places.claude_home.join("skills/ari")
    }
    fn claude(&self) -> PathBuf {
        self.places.claude_home.join("CLAUDE.md")
    }
    fn old_skill(&self) {
        fs::create_dir_all(self.skill().join("nested")).unwrap();
        fs::write(self.skill().join("SKILL.md"), b"old skill\0\xff").unwrap();
        fs::write(self.skill().join("nested/data"), b"nested bytes").unwrap();
        symlink("missing", self.skill().join("dangling")).unwrap();
    }
    fn no_backup(&self) {
        assert!(
            !self
                .places
                .home
                .join(".claude/voice-backups/ari")
                .join(STAMP)
                .exists()
        );
    }
}
#[test]
fn find_skill_lines_both_claimed_and_nothing() {
    let f = Fixture::new();
    assert!(!find(&f.places, "ari").unwrap().any());
    f.old_skill();
    let found = find(&f.places, "ari").unwrap();
    assert_eq!(found.skill, Some(f.skill()));
    assert!(found.lines.is_empty());
    fs::write(f.claude(), OLD).unwrap();
    let found = find(&f.places, "ari").unwrap();
    assert!(found.any());
    assert_eq!(found.claude_md, f.claude());
    assert_eq!(
        found.lines,
        vec![
            (2, "Ask Ari (/ari) before choosing.".into()),
            (7, "/ARI".into())
        ]
    );
    fs::remove_dir_all(f.skill()).unwrap();
    let found = find(&f.places, "ari").unwrap();
    assert!(found.skill.is_none());
    assert_eq!(found.lines.len(), 2);
    fs::remove_file(f.claude()).unwrap();
    install::turn_on(&f.member, &f.places).unwrap();
    assert!(!find(&f.places, "ari").unwrap().any());
}
#[test]
fn replacement_preserves_verified_bytes_other_lines_endings_and_mode() {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), OLD).unwrap();
    fs::set_permissions(f.claude(), fs::Permissions::from_mode(0o640)).unwrap();
    let result = replace(&f.places, "ari", STAMP).unwrap();
    assert!(result.skill);
    assert_eq!(result.lines, 2);
    assert!(!f.skill().exists());
    assert_eq!(
        fs::read(result.backup.join("skill/SKILL.md")).unwrap(),
        b"old skill\0\xff"
    );
    assert_eq!(
        fs::read(result.backup.join("skill/nested/data")).unwrap(),
        b"nested bytes"
    );
    assert_eq!(
        fs::read_link(result.backup.join("skill/dangling")).unwrap(),
        PathBuf::from("missing")
    );
    assert_eq!(
        fs::read(result.backup.join("CLAUDE.md")).unwrap(),
        OLD.as_bytes()
    );
    assert_eq!(fs::read(f.claude()).unwrap(), KEPT.as_bytes());
    assert_eq!(
        fs::metadata(f.claude()).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(
        !f.skill()
            .with_file_name(format!(".ari.voice-replacing-{STAMP}"))
            .exists()
    );
}
#[test]
fn replace_then_turn_on_and_off_never_restores_old_parts() {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), "keep\n/ari\n").unwrap();
    let result = replace(&f.places, "ari", STAMP).unwrap();
    install::turn_on(&f.member, &f.places).unwrap();
    install::turn_off(&f.places).unwrap();
    assert!(!f.skill().exists());
    assert_eq!(fs::read(f.claude()).unwrap(), b"keep\n");
    assert!(result.backup.join("skill/SKILL.md").is_file());
}
#[test]
fn changed_claude_rolls_skill_back_and_discards_backup() {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), OLD).unwrap();
    let result = replace_checked(&f.places, "ari", STAMP, &|| {}, &|| {
        fs::write(f.claude(), "concurrent edit\n/ari\n").unwrap();
    });
    assert!(matches!(result, Err(Error::Invalid(message)) if message.contains("changed")));
    assert_eq!(
        fs::read(f.skill().join("SKILL.md")).unwrap(),
        b"old skill\0\xff"
    );
    assert_eq!(fs::read(f.claude()).unwrap(), b"concurrent edit\n/ari\n");
    f.no_backup();
}
#[test]
fn changed_skill_rolls_back_and_discards_backup() {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), OLD).unwrap();
    let result = replace_checked(&f.places, "ari", STAMP, &|| {}, &|| {
        fs::write(f.skill().join("SKILL.md"), "concurrent skill").unwrap();
    });
    assert!(matches!(result, Err(Error::Invalid(message)) if message.contains("changed")));
    assert_eq!(
        fs::read(f.skill().join("SKILL.md")).unwrap(),
        b"concurrent skill"
    );
    assert_eq!(fs::read(f.claude()).unwrap(), OLD.as_bytes());
    f.no_backup();
}
#[test]
fn refuses_linked_and_non_folder_skills_without_any_changes() {
    for link in [false, true] {
        let f = Fixture::new();
        fs::create_dir_all(f.skill().parent().unwrap()).unwrap();
        let target = f.places.home.join("outside");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("data"), "keep").unwrap();
        if link {
            symlink(&target, f.skill()).unwrap();
        } else {
            fs::write(f.skill(), "keep").unwrap();
        }
        fs::write(f.claude(), OLD).unwrap();
        let result = replace(&f.places, "ari", STAMP);
        assert!(
            matches!(result, Err(Error::Invalid(message)) if message == "The older skill isn't a plain folder, so Voice left it. Move it away yourself, then turn the assistant on.")
        );
        assert_eq!(fs::read(f.claude()).unwrap(), OLD.as_bytes());
        assert_eq!(fs::read(target.join("data")).unwrap(), b"keep");
        if link {
            assert_eq!(fs::read_link(f.skill()).unwrap(), target);
        } else {
            assert_eq!(fs::read(f.skill()).unwrap(), b"keep");
        }
        f.no_backup();
    }
}
#[test]
fn no_old_one_is_a_clear_error_and_invalid_slug_is_refused() {
    let f = Fixture::new();
    assert!(
        matches!(replace(&f.places, "ari", STAMP), Err(Error::Invalid(message)) if message == "Voice found no older ari to replace.")
    );
    assert!(find(&f.places, "../ari").is_err());
    f.no_backup();
}

#[test]
fn stale_line_numbers_roll_skill_back_and_discard_backup() {
    stale_lines_refused("keep\r\nunrelated instruction\r\n/ari");
}

#[test]
fn stale_line_past_end_rolls_skill_back_and_discards_backup() {
    stale_lines_refused("keep\n");
}

fn stale_lines_refused(changed: &str) {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), "keep\r\n/ari\r\n").unwrap();
    let result = replace_checked(
        &f.places,
        "ari",
        STAMP,
        &|| fs::write(f.claude(), changed).unwrap(),
        &|| {},
    );
    assert!(matches!(result, Err(Error::Invalid(message)) if message == CHANGED));
    assert_eq!(fs::read(f.claude()).unwrap(), changed.as_bytes());
    assert_eq!(
        fs::read(f.skill().join("SKILL.md")).unwrap(),
        b"old skill\0\xff"
    );
    assert!(
        !f.skill()
            .with_file_name(format!(".ari.voice-replacing-{STAMP}"))
            .exists()
    );
    f.no_backup();
}

#[test]
fn undeletable_aside_is_a_success_with_verified_backup() {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), "keep\n/ari\n").unwrap();
    fs::set_permissions(f.skill().join("nested"), fs::Permissions::from_mode(0o555)).unwrap();
    let result = replace(&f.places, "ari", STAMP);
    let aside = f
        .skill()
        .with_file_name(format!(".ari.voice-replacing-{STAMP}"));
    // Restore before asserting so even a failed assertion leaves clean scratch files.
    fs::set_permissions(aside.join("nested"), fs::Permissions::from_mode(0o755)).unwrap();
    let replaced = result.expect("replacement already committed; cleanup failure is success");
    assert_eq!(replaced.leftover, Some(aside));
    assert_eq!(fs::read(f.claude()).unwrap(), b"keep\n");
    assert!(!f.skill().exists());
    assert_eq!(
        fs::read(replaced.backup.join("skill/nested/data")).unwrap(),
        b"nested bytes"
    );
    assert_eq!(
        fs::read(replaced.backup.join("CLAUDE.md")).unwrap(),
        b"keep\n/ari\n"
    );
}

#[test]
fn rollback_failure_sentence_is_recognized() {
    assert!(says(
        "Voice couldn't put the older ari skill back. It's at ~/.claude/skills/.ari.voice-replacing-<stamp>, and a copy is in ~/.claude/voice-backups/ari/<stamp>/."
    ));
}

#[test]
fn lone_carriage_return_on_final_found_line_replaces() {
    let f = Fixture::new();
    fs::write(f.claude(), "keep\n/ari\r").unwrap();
    let result = replace(&f.places, "ari", STAMP).unwrap();
    assert_eq!(result.lines, 1);
    assert_eq!(fs::read(f.claude()).unwrap(), b"keep\n");
    assert_eq!(
        fs::read(result.backup.join("CLAUDE.md")).unwrap(),
        b"keep\n/ari\r"
    );
}

#[test]
fn failed_replace_removes_only_a_new_empty_backup_parent() {
    for preexisting in [false, true] {
        let f = Fixture::new();
        f.old_skill();
        fs::write(f.claude(), "keep\n/ari\n").unwrap();
        let parent = f.places.home.join(".claude/voice-backups/ari");
        if preexisting {
            fs::create_dir_all(&parent).unwrap();
        }
        let result = replace_checked(&f.places, "ari", STAMP, &|| {}, &|| {
            fs::write(f.skill().join("SKILL.md"), "changed skill").unwrap();
        });
        assert!(matches!(result, Err(Error::Invalid(message)) if message == CHANGED));
        assert_eq!(parent.exists(), preexisting);
    }
}

#[test]
fn failed_backup_removes_new_empty_backup_parent() {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let f = Fixture::new();
    f.old_skill();
    let fifo = CString::new(f.skill().join("pipe").as_os_str().as_bytes()).unwrap();
    // SAFETY: fifo is a live NUL-terminated path and mode is valid.
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(replace(&f.places, "ari", STAMP).is_err());
    assert!(!f.places.home.join(".claude/voice-backups/ari").exists());
}

#[test]
fn changed_unfound_byte_after_copy_refuses_and_preserves_concurrent_edit() {
    let f = Fixture::new();
    f.old_skill();
    fs::write(f.claude(), "keep\n/ari\n").unwrap();
    let result = replace_checked(&f.places, "ari", STAMP, &|| {}, &|| {
        fs::write(f.claude(), "Keep\n/ari\n").unwrap();
    });
    assert!(matches!(result, Err(Error::Invalid(message)) if message == CHANGED));
    assert_eq!(
        fs::read(f.skill().join("SKILL.md")).unwrap(),
        b"old skill\0\xff"
    );
    assert_eq!(fs::read(f.claude()).unwrap(), b"Keep\n/ari\n");
    assert!(
        !f.skill()
            .with_file_name(format!(".ari.voice-replacing-{STAMP}"))
            .exists()
    );
    assert!(!f.places.home.join(".claude/voice-backups/ari").exists());
}
