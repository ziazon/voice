#[path = "../../core/tests/common/mod.rs"]
mod common;
use common::Scratch;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
};
use voice_ingest::{
    anonymize::Rules,
    skills::{self, Agreements, Source, State},
};
fn skill(home: &Path, relative: &str) {
    let p = home.join(relative);
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("SKILL.md"), "A useful skill.\n").unwrap();
}
#[test]
fn finds_each_root_and_deduplicates_synced_names() {
    let s = Scratch::new("skill-roots");
    let h = &s.0;
    for p in [
        ".claude/skills/notes",
        ".claude/skills/synced/b/shared",
        ".claude/skills/synced/a/shared",
        ".claude/skills/synced/b/another",
        ".claude/skills/synced/a/another",
        ".codex/skills/coding",
        "project/.claude/skills/planning",
        ".claude/plugins/cache/tool/skills/tools",
    ] {
        skill(h, p);
    }
    fs::write(h.join(".claude/plugins/installed_plugins.json"), serde_json::to_vec(&serde_json::json!({"plugins":{"tool@market":[{"installPath":h.join(".claude/plugins/cache/tool")}]}})).unwrap()).unwrap();
    let rules = Rules::from_value(
        Some(&serde_json::json!({"projects":[h.join("project")]})),
        h,
    )
    .unwrap();
    let found = skills::discover(h, &rules).unwrap();
    assert_eq!(
        found
            .skills
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "claude:notes",
            "codex:coding",
            "plugin:tool:tools",
            "project:project:planning",
            "synced:another",
            "synced:shared"
        ]
    );
    assert_eq!(
        found.notes,
        vec![
            "Some claude.ai skills are in more than one synced folder; each is listed once, from the first folder by name."
        ]
    );
    assert!(
        found
            .skills
            .last()
            .unwrap()
            .folder
            .ends_with("synced/a/shared")
    );
    assert!(
        found
            .skills
            .iter()
            .find(|s| s.id == "synced:another")
            .unwrap()
            .folder
            .ends_with("synced/a/another")
    );
    assert!(found.skills[0].shown.starts_with("~/"));
}
#[test]
fn links_and_hidden_and_denied_roots_are_safe() {
    let s = Scratch::new("skill-links");
    let h = &s.0;
    skill(h, "outside");
    skill(h, ".ssh/denied");
    skill(h, ".codex/skills/.hidden");
    fs::create_dir_all(h.join(".claude/skills")).unwrap();
    symlink(h.join("outside"), h.join(".claude/skills/linked")).unwrap();
    symlink(h.join(".ssh/denied"), h.join(".claude/skills/denied")).unwrap();
    fs::write(h.join("canary"), "outside canary must not be read").unwrap();
    symlink(h.join("canary"), h.join("outside/link")).unwrap();
    skill(h, "outside/.git");
    let found = skills::discover(h, &Rules::builtin()).unwrap();
    assert_eq!(found.skills.len(), 1);
    let files = skills::read(&found.skills[0]).unwrap();
    assert_eq!(files.len(), 2);
    assert!(matches!(files[1].1, Source::Link));
}
#[test]
fn plugin_escape_and_malformed_list_are_not_read() {
    let s = Scratch::new("skill-plugin");
    let h = &s.0;
    skill(h, "external/skills/escaped");
    fs::create_dir_all(h.join(".claude/plugins")).unwrap();
    let p = h.join(".claude/plugins/installed_plugins.json");
    fs::write(&p,serde_json::to_vec(&serde_json::json!({"plugins":{"tool@market":[{"installPath":h.join("external")},{"installPath":3}]}})).unwrap()).unwrap();
    assert!(
        skills::discover(h, &Rules::builtin())
            .unwrap()
            .skills
            .is_empty()
    );
    fs::write(p, b"not json").unwrap();
    assert_eq!(
        skills::discover(h, &Rules::builtin()).unwrap().notes,
        vec!["Couldn't read Claude Code's list of plugins, so their skills aren't listed."]
    );
}
#[test]
fn bounded_reader_refuses_count_size_and_depth() {
    for case in ["count", "bytes", "depth"] {
        let s = Scratch::new("skill-limit");
        let h = &s.0;
        skill(h, ".codex/skills/limited");
        let folder = h.join(".codex/skills/limited");
        match case {
            "count" => {
                for n in 0..500 {
                    fs::write(folder.join(format!("f{n}")), b"").unwrap();
                }
            }
            "bytes" => {
                for n in 0..20 {
                    fs::write(folder.join(format!("f{n}")), vec![b'a'; 1 << 20]).unwrap();
                }
            }
            _ => {
                let p = folder.join("a/b/c/d/e/f/g/h/i");
                fs::create_dir_all(&p).unwrap();
                fs::write(p.join("file"), b"").unwrap();
            }
        }
        let found = skills::discover(h, &Rules::builtin()).unwrap();
        assert!(skills::read(&found.skills[0]).is_err(), "{case}");
    }
}
#[test]
fn agreement_states_and_private_roundtrip() {
    let s = Scratch::new("skills-agree");
    let h = &s.0;
    skill(h, ".claude/skills/notes");
    let found = skills::discover(h, &Rules::builtin()).unwrap();
    let skill = &found.skills[0];
    let mut records = Agreements::load(h, "pebble").unwrap();
    assert!(matches!(
        records.state(skill, &Rules::builtin()),
        State::Waiting
    ));
    records.agree(skill, "2026-10-31");
    records.save(h, "pebble").unwrap();
    let records = Agreements::load(h, "pebble").unwrap();
    assert_eq!(
        records.state(skill, &Rules::builtin()).words(),
        "agreed 2026-10-31"
    );
    let p = h.join(".claude/state/pebble/anonymizer/skills.json");
    assert_eq!(fs::metadata(p).unwrap().permissions().mode() & 0o777, 0o600);
}
#[test]
fn output_replaces_only_stamped_folders_and_rejects_read_roots() {
    let s = Scratch::new("skills-output");
    let h = &s.0;
    skill(h, ".claude/skills/notes");
    let found = skills::discover(h, &Rules::builtin()).unwrap();
    let out = h.join("copy");
    let p = skills::prepare_output(&out, h, &found.skills).unwrap();
    skills::write_skill(&p, "notes", &[("SKILL.md".into(), "public text".into())]).unwrap();
    assert_eq!(
        fs::metadata(&p).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(p.join("notes/SKILL.md"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    skills::prepare_output(&out, h, &found.skills).unwrap();
    assert!(!out.join("notes").exists());
    fs::create_dir(h.join("unknown")).unwrap();
    fs::write(h.join("unknown/file"), b"keep").unwrap();
    assert!(skills::prepare_output(&h.join("unknown"), h, &found.skills).is_err());
    assert!(skills::prepare_output(&h.join(".claude/skills/new"), h, &found.skills).is_err());
    assert!(skills::prepare_output(&found.skills[0].folder.join("new"), h, &found.skills).is_err());
    assert!(skills::write_skill(&out, "../escape", &[]).is_err());
    assert!(skills::write_skill(&out, "notes", &[("../escape".into(), String::new())]).is_err());
}

#[test]
fn private_hidden_denied_and_drop_files_are_left_unread() {
    let scratch = Scratch::new("skills-unread");
    let home = &scratch.0;
    skill(home, ".codex/skills/notes");
    let folder = home.join(".codex/skills/notes");
    for name in [
        "LOCAL.md",
        "LEARNINGS.md",
        ".hidden",
        "credentials.pem",
        "NOTES-private.md",
    ] {
        fs::write(folder.join(name), "unread private canary").unwrap();
    }
    let found = skills::discover(home, &Rules::builtin()).unwrap();
    let files = skills::read(&found.skills[0]).unwrap();
    for (path, source) in &files {
        if ["LOCAL.md", "LEARNINGS.md", ".hidden", "credentials.pem"]
            .contains(&path.to_str().unwrap())
        {
            assert!(
                matches!(source, Source::Bytes(bytes) if bytes.is_empty()),
                "{}",
                path.display()
            );
        }
    }
    // The caller's private drop-file rules are also applied before opening a file.
    let rules = Rules::from_value(
        Some(&serde_json::json!({"drop_files":["NOTES-*.md"]})),
        home,
    )
    .unwrap();
    let files = skills::read_with_rules(&found.skills[0], &rules).unwrap();
    assert!(
        files
            .iter()
            .any(|(path, source)| path == Path::new("NOTES-private.md")
                && matches!(source, Source::Bytes(bytes) if bytes.is_empty()))
    );
}

#[test]
fn malformed_agreements_never_print_their_contents() {
    let scratch = Scratch::new("skills-invalid-agreements");
    for value in [
        serde_json::json!({"version":2,"skills":{}}),
        serde_json::json!({"version":1,"skills":{"private-canary": {"agreed_on":"private-canary","folder":"private-canary"}}}),
        serde_json::json!({"version":1,"skills":{},"private-canary":true}),
    ] {
        voice_ingest::private::write_private_at(
            &scratch.0,
            "pebble",
            Some("anonymizer"),
            "skills.json",
            &serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        let message = Agreements::load(&scratch.0, "pebble")
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            "The anonymizer's skills.json file can't be read. Move it away and agree to your skills again."
        );
        assert!(!message.contains("private-canary"));
    }
}

#[test]
fn output_rejects_links_and_files_but_accepts_empty_folders() {
    let scratch = Scratch::new("skills-output-types");
    let h = &scratch.0;
    let empty = h.join("empty");
    fs::create_dir(&empty).unwrap();
    skills::prepare_output(&empty, h, &[]).unwrap();
    fs::write(h.join("file"), b"keep").unwrap();
    symlink(&empty, h.join("link")).unwrap();
    assert!(skills::prepare_output(&h.join("file"), h, &[]).is_err());
    assert!(skills::prepare_output(&h.join("link"), h, &[]).is_err());
    assert_eq!(fs::read(h.join("file")).unwrap(), b"keep");
}

#[test]
fn a_discovered_root_replaced_by_a_link_is_never_read() {
    let scratch = Scratch::new("skills-root-swap");
    let home = &scratch.0;
    skill(home, ".codex/skills/notes");
    skill(home, "outside");
    fs::write(home.join("outside/SKILL.md"), "outside private canary").unwrap();
    let found = skills::discover(home, &Rules::builtin()).unwrap();
    let skill = &found.skills[0];
    fs::rename(&skill.folder, home.join("original")).unwrap();
    symlink(home.join("outside"), &skill.folder).unwrap();
    assert!(skills::read(skill).is_err());
}

#[test]
fn reader_accepts_exactly_500_files_and_eight_folders() {
    let scratch = Scratch::new("skills-boundaries");
    let home = &scratch.0;
    skill(home, ".codex/skills/notes");
    let folder = home.join(".codex/skills/notes");
    for n in 0..498 {
        fs::write(folder.join(format!("f{n}")), b"").unwrap();
    }
    fs::create_dir_all(folder.join("a/b/c/d/e/f/g/h")).unwrap();
    fs::write(folder.join("a/b/c/d/e/f/g/h/leaf"), b"public").unwrap();
    let found = skills::discover(home, &Rules::builtin()).unwrap();
    assert_eq!(skills::read(&found.skills[0]).unwrap().len(), 500);
}

#[test]
fn nested_linked_folder_is_a_link_source_and_is_not_walked() {
    let scratch = Scratch::new("skills-nested-link");
    let home = &scratch.0;
    skill(home, ".codex/skills/notes");
    skill(home, "outside");
    fs::create_dir_all(home.join(".codex/skills/notes/references")).unwrap();
    symlink(
        home.join("outside"),
        home.join(".codex/skills/notes/references/outside"),
    )
    .unwrap();
    let found = skills::discover(home, &Rules::builtin()).unwrap();
    let files = skills::read(&found.skills[0]).unwrap();
    assert_eq!(files.len(), 2);
    assert!(
        files
            .iter()
            .any(|(path, source)| path == Path::new("references/outside")
                && matches!(source, Source::Link))
    );
    let result = voice_ingest::anonymize::skill(
        &files,
        &Rules::builtin(),
        &mut voice_ingest::anonymize::Mapping::default(),
    );
    assert!(
        result
            .left_out
            .iter()
            .any(|(path, reason)| path == Path::new("references/outside")
                && matches!(reason, voice_ingest::anonymize::LeftOut::Link))
    );
}

#[test]
fn oversized_file_is_left_out_without_counting_unread_bytes() {
    let scratch = Scratch::new("skills-oversized");
    let home = &scratch.0;
    skill(home, ".codex/skills/notes");
    let folder = home.join(".codex/skills/notes");
    fs::File::create(folder.join("large.md"))
        .unwrap()
        .set_len(32 << 20)
        .unwrap();
    let found = skills::discover(home, &Rules::builtin()).unwrap();
    let files = skills::read(&found.skills[0]).unwrap();
    assert!(
        files
            .iter()
            .any(|(path, source)| path == Path::new("large.md")
                && matches!(source, Source::TooLarge))
    );
    let result = voice_ingest::anonymize::skill(&files, &Rules::builtin(), &mut Default::default());
    assert!(result.refused.is_empty());
    assert_eq!(
        result.left_out,
        vec![(
            "large.md".into(),
            voice_ingest::anonymize::LeftOut::TooLarge
        )]
    );
}

#[test]
fn verifier_06_all_discovery_roots_protect_future_output() {
    let scratch = Scratch::new("verifier-roots");
    for root in [".claude/skills", ".codex/skills", ".claude/plugins"] {
        for tail in ["", "/new"] {
            assert!(
                skills::prepare_output(&scratch.0.join(format!("{root}{tail}")), &scratch.0, &[])
                    .is_err(),
                "{root}{tail}"
            );
        }
    }
}

#[test]
fn verifier_07_writer_names_failed_relative_path() {
    let scratch = Scratch::new("verifier-writer");
    let out = skills::prepare_output(&scratch.0.join("copy"), &scratch.0, &[]).unwrap();
    let files = vec![("SKILL.md".into(), "public".into())];
    skills::write_skill(&out, "notes", &files).unwrap();
    let error = skills::write_skill(&out, "notes", &files).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Couldn't write notes/SKILL.md in the copy. Check there's space, then run anonymize again."
    );
}

#[test]
fn verifier_12_plugin_skill_cannot_escape_its_install() {
    let scratch = Scratch::new("verifier-plugin");
    let h = &scratch.0;
    skill(h, ".codex/skills/notes");
    skill(h, ".claude/plugins/cache/tool/skills/inside");
    symlink(
        h.join(".codex/skills/notes"),
        h.join(".claude/plugins/cache/tool/skills/outside"),
    )
    .unwrap();
    fs::write(h.join(".claude/plugins/installed_plugins.json"), serde_json::json!({"plugins":{"tool@market":[{"installPath":h.join(".claude/plugins/cache/tool")}]}}).to_string()).unwrap();
    let found = skills::discover(h, &Rules::builtin()).unwrap();
    assert!(found.skills.iter().any(|s| s.id == "codex:notes"));
    assert!(found.skills.iter().any(|s| s.id == "plugin:tool:inside"));
    assert!(!found.skills.iter().any(|s| s.id == "plugin:tool:outside"));
}

#[test]
fn verifier_13_odd_file_name_is_left_out() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let scratch = Scratch::new("verifier-odd");
    skill(&scratch.0, ".codex/skills/notes");
    let odd = OsString::from_vec(b"odd-\xff".to_vec());
    let written = fs::write(scratch.0.join(".codex/skills/notes").join(&odd), "public");
    // macOS filesystems can reject non-UTF-8 names; still exercise the public transform.
    if let Err(error) = &written {
        #[cfg(target_os = "macos")]
        let _ = error;
        #[cfg(not(target_os = "macos"))]
        panic!("could not create odd-name fixture: {error}");
    }
    let found = skills::discover(&scratch.0, &Rules::builtin()).unwrap();
    let mut files = skills::read(&found.skills[0]).unwrap();
    if written.is_err() {
        files.push((odd.into(), Source::Bytes(b"public".to_vec())));
    }
    let result = voice_ingest::anonymize::skill(&files, &Rules::builtin(), &mut Default::default());
    assert!(result.refused.is_empty());
    assert_eq!(result.kept.len(), 1);
    assert!(
        result
            .left_out
            .iter()
            .any(|(_, reason)| { voice_ingest::anonymize::left_out_reason(reason) == "odd name" })
    );
}

#[test]
fn verifier_15_output_cannot_contain_discovered_skill() {
    let scratch = Scratch::new("verifier-parent");
    skill(&scratch.0, "copy/notes");
    let out = scratch.0.join("copy");
    fs::write(
        out.join(".anonymized-copy"),
        "Made by voice anonymize. Running it again replaces this folder.\n",
    )
    .unwrap();
    let folder = out.join("notes").canonicalize().unwrap();
    let found = vec![skills::Skill {
        id: "codex:notes".into(),
        name: "notes".into(),
        root: skills::Root::Codex,
        shown: folder.display().to_string(),
        folder,
    }];
    assert!(skills::prepare_output(&out, &scratch.0, &found).is_err());
    assert!(out.join("notes/SKILL.md").exists());
}

#[test]
fn verifier_06_future_roots_are_protected_in_another_case() {
    let scratch = Scratch::new("verifier-root-case");
    for root in [
        ".CLAUDE/SKILLS/new",
        ".CODEX/SKILLS/new",
        ".CLAUDE/PLUGINS/new",
    ] {
        assert!(
            skills::prepare_output(&scratch.0.join(root), &scratch.0, &[]).is_err(),
            "{root}"
        );
    }
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
fn unreadable_entries_are_left_out_and_reading_continues() {
    // SAFETY: geteuid has no preconditions and only reads the effective user id.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let scratch = Scratch::new("skills-unreadable");
    skill(&scratch.0, ".codex/skills/notes");
    let folder = scratch.0.join(".codex/skills/notes");
    fs::write(folder.join("pebble-freight-notes.md"), "private").unwrap();
    fs::create_dir(folder.join("pebble-freight-folder")).unwrap();
    fs::write(folder.join("pebble-freight-folder/inside.md"), "private").unwrap();
    let found = skills::discover(&scratch.0, &Rules::builtin()).unwrap();
    let _restore = RestoreModes::deny(&[
        folder.join("pebble-freight-notes.md"),
        folder.join("pebble-freight-folder"),
    ]);
    let files = skills::read(&found.skills[0]);
    assert!(
        files.is_ok(),
        "unreadable entries must not stop the walk: {files:?}"
    );
    let files = files.unwrap();
    assert_eq!(files.len(), 3);
    for name in ["pebble-freight-notes.md", "pebble-freight-folder"] {
        let (_, source) = files
            .iter()
            .find(|(path, _)| path == Path::new(name))
            .unwrap();
        assert_eq!(format!("{source:?}"), "Unreadable");
    }
}

#[test]
fn unreadable_skill_root_reports_no_path() {
    let scratch = Scratch::new("skills-unreadable-root");
    skill(&scratch.0, ".codex/skills/pebble-freight");
    let found = skills::discover(&scratch.0, &Rules::builtin()).unwrap();
    fs::remove_dir_all(&found.skills[0].folder).unwrap();
    assert_eq!(
        skills::read(&found.skills[0]).unwrap_err().to_string(),
        "a folder in it couldn't be read"
    );
}

#[test]
fn unreadable_skill_folders_are_noted_once_without_names() {
    // SAFETY: geteuid has no preconditions and only reads the effective user id.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let scratch = Scratch::new("skills-unreadable-discovery");
    skill(&scratch.0, ".claude/skills/pebble-freight-skill");
    skill(&scratch.0, ".codex/skills/other-unreadable");
    skill(&scratch.0, ".codex/skills/fine");
    let _restore = RestoreModes::deny(&[
        scratch.0.join(".claude/skills/pebble-freight-skill"),
        scratch.0.join(".codex/skills/other-unreadable"),
    ]);
    let found = skills::discover(&scratch.0, &Rules::builtin()).unwrap();
    assert_eq!(
        found.notes,
        vec!["A skill folder on this computer couldn't be read, so it isn't listed."]
    );
    assert_eq!(found.skills.len(), 1);
    assert_eq!(found.skills[0].name, "fine");
}

#[test]
fn hard_linked_skill_files_are_left_out_of_read_and_copy() {
    let source = Scratch::new("skill-source");
    let outside = Scratch::new("skill-outside");
    skill(&source.0, ".claude/skills/notes");
    let canary = outside.0.join("canary.md");
    fs::write(&canary, "canary-outside").unwrap();
    fs::hard_link(canary, source.0.join(".claude/skills/notes/notes.md")).unwrap();
    let rules = Rules::builtin();
    let found = skills::discover(&source.0, &rules).unwrap();
    assert_eq!(found.skills.len(), 1);
    let files = skills::read(&found.skills[0]).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].0, Path::new("SKILL.md"));
    assert!(matches!(&files[0].1, Source::Bytes(bytes) if bytes == b"A useful skill.\n"));
    assert_eq!(files[1].0, Path::new("notes.md"));
    assert!(matches!(files[1].1, Source::HardLink));
    let result = voice_ingest::anonymize::skill(&files, &rules, &mut Default::default());
    assert!(result.refused.is_empty());
    assert_eq!(result.left_out.len(), 1);
    assert_eq!(result.left_out[0].0, Path::new("notes.md"));
    assert_eq!(
        voice_ingest::anonymize::left_out_reason(&result.left_out[0].1),
        "a hard link"
    );
    assert!(
        voice_ingest::anonymize::report("notes", &result)
            .contains("notes/notes.md (left out: a hard link)")
    );
    assert_eq!(result.kept.len(), 1);
    assert!(
        result
            .kept
            .iter()
            .all(|(_, text)| !text.contains("canary-outside"))
    );
    let copied = Scratch::new("skill-copy");
    skills::write_skill(&copied.0, "notes", &result.kept).unwrap();
    assert!(!copied.0.join("notes/notes.md").exists());
    let files = fs::read_dir(copied.0.join("notes"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        !fs::read_to_string(files[0].path())
            .unwrap()
            .contains("canary-outside")
    );
}
