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
    let offered = offer(scratch.path()).unwrap();
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
    let offered = offer(scratch.path()).unwrap();
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
        offer(scratch.path())
            .unwrap()
            .iter()
            .map(|file| file.name)
            .collect::<Vec<_>>(),
        ["lens.md"]
    );
    assert!(offer(&scratch.path().join("missing")).unwrap().is_empty());
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
    bring(&from, &to, &NAMES.map(str::to_owned)).unwrap();
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
        )
        .unwrap_err()
        .to_string();
        assert_eq!(
            message,
            format!(
                "Voice can't bring in {}, so it changed nothing. Untick it and try again.",
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
    assert_eq!(offer(&from).unwrap()[0].problem, None);
    assert_eq!(
        bring(&from, &to, &["private.json".into()])
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
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Voice can't bring in IDENTITY.md, so it changed nothing. Untick it and try again."
    );
    assert_eq!(fs::read(to.join("IDENTITY.md")).unwrap(), before);
}
