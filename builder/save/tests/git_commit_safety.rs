pub mod common;
use common::{
    Scratch,
    pb6::{Wrapper, git, init},
};
use persona_builder_save::git::{Env, Git, Identity, commit_folder};
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn user_commit_ignores_configured_gpg_signing() {
    let scratch = Scratch::new("commit-signing");
    let repo = scratch.0.join("repo");
    init(&repo, false);
    let signer = scratch.0.join("failing-gpg");
    fs::write(&signer, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&signer, fs::Permissions::from_mode(0o755)).unwrap();
    git(&repo, &["config", "commit.gpgsign", "true"]);
    git(&repo, &["config", "gpg.program", signer.to_str().unwrap()]);
    let folder = scratch.0.join("member");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("model.md"), "A fictional assistant.\n").unwrap();
    let wrapper = Wrapper::new(&scratch.0, false);
    let result = commit_folder(
        &Git::at(wrapper.program.clone()),
        Env::User,
        &repo.join(".git"),
        "persona/wren",
        &folder,
        "crew/wren",
        &Identity {
            name: "Example Builder".into(),
            email: "builder@example.invalid".into(),
        },
        "Save assistant",
    );
    assert!(
        result.is_ok(),
        "commit must bypass signing: {:?}",
        result.err()
    );
    let calls = wrapper.calls();
    let commit = calls
        .iter()
        .find(|call| {
            call["argv"]
                .as_array()
                .unwrap()
                .iter()
                .any(|arg| arg == "commit-tree")
        })
        .unwrap();
    assert!(
        commit["argv"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "--no-gpg-sign"),
        "commit-tree must explicitly disable signing: {}",
        commit["argv"]
    );
    let committed = result.unwrap();
    assert!(committed.changed);
    assert_eq!(git(&repo, &["rev-parse", "persona/wren"]), committed.commit);
    assert_eq!(
        git(&repo, &["show", "persona/wren:crew/wren/model.md"]),
        "A fictional assistant."
    );
}
