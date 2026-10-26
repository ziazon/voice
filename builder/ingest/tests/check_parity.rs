mod common;
use common::{Scratch, checked, parity_tools_available, repo};
use persona_builder::{
    crew::{NewMember, new_member},
    error::Error,
    member::read_member,
};
use persona_builder_ingest::{
    check::{privacy_check, render, scan, tracked_or_walked},
    filters::Filters,
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn member(folder: &Path) {
    new_member(
        &repo().join("crew/_template"),
        folder,
        &NewMember {
            decisions: persona_builder::crew::Decisions::Python,
            name: "Wren",
            pronouns: "she/her",
            principal: "Rowan Vale",
            principal_pronouns: "they/them",
            repo_shown: "~/ai",
            date: "2026-10-24",
        },
    )
    .unwrap();
}
fn put(folder: &Path, name: &str, text: impl AsRef<[u8]>) {
    fs::create_dir_all(folder.join(name).parent().unwrap()).unwrap();
    fs::write(folder.join(name), text).unwrap();
}
fn edit(folder: &Path, f: impl FnOnce(&mut Value)) {
    let path = folder.join("member.json");
    let mut v = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    f(&mut v);
    fs::write(path, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}
fn oracle(folder: &Path, home: &Path) {
    let rust = privacy_check(folder).unwrap();
    let python = Command::new("python3")
        .args(["-I"])
        .arg(repo().join("scripts/crew_decisions.py"))
        .arg("--member")
        .arg(folder.join("member.json"))
        .args(["privacy-check", "--dir"])
        .arg(folder)
        .env("HOME", home)
        .output()
        .unwrap();
    assert_eq!(
        python.status.code(),
        Some(i32::from(!rust.hits.is_empty())),
        "{}",
        String::from_utf8_lossy(&python.stderr)
    );
    assert_eq!(render(&rust).as_bytes(), python.stdout);
}
#[test]
fn folder_cases_match_python_walked_and_tracked() {
    if !parity_tools_available() {
        return;
    }
    for tracked in [false, true] {
        for case in 0..10 {
            let s = Scratch::new("folder-parity");
            let folder = s.0.join("crew");
            member(&folder);
            match case {
                0 => {}
                1 => {
                    put(&folder, "decisions.md", "my doctor\n");
                    put(&folder, "briefs/extra.md", "wren@example.org\n");
                }
                2 => edit(&folder, |v| {
                    v["codebook"]["domains"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!("wren@example.org"))
                }),
                3 => edit(&folder, |v| {
                    let p = v["filters"]["personal"].as_str().unwrap().to_owned();
                    v["filters"]["personal"] = json!(p + r"|\bgarden club\b");
                }),
                4 => {
                    put(
                        &folder,
                        "private.json",
                        r#"{"filters":{"personal":"garden club"}}"#,
                    );
                    put(&folder, "autonomy.md", "the garden club rota\n");
                }
                5 => {
                    for name in [
                        "engineers/example.md",
                        "briefs/sub/deep.md",
                        "report-template.html",
                        "notes.txt",
                        "extra.json",
                    ] {
                        put(&folder, name, "my doctor again\n");
                    }
                }
                6 => put(&folder, "clip.bin", b"caf\xe9"),
                7 => edit(&folder, |v| {
                    v["codebook"]["groups"]["wren@example.org"] = json!(["scope"])
                }),
                8 => {
                    put(&folder, ".gitignore", "ignored.md\n");
                    put(&folder, "ignored.md", "my doctor\n");
                }
                9 => {
                    put(&folder, "a-b.md", "my doctor\n");
                    put(&folder, "a/b.md", "my doctor\n");
                }
                _ => unreachable!(),
            }
            if tracked {
                checked(
                    Command::new("git")
                        .arg("-C")
                        .arg(&folder)
                        .args(["init", "-q"]),
                );
                checked(
                    Command::new("git")
                        .arg("-C")
                        .arg(&folder)
                        .args(["add", "-A"]),
                );
            }
            oracle(&folder, &s.0);
            if case == 9 {
                assert_eq!(
                    privacy_check(&folder).unwrap().hits,
                    ["a/b.md:1 (personal)", "a-b.md:1 (personal)"]
                );
            }
        }
    }
}
#[test]
fn multiline_codebook_reports_only_first_hit_walked_and_tracked() {
    if !parity_tools_available() {
        return;
    }
    for tracked in [false, true] {
        let s = Scratch::new("multiline-codebook-parity");
        let folder = s.0.join("crew");
        member(&folder);
        edit(&folder, |v| {
            v["codebook"]["domains"] = json!(["my doctor\nrobin@example.org"]);
        });
        if tracked {
            checked(
                Command::new("git")
                    .arg("-C")
                    .arg(&folder)
                    .args(["init", "-q"]),
            );
            checked(
                Command::new("git")
                    .arg("-C")
                    .arg(&folder)
                    .args(["add", "-A"]),
            );
        }
        oracle(&folder, &s.0);
        assert_eq!(
            privacy_check(&folder).unwrap().hits,
            ["member.json:codebook.domains[0] (personal)"]
        );
    }
}
#[test]
fn walked_files_honor_ignores_with_a_corrupt_index() {
    if !parity_tools_available() {
        return;
    }
    let s = Scratch::new("corrupt-index-parity");
    let folder = s.0.join("crew");
    member(&folder);
    checked(
        Command::new("git")
            .arg("-C")
            .arg(&folder)
            .args(["init", "-q"]),
    );
    put(&folder, ".git/index", b"not an index");
    put(&folder, ".gitignore", "notes.txt\n# my doctor\n");
    put(&folder, "notes.txt", "my doctor again");
    oracle(&folder, &s.0);
    let files = tracked_or_walked(&folder).unwrap();
    assert!(!files.contains(&PathBuf::from("notes.txt")));
    assert!(files.contains(&PathBuf::from(".gitignore")));
    let report = privacy_check(&folder).unwrap();
    assert!(!report.hits.iter().any(|hit| hit.starts_with("notes.txt:")));
    assert_eq!(report.hits, [".gitignore:2 (personal)"]);
}
#[test]
fn invalid_folders_have_exact_errors() {
    let s = Scratch::new("invalid-folder");
    let path = s.0.canonicalize().unwrap().join("member.json");
    assert_eq!(
        privacy_check(&s.0).err().map(|e| e.to_string()),
        Some(format!("{}: no member.json to check", path.display()))
    );
    fs::write(&path, "[1, 2]").unwrap();
    assert_eq!(
        privacy_check(&s.0).err().map(|e| e.to_string()),
        Some(format!(
            "{}: member.json must be a JSON object",
            path.display()
        ))
    );
}
#[test]
fn listing_skips_links_and_junk_and_scan_refuses_a_replaced_link() {
    let s = Scratch::new("listing");
    let folder = s.0.join("crew");
    member(&folder);
    for name in [
        ".DS_Store",
        "__MACOSX/a",
        "nested/._junk",
        "Thumbs.db",
        "desktop.ini",
        ".git/junk",
    ] {
        put(&folder, name, "my doctor");
    }
    let link = folder.join("link.md");
    std::os::unix::fs::symlink(folder.join("decisions.md"), &link).unwrap();
    let files = tracked_or_walked(&folder).unwrap();
    assert!(files.contains(&PathBuf::from("decisions.md")));
    assert!(!files.contains(&PathBuf::from("link.md")));
    assert!(!files.iter().any(|p| p.to_string_lossy().contains("junk")));
    let filters =
        Filters::compile(&read_member(&folder.join("member.json")).unwrap().filters).unwrap();
    assert!(matches!(
        scan(&folder, &["link.md".into()], &filters),
        Err(Error::Io { .. })
    ));
    let report = scan(&folder, &["decisions.md".into()], &filters).unwrap();
    assert!(!report.filters_differ);
}
#[test]
fn listing_preserves_non_utf8_names_where_supported() {
    use std::os::unix::ffi::OsStringExt;
    let s = Scratch::new("non-utf8");
    let name = PathBuf::from(std::ffi::OsString::from_vec(b"file-\xff".to_vec()));
    if let Err(error) = fs::write(s.0.join(&name), "clean") {
        eprintln!("skipped non-UTF-8 filename probe: filesystem refused it ({error})");
        return;
    }
    assert_eq!(tracked_or_walked(&s.0).unwrap(), vec![name]);
}
