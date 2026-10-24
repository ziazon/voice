use super::*;
use crate::{
    crew::{NewMember, new_member},
    test_support::{Scratch, repo},
};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

struct Fixture {
    scratch: Scratch,
    member: PathBuf,
    places: Places,
}
impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let member = scratch.path().join("crew/ari");
        make_member(&member, "Ari");
        let home = scratch.path().join("home");
        let claude_home = home.join(".claude");
        fs::create_dir_all(&claude_home).unwrap();
        let home = home.canonicalize().unwrap();
        let claude_home = claude_home.canonicalize().unwrap();
        Self {
            scratch,
            member,
            places: Places { home, claude_home },
        }
    }
    fn path(&self, path: &str) -> PathBuf {
        self.places.claude_home.join(path)
    }
    fn on(&self) -> TurnedOn {
        let result = turn_on(&self.member, &self.places);
        assert!(result.is_ok(), "turn_on: {result:?}");
        result.unwrap()
    }
    fn off(&self) -> TurnedOff {
        turn_off(&self.places).unwrap()
    }
}
fn make_member(path: &Path, name: &str) {
    new_member(
        &repo().join("crew/_template"),
        path,
        &NewMember {
            name,
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/voice",
            date: "2026-10-24",
        },
    )
    .unwrap();
}
fn tree(path: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, (u32, Vec<u8>)>) {
        let meta = fs::symlink_metadata(path).unwrap();
        let bytes = if meta.is_symlink() {
            fs::read_link(path)
                .unwrap()
                .as_os_str()
                .as_encoded_bytes()
                .to_vec()
        } else if meta.is_file() {
            fs::read(path).unwrap()
        } else {
            vec![]
        };
        out.insert(
            path.strip_prefix(root).unwrap().to_owned(),
            (meta.permissions().mode(), bytes),
        );
        if meta.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                walk(root, &entry.unwrap().path(), out);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(path, path, &mut out);
    out
}
const BLOCK: &str = "<!-- persona-builder:begin ari -->\n@~/.claude/persona-builder/ari/RULES.md\n<!-- persona-builder:end ari -->\n";
const RULES: &str = "# Ari\n\nBefore you ask me a question (a choice between options, a go or no-go, a plan's open questions),\nask Ari first with the /ari skill. Ari says what I would likely pick and how sure\nthat answer is. Ask me only what Ari hands back, and show me Ari's answer beside the\nquestion.\n";

#[test]
fn empty_install_has_exact_contents_record_and_private_modes() {
    let f = Fixture::new();
    let on = f.on();
    assert_eq!(on.name, "Ari");
    assert_eq!(on.slug, "ari");
    assert_eq!(on.skill, f.path("skills/ari"));
    assert_eq!(on.claude_md, f.path("CLAUDE.md"));
    assert_eq!(on.import_line, "@~/.claude/persona-builder/ari/RULES.md");
    assert_eq!(
        fs::read(f.path("skills/ari/SKILL.md")).unwrap(),
        fs::read(f.member.join("SKILL.md")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(f.path("persona-builder/ari/RULES.md")).unwrap(),
        RULES
    );
    assert_eq!(fs::read_to_string(f.path("CLAUDE.md")).unwrap(), BLOCK);
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(f.path("persona-builder/installed.json")).unwrap())
            .unwrap();
    assert_eq!(
        record["member_dir"],
        f.member.canonicalize().unwrap().display().to_string()
    );
    assert_eq!(record["claude_md_created"], true);
    let added = record["added"].as_array().unwrap();
    assert_eq!(added.len(), 6);
    for entry in added {
        let path = Path::new(entry["path"].as_str().unwrap());
        let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, if entry["kind"] == "dir" { 0o700 } else { 0o600 });
        if entry["kind"] == "file" {
            assert_eq!(fs::read_to_string(path).unwrap(), entry["text"]);
        }
    }
    for file in ["CLAUDE.md", "persona-builder/installed.json"] {
        assert_eq!(
            fs::metadata(f.path(file)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn on_off_restores_user_bytes_and_other_skills() {
    for original in ["notes", "notes\n", "notes\r\n"] {
        let f = Fixture::new();
        fs::write(f.path("CLAUDE.md"), original).unwrap();
        fs::set_permissions(f.path("CLAUDE.md"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::create_dir_all(f.path("skills/other")).unwrap();
        fs::write(f.path("skills/other/SKILL.md"), "other").unwrap();
        let before = tree(&f.places.claude_home);
        f.on();
        f.off();
        assert_eq!(tree(&f.places.claude_home), before);
    }
}
#[test]
fn removes_created_claude_file_but_keeps_user_additions() {
    for addition in ["", "user line\n"] {
        let f = Fixture::new();
        f.on();
        fs::write(f.path("CLAUDE.md"), format!("{BLOCK}{addition}")).unwrap();
        f.off();
        if addition.is_empty() {
            assert!(!f.path("CLAUDE.md").exists());
        } else {
            assert_eq!(fs::read_to_string(f.path("CLAUDE.md")).unwrap(), addition);
        }
        assert!(!f.path("skills").exists());
        assert!(!f.path("persona-builder").exists());
        assert!(f.off().name.is_none());
    }
}
#[test]
fn second_assistant_is_refused_without_changes() {
    let f = Fixture::new();
    f.on();
    let other = f.scratch.path().join("crew/mo-lee");
    make_member(&other, "Mo Lee");
    let before = tree(&f.places.claude_home);
    assert_eq!(
        turn_on(&other, &f.places).unwrap_err().to_string(),
        "Ari is on. Turn Ari off first: persona-builder off"
    );
    assert_eq!(tree(&f.places.claude_home), before);
}
#[test]
fn unowned_skill_directory_is_refused_without_changes() {
    let f = Fixture::new();
    fs::create_dir_all(f.path("skills/ari")).unwrap();
    let before = tree(&f.places.claude_home);
    let error = turn_on(&f.member, &f.places).unwrap_err().to_string();
    assert_eq!(
        error,
        format!(
            "{} exists and Persona Builder did not add it. Move it away, then turn Ari on again",
            f.path("skills/ari").display()
        )
    );
    assert_eq!(tree(&f.places.claude_home), before);
}
#[test]
fn refuses_symlink_skill_and_record_and_leaves_recorded_symlink() {
    for item in ["skills/ari", "persona-builder/installed.json"] {
        let f = Fixture::new();
        let target = f.scratch.path().join("target");
        fs::write(&target, "untouched").unwrap();
        let path = f.path(item);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&target, &path).unwrap();
        let before = tree(&f.places.claude_home);
        assert_eq!(
            turn_on(&f.member, &f.places).unwrap_err().to_string(),
            format!("{}: symlink refused", path.display())
        );
        assert_eq!(tree(&f.places.claude_home), before);
        assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
    }
    let f = Fixture::new();
    f.on();
    let target = f.scratch.path().join("target");
    fs::write(&target, "untouched").unwrap();
    let skill = f.path("skills/ari/SKILL.md");
    fs::remove_file(&skill).unwrap();
    symlink(&target, &skill).unwrap();
    let off = f.off();
    assert!(
        off.left
            .iter()
            .any(|left| left.path == skill && left.why == "it is a symlink")
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
    assert!(fs::symlink_metadata(skill).unwrap().is_symlink());
}
#[test]
fn symlinked_claude_file_keeps_link_target_bytes_and_mode() {
    let f = Fixture::new();
    let target = f.scratch.path().join("instructions");
    fs::write(&target, "notes").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&target, f.path("CLAUDE.md")).unwrap();
    f.on();
    assert!(
        fs::read_to_string(&target)
            .unwrap()
            .contains("persona-builder:begin")
    );
    assert!(
        fs::symlink_metadata(f.path("CLAUDE.md"))
            .unwrap()
            .is_symlink()
    );
    f.off();
    assert_eq!(fs::read_to_string(&target).unwrap(), "notes");
    assert_eq!(
        fs::metadata(target).unwrap().permissions().mode() & 0o777,
        0o640
    );
}
#[test]
fn changed_skill_is_left_and_reported() {
    let f = Fixture::new();
    f.on();
    let skill = f.path("skills/ari/SKILL.md");
    fs::write(&skill, "user edit").unwrap();
    let off = f.off();
    assert_eq!(off.name.as_deref(), Some("Ari"));
    assert!(
        off.left
            .iter()
            .any(|left| left.path == skill && left.why == "you changed it after it was turned on")
    );
    assert_eq!(fs::read_to_string(skill).unwrap(), "user edit");
    assert!(!f.path("persona-builder/installed.json").exists());
}
#[test]
fn turning_on_twice_refreshes_one_block() {
    let f = Fixture::new();
    f.on();
    fs::write(f.member.join("SKILL.md"), "---\nname: ari\n---\nnew skill").unwrap();
    f.on();
    assert_eq!(fs::read_to_string(f.path("CLAUDE.md")).unwrap(), BLOCK);
    assert_eq!(
        fs::read_to_string(f.path("skills/ari/SKILL.md")).unwrap(),
        "---\nname: ari\n---\nnew skill"
    );
    f.off();
}
#[test]
fn persona_builder_file_is_refused_before_any_write() {
    let f = Fixture::new();
    fs::write(f.path("CLAUDE.md"), "notes").unwrap();
    fs::write(f.path("persona-builder"), "not a folder").unwrap();
    let before = tree(&f.places.claude_home);
    assert!(
        turn_on(&f.member, &f.places)
            .unwrap_err()
            .to_string()
            .contains(&f.path("persona-builder").display().to_string())
    );
    assert_eq!(tree(&f.places.claude_home), before);
}
#[test]
fn edited_record_cannot_remove_outside_file() {
    let f = Fixture::new();
    f.on();
    let outside = f.scratch.path().join("outside");
    fs::write(&outside, "untouched").unwrap();
    let path = f.path("persona-builder/installed.json");
    let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record["added"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"kind":"file", "path":outside, "text":"untouched"}));
    fs::write(&path, serde_json::to_string(&record).unwrap()).unwrap();
    let before = tree(&f.places.claude_home);
    assert!(turn_off(&f.places).is_err());
    assert_eq!(fs::read_to_string(outside).unwrap(), "untouched");
    assert_eq!(tree(&f.places.claude_home), before);
}
#[test]
fn status_tracks_missing_changed_line_and_outdated_member() {
    let f = Fixture::new();
    let initial = status(&f.places);
    assert!(initial.is_ok(), "{initial:?}");
    assert!(initial.unwrap().is_none());
    f.on();
    let current = status(&f.places).unwrap().unwrap();
    assert!(current.line);
    assert!(!current.out_of_date);
    assert_eq!(current.files.len(), 2);
    assert!(
        current
            .files
            .iter()
            .all(|(_, state)| *state == FileState::Present)
    );
    fs::remove_file(f.path("persona-builder/ari/RULES.md")).unwrap();
    fs::write(f.path("skills/ari/SKILL.md"), "edit").unwrap();
    fs::write(f.path("CLAUDE.md"), "notes").unwrap();
    fs::write(f.member.join("SKILL.md"), "---\nname: ari\n---\nupdated").unwrap();
    let current = status(&f.places).unwrap().unwrap();
    assert!(!current.line);
    assert!(current.out_of_date);
    assert_eq!(
        current.files,
        vec![
            (f.path("skills/ari/SKILL.md"), FileState::Changed),
            (f.path("persona-builder/ari/RULES.md"), FileState::Missing)
        ]
    );
    fs::remove_file(f.member.join("SKILL.md")).unwrap();
    assert!(status(&f.places).unwrap().unwrap().out_of_date);
}
#[test]
fn import_paths_escape_spaces_and_use_absolute_path_outside_home() {
    let mut f = Fixture::new();
    let home = f.scratch.path().join("home with space");
    let claude = home.join(".claude with space");
    fs::create_dir_all(&claude).unwrap();
    f.places = Places {
        home: home.canonicalize().unwrap(),
        claude_home: claude.canonicalize().unwrap(),
    };
    assert_eq!(
        f.on().import_line,
        "@~/.claude\\ with\\ space/persona-builder/ari/RULES.md"
    );
    f.off();
    f.places.claude_home = f.scratch.path().join("outside folder");
    fs::create_dir(&f.places.claude_home).unwrap();
    let expected = format!(
        "@{}/persona-builder/ari/RULES.md",
        f.places.claude_home.canonicalize().unwrap().display()
    )
    .replace(' ', "\\ ");
    assert_eq!(f.on().import_line, expected);
}

#[test]
fn refresh_failure_restores_previous_install_including_record() {
    let f = Fixture::new();
    f.on();
    fs::write(f.member.join("SKILL.md"), "---\nname: ari\n---\nupdated").unwrap();
    let temporary = f.path(&format!(
        "persona-builder/.installed.json.tmp-{}",
        std::process::id()
    ));
    fs::write(&temporary, "leave this alone").unwrap();
    let before = tree(&f.places.claude_home);
    assert!(turn_on(&f.member, &f.places).is_err());
    assert_eq!(tree(&f.places.claude_home), before);
}

#[test]
fn late_install_failure_restores_claude_and_keeps_existing_temporary() {
    let f = Fixture::new();
    fs::write(f.path("CLAUDE.md"), "notes").unwrap();
    fs::create_dir(f.path("persona-builder")).unwrap();
    let temporary = f.path(&format!(
        "persona-builder/.installed.json.tmp-{}",
        std::process::id()
    ));
    fs::write(&temporary, "leave this alone").unwrap();
    let before = tree(&f.places.claude_home);
    assert!(turn_on(&f.member, &f.places).is_err());
    assert_eq!(tree(&f.places.claude_home), before);
}

#[test]
fn malformed_block_stops_off_before_any_deletion() {
    let f = Fixture::new();
    f.on();
    fs::write(f.path("CLAUDE.md"), "<!-- persona-builder:begin ari -->\n").unwrap();
    let before = tree(&f.places.claude_home);
    let error = turn_off(&f.places).unwrap_err().to_string();
    assert!(error.contains(&f.path("CLAUDE.md").display().to_string()));
    assert_eq!(tree(&f.places.claude_home), before);
}

#[test]
fn missing_claude_and_installed_files_are_fine_when_turning_off() {
    let f = Fixture::new();
    f.on();
    for path in [
        "CLAUDE.md",
        "skills/ari/SKILL.md",
        "persona-builder/ari/RULES.md",
    ] {
        fs::remove_file(f.path(path)).unwrap();
    }
    let off = f.off();
    assert!(off.left.is_empty());
    assert_eq!(tree(&f.places.claude_home).len(), 1);
}

#[test]
fn rollback_preserves_claude_write_kind_for_an_arbitrarily_named_target() {
    let f = Fixture::new();
    let target = f.scratch.path().join("installed.json");
    fs::write(&target, "notes").unwrap();
    symlink(&target, f.path("CLAUDE.md")).unwrap();
    fs::write(
        f.scratch
            .path()
            .join(format!(".installed.json.tmp-{}", std::process::id())),
        "user file",
    )
    .unwrap();
    fs::create_dir(f.path("persona-builder")).unwrap();
    fs::write(
        f.path(&format!(
            "persona-builder/.installed.json.tmp-{}",
            std::process::id()
        )),
        "user file",
    )
    .unwrap();
    let before = tree(f.scratch.path());
    assert!(turn_on(&f.member, &f.places).is_err());
    assert!(
        tree(f.scratch.path()) == before,
        "rollback changed the symlink target or its folder"
    );
}

#[test]
fn failed_refresh_restores_deleted_files_despite_unrelated_temporary_names() {
    let f = Fixture::new();
    f.on();
    fs::write(
        f.path(&format!(
            "skills/ari/.CLAUDE.md.persona-builder-{}",
            std::process::id()
        )),
        "user file",
    )
    .unwrap();
    let before = tree(&f.places.claude_home);
    assert!(turn_on(&f.member, &f.places).is_err());
    assert!(
        tree(&f.places.claude_home) == before,
        "rollback lost an installed file"
    );
}

#[test]
fn refresh_checks_claude_restore_temporary_before_removing_original() {
    let f = Fixture::new();
    f.on();
    fs::write(
        f.path(&format!(
            ".CLAUDE.md.persona-builder-{}",
            std::process::id()
        )),
        "user file",
    )
    .unwrap();
    let before = tree(&f.places.claude_home);
    assert!(turn_on(&f.member, &f.places).is_err());
    assert!(
        tree(&f.places.claude_home) == before,
        "rollback lost CLAUDE.md"
    );
}

#[test]
fn unsafe_names_are_refused_before_writing() {
    for what in ["name", "principal"] {
        for value in [
            "Ari\n@~/.ssh/x",
            "@x",
            "a`b",
            "{{SLUG}}",
            "a\tb",
            &"a".repeat(65),
            "",
            " Ari",
            "Ari ",
            "a<b",
            "a>b",
            "a}b",
            "a[b",
            "a]b",
            "a(b",
            "a)b",
            "a#b",
            "a\\b",
            "a\u{85}b",
        ] {
            let f = Fixture::new();
            let member = &f.member;
            let before = tree(&f.places.claude_home);
            let path = member.join("member.json");
            let mut config: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            config[what] = value.into();
            fs::write(path, serde_json::to_string(&config).unwrap()).unwrap();
            let result = turn_on(member, &f.places);
            assert!(
                matches!(&result, Err(Error::Invalid(_))),
                "{what} {value:?} was not refused: {result:?}"
            );
            assert_eq!(
                result.unwrap_err().to_string(),
                format!(
                    "the {what} {value:?} can't be used: a name is one line of up to 64 characters without @ < > {{ }} [ ] ( ) # \\ or backticks"
                )
            );
            assert_eq!(tree(&f.places.claude_home), before);
        }
    }
}

#[test]
fn ordinary_names_are_accepted() {
    for what in ["name", "principal"] {
        for value in ["Mo Lee", "Zoë", "Sam O'Neil", &"ë".repeat(64)] {
            let f = Fixture::new();
            let member = &f.member;

            let path = member.join("member.json");
            let mut config: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            config[what] = value.into();
            fs::write(path, serde_json::to_string(&config).unwrap()).unwrap();
            let result = turn_on(member, &f.places);
            assert!(result.is_ok(), "{what} {value:?}: {result:?}");
        }
    }
}

fn refused_skill(text: &str, message: &str) {
    let f = Fixture::new();
    let path = f.member.join("SKILL.md");
    fs::write(&path, text).unwrap();
    let before = tree(&f.places.claude_home);
    let result = turn_on(&f.member, &f.places);
    assert!(
        matches!(&result, Err(Error::Invalid(_))),
        "skill was not refused: {result:?}"
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        format!("{} {message}", path.canonicalize().unwrap().display())
    );
    assert_eq!(tree(&f.places.claude_home), before);
}

#[test]
fn skill_without_front_matter_is_refused() {
    for text in ["# Ari\n", "---\nname: ari\n"] {
        refused_skill(text, "has no front matter (the lines between ---)");
    }
}

#[test]
fn skill_without_matching_name_is_refused() {
    for text in [
        "---\nname: other\n---\n",
        "---\ndescription: Ari\n---\nname: ari\n",
        "---\nname: ari \n---\n",
    ] {
        refused_skill(text, "must be named ari (a line name: ari)");
    }
}

#[test]
fn skill_with_hooks_is_refused() {
    refused_skill(
        "---\nname: ari\nhooks: {}\n---\n",
        "asks for hooks, which can run commands without asking. Persona Builder turns on only a skill without them",
    );
}

#[test]
fn skill_with_allowed_tools_is_refused() {
    refused_skill(
        "---\nname: ari\nallowed-tools: Bash\n---\n",
        "asks for allowed-tools, which can run commands without asking. Persona Builder turns on only a skill without them",
    );
}

#[test]
fn skill_with_shell_command_is_refused() {
    for text in [
        "---\nname: ari\n---\n!`echo example`\n",
        "---\nname: ari\ndescription: !`echo example`\n---\n",
    ] {
        refused_skill(
            text,
            "runs a shell command (!`). Persona Builder turns on only a skill without one",
        );
    }
}

#[test]
fn symlinked_skills_outside_claude_home_supports_on_and_off() {
    let f = Fixture::new();
    let target = f.scratch.path().join("shared-skills");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("other.md"), "keep").unwrap();
    symlink(&target, f.path("skills")).unwrap();
    let before = tree(&f.places.claude_home);
    let target_before = tree(&target);
    f.on();
    assert_eq!(
        fs::read(target.join("ari/SKILL.md")).unwrap(),
        fs::read(f.member.join("SKILL.md")).unwrap()
    );
    let off = turn_off(&f.places);
    assert!(off.is_ok(), "turn_off: {off:?}");
    assert!(off.unwrap().left.is_empty());
    assert_eq!(tree(&f.places.claude_home), before);
    assert_eq!(tree(&target), target_before);
    assert!(fs::symlink_metadata(f.path("skills")).unwrap().is_symlink());
}

#[test]
fn refresh_refuses_user_changes_and_restores_the_install() {
    for changed in ["skills/ari/SKILL.md", "persona-builder/ari/RULES.md"] {
        let f = Fixture::new();
        f.on();
        let path = f.path(changed);
        fs::write(&path, "user edit").unwrap();
        let before = tree(&f.places.claude_home);
        let result = turn_on(&f.member, &f.places);
        assert!(
            matches!(&result, Err(Error::Invalid(_))),
            "refresh was not refused: {result:?}"
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            format!(
                "{} changed after Ari was turned on. Move it away, then turn Ari on again",
                path.display()
            )
        );
        assert_eq!(tree(&f.places.claude_home), before);
    }
}

#[test]
fn record_cannot_claim_claude_file_or_itself() {
    for claimed in ["CLAUDE.md", "persona-builder/installed.json"] {
        for kind in ["file", "dir"] {
            let f = Fixture::new();
            f.on();
            let record_path = f.path("persona-builder/installed.json");
            let mut record: serde_json::Value =
                serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();
            let path = f.path(claimed);
            let text = fs::read_to_string(&path).unwrap();
            record["added"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"kind": kind, "path": path, "text": text}));
            fs::write(&record_path, serde_json::to_string(&record).unwrap()).unwrap();
            let before = tree(&f.places.claude_home);
            let result = turn_off(&f.places);
            assert!(
                matches!(&result, Err(Error::Invalid(_))),
                "record claiming {claimed} as {kind} was not refused: {result:?}"
            );
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains(&record_path.display().to_string())
            );
            assert_eq!(tree(&f.places.claude_home), before);
        }
    }
}
