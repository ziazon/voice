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
