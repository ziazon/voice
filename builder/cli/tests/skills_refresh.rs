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

const STALE: &str = "Voice marked your assistant's copy of your skills as out of date and left its files alone. Fix this, then run voice skills refresh again.";
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
fn invalid_rules_leave_copies_and_drop_manifest() {
    let home = Scratch::new("mirror-failure");
    two(&home);
    success(refresh(&home));
    rules(&home, json!({"unknown":true}));
    let output = refresh(&home);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .ends_with(STALE)
    );
    assert!(mirror(&home).join("notes/SKILL.md").is_file());
    assert!(mirror(&home).join("writing/SKILL.md").is_file());
    assert!(mirror(&home).join(".anonymized-copy").is_file());
    assert!(!mirror(&home).join(".manifest.json").exists());
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
fn invalid_rules_with_unreadable_stamp_leave_copies() {
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
    assert!(stderr.trim_end().ends_with(STALE), "{stderr}");
    assert!(!mirror(&home).join(".manifest.json").exists());
    assert!(mirror(&home).join(".anonymized-copy").exists());
    assert!(mirror(&home).join("notes/SKILL.md").is_file());
    assert!(mirror(&home).join("writing/SKILL.md").is_file());
}

#[test]
fn invalid_rules_with_readonly_skill_leave_copies_and_recover() {
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
    assert!(mirror(&home).join("notes/SKILL.md").is_file());
    assert!(mirror(&home).join("writing/SKILL.md").is_file());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.trim_end().ends_with(STALE), "{stderr}");
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

#[test]
fn refresh_masks_errors_while_removing_or_opening_mirror() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    for removing in [false, true] {
        let home = Scratch::new("mirror-masked-errors");
        let path = private_slug(&home);
        success(refresh(&home));
        let _mode = if removing {
            RestoreMode::set(path.join("notes"), 0o500)
        } else {
            RestoreMode::set(path.clone(), 0o755)
        };
        let output = refresh(&home);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("~/.claude/state/*/mirror"), "{stderr}");
        for private in ["pebble-freight", "notes", home.0.to_str().unwrap()] {
            assert!(!stderr.contains(private), "{stderr}");
        }
        if removing {
            assert!(
                stderr
                    .trim_end()
                    .ends_with(&not_removed("~/.claude/state/*/mirror")),
                "{stderr}"
            );
        } else {
            assert!(
                stderr.starts_with("voice: ~/.claude/state/*/mirror can be opened by others"),
                "{stderr}"
            );
            assert!(!stderr.contains("/private"), "{stderr}");
            assert!(!stderr.contains("couldn't remove"), "{stderr}");
            assert!(!path.join(".manifest.json").exists());
        }
    }
}

#[test]
fn withdrawal_reports_mirror_open_errors_without_stderr() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-withdraw-open-error");
    let path = private_slug(&home);
    success(refresh(&home));
    let _mode = RestoreMode::set(path.clone(), 0o755);
    let output = run(&home, &["skills", "withdraw", "member", "claude:notes"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "claude:notes: no longer copied.\nVoice couldn't remove your assistant's copy of your skills. Move anything you want to keep out of the folder ~/.claude/state/*/mirror, delete the folder, then run voice skills refresh member\n"
    );
    assert!(output.stderr.is_empty());
    assert!(!path.join(".manifest.json").exists());
}

#[test]
fn unreadable_stamp_with_valid_rules_is_a_stale_copy() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-stale-stamp");
    two(&home);
    success(refresh(&home));
    let _mode = RestoreMode::set(mirror(&home).join(".anonymized-copy"), 0o000);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr
            .trim_end()
            .ends_with(&not_removed("~/.claude/state/pebble/mirror")),
        "{stderr}"
    );
    assert!(!stderr.contains("didn't put there"), "{stderr}");
    assert!(!mirror(&home).join(".manifest.json").exists());
    assert!(mirror(&home).join(".anonymized-copy").exists());
}

#[test]
fn refresh_leaves_overlapping_project_and_drops_manifest() {
    let home = Scratch::new("mirror-overlap-refresh");
    let path = private_slug(&home);
    success(refresh(&home));
    fs::create_dir(path.join("proj")).unwrap();
    fs::write(path.join("proj/work.txt"), "Keep this work.\n").unwrap();
    rules(
        &home,
        json!({"terms":["Pebble Freight"], "projects":["~/.claude/state/pebble-freight/mirror/proj"]}),
    );
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: ~/.claude/state/*/mirror overlaps one of your skill or project folders, so Voice left it as it is. Move that folder, or the link to it, so neither is inside the other, then run voice skills refresh again.\n"
    );
    assert_eq!(
        fs::read_to_string(path.join("proj/work.txt")).unwrap(),
        "Keep this work.\n"
    );
    assert!(!path.join(".manifest.json").exists());
}

#[test]
fn withdrawal_leaves_overlapping_project_and_drops_manifest() {
    let home = Scratch::new("mirror-overlap-withdraw");
    let path = private_slug(&home);
    success(refresh(&home));
    fs::create_dir(path.join("proj")).unwrap();
    fs::write(path.join("proj/work.txt"), "Keep this work.\n").unwrap();
    rules(
        &home,
        json!({"terms":["Pebble Freight"], "projects":["~/.claude/state/pebble-freight/mirror/proj"]}),
    );
    let output = run(&home, &["skills", "withdraw", "member", "claude:notes"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "claude:notes: no longer copied.\nVoice couldn't remove your assistant's copy of your skills. Move anything you want to keep out of the folder ~/.claude/state/*/mirror, delete the folder, then run voice skills refresh member\n"
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(path.join("proj/work.txt")).unwrap(),
        "Keep this work.\n"
    );
    assert!(!path.join(".manifest.json").exists());
}

#[test]
fn refresh_removes_copy_after_anonymizer_save_fails_and_recovers() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-anonymizer-save");
    two(&home);
    success(refresh(&home));
    let mode = RestoreMode::set(home.0.join(".claude/state/pebble/anonymizer"), 0o500);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.trim_end().ends_with("Voice removed your assistant's copy of your skills, so it can't be out of date. Fix this, then run voice skills refresh again."),
        "{stderr}"
    );
    assert!(
        stderr.contains("~/.claude/state/pebble/anonymizer"),
        "{stderr}"
    );
    assert!(!stderr.contains(home.0.to_str().unwrap()), "{stderr}");
    assert!(!mirror(&home).exists());
    drop(mode);
    success(refresh(&home));
}

#[test]
fn refresh_masks_anonymizer_folder_errors_and_drops_manifest() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-anonymizer-mode");
    let path = private_slug(&home);
    success(refresh(&home));
    let _mode = RestoreMode::set(path.parent().unwrap().join("anonymizer"), 0o755);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("~/.claude/state/*/anonymizer"), "{stderr}");
    assert!(stderr.trim_end().ends_with(STALE), "{stderr}");
    for private in ["pebble-freight", "/private", home.0.to_str().unwrap()] {
        assert!(!stderr.contains(private), "{stderr}");
    }
    assert!(!path.join(".manifest.json").exists());
    assert!(path.join("notes/SKILL.md").is_file());
    assert!(path.join(".anonymized-copy").is_file());
}

#[test]
fn refresh_names_unreadable_placeholders_file() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-placeholders-mode");
    two(&home);
    success(refresh(&home));
    let _mode = RestoreMode::set(
        home.0
            .join(".claude/state/pebble/anonymizer/placeholders.json"),
        0o000,
    );
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("~/.claude/state/pebble/anonymizer/placeholders.json"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("couldn't change ~/.claude/state/pebble/mirror:"),
        "{stderr}"
    );
}

#[test]
fn unchecked_placeholders_leave_overlapping_project() {
    unchecked_failure_leaves_overlapping_project(false);
}

#[test]
fn unchecked_rules_leave_overlapping_project() {
    unchecked_failure_leaves_overlapping_project(true);
}

fn unchecked_failure_leaves_overlapping_project(bad_rules: bool) {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-unchecked-project");
    let path = private_slug(&home);
    success(refresh(&home));
    fs::create_dir(path.join("proj")).unwrap();
    fs::write(path.join("proj/work.txt"), "Keep this work.\n").unwrap();
    let mut value = json!({"terms":["Pebble Freight"], "projects":["~/.claude/state/pebble-freight/mirror/proj"]});
    let _mode = if bad_rules {
        value["unknown"] = json!(true);
        None
    } else {
        Some(RestoreMode::set(
            path.parent().unwrap().join("anonymizer/placeholders.json"),
            0o000,
        ))
    };
    rules(&home, value);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.trim_end().ends_with(STALE), "{stderr}");
    assert!(!stderr.contains("pebble-freight"), "{stderr}");
    assert!(!stderr.contains(home.0.to_str().unwrap()), "{stderr}");
    if !bad_rules {
        assert!(
            stderr.contains("~/.claude/state/*/anonymizer/placeholders.json"),
            "{stderr}"
        );
    }
    assert_eq!(
        fs::read_to_string(path.join("proj/work.txt")).unwrap(),
        "Keep this work.\n"
    );
    assert!(path.join("notes/SKILL.md").is_file());
    assert!(!path.join(".manifest.json").exists());
    assert!(path.join(".anonymized-copy").is_file());
}

#[test]
fn invalid_rules_mask_slug_when_mirror_cannot_be_opened() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-unchecked-shown");
    let path = private_slug(&home);
    success(refresh(&home));
    rules(&home, json!({"unknown":true}));
    let _mode = RestoreMode::set(path.clone(), 0o755);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("voice: ~/.claude/state/*/mirror can be opened by others"),
        "{stderr}"
    );
    assert!(!stderr.contains("pebble-freight"), "{stderr}");
    assert!(!stderr.contains(home.0.to_str().unwrap()), "{stderr}");
    assert!(path.join("notes/SKILL.md").is_file());
    assert!(!path.join(".manifest.json").exists());
    assert!(path.join(".anonymized-copy").is_file());
}

fn not_marked(path: &str) -> String {
    format!(
        "Voice couldn't mark your assistant's copy of your skills as out of date, so it may be out of date. Make sure you can change the folder {path}, fix this, then run voice skills refresh again."
    )
}

#[test]
fn readonly_mirror_reports_unchecked_manifest_failure() {
    readonly_mirror_reports_manifest_failure(false);
}

#[test]
fn readonly_mirror_reports_overlap_manifest_failure() {
    readonly_mirror_reports_manifest_failure(true);
}

fn readonly_mirror_reports_manifest_failure(overlap: bool) {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid reads the effective user id and has no preconditions.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("mirror-not-marked");
    two(&home);
    success(refresh(&home));
    if overlap {
        fs::create_dir(mirror(&home).join("proj")).unwrap();
        fs::write(mirror(&home).join("proj/work.txt"), "Keep this work.\n").unwrap();
        rules(
            &home,
            json!({"projects":["~/.claude/state/pebble/mirror/proj"]}),
        );
    } else {
        rules(&home, json!({"unknown":true}));
    }
    let _mode = RestoreMode::set(mirror(&home), 0o500);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    let shown = if overlap {
        "~/.claude/state/pebble/mirror"
    } else {
        "~/.claude/state/*/mirror"
    };
    assert!(stderr.trim_end().ends_with(&not_marked(shown)), "{stderr}");
    assert!(!stderr.trim_end().ends_with(STALE), "{stderr}");
    if overlap {
        assert!(stderr.starts_with("voice: ~/.claude/state/pebble/mirror overlaps one of your skill or project folders, so Voice left it as it is. Move that folder, or the link to it, so neither is inside the other, then run voice skills refresh again."), "{stderr}");
        assert_eq!(
            fs::read_to_string(mirror(&home).join("proj/work.txt")).unwrap(),
            "Keep this work.\n"
        );
    }
    assert!(mirror(&home).join(".manifest.json").is_file());
}

fn add_project(home: &Scratch, path: &str) {
    let file = home.0.join("member/private.json");
    let mut value: serde_json::Value = match fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({"anonymizer":{}}),
        Err(error) => panic!("{error}"),
    };
    let projects = value["anonymizer"]
        .as_object_mut()
        .unwrap()
        .entry("projects")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .unwrap();
    if !projects.contains(&json!(path)) {
        projects.push(json!(path));
    }
    fs::write(file, value.to_string()).unwrap();
}

fn linked_root_entry(home: &Scratch, case: &str) -> PathBuf {
    let (link, target, file) = match case {
        "codex" => (".codex/skills", "codex", "README.md"),
        "project" => {
            add_project(home, "~/proj");
            ("proj/.claude/skills", "proj-skills", "wip/plan.md")
        }
        "denied" => (".claude/skills/release.key", "release.key", "SKILL.md"),
        "drafts" => (".claude/skills/drafts", "drafts", "ideas.md"),
        "hidden" => (".claude/skills/.wip", "wip", "notes.md"),
        "denied-project" => {
            add_project(home, "~/vault.key");
            ("vault.key/.claude/skills", "vault-skills", "plan.md")
        }
        "denied-project-entry" => {
            add_project(home, "~/notes.key");
            fs::create_dir_all(home.0.join("notes.key/.claude/skills")).unwrap();
            ("notes.key/.claude/skills/drafts", "vd", "plan.md")
        }
        "synced" => (".claude/skills/synced", "synced", "drafts/ideas.md"),
        "synced-entry" => {
            fs::create_dir_all(home.0.join(".claude/skills/synced/acct")).unwrap();
            (
                ".claude/skills/synced/acct/drafts",
                "synced-drafts",
                "ideas.md",
            )
        }
        "plugin" => {
            let install = home.0.join(".claude/plugins/cache/ink");
            fs::create_dir_all(install.join("skills")).unwrap();
            fs::write(
                home.0.join(".claude/plugins/installed_plugins.json"),
                json!({"plugins":{"ink@local":[{"installPath":install}]}}).to_string(),
            )
            .unwrap();
            (
                ".claude/plugins/cache/ink/skills/drafts",
                "plugin-drafts",
                "SKILL.md",
            )
        }
        _ => panic!("unknown fixture"),
    };
    let target = mirror(home).join(target);
    let file = target.join(file);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "Keep this work.\n").unwrap();
    let link = home.0.join(link);
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    symlink(target, link).unwrap();
    file
}

fn refresh_leaves_linked_root_entry(case: &str) {
    let home = Scratch::new("mirror-linked-root");
    two(&home);
    success(refresh(&home));
    let file = linked_root_entry(&home, case);
    let output = refresh(&home);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: ~/.claude/state/pebble/mirror overlaps one of your skill or project folders, so Voice left it as it is. Move that folder, or the link to it, so neither is inside the other, then run voice skills refresh again.\n"
    );
    assert_eq!(fs::read_to_string(file).unwrap(), "Keep this work.\n");
    assert!(!mirror(&home).join(".manifest.json").exists());
}

#[test]
fn refresh_protects_linked_codex_root() {
    refresh_leaves_linked_root_entry("codex");
}

#[test]
fn refresh_protects_linked_project_root() {
    refresh_leaves_linked_root_entry("project");
}

#[test]
fn refresh_protects_linked_denied_entry() {
    refresh_leaves_linked_root_entry("denied");
}

#[test]
fn refresh_protects_linked_draft_entry() {
    refresh_leaves_linked_root_entry("drafts");
}

#[test]
fn refresh_protects_linked_hidden_entry() {
    refresh_leaves_linked_root_entry("hidden");
}

#[test]
fn refresh_protects_linked_denied_project_root() {
    refresh_leaves_linked_root_entry("denied-project");
}

#[test]
fn refresh_protects_linked_denied_project_entry() {
    refresh_leaves_linked_root_entry("denied-project-entry");
}

#[test]
fn refresh_protects_linked_synced_account_entry() {
    refresh_leaves_linked_root_entry("synced-entry");
}

#[test]
fn refresh_protects_linked_synced_entry() {
    refresh_leaves_linked_root_entry("synced");
}

#[test]
fn refresh_protects_linked_plugin_entry() {
    refresh_leaves_linked_root_entry("plugin");
}

#[test]
fn withdrawal_protects_linked_denied_entry() {
    withdrawal_leaves_linked_root_entry("denied");
}

#[test]
fn withdrawal_protects_linked_denied_project_entry() {
    withdrawal_leaves_linked_root_entry("denied-project-entry");
}

fn withdrawal_leaves_linked_root_entry(case: &str) {
    let home = Scratch::new("mirror-linked-withdraw");
    two(&home);
    success(refresh(&home));
    let file = linked_root_entry(&home, case);
    let output = run(&home, &["skills", "withdraw", "member", "claude:notes"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "claude:notes: no longer copied.\nVoice couldn't remove your assistant's copy of your skills. Move anything you want to keep out of the folder ~/.claude/state/pebble/mirror, delete the folder, then run voice skills refresh member\n"
    );
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_to_string(file).unwrap(), "Keep this work.\n");
    assert!(!mirror(&home).join(".manifest.json").exists());
}

#[test]
fn anonymize_protects_linked_denied_entry() {
    anonymize_leaves_linked_root_entry("denied");
}

#[test]
fn anonymize_protects_linked_denied_project_entry() {
    anonymize_leaves_linked_root_entry("denied-project-entry");
}

fn anonymize_leaves_linked_root_entry(case: &str) {
    let home = Scratch::new("mirror-linked-anonymize");
    two(&home);
    success(refresh(&home));
    let file = linked_root_entry(&home, case);
    let output = run(
        &home,
        &[
            "anonymize",
            "member",
            "--out",
            "~/.claude/state/pebble/mirror",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "voice: Choose an output folder that is empty or was made by voice anonymize, outside your skill folders.\n"
    );
    assert_eq!(fs::read_to_string(file).unwrap(), "Keep this work.\n");
}

#[test]
fn anonymize_fresh_output_with_linked_roots_still_works() {
    let home = Scratch::new("mirror-linked-fresh-output");
    two(&home);
    success(refresh(&home));
    let files = [
        "codex",
        "project",
        "denied",
        "drafts",
        "synced",
        "plugin",
        "hidden",
        "denied-project",
        "denied-project-entry",
    ]
    .map(|case| linked_root_entry(&home, case));
    let private: serde_json::Value =
        serde_json::from_slice(&fs::read(home.0.join("member/private.json")).unwrap()).unwrap();
    assert_eq!(
        private["anonymizer"]["projects"],
        json!(["~/proj", "~/vault.key", "~/notes.key"])
    );
    success(run(&home, &["anonymize", "member", "--out", "fresh"]));
    assert!(home.0.join("fresh/notes/SKILL.md").is_file());
    assert!(home.0.join("fresh/writing/SKILL.md").is_file());
    check_manifest(&home.0.join("fresh"));
    for file in files {
        assert_eq!(fs::read_to_string(file).unwrap(), "Keep this work.\n");
    }
}
