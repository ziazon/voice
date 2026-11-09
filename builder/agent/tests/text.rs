use voice_agent::text;

#[test]
fn the_wording_says_this_computer_never_this_mac() {
    let source = include_str!("../src/text.rs");
    assert!(!source.contains("this Mac"), "a line still says this Mac");
    assert!(!source.contains("your Mac"), "a line still says your Mac");
    for line in [text::NOT_INSTALLED, text::DOES_NOT_RUN, text::PROXY_FAILED] {
        assert!(line.contains("this computer"), "{line}");
    }
}

#[test]
fn console_sign_in_uses_the_approved_words() {
    assert_eq!(
        text::NOT_INSTALLED,
        "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with an Anthropic Console account, which is billed for what it uses (API billing)."
    );
    assert_eq!(
        text::SIGNED_OUT,
        "Claude Code isn't signed in for Voice. Sign in with your Anthropic Console account on Connect Claude, or with `voice claude sign-in`, then try again."
    );
    assert_eq!(
        text::SIGN_IN_WAITING,
        "Finish signing in on the page that opened in your browser. Voice carries on when you're done."
    );
    assert_eq!(
        text::SIGN_IN_FAILED,
        "Claude Code didn't finish signing in. Try again. If no sign-in page opened in your browser, check that this computer has a default browser."
    );
    assert!(
        text::signed_out_at("2.1.292", "~/bin/claude")
            .ends_with("so sign in once with an Anthropic Console account.")
    );
}
