mod common;
use common::Scratch;
use std::{fs, os::unix::fs::symlink};
use voice_core::deny::DenyList;
use voice_ingest::calendar::read_export;
const BAD: &str = "Voice couldn't read this calendar. Check it's a calendar you exported (an .ics file), then choose it again.";
const TOO_MANY: &str =
    "This folder has more than 64 calendar files. Choose a folder with fewer, or one file.";
const HARD_LINK: &str = "Voice won't read this calendar: the same file has another name somewhere else on this computer (a hard link). Make a copy of it and choose the copy.";
#[test]
fn one_ics_file_is_read() {
    let scratch = Scratch::new("calendar-file");
    let path = scratch.0.join("export.ICS");
    fs::write(&path, b"calendar bytes").unwrap();
    assert_eq!(
        read_export(&path, &DenyList::builtin(), false).unwrap(),
        [b"calendar bytes".to_vec()]
    );
}
#[test]
fn a_folder_reads_only_real_ics_files_in_name_order() {
    let scratch = Scratch::new("calendar-folder");
    fs::write(scratch.0.join("z.ics"), "second").unwrap();
    fs::write(scratch.0.join("a.iCs"), "first").unwrap();
    fs::write(scratch.0.join("notes.txt"), "ignored").unwrap();
    fs::create_dir(scratch.0.join("folder.ics")).unwrap();
    symlink("a.iCs", scratch.0.join("link.ics")).unwrap();
    assert_eq!(
        read_export(&scratch.0, &DenyList::builtin(), true).unwrap(),
        [b"first".to_vec(), b"second".to_vec()]
    );
}
#[test]
fn hard_links_are_refused_in_both_forms_without_paths() {
    let scratch = Scratch::new("calendar-hardlink");
    let path = scratch.0.join("a.ics");
    fs::write(&path, "private canary").unwrap();
    fs::hard_link(&path, scratch.0.join("b.txt")).unwrap();
    for (path, folder) in [(&path, false), (&scratch.0, true)] {
        let result = read_export(path, &DenyList::builtin(), folder);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), HARD_LINK);
    }
}
#[test]
fn a_chosen_link_or_wrong_file_kind_is_refused() {
    let scratch = Scratch::new("calendar-link");
    fs::write(scratch.0.join("real.ics"), "bytes").unwrap();
    fs::write(scratch.0.join("notes.txt"), "bytes").unwrap();
    symlink("real.ics", scratch.0.join("link.ics")).unwrap();
    symlink(&scratch.0, scratch.0.join("linked-folder")).unwrap();
    for (name, folder) in [
        ("link.ics", false),
        ("notes.txt", false),
        ("real.ics", true),
        ("linked-folder", true),
        ("missing.ics", false),
    ] {
        let result = read_export(&scratch.0.join(name), &DenyList::builtin(), folder);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), BAD);
    }
    let result = read_export(&scratch.0, &DenyList::builtin(), false);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), BAD);
}
#[test]
fn denied_files_are_refused_in_both_forms() {
    let scratch = Scratch::new("calendar-denied");
    fs::write(scratch.0.join("blocked.ics"), "private canary").unwrap();
    let deny = DenyList::new(&scratch.0, &["blocked.ics".into()]).unwrap();
    for (path, folder) in [
        (scratch.0.join("blocked.ics"), false),
        (scratch.0.clone(), true),
    ] {
        let result = read_export(&path, &deny, folder);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), BAD);
    }
}
#[test]
fn sixty_five_files_refuse_the_folder() {
    let scratch = Scratch::new("calendar-many");
    for n in 0..65 {
        fs::write(scratch.0.join(format!("{n}.ics")), "bytes").unwrap();
    }
    let result = read_export(&scratch.0, &DenyList::builtin(), true);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), TOO_MANY);
    fs::remove_file(scratch.0.join("64.ics")).unwrap();
    assert_eq!(
        read_export(&scratch.0, &DenyList::builtin(), true)
            .unwrap()
            .len(),
        64
    );
}
#[test]
fn a_folder_without_calendar_files_is_bad() {
    let scratch = Scratch::new("calendar-empty");
    fs::write(scratch.0.join("notes.txt"), "ignored").unwrap();
    symlink("notes.txt", scratch.0.join("link.ics")).unwrap();
    fs::create_dir(scratch.0.join("nested.ics")).unwrap();
    let result = read_export(&scratch.0, &DenyList::builtin(), true);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), BAD);
}
