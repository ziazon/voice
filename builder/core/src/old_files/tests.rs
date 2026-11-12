use super::*;
use crate::{crew, member, test_support::Scratch};
use std::{
    ffi::CString,
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

const LINK: &str = "It's a link, so Voice leaves it.";
const FILE: &str = "It isn't a plain file, so Voice leaves it.";
const FOLDER: &str = "It isn't a plain folder of files, so Voice leaves it.";
const BIG: &str = "It's too big for Voice to read, so Voice leaves it.";
const TEXT: &str = "It isn't plain text, so Voice leaves it.";
const AUTONOMY: &str = "Voice can't read its list of who decides what, so Voice leaves it.";
const PRIVATE: &str = "It changes more than the privacy settings, so Voice leaves it.";

fn snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, (String, Vec<u8>)> {
    fn walk(
        root: &Path,
        at: &Path,
        out: &mut std::collections::BTreeMap<PathBuf, (String, Vec<u8>)>,
    ) {
        let meta = fs::symlink_metadata(at).unwrap();
        let (kind, bytes) = if meta.is_symlink() {
            (
                "link",
                fs::read_link(at)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec(),
            )
        } else if meta.is_dir() {
            ("folder", vec![])
        } else if meta.is_file() {
            ("file", fs::read(at).unwrap())
        } else {
            ("special", vec![])
        };
        out.insert(
            at.strip_prefix(root).unwrap().to_owned(),
            (kind.into(), bytes),
        );
        if meta.is_dir() {
            for entry in fs::read_dir(at).unwrap() {
                walk(root, &entry.unwrap().path(), out);
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn all_files(folder: &Path) {
    fs::create_dir_all(folder.join("briefs")).unwrap();
    for name in NAMES {
        let text = match name {
            "briefs" => continue,
            "autonomy.md" => "- [x] `scope`: ask Robin\n",
            "private.json" => r#"{"filters":{"personal":"fictional private topic"}}"#,
            _ => "older assistant's invented text\n",
        };
        fs::write(folder.join(name), text).unwrap();
    }
    fs::write(folder.join("briefs/older.md"), "invented older brief\n").unwrap();
}
fn template(folder: &Path) {
    crew::new_member_embedded(
        folder,
        &crew::NewMember {
            name: "Ari",
            pronouns: "they/them",
            principal: "Robin",
            principal_pronouns: "they/them",
            repo_shown: "~/kit",
            date: "2026-11-12",
            decisions: crew::Decisions::Cli("voice"),
        },
    )
    .unwrap();
}
fn problem(name: &str, prepare: impl FnOnce(&Path), expected: &str) {
    let scratch = Scratch::new();
    prepare(scratch.path());
    let offered = offer(scratch.path(), &|_| true).unwrap();
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].name, name);
    assert_eq!(offered[0].problem, Some(expected));
}
fn plain_problem(name: &str, bytes: &[u8], expected: &str) {
    problem(
        name,
        |folder| fs::write(folder.join(name), bytes).unwrap(),
        expected,
    );
}

#[test]
fn offers_all_eight_in_order_without_unlisted_files() {
    let scratch = Scratch::new();
    all_files(scratch.path());
    fs::write(scratch.path().join("SKILL.md"), "not offered").unwrap();
    fs::write(scratch.path().join("member.json"), "not offered").unwrap();
    let offered = offer(scratch.path(), &|_| true).unwrap();
    assert_eq!(
        offered.iter().map(|file| file.name).collect::<Vec<_>>(),
        NAMES
    );
    assert!(offered.iter().all(|file| file.problem.is_none()));
}
#[test]
fn omits_absent_names_and_missing_old_folder() {
    let scratch = Scratch::new();
    fs::write(scratch.path().join("lens.md"), "lens").unwrap();
    assert_eq!(
        offer(scratch.path(), &|_| true)
            .unwrap()
            .iter()
            .map(|file| file.name)
            .collect::<Vec<_>>(),
        ["lens.md"]
    );
    assert!(
        offer(&scratch.path().join("missing"), &|_| true)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn leaves_links() {
    problem(
        "IDENTITY.md",
        |p| symlink("missing", p.join("IDENTITY.md")).unwrap(),
        LINK,
    );
}
#[test]
fn leaves_non_file() {
    problem(
        "IDENTITY.md",
        |p| fs::create_dir(p.join("IDENTITY.md")).unwrap(),
        FILE,
    );
}
#[test]
fn leaves_fifo_without_blocking() {
    problem(
        "lens.md",
        |p| {
            let name = CString::new(p.join("lens.md").as_os_str().as_encoded_bytes()).unwrap();
            // SAFETY: name is NUL-terminated and valid for the duration of mkfifo.
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        },
        FILE,
    );
}
#[test]
fn leaves_oversized_file() {
    plain_problem("IDENTITY.md", &vec![b'a'; 1024 * 1024 + 1], BIG);
}
#[test]
fn leaves_oversized_decisions() {
    plain_problem("decisions.md", &vec![b'a'; 256 * 1024 + 1], BIG);
}
#[test]
fn leaves_oversized_codebook() {
    problem(
        "briefs",
        |p| {
            fs::create_dir(p.join("briefs")).unwrap();
            fs::write(p.join("briefs/codebook.md"), vec![b'a'; 64 * 1024 + 1]).unwrap();
        },
        BIG,
    );
}
#[test]
fn leaves_nul() {
    plain_problem("lens.md", b"plain\0text", TEXT);
}
#[test]
fn leaves_invalid_utf8() {
    plain_problem("lens.md", &[0xff], TEXT);
}
#[test]
fn leaves_repeated_autonomy_dial() {
    plain_problem(
        "autonomy.md",
        b"- [x] `scope`: one\n- [ ] `scope`: two\n",
        AUTONOMY,
    );
}
#[test]
fn leaves_private_name() {
    plain_problem("private.json", br#"{"name":"Mo Lee"}"#, PRIVATE);
}
#[test]
fn leaves_private_state_dir() {
    plain_problem("private.json", br#"{"state_dir":"elsewhere"}"#, PRIVATE);
}
#[test]
fn leaves_private_array() {
    plain_problem("private.json", b"[]", PRIVATE);
}
#[test]
fn leaves_private_invalid_json() {
    plain_problem("private.json", b"{", PRIVATE);
}
#[test]
fn leaves_briefs_subfolder() {
    problem(
        "briefs",
        |p| fs::create_dir_all(p.join("briefs/nested")).unwrap(),
        FOLDER,
    );
}
#[test]
fn leaves_briefs_link() {
    problem(
        "briefs",
        |p| {
            fs::create_dir(p.join("briefs")).unwrap();
            symlink("missing", p.join("briefs/linked.md")).unwrap();
        },
        FOLDER,
    );
}
#[test]
fn leaves_linked_briefs_folder() {
    problem(
        "briefs",
        |p| symlink("missing", p.join("briefs")).unwrap(),
        LINK,
    );
}
#[test]
fn leaves_briefs_plain_file() {
    plain_problem("briefs", b"text", FOLDER);
}
#[test]
fn leaves_65_briefs() {
    problem(
        "briefs",
        |p| {
            fs::create_dir(p.join("briefs")).unwrap();
            for n in 0..65 {
                fs::write(p.join(format!("briefs/{n}.md")), "brief").unwrap();
            }
        },
        FOLDER,
    );
}
#[test]
fn brings_exact_bytes_private_modes_and_keeps_other_template_briefs() {
    let scratch = Scratch::new();
    let from = scratch.path().join("old");
    let to = scratch.path().join("new");
    all_files(&from);
    template(&to);
    fs::set_permissions(to.join("IDENTITY.md"), fs::Permissions::from_mode(0o644)).unwrap();
    let skill = fs::read(to.join("SKILL.md")).unwrap();
    let config = fs::read(to.join("member.json")).unwrap();
    fs::write(to.join("briefs/stays.md"), "template brief").unwrap();
    bring(&from, &to, &NAMES.map(str::to_owned), &|_| true).unwrap();
    for name in NAMES {
        let name = if name == "briefs" {
            "briefs/older.md"
        } else {
            name
        };
        assert_eq!(
            fs::read(to.join(name)).unwrap(),
            fs::read(from.join(name)).unwrap()
        );
        assert_eq!(
            fs::metadata(to.join(name)).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(fs::read(to.join("SKILL.md")).unwrap(), skill);
    assert_eq!(fs::read(to.join("member.json")).unwrap(), config);
    assert_eq!(
        fs::read_to_string(to.join("briefs/stays.md")).unwrap(),
        "template brief"
    );
    let found = member::read_member(&to.join("member.json")).unwrap();
    assert!(found.filters.personal.contains("fictional private topic"));
}
#[test]
fn refuses_unknown_absent_and_duplicate_before_writing() {
    for names in [
        vec!["IDENTITY.md", "unknown.md"],
        vec!["IDENTITY.md", "lens.md"],
        vec!["IDENTITY.md", "IDENTITY.md"],
    ] {
        let scratch = Scratch::new();
        let from = scratch.path().join("old");
        let to = scratch.path().join("new");
        fs::create_dir(&from).unwrap();
        fs::write(from.join("IDENTITY.md"), "older").unwrap();
        template(&to);
        let before = fs::read(to.join("IDENTITY.md")).unwrap();
        let message = bring(
            &from,
            &to,
            &names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>(),
            &|_| true,
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            message,
            format!(
                "Voice can't bring in {}, so it changed nothing. Leave it out and try again.",
                names[1]
            )
        );
        assert!(says(&message));
        assert_eq!(fs::read(to.join("IDENTITY.md")).unwrap(), before);
    }
}
#[test]
fn refuses_invalid_final_member_with_folder_check_sentence() {
    let scratch = Scratch::new();
    let from = scratch.path().join("old");
    let to = scratch.path().join("new");
    fs::create_dir(&from).unwrap();
    fs::write(from.join("private.json"), br#"{"filters":{"personal":7}}"#).unwrap();
    template(&to);
    assert_eq!(offer(&from, &|_| true).unwrap()[0].problem, None);
    assert_eq!(
        bring(&from, &to, &["private.json".into()], &|_| true)
            .unwrap_err()
            .to_string(),
        CANT_BRING
    );
    assert!(says(CANT_BRING));
    assert!(!says("an unrelated failure"));
}

#[test]
fn refuses_missing_source_folder_before_writing() {
    let scratch = Scratch::new();
    let to = scratch.path().join("new");
    template(&to);
    let before = fs::read(to.join("IDENTITY.md")).unwrap();
    let error = bring(
        &scratch.path().join("missing"),
        &to,
        &["IDENTITY.md".into()],
        &|_| true,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Voice can't bring in IDENTITY.md, so it changed nothing. Leave it out and try again."
    );
    assert_eq!(fs::read(to.join("IDENTITY.md")).unwrap(), before);
}

#[test]
fn fixup_leaves_hard_links() {
    problem(
        "IDENTITY.md",
        |p| {
            fs::write(p.join("other"), "unchanged").unwrap();
            fs::hard_link(p.join("other"), p.join("IDENTITY.md")).unwrap();
        },
        "It shares its contents with another file, so Voice leaves it.",
    );
}
#[test]
fn fixup_unreadable_files_and_briefs_are_problems() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    for name in ["lens.md", "briefs/one.md", "briefs"] {
        let s = Scratch::new();
        fs::create_dir(s.path().join("briefs")).unwrap();
        if name != "briefs" {
            fs::write(s.path().join(name), "text").unwrap();
        }
        let path = s.path().join(name);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o0)).unwrap();
        let result = offer(s.path(), &|_| true);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let offered = result.unwrap();
        assert_eq!(
            offered
                .iter()
                .find(|f| f.name
                    == if name.starts_with("briefs") {
                        "briefs"
                    } else {
                        name
                    })
                .unwrap()
                .problem,
            Some("Voice can't read it, so Voice leaves it.")
        );
    }
}
#[test]
fn fixup_uppercase_codebook_has_same_cap() {
    problem(
        "briefs",
        |p| {
            fs::create_dir(p.join("briefs")).unwrap();
            fs::write(p.join("briefs/CODEBOOK.md"), vec![b'a'; 64 * 1024 + 1]).unwrap();
        },
        BIG,
    );
}
#[test]
fn fixup_ignores_ds_store_when_offering_and_copying() {
    let s = Scratch::new();
    let from = s.path().join("old");
    let to = s.path().join("new");
    fs::create_dir_all(from.join("briefs")).unwrap();
    fs::write(from.join("briefs/.DS_Store"), [0, 255]).unwrap();
    fs::write(from.join("briefs/one.md"), "brief").unwrap();
    assert_eq!(offer(&from, &|_| true).unwrap()[0].problem, None);
    template(&to);
    bring(&from, &to, &["briefs".into()], &|_| true).unwrap();
    assert_eq!(fs::read(to.join("briefs/one.md")).unwrap(), b"brief");
    assert!(!to.join("briefs/.DS_Store").exists());
}
#[test]
fn fixup_accepts_exact_caps() {
    let s = Scratch::new();
    fs::write(s.path().join("IDENTITY.md"), vec![b'a'; 1024 * 1024]).unwrap();
    fs::write(s.path().join("decisions.md"), vec![b'a'; 256 * 1024]).unwrap();
    fs::create_dir(s.path().join("briefs")).unwrap();
    fs::write(s.path().join("briefs/codebook.md"), vec![b'a'; 64 * 1024]).unwrap();
    for n in 0..63 {
        fs::write(s.path().join(format!("briefs/{n}.md")), "brief").unwrap();
    }
    let offered = offer(s.path(), &|_| true).unwrap();
    assert_eq!(offered.len(), 3);
    assert!(offered.iter().all(|f| f.problem.is_none()));
}
fn destination_link_stays_untouched(location: &str) {
    let s = Scratch::new();
    let from = s.path().join("old");
    let to = s.path().join("new");
    let outside = s.path().join("outside");
    all_files(&from);
    template(&to);
    if location.is_empty() {
        template(&outside);
    } else {
        fs::create_dir(&outside).unwrap();
    }
    fs::write(outside.join("sentinel"), "untouched").unwrap();
    let link = if location.is_empty() {
        to.clone()
    } else {
        to.join(location)
    };
    if link.is_dir() {
        fs::remove_dir_all(&link).unwrap();
    } else if link.exists() {
        fs::remove_file(&link).unwrap();
    }
    let target = if location.ends_with(".md") {
        outside.join("sentinel")
    } else {
        outside.clone()
    };
    symlink(&target, &link).unwrap();
    let before = snapshot(&outside);
    let name = if location.starts_with("briefs") {
        "briefs"
    } else {
        "IDENTITY.md"
    };
    let result = bring(&from, &to, &[name.into()], &|_| true);
    if location.ends_with(".md") {
        result.unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().is_file());
        assert_eq!(
            fs::read(&link).unwrap(),
            fs::read(from.join(location)).unwrap()
        );
    } else {
        assert_eq!(result.unwrap_err().to_string(), CANT_BRING);
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    }
    assert_eq!(snapshot(&outside), before);
}
#[test]
fn fixup3_replaces_destination_identity_link() {
    destination_link_stays_untouched("IDENTITY.md");
}

#[test]
fn fixup_refuses_destination_briefs_folder_link() {
    destination_link_stays_untouched("briefs");
}

#[test]
fn fixup3_replaces_destination_brief_file_link() {
    destination_link_stays_untouched("briefs/older.md");
}

#[test]
fn fixup_refuses_destination_root_link() {
    destination_link_stays_untouched("");
}

#[test]
fn fixup_refuses_linked_source_and_changed_backup_before_writing() {
    for linked in [false, true] {
        let s = Scratch::new();
        let from = s.path().join("old");
        let to = s.path().join("new");
        all_files(&from);
        template(&to);
        assert!(
            offer(&from, &|_| true)
                .unwrap()
                .iter()
                .all(|f| f.problem.is_none())
        );
        let source = if linked {
            let link = s.path().join("link");
            symlink(&from, &link).unwrap();
            link
        } else {
            fs::write(from.join("IDENTITY.md"), b"bad\0text").unwrap();
            from
        };
        let before = snapshot(&to);
        assert_eq!(
            bring(&source, &to, &["IDENTITY.md".into()], &|_| true)
                .unwrap_err()
                .to_string(),
            "Voice can't bring in IDENTITY.md, so it changed nothing. Leave it out and try again."
        );
        assert_eq!(snapshot(&to), before);
    }
}

#[test]
fn fixup_private_validator_can_refuse() {
    let s = Scratch::new();
    fs::write(s.path().join("private.json"), "{}").unwrap();
    assert_eq!(
        offer(s.path(), &|_| false).unwrap()[0].problem,
        Some(PRIVATE)
    );
    let to = s.path().join("new");
    template(&to);
    let before = snapshot(&to);
    assert!(bring(s.path(), &to, &["private.json".into()], &|_| false).is_err());
    assert_eq!(snapshot(&to), before);
}

#[test]
fn fixup_recognizes_new_sentences() {
    assert_eq!(
        CANT_BRING,
        "Voice couldn't bring in the older assistant's own files, so it changed nothing. Leave out private.json and autonomy.md, then try again."
    );
    assert!(says(
        "It shares its contents with another file, so Voice leaves it."
    ));
    assert!(says("Voice can't read it, so Voice leaves it."));
    assert!(says(
        "Voice can't bring in lens.md, so it changed nothing. Leave it out and try again."
    ));
}

fn destination_hard_link_stays_untouched(relative: &str) {
    let s = Scratch::new();
    let from = s.path().join("old");
    let to = s.path().join("new");
    let outside = s.path().join("outside.md");
    all_files(&from);
    template(&to);
    fs::write(&outside, "outside invented text").unwrap();
    let target = to.join(relative);
    if target.exists() {
        fs::remove_file(&target).unwrap();
    }
    fs::hard_link(&outside, &target).unwrap();
    let before = fs::read(&outside).unwrap();
    let name = if relative.starts_with("briefs/") {
        "briefs"
    } else {
        relative
    };
    bring(&from, &to, &[name.into()], &|_| true).unwrap();
    assert_eq!(fs::read(&outside).unwrap(), before, "{relative}");
    assert_eq!(fs::metadata(&outside).unwrap().nlink(), 1, "{relative}");
    assert_eq!(
        fs::read(&target).unwrap(),
        fs::read(from.join(relative)).unwrap()
    );
    assert_eq!(fs::metadata(&target).unwrap().nlink(), 1, "{relative}");
}

#[test]
fn fixup3_replaces_destination_identity_hard_link() {
    destination_hard_link_stays_untouched("IDENTITY.md");
}

#[test]
fn fixup3_replaces_destination_brief_hard_link() {
    destination_hard_link_stays_untouched("briefs/older.md");
}

#[test]
fn fixup3_unsearchable_briefs_are_a_read_problem() {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let s = Scratch::new();
    let briefs = s.path().join("briefs");
    fs::create_dir(&briefs).unwrap();
    fs::write(briefs.join("one.md"), "invented brief").unwrap();
    fs::set_permissions(&briefs, fs::Permissions::from_mode(0o400)).unwrap();
    let result = offer(s.path(), &|_| true);
    fs::set_permissions(&briefs, fs::Permissions::from_mode(0o700)).unwrap();
    let offered = result.unwrap();
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].name, "briefs");
    assert_eq!(offered[0].problem, Some(READ));
}
