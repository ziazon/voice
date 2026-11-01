#[path = "../../core/tests/common/mod.rs"]
mod common;

use common::{Scratch, repo};
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn run(home: &Scratch, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_persona-builder"))
        .env_clear()
        .env("HOME", &home.0)
        .current_dir(&home.0)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn setup(home: &Scratch, rules: serde_json::Value) {
    fs::create_dir(home.0.join("member")).unwrap();
    let mut member: serde_json::Value =
        serde_json::from_slice(&fs::read(repo().join("crew/_template/member.json")).unwrap())
            .unwrap();
    member["slug"] = json!("pebble");
    member["name"] = json!("Pebble");
    member["principal"] = json!("Rowan Vale");
    member["state_dir"] = json!("~/.claude/state/pebble");
    fs::write(home.0.join("member/member.json"), member.to_string()).unwrap();
    fs::write(
        home.0.join("member/private.json"),
        json!({"anonymizer":rules}).to_string(),
    )
    .unwrap();
}
fn skill(home: &Scratch, relative: &str, text: &str) {
    let folder = home.0.join(relative);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("SKILL.md"), text).unwrap();
}
fn agree(home: &Scratch, ids: &[&str]) {
    let mut args = vec!["skills", "agree", "member"];
    args.extend(ids);
    success(run(home, &args));
}
fn anonymize(home: &Scratch, out: &str) -> Output {
    run(home, &["anonymize", "member", "--out", out])
}

#[test]
fn repeat_verifier_punctuation_refuses_skill_end_to_end() {
    let home = Scratch::new("anonymize-punctuation");
    setup(&home, json!({"terms":["Pebble Freight"]}));
    skill(
        &home,
        ".claude/skills/notes",
        "Public\nmypebble-freight.com\n",
    );
    agree(&home, &["claude:notes"]);
    let output = anonymize(&home, "copy");
    assert_eq!(output.status.code(), Some(1));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("notes/SKILL.md:2 (your term)"), "{report}");
    assert!(!report.to_lowercase().contains("pebble-freight"));
    assert!(!home.0.join("copy/notes").exists());
}
#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn list_states_agreement_and_withdrawal() {
    let home = Scratch::new("anonymize-states");
    setup(&home, json!({"never_copy":["family-*"]}));
    skill(&home, ".claude/skills/notes", "Useful notes\n");
    skill(&home, ".claude/skills/family-notes", "Useful notes\n");
    skill(
        &home,
        ".claude/skills/synced/account/synced-notes",
        "Useful notes\n",
    );
    let listed = success(run(&home, &["skills", "list", "member"]));
    let lines = listed.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[0],
        "1. claude:family-notes\tnever copied: on your never_copy list\t~/.claude/skills/family-notes"
    );
    assert_eq!(
        lines[1],
        "2. claude:notes\twaiting for your yes\t~/.claude/skills/notes"
    );
    assert_eq!(
        lines[2],
        "3. synced:synced-notes\ta claude.ai skill: not offered, agree by name to copy it\t~/.claude/skills/synced/account/synced-notes"
    );
    assert!(listed.contains("never copied: on your never_copy list"));
    assert!(listed.contains("a claude.ai skill: not offered, agree by name to copy it"));
    let output = run(
        &home,
        &["skills", "agree", "member", "claude:notes", "claude:absent"],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("claude:absent isn't one of the skills on this computer.")
    );
    assert!(
        !home
            .0
            .join(".claude/state/pebble/anonymizer/skills.json")
            .exists()
    );
    agree(&home, &["claude:notes", "claude:family-notes"]);
    assert!(
        success(run(&home, &["skills", "list", "member"])).contains("2. claude:notes\tagreed ")
    );
    success(anonymize(&home, "copy"));
    assert!(!home.0.join("copy/family-notes").exists());
    let state = home.0.join(".claude/state/pebble/anonymizer/skills.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
    value["skills"]["claude:notes"]["folder"] = json!("~/old/notes");
    fs::write(&state, value.to_string()).unwrap();
    assert!(success(run(&home, &["skills", "list", "member"])).contains(
        "2. claude:notes\tmoved since you agreed: agree again to copy it\t~/.claude/skills/notes"
    ));
    assert!(success(anonymize(&home, "copy")).contains("2 skills wait for your yes"));
    assert!(!home.0.join("copy/notes").exists());
    assert_eq!(
        success(run(
            &home,
            &["skills", "withdraw", "member", "claude:notes", "unknown"]
        )),
        "claude:notes: no longer copied.\nunknown: no longer copied.\n"
    );
}

#[test]
fn copies_replace_privately_and_keep_placeholders() {
    let home = Scratch::new("anonymize-copy");
    setup(
        &home,
        json!({"terms":[{"term":"Pebble Freight","kind":"company"}]}),
    );
    skill(&home, ".claude/skills/notes", "Work for Pebble Freight.\n");
    fs::write(
        home.0.join(".claude/skills/notes/LOCAL.md"),
        "private overlay",
    )
    .unwrap();
    agree(&home, &["claude:notes"]);
    let report = success(anonymize(&home, "copy"));
    assert!(report.contains("notes: copied, 1 files, 1 left out"));
    assert!(report.contains("  notes/LOCAL.md (left out: private file)"));
    assert!(home.0.join("copy/.anonymized-copy").is_file());
    let text = fs::read_to_string(home.0.join("copy/notes/SKILL.md")).unwrap();
    assert_eq!(text, "Work for <company-1>.\n");
    fs::write(home.0.join("copy/stale"), "old").unwrap();
    success(anonymize(&home, "copy"));
    assert!(!home.0.join("copy/stale").exists());
    assert_eq!(
        fs::read_to_string(home.0.join("copy/notes/SKILL.md")).unwrap(),
        text
    );
    #[cfg(unix)]
    {
        for path in ["copy", "copy/notes", ".claude/state/pebble/anonymizer"] {
            assert_eq!(mode(&home.0.join(path)), 0o700, "{path}");
        }
        for path in [
            "copy/.anonymized-copy",
            "copy/notes/SKILL.md",
            ".claude/state/pebble/anonymizer/skills.json",
            ".claude/state/pebble/anonymizer/placeholders.json",
        ] {
            assert_eq!(mode(&home.0.join(path)), 0o600, "{path}");
        }
    }
}

#[test]
fn refused_skill_reports_locations_without_leaking_and_clean_skill_still_copies() {
    let home = Scratch::new("anonymize-refused");
    setup(
        &home,
        json!({"terms":["Rowan Vale"], "patterns":["pebble-[0-9]+"], "allow":["rowanberry"]}),
    );
    let token = format!("ghp_{}", "a".repeat(36));
    let email = format!("canary@{}.{}", "pebble-mail", "co");
    let phone = "212-555-0199";
    skill(&home, ".claude/skills/clean", "Useful notes\n");
    skill(
        &home,
        ".claude/skills/private",
        &format!("{email}\n{phone}\n{token}\nRowan Vale\npebble-777\nrowanberry\n"),
    );
    agree(&home, &["claude:clean", "claude:private"]);
    let output = anonymize(&home, "copy");
    assert_eq!(output.status.code(), Some(1));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("private: not copied,"));
    assert!(report.contains("  private/SKILL.md:1 (email)"));
    assert!(report.contains("Copied 1 of 2 skills into"));
    for canary in [
        email.as_str(),
        phone,
        &token,
        "Rowan Vale",
        "pebble-777",
        "pebble-[0-9]+",
        "rowanberry",
    ] {
        assert!(!report.contains(canary));
    }
    assert!(home.0.join("copy/clean/SKILL.md").exists());
    assert!(!home.0.join("copy/private").exists());
}

#[test]
fn unsafe_output_paths_are_refused() {
    let home = Scratch::new("anonymize-output");
    setup(&home, json!({}));
    skill(&home, ".claude/skills/notes", "Useful notes\n");
    agree(&home, &["claude:notes"]);
    fs::write(home.0.join("file"), "canary").unwrap();
    fs::create_dir(home.0.join("full")).unwrap();
    fs::write(home.0.join("full/keep"), "canary").unwrap();
    for out in [
        "file",
        "full",
        ".claude/skills/copy",
        ".claude/skills/notes/copy",
    ] {
        assert!(!anonymize(&home, out).status.success(), "{out}");
    }
    #[cfg(unix)]
    {
        symlink(home.0.join("full"), home.0.join("link")).unwrap();
        assert!(!anonymize(&home, "link").status.success());
    }
    assert_eq!(
        fs::read_to_string(home.0.join("full/keep")).unwrap(),
        "canary"
    );
}

#[test]
fn same_name_is_not_overwritten_and_synced_can_be_agreed_by_name() {
    let home = Scratch::new("anonymize-duplicate");
    setup(&home, json!({}));
    skill(&home, ".claude/skills/notes", "First notes\n");
    skill(&home, ".codex/skills/notes", "Second notes\n");
    skill(
        &home,
        ".claude/skills/synced/account/shared",
        "Shared notes\n",
    );
    agree(&home, &["claude:notes", "codex:notes", "synced:shared"]);
    let output = anonymize(&home, "copy");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "codex:notes: not copied, another skill you agreed to has the same name (claude:notes)"
    ));
    assert_eq!(
        fs::read_to_string(home.0.join("copy/notes/SKILL.md")).unwrap(),
        "First notes\n"
    );
    assert!(home.0.join("copy/shared/SKILL.md").is_file());
}

#[test]
fn verifier_03_private_skill_name_report_is_anonymous() {
    let home = Scratch::new("verifier-name");
    setup(&home, json!({"terms":["Rowan Vale"]}));
    skill(&home, ".claude/skills/aaa", "Public");
    skill(&home, ".claude/skills/rowan-vale-helper", "Public");
    agree(&home, &["claude:rowan-vale-helper"]);
    let listed = success(run(&home, &["skills", "list", "member"]));
    let lines = listed.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("1. claude:aaa\t"), "{listed}");
    let (number, details) = lines[1].split_once(". ").expect("numbered skill line");
    assert_eq!(number, "2");
    assert!(details.starts_with("claude:rowan-vale-helper\tagreed "));
    assert!(details.ends_with("\t~/.claude/skills/rowan-vale-helper"));
    let output = anonymize(&home, "copy");
    assert_eq!(output.status.code(), Some(1));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains(&format!("A skill you agreed to: not copied, its name holds a private detail (number {number} in persona-builder skills list)")), "{report}");
    assert!(!report.to_lowercase().contains("rowan"));
    assert!(!report.contains("SKILL.md"));
}

#[test]
fn verifier_06_project_discovery_root_is_protected_even_if_absent() {
    let home = Scratch::new("verifier-project-root");
    setup(&home, json!({"projects":[home.0.join("project")]}));
    assert!(
        !anonymize(&home, "project/.claude/skills/new")
            .status
            .success()
    );
}

#[test]
fn verifier_13_skill_names_collide_ignoring_case() {
    let home = Scratch::new("verifier-name-case");
    setup(&home, json!({}));
    skill(&home, ".claude/skills/Notes", "First");
    skill(&home, ".codex/skills/notes", "Second");
    agree(&home, &["claude:Notes", "codex:notes"]);
    let output = anonymize(&home, "copy");
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains(
        "codex:notes: not copied, another skill you agreed to has the same name (claude:Notes)"
    ));
}

#[test]
fn verifier_15_output_cannot_contain_project() {
    let home = Scratch::new("verifier-project-parent");
    setup(&home, json!({"projects":[home.0.join("copy/project")]}));
    fs::create_dir_all(home.0.join("copy/project")).unwrap();
    fs::write(home.0.join("copy/project/keep"), "public").unwrap();
    fs::write(
        home.0.join("copy/.anonymized-copy"),
        "Made by persona-builder anonymize. Running it again replaces this folder.\n",
    )
    .unwrap();
    assert!(!anonymize(&home, "copy").status.success());
    assert!(home.0.join("copy/project/keep").exists());
}

#[test]
fn verifier_07_mapping_survives_a_failed_file_write() {
    let home = Scratch::new("verifier-write-failure");
    setup(&home, json!({"terms":["Pebble Freight"]}));
    skill(
        &home,
        ".claude/skills/notes",
        &format!("Pebble Freight\n{}", "x".repeat(64 << 10)),
    );
    agree(&home, &["claude:notes"]);
    // Limit only the child: mapping and stamp fit, but the copied skill cannot fit.
    // Ignoring SIGXFSZ lets the writer report its error instead of killing the child.
    let output = Command::new("/bin/sh")
        .env_clear()
        .env("HOME", &home.0)
        .current_dir(&home.0)
        .args([
            "-c",
            "ulimit -f 8; trap '' XFSZ; exec \"$@\"",
            "vo3-test",
            env!("CARGO_BIN_EXE_persona-builder"),
            "anonymize",
            "member",
            "--out",
            "copy",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Couldn't write notes/SKILL.md in the copy. Check there's space, then run anonymize again."), "{}", String::from_utf8_lossy(&output.stderr));
    let bytes = fs::read(
        home.0
            .join(".claude/state/pebble/anonymizer/placeholders.json"),
    )
    .expect("mapping must survive the writer error");
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["kinds"]["term"][0], "pebble freight");
}

#[test]
fn verifier_03_duplicate_ids_and_output_paths_do_not_print_private_components() {
    let home = Scratch::new("verifier-private-id");
    setup(
        &home,
        json!({"terms":["Rowan Vale"], "projects":[home.0.join("rowan-vale")]}),
    );
    skill(&home, ".claude/skills/notes", "First");
    skill(&home, "rowan-vale/.claude/skills/notes", "Second");
    agree(&home, &["claude:notes", "project:rowan-vale:notes"]);
    let output = anonymize(&home, "rowan-vale-copy");
    assert_eq!(output.status.code(), Some(1));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(!report.to_lowercase().contains("rowan"), "{report}");
}

struct RestoreModes(Vec<(std::path::PathBuf, fs::Permissions)>);
impl RestoreModes {
    fn deny(paths: &[std::path::PathBuf]) -> Self {
        let mut guard = Self(vec![]);
        for path in paths {
            guard
                .0
                .push((path.clone(), fs::metadata(path).unwrap().permissions()));
            fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
        }
        guard
    }
}
impl Drop for RestoreModes {
    fn drop(&mut self) {
        for (path, permissions) in self.0.iter().rev() {
            let _ = fs::set_permissions(path, permissions.clone());
        }
    }
}

#[test]
fn unreadable_private_paths_are_hidden_and_other_skills_still_copy() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no preconditions and only reads the effective user id.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("anonymize-unreadable");
    setup(&home, json!({"terms":["Pebble Freight"]}));
    skill(&home, ".claude/skills/aaa", "Public notes\n");
    skill(&home, ".claude/skills/zzz", "Other public notes\n");
    let folder = home.0.join(".claude/skills/aaa");
    fs::write(folder.join("pebble-freight-notes.md"), "private").unwrap();
    fs::create_dir(folder.join("pebble-freight-folder")).unwrap();
    fs::write(folder.join("pebble-freight-folder/inside.md"), "private").unwrap();
    agree(&home, &["claude:aaa", "claude:zzz"]);
    let _restore = RestoreModes::deny(&[
        folder.join("pebble-freight-notes.md"),
        folder.join("pebble-freight-folder"),
    ]);
    let output = anonymize(&home, "copy");
    let report = String::from_utf8_lossy(&output.stdout);
    let errors = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{report}\n{errors}");
    assert!(
        report.contains("aaa: not copied, 2 things to fix"),
        "{report}"
    );
    assert_eq!(report.matches("  aaa/* (file name)").count(), 2);
    assert!(report.contains("Copied 1 of 2 skills into"), "{report}");
    assert!(!report.to_lowercase().contains("pebble-freight"));
    assert!(!errors.to_lowercase().contains("pebble-freight"));
    assert!(home.0.join("copy/zzz/SKILL.md").is_file());
    assert!(!home.0.join("copy/aaa").exists());
}

#[test]
fn unreadable_file_without_private_name_is_left_out_and_skill_still_copies() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no preconditions and only reads the effective user id.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("anonymize-unreadable-public-name");
    setup(&home, json!({"terms":["Pebble Freight"]}));
    skill(&home, ".claude/skills/notes", "Public notes\n");
    let extra = home.0.join(".claude/skills/notes/notes-extra.md");
    fs::write(&extra, "private").unwrap();
    agree(&home, &["claude:notes"]);
    let _restore = RestoreModes::deny(&[extra]);
    let output = anonymize(&home, "copy");
    let report = String::from_utf8_lossy(&output.stdout);
    let errors = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{report}\n{errors}");
    assert!(
        report.contains("notes: copied, 1 files, 1 left out"),
        "{report}"
    );
    assert!(
        report.contains("  notes/notes-extra.md (left out: unreadable)\n"),
        "{report}"
    );
    assert!(home.0.join("copy/notes/SKILL.md").is_file());
    assert!(!home.0.join("copy/notes/notes-extra.md").exists());
}

#[test]
fn unreadable_agreed_skill_folder_is_noted_without_leaking_its_name() {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no preconditions and only reads the effective user id.
    if unsafe { geteuid() } == 0 {
        return;
    }
    let home = Scratch::new("anonymize-unreadable-folder");
    setup(&home, json!({"terms":["Pebble Freight"]}));
    skill(
        &home,
        ".claude/skills/pebble-freight-skill",
        "Public notes\n",
    );
    skill(&home, ".claude/skills/fine", "Useful notes\n");
    agree(&home, &["claude:pebble-freight-skill", "claude:fine"]);
    let _restore = RestoreModes::deny(&[home.0.join(".claude/skills/pebble-freight-skill")]);
    let output = anonymize(&home, "copy");
    let report = String::from_utf8_lossy(&output.stdout);
    let errors = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{report}\n{errors}");
    let note = "A skill folder on this computer couldn't be read, so it isn't listed.";
    assert_eq!(report.matches(note).count(), 1, "{report}");
    assert!(report.find("fine: copied, 1 files").unwrap() < report.find(note).unwrap());
    assert!(report.find(note).unwrap() < report.find("Copied 1 of 1 skills into").unwrap());
    assert!(!report.contains("pebble-freight-skill"));
    assert!(!errors.contains("pebble-freight-skill"));
    assert_eq!(
        fs::read_to_string(home.0.join("copy/fine/SKILL.md")).unwrap(),
        "Useful notes\n"
    );
}
