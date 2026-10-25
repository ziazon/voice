use persona_builder_save::text;

#[test]
fn the_wording_says_this_computer_never_this_mac() {
    let source = include_str!("../src/text.rs");
    assert!(!source.contains("this Mac"), "a line still says this Mac");
    assert!(!source.contains("MAC_ONLY"));
    for line in [
        text::CHOICE_LOCAL,
        text::NO_GITHUB_ACCOUNT,
        text::SIGNED_OUT,
        text::NOT_SET_UP,
        text::KEPT_LOCAL,
        text::CHOSE_LOCAL,
        text::NO_SAFE_STORE,
    ] {
        assert!(line.contains("this computer"), "{line}");
    }
    assert!(text::sign_in_failed("invented").contains("this computer only"));
}

#[test]
fn the_store_failure_names_no_os_store() {
    for word in ["Keychain", "Mac", "Windows", "Linux", "Credential Manager"] {
        assert!(!text::KEYCHAIN_FAILED.contains(word), "{word}");
    }
    assert!(text::KEYCHAIN_FAILED.contains("this computer's password store"));
    assert!(text::KEYCHAIN_FAILED.contains("keep your assistant on this computer only"));
}

#[test]
fn no_safe_store_offers_this_computer_only_and_no_file() {
    assert!(text::NO_SAFE_STORE.contains("keep your assistant on this computer only"));
}

#[test]
fn needs_git_gives_the_mac_hint_only_on_a_mac() {
    let mac = text::needs_git(true);
    assert!(mac.contains("xcode-select --install"));
    assert!(!mac.contains("git-scm.com"));
    let other = text::needs_git(false);
    assert!(other.contains("https://git-scm.com/downloads"));
    assert!(!other.contains("xcode-select"));
    assert!(!other.contains("Terminal app"));
    assert_eq!(text::NEEDS_GIT, text::needs_git(cfg!(target_os = "macos")));
}

#[test]
fn is_said_accepts_only_save_failure_sentences() {
    let constants = [
        text::NOT_SET_UP,
        text::SIGN_IN_CANCELLED,
        text::SIGN_IN_EXPIRED,
        text::SIGN_IN_FIRST,
        text::NO_REPO_PERMISSION,
        text::NOT_ON_GITHUB,
        text::BAD_REPO_NAME,
        text::NOT_OFFERED,
        text::CHOOSE_FIRST,
        text::NEEDS_GIT,
        text::GIT_TOO_OLD,
        text::NO_SAFE_STORE,
        text::KEYCHAIN_FAILED,
        text::GITHUB_UNREACHABLE,
    ];
    for message in constants {
        assert!(text::is_said(message), "{message}");
        assert!(!text::is_said(&format!("{message}\n")), "{message}");
    }
    let formatted = [
        (
            text::sign_in_failed("invented"),
            "GitHub sign-in didn't work (GitHub said: ",
        ),
        (text::repo_public("example/assistant"), ""),
        (
            text::repo_unconfirmed("example/assistant"),
            "Persona Builder couldn't confirm that ",
        ),
        (
            text::repo_name_taken("assistant"),
            "GitHub wouldn't make a repo called ",
        ),
        (
            text::privacy_hits("notes.md:1"),
            "The privacy check found something private, so nothing was sent. Look at: ",
        ),
        (
            text::links_in_folder("notes.md"),
            "Persona Builder won't save a folder that holds links to other places: ",
        ),
        (
            text::push_failed("try again"),
            "GitHub didn't take the save: ",
        ),
        (text::branch_in_use("main"), "The branch "),
    ];
    for (message, prefix) in formatted {
        assert!(text::is_said(&message), "{message}");
        assert!(!text::is_said(&format!("{message}\n")), "{message}");
        assert!(!text::is_said(prefix), "{prefix}");
    }
    for message in [
        "git push failed",
        "state folder is a link",
        "save.json: not a save record",
        "bad branch or folder",
        "unsupported file name",
        "not a GitHub address",
        "",
        "GitHub sign-in didn't work (GitHub said: ). Try again later, or keep your assistant on this computer only.",
        "The branch ma\nin is open in your repo right now, so Persona Builder didn't save. Switch to another branch, then try again.",
        "GitHub didn't take the save: try\ragain",
    ] {
        assert!(!text::is_said(message), "{message}");
    }
}
