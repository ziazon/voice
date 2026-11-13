#[allow(dead_code)]
mod common;
use common::{FakeHttp, Scratch, pb6::*, reply};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use voice_core::error::Result;
use voice_save::{
    flow::{MemberPaths, PrivacyCheck, RemoteCheck, Saver, Took, place_opened},
    git::Git,
    github::{Github, RepoRef},
    keychain::TokenStore,
    record::{Role, SaveRecord, read_record, write_record},
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
impl RemoteCheck for Privacy {
    fn confirm(&self, _: &Secret, _: &[String]) -> Result<()> {
        Ok(())
    }
}
fn account() -> voice_save::github::HttpResponse {
    let mut response = reply(200, serde_json::json!({"login":"rowan","id":12}));
    response
        .headers
        .push(("x-oauth-scopes".into(), "repo".into()));
    response
}
fn responses() -> Vec<voice_save::github::HttpResponse> {
    vec![
        account(),
        reply(
            200,
            serde_json::json!({"private":true,"visibility":"private"}),
        ),
    ]
}
struct Fixture {
    scratch: Scratch,
    source: PathBuf,
    build_home: PathBuf,
    building: MemberPaths,
    home: PathBuf,
    member: MemberPaths,
    wrap: Wrapper,
}
impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new("take");
        let source = scratch.0.join("source");
        member(&source);
        fs::create_dir(source.join("old")).unwrap();
        fs::write(source.join("old/a.md"), "old note\n").unwrap();
        let build_home = scratch.0.join("builder-home");
        fs::create_dir(&build_home).unwrap();
        let building = MemberPaths::load(&source.join("member.json"), &build_home).unwrap();
        let bare = scratch.0.join("remote.git");
        init(&bare, true);
        let http = FakeHttp::new(vec![
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
        let github = Github::new(
            &http,
            voice_save::github::Endpoints::github(),
            "Iv1.invented",
        )
        .unwrap();
        let git = Git::system();
        let saver = Saver {
            git: &git,
            github: Some(&github),
            tokens: &Tokens,
        };
        saver.choose_github(&building, "wren-assistant").unwrap();
        common::pb6::git(
            &building.state_dir.join("backup.git"),
            &[
                "config",
                &format!("url.{}.pushInsteadOf", file_url(&bare)),
                "https://github.com/rowan/wren-assistant.git",
            ],
        );
        saver
            .save_now_from(&building, &build_home, &Privacy, &Privacy, Some("Studio"))
            .unwrap();
        http.assert_drained();
        let home = scratch.0.join("home");
        fs::create_dir(&home).unwrap();
        let wrap = Wrapper::with_remote(&scratch.0, &bare, "rowan/wren-assistant");
        let git = Git::at(wrap.program.clone());
        let http = FakeHttp::new(responses());
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
        let repo = RepoRef {
            owner: "rowan".into(),
            name: "wren-assistant".into(),
        };
        let staged = saver
            .stage_open(&home, &repo, "wren", "2026-11-13-120000", &Privacy)
            .unwrap();
        let (member, _, _) = place_opened(&staged, &home, &repo).unwrap();
        http.assert_drained();
        wrap.clear();
        Self {
            scratch,
            source,
            build_home,
            building,
            home,
            member,
            wrap,
        }
    }
    fn save(&self, builder: &str) {
        let http = FakeHttp::new(vec![account()]);
        let github = Github::new(
            &http,
            voice_save::github::Endpoints::github(),
            "Iv1.invented",
        )
        .unwrap();
        let git = Git::system();
        Saver {
            git: &git,
            github: Some(&github),
            tokens: &Tokens,
        }
        .save_now_from(
            &self.building,
            &self.build_home,
            &Privacy,
            &Privacy,
            Some(builder),
        )
        .unwrap();
        http.assert_drained();
    }
    fn take(&self, privacy: &dyn PrivacyCheck) -> Result<Took> {
        let git = Git::at(self.wrap.program.clone());
        let http = FakeHttp::new(responses());
        let github = Github::new(
            &http,
            voice_save::github::Endpoints::github(),
            "Iv1.invented",
        )
        .unwrap();
        Saver {
            git: &git,
            github: Some(&github),
            tokens: &Tokens,
        }
        .take_newest(&self.home, &self.member, "2026-11-13-120001", privacy)
    }
    fn backup(&self) -> PathBuf {
        self.member.state_dir.join("backup.git")
    }
    fn assert_clean(&self) {
        assert!(
            !git_output(
                &self.backup(),
                &["rev-parse", "--verify", "refs/voice/incoming"]
            )
            .status
            .success()
        );
        assert_eq!(
            fs::read_dir(self.home.join(".claude/crew"))
                .unwrap()
                .count(),
            1
        );
    }
}
fn listing(dir: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    fn visit(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, (u32, Vec<u8>)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_owned();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let mode = metadata.permissions().mode() & 0o7777;
            let kind = metadata.file_type();
            if kind.is_dir() {
                out.insert(rel, (mode, b"directory".to_vec()));
                visit(root, &path, out);
            } else if kind.is_symlink() {
                out.insert(
                    rel,
                    (
                        mode,
                        fs::read_link(path)
                            .unwrap()
                            .as_os_str()
                            .as_encoded_bytes()
                            .to_vec(),
                    ),
                );
            } else {
                out.insert(rel, (mode, fs::read(path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(dir, dir, &mut out);
    out
}
#[test]
fn takes_changed_added_and_removed_files_then_is_already_newest() {
    let f = Fixture::new();
    fs::write(
        f.member.dir.join("private.json"),
        b"private bytes untouched\n",
    )
    .unwrap();
    fs::write(f.source.join("SKILL.md"), b"updated skill\n").unwrap();
    fs::create_dir(f.source.join("notes")).unwrap();
    fs::write(f.source.join("notes/b.md"), b"new note\n").unwrap();
    fs::remove_file(f.source.join("old/a.md")).unwrap();
    f.save("Studio");
    let result = f.take(&Privacy);
    assert!(
        matches!(result, Ok(Took::Updated { ref full, changed: 3 }) if full == "rowan/wren-assistant"),
        "{result:?}"
    );
    assert_eq!(
        fs::read(f.member.dir.join("SKILL.md")).unwrap(),
        b"updated skill\n"
    );
    assert_eq!(
        fs::read(f.member.dir.join("notes/b.md")).unwrap(),
        b"new note\n"
    );
    assert!(!f.member.dir.join("old").exists());
    assert_eq!(
        fs::read(f.member.dir.join("private.json")).unwrap(),
        b"private bytes untouched\n"
    );
    assert_eq!(
        common::pb6::git(&f.backup(), &["rev-parse", "refs/heads/main"]),
        common::pb6::git(
            &f.building.state_dir.join("backup.git"),
            &["rev-parse", "refs/heads/main"]
        )
    );
    f.assert_clean();
    let before = listing(&f.member.dir);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let result = f.take(&Privacy);
    assert!(
        matches!(result, Ok(Took::AlreadyNewest { ref full }) if full == "rowan/wren-assistant"),
        "{result:?}"
    );
    assert!(listing(&f.member.dir) == before, "member files changed");
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    f.assert_clean();
}
fn nonmember_record(record: Option<SaveRecord>) {
    let f = Fixture::new();
    let git = Git::at(f.wrap.program.clone());
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
    {
        match record {
            Some(record) => write_record(&f.member.state_dir, &record).unwrap(),
            None => fs::remove_file(f.member.state_dir.join("save.json")).unwrap(),
        }
        let before = listing(&f.member.dir);
        let error = saver
            .take_newest(&f.home, &f.member, "2026-11-13-120001", &Privacy)
            .err()
            .unwrap();
        assert_eq!(
            error.to_string(),
            "This computer saves Wren, so there's no newer version to take here."
        );
        assert!(listing(&f.member.dir) == before, "member files changed");
        assert!(f.wrap.calls().is_empty());
        assert!(http.requests.borrow().is_empty());
    }
}
fn local_refusal(scenario: &str) {
    use std::os::unix::fs::symlink;
    {
        let f = Fixture::new();
        fs::write(f.source.join("SKILL.md"), b"updated\n").unwrap();
        let rel = if scenario == "added" {
            fs::write(f.source.join("new.md"), b"new\n").unwrap();
            "new.md"
        } else {
            "SKILL.md"
        };
        f.save("Studio");
        match scenario {
            "changed" => fs::write(f.member.dir.join(rel), b"local edit\n").unwrap(),
            "link" => {
                fs::remove_file(f.member.dir.join(rel)).unwrap();
                symlink("identity.md", f.member.dir.join(rel)).unwrap();
            }
            "added" => fs::write(f.member.dir.join(rel), b"local file\n").unwrap(),
            _ => unreachable!(),
        }
        let before = listing(&f.member.dir);
        let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
        let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
        let error = f.take(&Privacy).err().unwrap();
        assert_eq!(
            error.to_string(),
            format!(
                "{rel} in Wren's folder changed on this computer, so Voice took nothing. Put it back as it was, or move it somewhere else, then try again."
            ),
            "{scenario}"
        );
        assert!(listing(&f.member.dir) == before, "member files changed");
        assert_eq!(
            fs::read(f.member.state_dir.join("save.json")).unwrap(),
            record
        );
        assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
        f.assert_clean();
        if scenario != "added" {
            assert!(!f.wrap.calls().iter().any(|call| {
                call["argv"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|arg| arg == "fetch")
            }));
        }
    }
}
#[test]
fn missing_changed_file_is_replaced() {
    let f = Fixture::new();
    fs::write(f.source.join("SKILL.md"), b"new bytes\n").unwrap();
    f.save("Studio");
    fs::remove_file(f.member.dir.join("SKILL.md")).unwrap();
    let result = f.take(&Privacy);
    assert!(
        matches!(result, Ok(Took::Updated { changed: 1, .. })),
        "{result:?}"
    );
    assert_eq!(
        fs::read(f.member.dir.join("SKILL.md")).unwrap(),
        b"new bytes\n"
    );
    f.assert_clean();
}
#[test]
fn rewritten_history_is_refused() {
    let f = Fixture::new();
    let remote = f.scratch.0.join("remote.git");
    let tree = common::pb6::git(&remote, &["rev-parse", "main^{tree}"]);
    let head = common::pb6::git(&remote, &["commit-tree", &tree, "-m", "Unrelated history"]);
    common::pb6::git(&remote, &["update-ref", "refs/heads/main", &head]);
    let before = listing(&f.member.dir);
    let old_head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let error = f.take(&Privacy).err().unwrap();
    assert_eq!(
        error.to_string(),
        "The copy saved in rowan/wren-assistant doesn't build on the version on this computer, so Voice took nothing."
    );
    assert!(listing(&f.member.dir) == before, "member files changed");
    assert_eq!(
        common::pb6::git(&f.backup(), &["rev-parse", "main"]),
        old_head
    );
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    f.assert_clean();
}
fn validation_refusal(scenario: &str) {
    use std::os::unix::fs::symlink;
    struct Hit;
    impl PrivacyCheck for Hit {
        fn hits(&self, _: &Path) -> Result<Vec<String>> {
            Ok(vec!["identity.md:1".into()])
        }
    }
    {
        let f = Fixture::new();
        let remote = f.scratch.0.join("remote.git");
        let work = f.scratch.0.join("edit");
        common::pb6::git(
            &f.scratch.0,
            &["clone", remote.to_str().unwrap(), work.to_str().unwrap()],
        );
        common::pb6::git(&work, &["config", "user.name", "Rowan Vale"]);
        common::pb6::git(&work, &["config", "user.email", "rowan@example.invalid"]);
        let folder = work.join("crew/wren");
        match scenario {
            "link" => symlink("identity.md", folder.join("link")).unwrap(),
            "state" => {
                let path = folder.join("member.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                value["state_dir"] = serde_json::json!("~/elsewhere");
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
            "privacy" => fs::write(folder.join("identity.md"), b"new identity\n").unwrap(),
            _ => unreachable!(),
        }
        common::pb6::git(&work, &["add", "crew"]);
        common::pb6::git(&work, &["commit", "-qm", "New version"]);
        common::pb6::git(&work, &["push", "-q", "origin", "main"]);
        let before = listing(&f.member.dir);
        let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
        let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
        let error = f
            .take(if scenario == "privacy" {
                &Hit
            } else {
                &Privacy
            })
            .err()
            .unwrap()
            .to_string();
        let expected = match scenario { "link" => voice_save::text::saved_copy_refused("rowan/wren-assistant", "link"), "state" => "The saved copy in rowan/wren-assistant has a member.json Voice can't use, so it opened nothing.".into(), "privacy" => voice_save::text::saved_copy_private("rowan/wren-assistant", "identity.md:1"), _ => unreachable!() };
        assert_eq!(error, expected, "{scenario}");
        assert!(listing(&f.member.dir) == before, "member files changed");
        assert_eq!(
            fs::read(f.member.state_dir.join("save.json")).unwrap(),
            record
        );
        assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
        f.assert_clean();
    }
}
#[test]
fn builder_changes_only_for_a_valid_different_trailer() {
    let f = Fixture::new();
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    fs::write(f.source.join("SKILL.md"), b"second\n").unwrap();
    f.save("Studio");
    assert!(matches!(f.take(&Privacy), Ok(Took::Updated { .. })));
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    fs::write(f.source.join("SKILL.md"), b"third\n").unwrap();
    f.save("Library");
    assert!(matches!(f.take(&Privacy), Ok(Took::Updated { .. })));
    assert!(
        matches!(read_record(&f.member.state_dir).unwrap(), Some(SaveRecord::Github { builder: Some(builder), .. }) if builder == "Library")
    );
    f.assert_clean();
}
#[test]
fn fetch_into_incoming_keeps_main_and_rejects_arbitrary_refs() {
    let f = Fixture::new();
    let git = Git::at(f.wrap.program.clone());
    let old = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    fs::write(f.source.join("SKILL.md"), b"new\n").unwrap();
    f.save("Studio");
    let result = voice_save::git::fetch_main_into(
        &git,
        &f.backup(),
        &Secret::new("invented-token".into()),
        "refs/voice/incoming",
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), old);
    assert_ne!(
        common::pb6::git(&f.backup(), &["rev-parse", "refs/voice/incoming"]),
        old
    );
    f.wrap.clear();
    for bad in ["--upload-pack=other", "refs/heads/other", ""] {
        assert!(matches!(
            voice_save::git::fetch_main_into(
                &git,
                &f.backup(),
                &Secret::new("invented-token".into()),
                bad
            ),
            Err(voice_core::error::Error::Invalid(_))
        ));
    }
    assert!(f.wrap.calls().is_empty());
}

#[test]
fn changed_file_refuses() {
    local_refusal("changed");
}
#[test]
fn linked_file_refuses() {
    local_refusal("link");
}
#[test]
fn added_path_collision_refuses() {
    local_refusal("added");
}
#[test]
fn new_tree_link_refuses() {
    validation_refusal("link");
}
#[test]
fn new_tree_private_folder_mismatch_refuses() {
    validation_refusal("state");
}
#[test]
fn new_tree_privacy_hit_refuses() {
    validation_refusal("privacy");
}
#[test]
fn apply_failure_rolls_back_prior_files_and_preserves_record() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::write(f.source.join("SKILL.md"), b"new first file\n").unwrap();
    fs::write(f.source.join("old/a.md"), b"new second file\n").unwrap();
    f.save("Library");
    let parent = f.member.dir.join("old");
    let before = listing(&f.member.dir);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    let result = f.take(&Privacy);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err(), "{result:?}");
    assert!(listing(&f.member.dir) == before, "member files changed");
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
    f.assert_clean();
}

#[test]
fn builder_record_refuses_before_git_or_http() {
    nonmember_record(Some(SaveRecord::Github {
        owner: "rowan".into(),
        name: "wren-assistant".into(),
        branch: "main".into(),
        role: Role::Builder,
        builder: None,
    }));
}
#[test]
fn local_record_refuses_before_git_or_http() {
    nonmember_record(Some(SaveRecord::Local));
}
#[test]
fn absent_record_refuses_before_git_or_http() {
    nonmember_record(None);
}
#[test]
fn incoming_private_files_are_never_materialized() {
    let f = Fixture::new();
    fs::write(f.member.dir.join("private.json"), b"local private bytes\n").unwrap();
    let remote = f.scratch.0.join("remote.git");
    let work = f.scratch.0.join("edit");
    common::pb6::git(
        &f.scratch.0,
        &["clone", remote.to_str().unwrap(), work.to_str().unwrap()],
    );
    common::pb6::git(&work, &["config", "user.name", "Rowan Vale"]);
    common::pb6::git(&work, &["config", "user.email", "rowan@example.invalid"]);
    fs::write(
        work.join("crew/wren/private.json"),
        b"remote private bytes\n",
    )
    .unwrap();
    fs::write(work.join("crew/wren/.DS_Store"), b"remote finder bytes\n").unwrap();
    common::pb6::git(&work, &["add", "-f", "crew"]);
    common::pb6::git(
        &work,
        &[
            "commit",
            "-qm",
            "Ignored private files\n\nSaved-on: invalid@name",
        ],
    );
    common::pb6::git(&work, &["push", "-q", "origin", "main"]);
    let before = listing(&f.member.dir);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let result = f.take(&Privacy);
    assert!(
        matches!(result, Ok(Took::Updated { changed: 0, .. })),
        "{result:?}"
    );
    assert!(listing(&f.member.dir) == before, "member files changed");
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    f.assert_clean();
}

#[test]
fn record_write_failure_rolls_back_files_and_main() {
    let f = Fixture::new();
    fs::write(
        f.member
            .dir
            .join(format!(".voice-rollback-{}-0", std::process::id())),
        b"another unrelated local file\n",
    )
    .unwrap();
    fs::write(
        f.member
            .dir
            .join(format!(".voice-rollback-{}", std::process::id())),
        b"unrelated local file\n",
    )
    .unwrap();
    fs::write(f.source.join("SKILL.md"), b"new skill\n").unwrap();
    f.save("Library");
    let before = listing(&f.member.dir);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    let obstruction = f
        .member
        .state_dir
        .join(format!("save.json.tmp-{}", std::process::id()));
    fs::write(&obstruction, b"keep obstruction\n").unwrap();
    assert!(f.take(&Privacy).is_err());
    assert!(listing(&f.member.dir) == before, "member files changed");
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
    assert_eq!(fs::read(obstruction).unwrap(), b"keep obstruction\n");
    f.assert_clean();
}

#[test]
fn missing_unchanged_file_is_restored_without_counting_as_remote_change() {
    let f = Fixture::new();
    let original = fs::read(f.member.dir.join("SKILL.md")).unwrap();
    fs::write(f.source.join("identity.md"), b"new identity\n").unwrap();
    f.save("Studio");
    fs::remove_file(f.member.dir.join("SKILL.md")).unwrap();
    let result = f.take(&Privacy);
    assert!(
        matches!(result, Ok(Took::Updated { changed: 1, .. })),
        "{result:?}"
    );
    assert_eq!(fs::read(f.member.dir.join("SKILL.md")).unwrap(), original);
    f.assert_clean();
}

#[test]
fn update_sentences_are_recognized_and_scrub_controls() {
    use voice_save::text;
    assert_eq!(
        text::not_a_member("Wr\nen\r"),
        "This computer saves Wren, so there's no newer version to take here."
    );
    assert_eq!(
        text::changed_here("Wr\nen", "notes/\tb.md"),
        "notes/b.md in Wren's folder changed on this computer, so Voice took nothing. Put it back as it was, or move it somewhere else, then try again."
    );
    assert_eq!(
        text::not_newer("rowan/\rwren-assistant"),
        "The copy saved in rowan/wren-assistant doesn't build on the version on this computer, so Voice took nothing."
    );
    for message in [
        text::not_a_member("Wren"),
        text::changed_here("Wren", "SKILL.md"),
        text::not_newer("rowan/wren-assistant"),
    ] {
        assert!(text::is_said(&message), "{message}");
    }
}

#[test]
fn rollback_preserves_an_existing_empty_parent_of_a_new_file() {
    let f = Fixture::new();
    fs::create_dir(f.member.dir.join("notes")).unwrap();
    fs::create_dir(f.source.join("notes")).unwrap();
    fs::write(f.source.join("notes/b.md"), b"new note\n").unwrap();
    f.save("Library");
    let before = listing(&f.member.dir);
    let obstruction = f
        .member
        .state_dir
        .join(format!("save.json.tmp-{}", std::process::id()));
    fs::write(obstruction, b"keep obstruction\n").unwrap();
    assert!(f.take(&Privacy).is_err());
    assert!(
        listing(&f.member.dir) == before,
        "rollback removed a pre-existing folder"
    );
    f.assert_clean();
}

#[test]
fn missing_git_is_reported_before_origin_check() {
    let f = Fixture::new();
    let before = listing(&f.member.dir);
    let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let git = Git::at(f.home.join("missing-git"));
    let http = FakeHttp::new(vec![]);
    let github = Github::new(
        &http,
        voice_save::github::Endpoints::github(),
        "Iv1.invented",
    )
    .unwrap();
    let error = Saver {
        git: &git,
        github: Some(&github),
        tokens: &Tokens,
    }
    .take_newest(&f.home, &f.member, "2026-11-13-120001", &Privacy)
    .unwrap_err();
    assert_eq!(error.to_string(), voice_save::text::NEEDS_GIT);
    assert!(http.requests.borrow().is_empty());
    assert_eq!(listing(&f.member.dir), before);
    assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
}

#[test]
fn mismatched_origin_is_refused_before_http_or_fetch() {
    let f = Fixture::new();
    common::pb6::git(
        &f.backup(),
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/rowan/other-assistant.git",
        ],
    );
    let before = listing(&f.member.dir);
    let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let git = Git::at(f.wrap.program.clone());
    let http = FakeHttp::new(vec![]);
    let github = Github::new(
        &http,
        voice_save::github::Endpoints::github(),
        "Iv1.invented",
    )
    .unwrap();
    let error = Saver {
        git: &git,
        github: Some(&github),
        tokens: &Tokens,
    }
    .take_newest(&f.home, &f.member, "2026-11-13-120001", &Privacy)
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "saved copy origin doesn't match the record"
    );
    assert!(!voice_save::text::is_said(&error.to_string()));
    assert!(http.requests.borrow().is_empty());
    assert!(!f.wrap.calls().iter().any(|call| {
        call["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "fetch")
    }));
    assert_eq!(listing(&f.member.dir), before);
    assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    f.assert_clean();
}

#[test]
fn non_folder_member_is_refused_before_git_or_http() {
    use std::os::unix::fs::symlink;
    for kind in ["link", "file", "missing"] {
        let f = Fixture::new();
        let moved = f.home.join("moved-member");
        fs::rename(&f.member.dir, &moved).unwrap();
        match kind {
            "link" => symlink(&moved, &f.member.dir).unwrap(),
            "file" => fs::write(&f.member.dir, b"keep this file").unwrap(),
            "missing" => {}
            _ => unreachable!(),
        }
        // Loading after the link exists canonicalizes dir to the moved folder.
        let reloaded = (kind == "link")
            .then(|| MemberPaths::load(&f.member.dir.join("member.json"), &f.home).unwrap());
        let member = reloaded.as_ref().unwrap_or(&f.member);
        let before = listing(&moved);
        let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
        let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
        let git = Git::at(f.wrap.program.clone());
        let http = FakeHttp::new(vec![]);
        let github = Github::new(
            &http,
            voice_save::github::Endpoints::github(),
            "Iv1.invented",
        )
        .unwrap();
        let error = Saver {
            git: &git,
            github: Some(&github),
            tokens: &Tokens,
        }
        .take_newest(&f.home, member, "2026-11-13-120001", &Privacy)
        .unwrap_err();
        assert!(
            matches!(error, voice_core::error::Error::Invalid(_)),
            "{error:?}"
        );
        assert!(!voice_save::text::is_said(&error.to_string()));
        assert!(http.requests.borrow().is_empty());
        assert!(f.wrap.calls().is_empty());
        assert_eq!(listing(&moved), before);
        assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
        assert_eq!(
            fs::read(f.member.state_dir.join("save.json")).unwrap(),
            record
        );
        match kind {
            "link" => fs::remove_file(&f.member.dir).unwrap(),
            "file" => {
                assert_eq!(fs::read(&f.member.dir).unwrap(), b"keep this file");
                fs::remove_file(&f.member.dir).unwrap();
            }
            "missing" => assert!(!f.member.dir.exists()),
            _ => unreachable!(),
        }
        fs::rename(moved, &f.member.dir).unwrap();
        f.assert_clean();
    }
}

#[test]
fn record_write_failure_restores_removed_file_and_parent() {
    let f = Fixture::new();
    fs::write(f.source.join("SKILL.md"), b"new skill\n").unwrap();
    fs::remove_file(f.source.join("old/a.md")).unwrap();
    f.save("Library");
    let before = listing(&f.member.dir);
    let record = fs::read(f.member.state_dir.join("save.json")).unwrap();
    let head = common::pb6::git(&f.backup(), &["rev-parse", "main"]);
    let obstruction = f
        .member
        .state_dir
        .join(format!("save.json.tmp-{}", std::process::id()));
    fs::write(&obstruction, b"keep obstruction\n").unwrap();
    assert!(f.take(&Privacy).is_err());
    assert_eq!(listing(&f.member.dir), before);
    assert_eq!(common::pb6::git(&f.backup(), &["rev-parse", "main"]), head);
    assert_eq!(
        fs::read(f.member.state_dir.join("save.json")).unwrap(),
        record
    );
    assert_eq!(fs::read(obstruction).unwrap(), b"keep obstruction\n");
    f.assert_clean();
}
