#[allow(dead_code)]
mod common;
use common::{FakeHttp, Scratch, pb6::*, reply};
use std::{fs, path::Path};
use voice_core::error::Result;
use voice_save::{
    flow::{PrivacyCheck, Saver, place_opened},
    git::Git,
    github::{Github, RepoRef},
    keychain::TokenStore,
    record::{Role, SaveRecord, read_record},
    secret::Secret,
};
struct Tokens;
impl TokenStore for Tokens {
    fn load(&self) -> Result<Option<Secret>> {
        Ok(Some(Secret::new("invented-token".into())))
    }
    fn save(&self, _: &Secret) -> Result<()> {
        Ok(())
    }
    fn forget(&self) -> Result<()> {
        Ok(())
    }
}
struct Privacy;
impl PrivacyCheck for Privacy {
    fn hits(&self, _: &Path) -> Result<Vec<String>> {
        Ok(vec![])
    }
}
#[test]
fn stages_then_places_without_private_files() {
    let scratch = Scratch::new("open");
    let source = scratch.0.join("source");
    let home = scratch.0.join("home");
    fs::create_dir(&home).unwrap();
    member(&source);
    let bare = scratch.0.join("remote.git");
    init(&bare, true);
    let git = Git::system();
    let build_home = scratch.0.join("builder-home");
    fs::create_dir(&build_home).unwrap();
    let paths =
        voice_save::flow::MemberPaths::load(&source.join("member.json"), &build_home).unwrap();
    let save_http = FakeHttp::new(vec![
        account(),
        reply(
            201,
            serde_json::json!({"owner":{"login":"rowan"},"name":"wren-assistant"}),
        ),
        reply(
            200,
            serde_json::json!({"private":true,"visibility":"private"}),
        ),
        account(),
    ]);
    let save_github = Github::new(
        &save_http,
        voice_save::github::Endpoints::github(),
        "Iv1.invented",
    )
    .unwrap();
    let saving = Saver {
        git: &git,
        github: Some(&save_github),
        tokens: &Tokens,
    };
    saving.choose_github(&paths, "wren-assistant").unwrap();
    let backup = paths.state_dir.join("backup.git");
    common::pb6::git(
        &backup,
        &[
            "config",
            &format!("url.{}.pushInsteadOf", file_url(&bare)),
            "https://github.com/rowan/wren-assistant.git",
        ],
    );
    struct Approve;
    impl voice_save::flow::RemoteCheck for Approve {
        fn confirm(&self, _: &Secret, _: &[String]) -> Result<()> {
            Ok(())
        }
    }
    saving
        .save_now_from(&paths, &build_home, &Privacy, &Approve, Some("Studio"))
        .unwrap();
    save_http.assert_drained();
    let wrap = Wrapper::with_remote(&scratch.0, &bare, "rowan/wren-assistant");
    let git = Git::at(wrap.program.clone());
    let http = FakeHttp::new(vec![
        account(),
        reply(
            200,
            serde_json::json!({"private":true,"visibility":"private"}),
        ),
    ]);
    let github = Github::new(
        &http,
        voice_save::github::Endpoints::github(),
        "Iv1.invented",
    )
    .unwrap();
    let repo = RepoRef {
        owner: "rowan".into(),
        name: "wren-assistant".into(),
    };
    let saver = Saver {
        git: &git,
        github: Some(&github),
        tokens: &Tokens,
    };
    let staged = saver
        .stage_open(&home, &repo, "wren", "2026-11-13-120000", &Privacy)
        .unwrap();
    assert!(!home.join(".claude/crew/wren").exists());
    assert_eq!(staged.builder.as_deref(), Some("Studio"));
    // Both conflicts refuse without moving the already checked staged copy.
    let destination = home.join(".claude/crew/wren");
    fs::create_dir(&destination).unwrap();
    assert!(
        matches!(place_opened(&staged, &home, &repo),Err(voice_core::error::Error::Exists(slug)) if slug=="wren")
    );
    fs::remove_dir(&destination).unwrap();
    let state = home.join(".claude/state/wren");
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("save.json"), "original").unwrap();
    assert_eq!(
        place_opened(&staged, &home, &repo)
            .err()
            .unwrap()
            .to_string(),
        voice_save::text::state_in_use("wren")
    );
    assert_eq!(fs::read(state.join("save.json")).unwrap(), b"original");
    fs::remove_file(state.join("save.json")).unwrap();
    let obstruction = state.join(format!("save.json.tmp-{}", std::process::id()));
    fs::write(&obstruction, "keep obstruction").unwrap();
    assert!(place_opened(&staged, &home, &repo).is_err());
    assert!(staged.git_dir.is_dir());
    assert!(staged.member_dir.is_dir());
    assert!(!destination.exists());
    assert!(!state.join("backup.git").exists());
    assert_eq!(fs::read(&obstruction).unwrap(), b"keep obstruction");
    fs::remove_file(&obstruction).unwrap();
    let paths = place_opened(&staged, &home, &repo).unwrap();
    assert!(!paths.dir.join("private.json").exists());
    assert_eq!(
        fs::read(paths.dir.join("member.json")).unwrap(),
        fs::read(source.join("member.json")).unwrap()
    );
    assert_eq!(
        read_record(&paths.state_dir).unwrap(),
        Some(SaveRecord::Github {
            owner: repo.owner,
            name: repo.name,
            branch: "main".into(),
            role: Role::Member,
            builder: Some("Studio".into())
        })
    );
    assert!(!staged.dir.exists());
    fn equal_files(source: &Path, dest: &Path) {
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if matches!(name.to_str(), Some("private.json" | ".DS_Store")) {
                continue;
            }
            if entry.file_type().unwrap().is_dir() {
                equal_files(&entry.path(), &dest.join(name));
            } else {
                assert_eq!(
                    fs::read(entry.path()).unwrap(),
                    fs::read(dest.join(name)).unwrap()
                );
            }
        }
    }
    equal_files(&source, &paths.dir);
    assert_eq!(
        common::pb6::git(
            &paths.state_dir.join("backup.git"),
            &["remote", "get-url", "origin"]
        ),
        "https://github.com/rowan/wren-assistant.git"
    );
    assert_eq!(
        common::pb6::git(
            &paths.state_dir.join("backup.git"),
            &["rev-parse", "--is-bare-repository"]
        ),
        "true"
    );
}

fn account() -> voice_save::github::HttpResponse {
    let mut response = reply(200, serde_json::json!({"login":"rowan","id":12}));
    response
        .headers
        .push(("x-oauth-scopes".into(), "repo".into()));
    response
}

#[test]
fn saved_copy_refusals_remove_staging_and_leave_home_untouched() {
    use std::os::unix::fs::symlink;
    for scenario in [
        "link",
        "submodule",
        "denied",
        "missing",
        "state",
        "privacy",
        "public",
        "traversal",
        "blob",
        "total",
        "count",
    ] {
        let scratch = Scratch::new("open-refusal");
        let repo_dir = scratch.0.join("repo");
        init(&repo_dir, false);
        let folder = repo_dir.join("crew/wren");
        member(&folder);
        fs::remove_file(folder.join("private.json")).unwrap();
        match scenario {
            "link" => symlink("identity.md", folder.join("link")).unwrap(),
            "denied" => fs::write(folder.join(".env"), "invented").unwrap(),
            "missing" => fs::remove_file(folder.join("member.json")).unwrap(),
            "state" => {
                let path = folder.join("member.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["state_dir"] = serde_json::json!("~/elsewhere");
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            _ => {}
        }
        common::pb6::git(&repo_dir, &["add", "-f", "crew"]);
        common::pb6::git(&repo_dir, &["commit", "-qm", "Save Wren"]);
        if scenario == "submodule" {
            let oid = common::pb6::git(&repo_dir, &["rev-parse", "HEAD"]);
            common::pb6::git(
                &repo_dir,
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("160000,{oid},crew/wren/module"),
                ],
            );
            common::pb6::git(&repo_dir, &["commit", "-qm", "Add module"]);
        }
        let home = scratch.0.join("home");
        fs::create_dir_all(home.join(".claude/crew")).unwrap();
        fs::write(home.join("untouched"), "original").unwrap();
        let wrap = Wrapper::with_remote(&scratch.0, &repo_dir, "rowan/wren-assistant");
        if matches!(scenario, "traversal" | "count" | "blob" | "total") {
            let oid = common::pb6::git(&repo_dir, &["rev-parse", "HEAD:crew/wren/member.json"]);
            let relative = if scenario == "traversal" {
                "../outside"
            } else {
                "member.json"
            };
            let repetitions = if scenario == "count" {
                2001
            } else if scenario == "total" {
                6
            } else {
                1
            };
            let tree = format!("100644 blob {oid}\tcrew/wren/{relative}\0").repeat(repetitions);
            let size = if scenario == "blob" {
                10485761
            } else if scenario == "total" {
                10485760
            } else {
                0
            };
            let source = fs::read_to_string(&wrap.program).unwrap();
            let intercept = format!(
                "if 'ls-tree' in sys.argv[1:]:\n    sys.stdout.buffer.write({}.encode())\n    sys.exit(0)\nif 'cat-file' in sys.argv[1:] and '-s' in sys.argv[1:]:\n    print({size})\n    sys.exit(0)\n",
                serde_json::to_string(&tree).unwrap()
            );
            // Insert after imports, before the real git delegation.
            fs::write(
                &wrap.program,
                source.replace(
                    "# macOS CoreFoundation",
                    &(intercept + "# macOS CoreFoundation"),
                ),
            )
            .unwrap();
        }
        let git = Git::at(wrap.program.clone());
        let http = FakeHttp::new(vec![
            account(),
            reply(
                200,
                serde_json::json!({"private":scenario!="public","visibility":if scenario=="public"{"public"}else{"private"}}),
            ),
        ]);
        let github = Github::new(
            &http,
            voice_save::github::Endpoints::github(),
            "Iv1.invented",
        )
        .unwrap();
        let saver = Saver {
            git: &git,
            github: Some(&github),
            tokens: &Tokens,
        };
        struct Hit;
        impl PrivacyCheck for Hit {
            fn hits(&self, _: &Path) -> Result<Vec<String>> {
                Ok(vec!["identity.md:1".into()])
            }
        }
        let privacy: &dyn PrivacyCheck = if scenario == "privacy" {
            &Hit
        } else {
            &Privacy
        };
        let result = saver.stage_open(
            &home,
            &RepoRef {
                owner: "rowan".into(),
                name: "wren-assistant".into(),
            },
            "wren",
            "2026-11-13-120000",
            privacy,
        );
        let error = result
            .err()
            .unwrap_or_else(|| panic!("{scenario} unexpectedly opened"))
            .to_string();
        let full = "rowan/wren-assistant";
        let expected = match scenario {
            "link" => voice_save::text::saved_copy_refused(full, "link"),
            "submodule" => voice_save::text::saved_copy_refused(full, "module"),
            "denied" => voice_save::text::saved_copy_refused(full, ".env"),
            "missing" | "state" => voice_save::text::saved_copy_refused(full, "member.json"),
            "privacy" => voice_save::text::saved_copy_private(full, "identity.md:1"),
            "public" => voice_save::text::repo_public(full),
            "traversal" => voice_save::text::saved_copy_refused(full, "../outside"),
            "count" | "blob" | "total" => voice_save::text::saved_copy_large(full),
            _ => unreachable!(),
        };
        assert_eq!(error, expected, "{scenario}");
        assert_eq!(fs::read(home.join("untouched")).unwrap(), b"original");
        assert_eq!(
            fs::read_dir(home.join(".claude/crew")).unwrap().count(),
            0,
            "{scenario}"
        );
        assert!(!home.join(".claude/state").exists());
        http.assert_drained();
    }
}

#[test]
fn member_computer_refuses_every_save_choice_before_git_http_or_privacy() {
    use voice_save::flow::{MemberPaths, RemoteCheck};
    struct Never;
    impl PrivacyCheck for Never {
        fn hits(&self, _: &Path) -> Result<Vec<String>> {
            panic!("privacy called")
        }
    }
    impl RemoteCheck for Never {
        fn confirm(&self, _: &Secret, _: &[String]) -> Result<()> {
            panic!("remote called")
        }
    }
    let scratch = Scratch::new("member-guard");
    let home = &scratch.0;
    let folder = home.join("member");
    member(&folder);
    let paths = MemberPaths::load(&folder.join("member.json"), home).unwrap();
    let wrap = Wrapper::new(home, false);
    let git = Git::at(wrap.program.clone());
    let http = FakeHttp::new(vec![]);
    let github = Github::new(
        &http,
        voice_save::github::Endpoints::github(),
        "Iv1.invented",
    )
    .unwrap();
    let saver = Saver {
        git: &git,
        github: Some(&github),
        tokens: &Tokens,
    };
    for builder in [None, Some("Studio".into())] {
        voice_save::record::write_record(
            &paths.state_dir,
            &SaveRecord::Github {
                owner: "rowan".into(),
                name: "wren-assistant".into(),
                branch: "main".into(),
                role: Role::Member,
                builder: builder.clone(),
            },
        )
        .unwrap();
        let expected = voice_save::text::member_computer("Wren", builder.as_deref());
        for error in [
            saver.choose_local(&paths).err(),
            saver.choose_existing(&paths, home, home, &Never).err(),
            saver.choose_github(&paths, "wren-assistant").err(),
            saver.save_now(&paths, home, &Never, &Never).err(),
            saver
                .save_now_from(&paths, home, &Never, &Never, Some("Studio"))
                .err(),
        ] {
            assert_eq!(error.unwrap().to_string(), expected);
        }
        assert!(http.requests.borrow().is_empty());
        assert!(wrap.calls().is_empty());
    }
}

#[test]
fn opening_never_materializes_private_json_or_finder_files_from_a_remote() {
    let scratch = Scratch::new("open-private-files");
    let remote = scratch.0.join("remote");
    init(&remote, false);
    let source = remote.join("crew/wren");
    member(&source);
    common::pb6::git(&remote, &["add", "-f", "crew"]);
    common::pb6::git(&remote, &["commit", "-qm", "Saved copy"]);
    let home = scratch.0.join("home");
    fs::create_dir(&home).unwrap();
    let wrap = Wrapper::with_remote(&scratch.0, &remote, "rowan/wren-assistant");
    let git = Git::at(wrap.program.clone());
    let http = FakeHttp::new(vec![
        account(),
        reply(
            200,
            serde_json::json!({"private":true,"visibility":"private"}),
        ),
    ]);
    let github = Github::new(
        &http,
        voice_save::github::Endpoints::github(),
        "Iv1.invented",
    )
    .unwrap();
    let staged = Saver {
        git: &git,
        github: Some(&github),
        tokens: &Tokens,
    }
    .stage_open(
        &home,
        &RepoRef {
            owner: "rowan".into(),
            name: "wren-assistant".into(),
        },
        "wren",
        "2026-11-13-120000",
        &Privacy,
    )
    .unwrap();
    assert!(!staged.member_dir.join("private.json").exists());
    assert!(!staged.member_dir.join(".DS_Store").exists());
    assert_eq!(
        fs::read(staged.member_dir.join("identity.md")).unwrap(),
        fs::read(source.join("identity.md")).unwrap()
    );
    fs::remove_dir_all(staged.dir).unwrap();
    http.assert_drained();
}

#[test]
fn placing_cleans_new_empty_state_after_a_member_move_failure() {
    let scratch = Scratch::new("place-state-cleanup");
    let home = &scratch.0;
    let dir = home.join(".claude/crew/.wren.voice-opening-test");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("member");
    member(&source);
    let staged = voice_save::flow::Staged {
        member: voice_core::member::read_member(&source.join("member.json")).unwrap(),
        member_dir: source.clone(),
        git_dir: dir.join("backup.git"),
        dir,
        builder: None,
    };
    fs::create_dir(&staged.git_dir).unwrap();
    let repo = RepoRef {
        owner: "rowan".into(),
        name: "wren-assistant".into(),
    };
    let destination = home.join(".claude/crew/wren");
    fs::create_dir(&destination).unwrap();
    assert!(place_opened(&staged, home, &repo).is_err());
    assert!(!home.join(".claude/state/wren").exists());
    fs::remove_dir(&destination).unwrap();
    // Fail the member move after the backup move has created the private folder.
    fs::remove_dir_all(&source).unwrap();
    assert!(place_opened(&staged, home, &repo).is_err());
    assert!(staged.git_dir.is_dir());
    assert!(!home.join(".claude/state/wren").exists());
}

#[test]
fn placing_succeeds_when_staging_cannot_be_removed() {
    let scratch = Scratch::new("place-staging-cleanup");
    let home = &scratch.0;
    let dir = home.join(".claude/crew/.wren.voice-opening-test");
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("member");
    member(&source);
    let staged = voice_save::flow::Staged {
        member: voice_core::member::read_member(&source.join("member.json")).unwrap(),
        member_dir: source,
        git_dir: dir.join("backup.git"),
        dir,
        builder: None,
    };
    fs::create_dir(&staged.git_dir).unwrap();
    fs::write(staged.dir.join("leftover"), "keep").unwrap();
    let repo = RepoRef {
        owner: "rowan".into(),
        name: "wren-assistant".into(),
    };
    let paths = place_opened(&staged, home, &repo).unwrap();
    assert!(paths.dir.join("member.json").is_file());
    assert!(paths.state_dir.join("backup.git").is_dir());
    assert!(read_record(&paths.state_dir).unwrap().is_some());
}

#[test]
fn choosing_local_replaces_a_damaged_save_record() {
    let scratch = Scratch::new("choose-damaged");
    let source = scratch.0.join("member");
    member(&source);
    let paths =
        voice_save::flow::MemberPaths::load(&source.join("member.json"), &scratch.0).unwrap();
    fs::create_dir_all(&paths.state_dir).unwrap();
    fs::write(paths.state_dir.join("save.json"), "{").unwrap();
    let git = Git::system();
    let saver = Saver {
        git: &git,
        github: None,
        tokens: &Tokens,
    };
    assert!(
        saver
            .save_now(&paths, &scratch.0, &Privacy, &NoRemote)
            .is_err()
    );
    saver.choose_local(&paths).unwrap();
    assert_eq!(
        read_record(&paths.state_dir).unwrap(),
        Some(SaveRecord::Local)
    );
}
struct NoRemote;
impl voice_save::flow::RemoteCheck for NoRemote {
    fn confirm(&self, _: &Secret, _: &[String]) -> Result<()> {
        panic!("remote called")
    }
}
