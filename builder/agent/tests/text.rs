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
    let (not_installed, signed_out) = if !cfg!(target_os = "macos") {
        (
            "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with an Anthropic Console account, which is billed for what it uses (API billing).",
            "Claude Code isn't signed in for Voice. Sign in with your Anthropic Console account on Connect Claude, or with `voice claude sign-in`, then try again.",
        )
    } else if voice_agent::claude::SUBSCRIPTION_OFFERED {
        (
            "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with your Claude subscription, so what it does uses your plan.",
            "Claude Code isn't signed in for Voice. Sign in with your Claude subscription on Connect Claude, or with `voice claude sign-in --subscription`, then try again.",
        )
    } else {
        (
            "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
            "Claude Code isn't signed in for Voice. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
        )
    };
    assert_eq!(text::NOT_INSTALLED, not_installed);
    assert_eq!(text::SIGNED_OUT, signed_out);
    assert_eq!(
        text::SIGN_IN_WAITING,
        "Finish signing in on the page that opened in your browser. Voice carries on when you're done."
    );
    assert_eq!(
        text::SIGN_IN_FAILED,
        "Claude Code didn't finish signing in. Try again. If no sign-in page opened in your browser, check that this computer has a default browser."
    );
}

#[test]
fn the_signed_out_text_names_the_sign_in_this_build_can_start() {
    let (expected, cli) = if !cfg!(target_os = "macos") {
        (
            "Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with an Anthropic Console account.",
            Some("Run voice claude sign-in to sign in."),
        )
    } else if voice_agent::claude::SUBSCRIPTION_OFFERED {
        (
            "Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself, so sign in once with your Claude subscription.",
            Some("Run voice claude sign-in --subscription to sign in."),
        )
    } else {
        (
            "Claude Code 2.1.292 at ~/bin/claude isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
            None,
        )
    };
    assert_eq!(text::signed_out_at("2.1.292", "~/bin/claude"), expected);
    assert_eq!(text::sign_in_cli(), cli);
}

#[test]
fn the_claude_code_messages_name_the_sign_in_this_build_can_start() {
    let (not_installed, signed_out, subscription_refused, subscription_not_offered) = if !cfg!(
        target_os = "macos"
    ) {
        (
            "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with an Anthropic Console account, which is billed for what it uses (API billing).",
            "Claude Code isn't signed in for Voice. Sign in with your Anthropic Console account on Connect Claude, or with `voice claude sign-in`, then try again.",
            "Claude Code is signed in for Voice with a Claude subscription, which this copy of Voice can't use. Sign in again with an Anthropic Console account on Connect Claude, or with `voice claude sign-in`.",
            "This copy of Voice signs in with an Anthropic Console account only.",
        )
    } else if voice_agent::claude::SUBSCRIPTION_OFFERED {
        (
            "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with your Claude subscription, so what it does uses your plan.",
            "Claude Code isn't signed in for Voice. Sign in with your Claude subscription on Connect Claude, or with `voice claude sign-in --subscription`, then try again.",
            "Claude Code is signed in for Voice with a Claude subscription, which this copy of Voice can't use. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
            "This copy of Voice can't sign in with a Claude subscription. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
        )
    } else {
        (
            "Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
            "Claude Code isn't signed in for Voice. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
            "Claude Code is signed in for Voice with a Claude subscription, which this copy of Voice can't use. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
            "This copy of Voice can't sign in with a Claude subscription. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.",
        )
    };
    assert_eq!(text::NOT_INSTALLED, not_installed);
    assert_eq!(text::SIGNED_OUT, signed_out);
    assert_eq!(text::SUBSCRIPTION_REFUSED, subscription_refused);
    assert_eq!(text::SUBSCRIPTION_NOT_OFFERED, subscription_not_offered);
}
