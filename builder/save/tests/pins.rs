pub mod common;

use common::{
    FakeHttp, Scratch,
    pb6::{Wrapper, file_url, git, init, member, seed},
};
use persona_builder_save::{
    flow::{self, GithubRemotes, MemberPaths, Saver},
    git::{self as saving, Env, Git, Identity},
    github::{Endpoints, Github},
    keychain::{MemoryStore, TokenStore},
    secret::Secret,
    text,
};

#[test]
fn own_env_turns_off_the_persons_git_config() {
    let scratch = Scratch::new("pins-own-env");
    let source = scratch.0.join("source");
    init(&source, false);
    seed(&source);
    let remote = scratch.0.join("remote.git");
    init(&remote, true);
    let wrapper = Wrapper::new(&scratch.0, false);
    let saving = Git::at(wrapper.program.clone());

    for (env, isolated) in [(Env::Own, true), (Env::User, false)] {
        wrapper.clear();
        saving::push(
            &saving,
            env,
            &source.join(".git"),
            &file_url(&remote),
            "main",
            None,
        )
        .unwrap();
        let calls = wrapper.calls();
        assert_eq!(calls.len(), 1);
        let names = calls[0]["env"].as_array().unwrap();
        for name in ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM"] {
            assert_eq!(names.iter().any(|value| value == name), isolated, "{name}");
        }
    }

    let backup = scratch.0.join("backup.git");
    init(&backup, true);
    let folder = scratch.0.join("member/crew/wren");
    member(&folder);
    wrapper.clear();
    saving::commit_folder(
        &saving,
        Env::Own,
        &backup,
        "main",
        &folder,
        "crew/wren",
        &Identity {
            name: "rowan-vale".into(),
            email: "4242+rowan-vale@users.noreply.github.com".into(),
        },
        "Save Wren",
    )
    .unwrap();
    let calls = wrapper.calls();
    assert!(!calls.is_empty());
    for call in calls {
        let names = call["env"].as_array().unwrap();
        for name in ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM"] {
            assert!(names.iter().any(|value| value == name), "missing {name}");
        }
    }
}

#[test]
fn choose_existing_refuses_a_repo_detect_did_not_offer() {
    let scratch = Scratch::new("pins-not-offered");
    let home = scratch.0.canonicalize().unwrap();
    let setup = home.join(".claude");
    let other = home.join("other");
    for repo in [&setup, &other] {
        init(repo, false);
        git(
            repo,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/rowan-vale/setup.git",
            ],
        );
    }
    let dir = home.join("member/crew/wren");
    let member = MemberPaths {
        member: member(&dir),
        dir,
        state_dir: home.join(".claude/state/wren"),
    };
    let wrapper = Wrapper::new(&home, false);
    let saving = Git::at(wrapper.program.clone());
    assert!(saving.find_repo(&member.dir).unwrap().is_none());
    let found = flow::detect(&saving, &home, &member.dir).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].root, setup);
    let http = FakeHttp::new(vec![]);
    let github = Github::new(&http, Endpoints::github(), "Iv1.invented").unwrap();
    let tokens = MemoryStore::new();
    tokens.save(&Secret::new("invented-token".into())).unwrap();
    let result = Saver {
        git: &saving,
        github: Some(&github),
        tokens: &tokens,
    }
    .choose_existing(&member, &home, &other, &GithubRemotes { github: &github });
    assert_eq!(result.unwrap_err().to_string(), text::NOT_OFFERED);
    assert!(!member.state_dir.join("save.json").exists());
    assert!(http.requests.borrow().is_empty());
}
