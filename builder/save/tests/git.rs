pub mod common;
use common::{
    Scratch,
    pb6::{Wrapper, file_url, git, git_output, init, seed},
};
use persona_builder_save::{
    git::{self as saving, Env, Git, Identity},
    secret::Secret,
    text,
};
use std::{
    fs,
    os::unix::{ffi::OsStringExt, fs::symlink},
    path::Path,
};

fn who() -> Identity {
    Identity {
        name: "rowan-vale".into(),
        email: "4242+rowan-vale@users.noreply.github.com".into(),
    }
}
fn commit(git: &Git, repo: &Path, folder: &Path) -> saving::Committed {
    saving::commit_folder(
        git,
        Env::Own,
        repo,
        "main",
        folder,
        "crew/wren",
        &who(),
        "Save Wren",
    )
    .unwrap()
}
fn files(repo: &Path, branch: &str) -> String {
    git(repo, &["ls-tree", "-r", "--name-only", branch])
}
fn source(root: &Path) -> std::path::PathBuf {
    let folder = root.join("member");
    fs::create_dir_all(folder.join("nested")).unwrap();
    for (name, body) in [
        ("a.md", "first\n"),
        ("nested/b.md", "nested\n"),
        ("private.json", "private"),
        (".DS_Store", "metadata"),
        ("nested/private.json", "private"),
        ("nested/.DS_Store", "metadata"),
    ] {
        fs::write(folder.join(name), body).unwrap();
    }
    folder
}

#[test]
fn finds_only_repos_and_preserves_remote_order_and_push_rewrites() {
    let scratch = Scratch::new("find-repo");
    let wrapper = Wrapper::new(&scratch.0, false);
    // Prove the test wrapper itself runs before reaching a PB6 stub.
    let probe = std::process::Command::new(&wrapper.program)
        .env_clear()
        .env("LC_ALL", "C")
        .arg("--version")
        .output()
        .unwrap();
    assert!(probe.status.success(), "wrapper preflight failed");
    assert_eq!(wrapper.calls()[0]["argv"], serde_json::json!(["--version"]));
    wrapper.clear();
    let saving = Git::at(wrapper.program.clone());
    saving.check_version().unwrap();
    assert_eq!(saving.find_repo(&scratch.0).unwrap(), None);
    assert_eq!(saving.find_repo(&scratch.0.join("missing")).unwrap(), None);
    let repo = scratch.0.join("repo");
    init(&repo, false);
    fs::create_dir(repo.join("sub")).unwrap();
    git(
        &repo,
        &["remote", "add", "zeta", "git@github.com:rowan-vale/z.git"],
    );
    git(
        &repo,
        &[
            "remote",
            "add",
            "alpha",
            "https://github.com/rowan-vale/a.git",
        ],
    );
    git(
        &repo,
        &[
            "config",
            "url.file:///invented/.pushInsteadOf",
            "https://github.com/",
        ],
    );
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "--add",
            "--push",
            "zeta",
            "git@github.com:rowan-vale/second.git",
        ],
    );
    let found = saving.find_repo(&repo.join("sub")).unwrap().unwrap();
    assert_eq!(found.root, repo.canonicalize().unwrap());
    assert_eq!(found.git_dir, repo.join(".git").canonicalize().unwrap());
    assert_eq!(
        found.remotes,
        vec![
            saving::Remote {
                name: "alpha".into(),
                push_urls: vec!["file:///invented/rowan-vale/a.git".into()]
            },
            saving::Remote {
                name: "zeta".into(),
                push_urls: vec!["git@github.com:rowan-vale/second.git".into()]
            },
        ]
    );
}

#[test]
fn commits_exact_files_reuses_unchanged_commit_and_removes_stale_files() {
    let scratch = Scratch::new("commit-bare");
    let repo = scratch.0.join("backup.git");
    init(&repo, true);
    let folder = source(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let first = commit(&saving, &repo, &folder);
    assert!(first.changed);
    assert_eq!(
        files(&repo, "main"),
        "crew/wren/a.md\ncrew/wren/nested/b.md"
    );
    assert_eq!(git(&repo, &["show", "main:crew/wren/a.md"]), "first");
    assert_eq!(
        git(&repo, &["show", "main:crew/wren/nested/b.md"]),
        "nested"
    );
    assert_eq!(
        git(
            &repo,
            &["show", "-s", "--format=%an <%ae>|%cn <%ce>|%s", "main"]
        ),
        "rowan-vale <4242+rowan-vale@users.noreply.github.com>|rowan-vale <4242+rowan-vale@users.noreply.github.com>|Save Wren"
    );
    let witness = scratch.0.join("index-witness");
    assert!(
        fs::symlink_metadata(&witness)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        !witness.exists(),
        "temporary index directory survived success"
    );
    fs::remove_file(&witness).unwrap();
    let second = commit(&saving, &repo, &folder);
    assert!(
        fs::symlink_metadata(&witness)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        !witness.exists(),
        "temporary index directory survived no-change return"
    );
    assert!(!second.changed);
    assert_eq!(second.commit, first.commit);
    // Seed unrelated content on the same branch through a real checkout.
    let checkout = scratch.0.join("checkout");
    git(
        &scratch.0,
        &[
            "clone",
            "-q",
            repo.to_str().unwrap(),
            checkout.to_str().unwrap(),
        ],
    );
    git(&checkout, &["config", "user.name", "Rowan Vale"]);
    git(
        &checkout,
        &["config", "user.email", "rowan@example.invalid"],
    );
    fs::create_dir(checkout.join("other")).unwrap();
    fs::write(checkout.join("other/keep.md"), "unrelated\n").unwrap();
    git(&checkout, &["add", "other/keep.md"]);
    git(&checkout, &["commit", "-qm", "Keep unrelated"]);
    git(&checkout, &["push", "-q", "origin", "main"]);
    fs::remove_file(folder.join("nested/b.md")).unwrap();
    fs::write(folder.join("a.md"), "edited\n").unwrap();
    let third = commit(&saving, &repo, &folder);
    assert!(third.changed);
    assert_eq!(files(&repo, "main"), "crew/wren/a.md\nother/keep.md");
    assert_eq!(git(&repo, &["show", "main:crew/wren/a.md"]), "edited");
    assert_eq!(git(&repo, &["show", "main:other/keep.md"]), "unrelated");
}

#[test]
fn leaves_the_persons_head_index_and_dirty_worktree_byte_for_byte() {
    let scratch = Scratch::new("commit-user");
    let repo = scratch.0.join("repo");
    init(&repo, false);
    seed(&repo);
    fs::write(repo.join("keep.md"), "staged\n").unwrap();
    git(&repo, &["add", "keep.md"]);
    fs::write(repo.join("keep.md"), "unstaged\n").unwrap();
    fs::write(repo.join("untracked.md"), "untracked\n").unwrap();
    let head = fs::read(repo.join(".git/HEAD")).unwrap();
    let oid = git(&repo, &["rev-parse", "HEAD"]);
    let status = git_output(&repo, &["status", "--porcelain"]).stdout;
    let index = fs::read(repo.join(".git/index")).unwrap();
    let folder = source(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    saving::commit_folder(
        &Git::at(wrapper.program),
        Env::User,
        &repo.join(".git"),
        "persona/wren",
        &folder,
        "crew/wren",
        &who(),
        "Save Wren",
    )
    .unwrap();
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(repo.join(".git/HEAD")).unwrap(), head);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), oid);
    assert_eq!(git_output(&repo, &["status", "--porcelain"]).stdout, status);
    assert_eq!(fs::read(repo.join("keep.md")).unwrap(), b"unstaged\n");
    assert_eq!(fs::read(repo.join("untracked.md")).unwrap(), b"untracked\n");
}

#[test]
fn refuses_nested_symlinks_even_when_named_as_an_excluded_file() {
    let scratch = Scratch::new("commit-link");
    let repo = scratch.0.join("backup.git");
    init(&repo, true);
    let folder = source(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program);
    for name in ["nested/link", "nested/private.json"] {
        let path = folder.join(name);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        symlink(scratch.0.join("outside"), &path).unwrap();
        assert_eq!(
            saving::commit_folder(
                &saving,
                Env::Own,
                &repo,
                "main",
                &folder,
                "crew/wren",
                &who(),
                "Save Wren"
            )
            .err()
            .unwrap()
            .to_string(),
            text::links_in_folder(name)
        );
        fs::remove_file(path).unwrap();
    }
    assert!(
        !git_output(&repo, &["rev-parse", "--verify", "refs/heads/main"])
            .status
            .success()
    );
}

#[test]
fn refuses_non_utf8_newline_control_and_backslash_names() {
    let scratch = Scratch::new("commit-names");
    let repo = scratch.0.join("backup.git");
    init(&repo, true);
    let folder = source(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program);
    for bytes in [
        b"bad\xff".to_vec(),
        b"bad\nname".to_vec(),
        b"bad\tname".to_vec(),
        b"bad\\name".to_vec(),
    ] {
        let path = folder.join(std::ffi::OsString::from_vec(bytes));
        if fs::write(&path, "refuse").is_err() {
            continue;
        }
        assert_eq!(
            saving::commit_folder(
                &saving,
                Env::Own,
                &repo,
                "main",
                &folder,
                "crew/wren",
                &who(),
                "Save Wren"
            )
            .err()
            .unwrap()
            .to_string(),
            "unsupported file name"
        );
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn refuses_branch_and_prefix_traversal_before_changing_refs() {
    let scratch = Scratch::new("commit-refs");
    let repo = scratch.0.join("backup.git");
    init(&repo, true);
    let folder = source(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program);
    for (branch, prefix) in [
        ("persona/../x", "crew/wren"),
        ("other", "crew/wren"),
        ("persona/Wren", "crew/wren"),
        ("main", "crew/../x"),
        ("main", "/crew/wren"),
        ("main", "crew/wren/sub"),
    ] {
        assert_eq!(
            saving::commit_folder(
                &saving,
                Env::Own,
                &repo,
                branch,
                &folder,
                prefix,
                &who(),
                "Save Wren"
            )
            .err()
            .unwrap()
            .to_string(),
            "bad branch or folder"
        );
    }
    assert!(git_output(&repo, &["show-ref"]).stdout.is_empty());
}

#[test]
fn pushes_real_branch_with_only_allowed_environment_and_no_credentials_in_argv() {
    let scratch = Scratch::new("push");
    let source = scratch.0.join("source");
    init(&source, false);
    seed(&source);
    let remote = scratch.0.join("remote.git");
    init(&remote, true);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());
    let token = Secret::new("invented-token".into());
    saving::push(
        &saving,
        Env::Own,
        &source.join(".git"),
        &file_url(&remote),
        "main",
        Some(&token),
    )
    .unwrap();
    assert_eq!(
        git(&remote, &["rev-parse", "main"]),
        git(&source, &["rev-parse", "main"])
    );
    let calls = wrapper.calls();
    assert_eq!(calls.len(), 1);
    let argv = calls[0]["argv"].as_array().unwrap();
    assert_eq!(argv[0], "-c");
    assert_eq!(argv[1], "credential.helper=");
    assert_eq!(argv.last().unwrap(), "refs/heads/main:refs/heads/main");
    assert!(
        argv.iter()
            .all(|arg| !arg.as_str().unwrap().contains(token.expose())
                && !arg.as_str().unwrap().contains("--force"))
    );
    let names = calls[0]["env"].as_array().unwrap();
    assert!(names.iter().any(|v| v == "GIT_CONFIG_VALUE_0"));
    for name in names {
        assert!(
            [
                "HOME",
                "PATH",
                "LC_ALL",
                "GIT_TERMINAL_PROMPT",
                "GIT_CONFIG_NOSYSTEM",
                "GIT_CONFIG_GLOBAL",
                "GIT_CONFIG_COUNT",
                "GIT_CONFIG_KEY_0",
                "GIT_CONFIG_VALUE_0"
            ]
            .contains(&name.as_str().unwrap()),
            "unexpected environment name: {name}"
        );
    }
    wrapper.clear();
    saving::push(
        &saving,
        Env::User,
        &source.join(".git"),
        &file_url(&remote),
        "main",
        None,
    )
    .unwrap();
    let calls = wrapper.calls();
    assert_eq!(calls.len(), 1);
    for name in calls[0]["env"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        assert!(!name.starts_with("GIT_CONFIG_"));
        assert!(
            [
                "HOME",
                "PATH",
                "LC_ALL",
                "GIT_TERMINAL_PROMPT",
                "SSH_AUTH_SOCK"
            ]
            .contains(&name)
        );
    }
    if std::env::var_os("SSH_AUTH_SOCK").is_some() {
        assert!(
            calls[0]["env"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "SSH_AUTH_SOCK")
        );
    }
}

#[test]
fn failing_push_hides_token_and_base64_credential_and_uses_last_line() {
    let scratch = Scratch::new("push-redaction");
    let wrapper = Wrapper::new(&scratch.0, true);
    let saving = Git::at(wrapper.program);
    let token = Secret::new("invented-token".into());
    let error = saving::push(
        &saving,
        Env::Own,
        &scratch.0,
        "missing",
        "main",
        Some(&token),
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        text::push_failed("fatal: [hidden] [hidden]")
    );
}

#[test]
fn failed_commit_removes_its_private_index_directory() {
    let scratch = Scratch::new("commit-cleanup");
    let repo = scratch.0.join("backup.git");
    init(&repo, true);
    let folder = source(&scratch.0);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program);
    let invalid_identity = Identity {
        name: "".into(),
        email: "".into(),
    };
    let error = saving::commit_folder(
        &saving,
        Env::Own,
        &repo,
        "main",
        &folder,
        "crew/wren",
        &invalid_identity,
        "Save Wren",
    )
    .err()
    .unwrap();
    assert_eq!(error.to_string(), "git commit-tree failed");
    let witness = scratch.0.join("index-witness");
    assert!(
        fs::symlink_metadata(&witness)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        !witness.exists(),
        "temporary index directory survived git failure"
    );
    assert!(git_output(&repo, &["show-ref"]).stdout.is_empty());
}

#[test]
fn folder_files_uses_component_order_and_skips_only_save_exclusions() {
    let scratch = Scratch::new("folder-files");
    let folder = source(&scratch.0);
    fs::create_dir(folder.join("a")).unwrap();
    fs::write(folder.join("a/b.md"), "nested").unwrap();
    fs::write(folder.join("a-b.md"), "flat").unwrap();
    assert_eq!(
        saving::folder_files(&folder).unwrap(),
        ["a/b.md", "a-b.md", "a.md", "nested/b.md"].map(std::path::PathBuf::from)
    );
}
#[test]
fn folder_files_refuses_links_even_with_excluded_names() {
    let scratch = Scratch::new("folder-files-link");
    let folder = source(&scratch.0);
    fs::remove_file(folder.join("nested/private.json")).unwrap();
    symlink(
        scratch.0.join("outside"),
        folder.join("nested/private.json"),
    )
    .unwrap();
    assert_eq!(
        saving::folder_files(&folder).err().map(|e| e.to_string()),
        Some(text::links_in_folder("nested/private.json"))
    );
}

#[test]
fn failing_push_strips_url_userinfo_before_showing_the_detail() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("push-userinfo");
    let program = scratch.0.join("failing-git");
    for (detail, shown) in [
        (
            "fatal: unable to access 'https://someone:invented-pat@github.com/x/y.git/': error",
            "fatal: unable to access 'https://github.com/x/y.git/': error",
        ),
        (
            "fatal: https://invented-token@github.com/x",
            "fatal: https://github.com/x",
        ),
        (
            "fatal: https://invented-token@github.com",
            "fatal: https://github.com",
        ),
        (
            "fatal: https://invented-token@github.com error",
            "fatal: https://github.com error",
        ),
        (
            "fatal: https://first@github.com/x https://second@github.com/y",
            "fatal: https://github.com/x https://github.com/y",
        ),
        (
            "fatal: https://github.com/x@y",
            "fatal: https://github.com/x@y",
        ),
        (
            "fatal: https://github.com error@host",
            "fatal: https://github.com error@host",
        ),
        ("fatal: https://github.com/x", "fatal: https://github.com/x"),
    ] {
        fs::write(
            &program,
            format!("#!/bin/sh\ncat <<'DETAIL' >&2\n{detail}\nDETAIL\nexit 1\n"),
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let error = saving::push(
            &Git::at(program.clone()),
            Env::User,
            &scratch.0,
            "missing",
            "main",
            None,
        )
        .unwrap_err();
        assert_eq!(error.to_string(), text::push_failed(shown));
    }
}
