#[path = "../../core/tests/common/mod.rs"]
mod common;
#[path = "common/skills.rs"]
mod fixtures;
use common::Scratch;
use fixtures::{agree, check_manifest, run, setup, sha256, skill, success};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Output,
};

const REMOVED: &str = "Voice removed your assistant's copy of your skills, so it can't be out of date. Fix this, then run voice skills refresh again.";
fn mirror(home: &Scratch) -> PathBuf {
    home.0.join(".claude/state/pebble/mirror")
}
fn refresh(home: &Scratch) -> Output {
    run(home, &["skills", "refresh", "member"])
}
fn rules(home: &Scratch, value: serde_json::Value) {
    fs::write(
        home.0.join("member/private.json"),
        json!({"anonymizer":value}).to_string(),
    )
    .unwrap();
}
fn two(home: &Scratch) {
    setup(
        home,
        json!({"terms":[{"term":"Pebble Freight","kind":"company"}]}),
    );
    skill(home, ".claude/skills/notes", "Work for Pebble Freight.\n");
    skill(home, ".claude/skills/writing", "Write plainly.\n");
    fs::create_dir(home.0.join(".claude/skills/notes/references")).unwrap();
    fs::write(
        home.0.join(".claude/skills/notes/references/a.md"),
        "Read this.\n",
    )
    .unwrap();
    agree(home, &["claude:notes", "claude:writing"]);
}
fn private_tree(path: &Path) {
    let meta = fs::symlink_metadata(path).unwrap();
    assert_eq!(
        meta.permissions().mode() & 0o777,
        if meta.is_dir() { 0o700 } else { 0o600 }
    );
    if meta.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            private_tree(&entry.unwrap().path());
        }
    } else {
        assert!(
            !String::from_utf8_lossy(&fs::read(path).unwrap())
                .to_lowercase()
                .contains("pebble")
        );
    }
}
#[test]
fn first_refresh_hashes_copies_and_rebuilds_on_every_run() {
    let home = Scratch::new("mirror-first");
    two(&home);
    let output = success(refresh(&home));
    assert_eq!(
        output,
        "notes: copied, 2 files\nwriting: copied, 1 files\nCopied 2 of 2 skills into your assistant's copy, ~/.claude/state/pebble/mirror.\nSince the last refresh: 2 new, 0 updated, 0 unchanged, 0 removed.\n"
    );
    private_tree(&mirror(&home));
    assert_eq!(
        fs::read_to_string(mirror(&home).join("notes/SKILL.md")).unwrap(),
        "Work for <company-1>.\n"
    );
    let value = check_manifest(&mirror(&home));
    assert_eq!(
        value["skills"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["notes", "writing"]
    );
    assert_eq!(
        value["skills"]["notes"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        vec!["SKILL.md", "references/a.md"]
    );
    let bytes = fs::read(mirror(&home).join(".manifest.json")).unwrap();
    assert!(
        !String::from_utf8_lossy(&bytes)
            .contains(&sha256(&home.0.join(".claude/skills/notes/SKILL.md")))
    );
    fs::write(mirror(&home).join("notes/SKILL.md"), "stale copy").unwrap();
    fs::write(mirror(&home).join("stale"), "stale file").unwrap();
    assert!(success(refresh(&home)).contains("0 new, 0 updated, 2 unchanged, 0 removed."));
    assert_eq!(
        bytes,
        fs::read(mirror(&home).join(".manifest.json")).unwrap()
    );
    assert!(!mirror(&home).join("stale").exists());
    assert_eq!(
        fs::read_to_string(mirror(&home).join("notes/SKILL.md")).unwrap(),
        "Work for <company-1>.\n"
    );
    skill(&home, ".claude/skills/writing", "Write clearly.\n");
    assert!(success(refresh(&home)).contains("0 new, 1 updated, 1 unchanged, 0 removed."));
    assert_eq!(
        fs::read_to_string(mirror(&home).join("writing/SKILL.md")).unwrap(),
        "Write clearly.\n"
    );
}
#[test]
fn withdrawn_never_copied_moved_and_gone_are_removed() {
    for reason in ["withdrawn", "never", "moved", "gone"] {
        let home = Scratch::new("mirror-remove");
        setup(&home, json!({}));
        skill(
            &home,
            ".claude/skills/pebble-freight-notes",
            "Public notes.\n",
        );
        agree(&home, &["claude:pebble-freight-notes"]);
        success(refresh(&home));
        match reason {
            "withdrawn" => {
                success(run(
                    &home,
                    &[
                        "skills",
                        "withdraw",
                        "member",
                        "claude:pebble-freight-notes",
                    ],
                ));
            }
            "never" => rules(&home, json!({"never_copy":["pebble-freight-notes"]})),
            "moved" => {
                fs::rename(
                    home.0.join(".claude/skills/pebble-freight-notes"),
                    home.0.join("moved-notes"),
                )
                .unwrap();
                symlink(
                    home.0.join("moved-notes"),
                    home.0.join(".claude/skills/pebble-freight-notes"),
                )
                .unwrap();
            }
            "gone" => {
                fs::remove_dir_all(home.0.join(".claude/skills/pebble-freight-notes")).unwrap()
            }
            _ => unreachable!(),
        }
        let mut value = if reason == "never" {
            json!({"never_copy":["pebble-freight-notes"]})
        } else {
            json!({})
        };
        value["terms"] = json!(["Pebble Freight"]);
        rules(&home, value);
        let output = refresh(&home);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.contains(if reason == "withdrawn" {
                "0 new, 0 updated, 0 unchanged, 0 removed."
            } else {
                "0 new, 0 updated, 0 unchanged, 1 removed."
            }),
            "{text}"
        );
        if matches!(reason, "withdrawn" | "moved") {
            assert!(text.contains("1 skills wait for your yes: voice skills list member\n"));
        }
        assert!(!text.to_lowercase().contains("pebble-freight"));
        assert!(
            !String::from_utf8_lossy(&output.stderr)
                .to_lowercase()
                .contains("pebble-freight")
        );
        assert!(!mirror(&home).join("pebble-freight-notes").exists());
    }
}
#[test]
fn later_rule_refuses_old_copy_and_returns_failure() {
    let home = Scratch::new("mirror-rule");
    setup(&home, json!({}));
    skill(&home, ".claude/skills/notes", "mypebble-freight.com\n");
    agree(&home, &["claude:notes"]);
    success(refresh(&home));
    rules(&home, json!({"terms":["Pebble Freight"]}));
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "notes: not copied, 2 things to fix\n  notes/SKILL.md:1 (skill file)\n  notes/SKILL.md:1 (your term)\nCopied 0 of 1 skills into your assistant's copy, ~/.claude/state/pebble/mirror.\nSince the last refresh: 0 new, 0 updated, 0 unchanged, 1 removed.\n"
    );
    assert!(!mirror(&home).join("notes").exists());
    assert!(
        check_manifest(&mirror(&home))["skills"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn failure_removes_only_voices_copy() {
    let home = Scratch::new("mirror-failure");
    two(&home);
    success(refresh(&home));
    rules(&home, json!({"unknown":true}));
    let output = refresh(&home);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .ends_with(REMOVED)
    );
    assert!(!mirror(&home).exists());
}
#[test]
fn unstamped_folder_is_left_alone() {
    let home = Scratch::new("mirror-unstamped");
    two(&home);
    fs::create_dir(mirror(&home)).unwrap();
    fs::set_permissions(mirror(&home), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(mirror(&home).join("notes.md"), "Keep this.").unwrap();
    let output = refresh(&home);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr,
        "voice: ~/.claude/state/pebble/mirror holds files Voice didn't put there. Move them away, then run voice skills refresh again.\n"
    );
    assert!(!stderr.contains("couldn't remove"));
    assert!(!stderr.contains("Delete"));
    assert_eq!(
        fs::read_to_string(mirror(&home).join("notes.md")).unwrap(),
        "Keep this."
    );
}
#[test]
fn bad_manifests_count_every_copy_as_new() {
    let home = Scratch::new("mirror-bad-manifest");
    two(&home);
    success(refresh(&home));
    for bytes in [
        b"not JSON".to_vec(),
        br#"{"version":2,"skills":{}}"#.to_vec(),
        vec![b'x'; 5 << 20],
    ] {
        fs::write(mirror(&home).join(".manifest.json"), bytes).unwrap();
        assert!(success(refresh(&home)).contains("2 new, 0 updated, 0 unchanged, 0 removed."));
    }
}
#[test]
fn mirror_link_is_refused_without_touching_target() {
    let home = Scratch::new("mirror-link");
    two(&home);
    let target = home.0.join("outside");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), "Keep this.").unwrap();
    symlink(&target, mirror(&home)).unwrap();
    assert!(!refresh(&home).status.success());
    assert_eq!(
        fs::read_to_string(target.join("keep")).unwrap(),
        "Keep this."
    );
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    assert!(
        fs::symlink_metadata(mirror(&home))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
#[test]
fn forget_removes_mirror() {
    let home = Scratch::new("mirror-forget");
    two(&home);
    success(refresh(&home));
    success(run(&home, &["forget", "member", "--yes"]));
    assert!(!mirror(&home).exists());
}

struct RestoreMode(PathBuf, u32);
impl RestoreMode {
    fn set(path: PathBuf, mode: u32) -> Self {
        let old = fs::metadata(&path).unwrap().permissions().mode();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        Self(path, old)
    }
}
impl Drop for RestoreMode {
    fn drop(&mut self) {
        fs::set_permissions(&self.0, fs::Permissions::from_mode(self.1)).unwrap();
    }
}
fn not_removed(path: &str) -> String {
    format!(
        "Voice couldn't remove your assistant's copy of your skills, so it may be out of date. Move anything you want to keep out of the folder {path}, delete the folder, then run voice skills refresh again."
    )
}
#[test]
fn unreadable_stamp_reports_mirror_not_removed() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-unreadable-stamp");
    two(&home);
    success(refresh(&home));
    let _mode = RestoreMode::set(mirror(&home).join(".anonymized-copy"), 0o000);
    rules(&home, json!({"unknown":true}));
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr
            .trim_end()
            .ends_with(&not_removed("~/.claude/state/pebble/mirror")),
        "{stderr}"
    );
    assert!(mirror(&home).join(".manifest.json").exists());
}

#[test]
fn partial_removal_drops_manifest_keeps_stamp_and_recovers() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-partial-removal");
    two(&home);
    success(refresh(&home));
    let mode = RestoreMode::set(mirror(&home).join("notes"), 0o500);
    rules(&home, json!({"unknown":true}));
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !mirror(&home).join(".manifest.json").exists(),
        "manifest survived failed removal"
    );
    assert!(mirror(&home).join(".anonymized-copy").exists());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr
            .trim_end()
            .ends_with(&not_removed("~/.claude/state/pebble/mirror")),
        "{stderr}"
    );
    drop(mode);
    rules(
        &home,
        json!({"terms":[{"term":"Pebble Freight","kind":"company"}]}),
    );
    success(refresh(&home));
    check_manifest(&mirror(&home));
    private_tree(&mirror(&home));
}

#[test]
fn withdrawal_removes_mirror_only_for_recorded_ids() {
    for recorded in [false, true] {
        let home = Scratch::new("mirror-withdraw");
        two(&home);
        success(refresh(&home));
        let id = if recorded {
            "claude:notes"
        } else {
            "claude:unknown"
        };
        let output = success(run(&home, &["skills", "withdraw", "member", id]));
        let mut expected = format!("{id}: no longer copied.\n");
        if recorded {
            expected.push_str("Your assistant's copy of your skills was removed, so nothing you withdrew stays in it. Make it again with: voice skills refresh member\n");
        }
        assert_eq!(output, expected);
        assert_eq!(mirror(&home).exists(), !recorded);
        if recorded {
            let output = success(refresh(&home));
            assert!(output.contains("1 new, 0 updated, 0 unchanged, 0 removed."));
            assert!(!mirror(&home).join("notes").exists());
        }
    }
}

#[test]
fn withdrawal_leaves_unstamped_mirror_and_returns_failure() {
    let home = Scratch::new("mirror-withdraw-unstamped");
    two(&home);
    success(refresh(&home));
    fs::remove_dir_all(mirror(&home)).unwrap();
    fs::create_dir(mirror(&home)).unwrap();
    fs::set_permissions(mirror(&home), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(mirror(&home).join("notes.md"), "Keep this.").unwrap();
    let output = run(&home, &["skills", "withdraw", "member", "claude:notes"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "claude:notes: no longer copied.\nVoice couldn't remove your assistant's copy of your skills. Move anything you want to keep out of the folder ~/.claude/state/pebble/mirror, delete the folder, then run voice skills refresh member\n"
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(mirror(&home).join("notes.md")).unwrap(),
        "Keep this."
    );
}

fn private_slug(home: &Scratch) -> PathBuf {
    setup(home, json!({"terms":["Pebble Freight"]}));
    let path = home.0.join("member/member.json");
    let mut member: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    member["slug"] = json!("pebble-freight");
    member["state_dir"] = json!("~/.claude/state/pebble-freight");
    fs::write(path, member.to_string()).unwrap();
    skill(home, ".claude/skills/notes", "Write plainly.\n");
    agree(home, &["claude:notes"]);
    home.0.join(".claude/state/pebble-freight/mirror")
}
#[test]
fn refresh_masks_private_slug_in_success_and_refusal() {
    let home = Scratch::new("mirror-private-slug");
    let path = private_slug(&home);
    let output = success(refresh(&home));
    assert!(output.contains("~/.claude/state/*/mirror."), "{output}");
    assert!(!output.contains("pebble-freight"));
    fs::remove_file(path.join(".anonymized-copy")).unwrap();
    fs::write(path.join("notes.md"), "Keep this.").unwrap();
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr,
        "voice: ~/.claude/state/*/mirror holds files Voice didn't put there. Move them away, then run voice skills refresh again.\n"
    );
    assert!(!stderr.contains("couldn't remove"));
    assert!(!stderr.contains("Delete"));
    assert_eq!(
        fs::read_to_string(path.join("notes.md")).unwrap(),
        "Keep this."
    );
    assert!(!stderr.contains("pebble-freight"));
}
#[test]
fn withdrawal_masks_private_paths() {
    for stamped in [true, false] {
        let home = Scratch::new("mirror-withdraw-private-paths");
        let path = private_slug(&home);
        success(refresh(&home));
        fs::rename(home.0.join("member"), home.0.join("pebble-freight")).unwrap();
        if !stamped {
            fs::remove_file(path.join(".anonymized-copy")).unwrap();
        }
        let output = run(
            &home,
            &["skills", "withdraw", "~/pebble-freight", "claude:notes"],
        );
        assert_eq!(output.status.code(), Some(if stamped { 0 } else { 1 }));
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("voice skills refresh ~/*"), "{stdout}");
        if !stamped {
            assert!(stdout.contains("~/.claude/state/*/mirror"), "{stdout}");
        }
        assert!(!stdout.contains("pebble-freight"));
        assert!(output.stderr.is_empty());
    }
}
