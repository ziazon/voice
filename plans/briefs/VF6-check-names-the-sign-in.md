# VF6 brief: `voice claude check` names the sign-in this build can start

When Claude Code isn't signed in for Voice, `voice claude check` and the *Connect Claude* screen say
*…so sign in once with an Anthropic Console account.* and the CLI adds *Run voice claude sign-in to
sign in.* Since VF3 a Console sign-in is refused on a Mac (`claude::starts(SignIn::Console)` is false
there, and `sign_in` stops with `CONSOLE_NOT_AVAILABLE`). So on a Mac the text sends a person to a
sign-in that can't start. Make it name the sign-in this build can start, on this platform.

Both places take their sentence from one function, `voice_agent::text::signed_out_at`: the CLI in
`builder/cli/src/main.rs` `claude_check`, and the screen through the API's status message
(`builder/api/src/assistants.rs`, the not-signed-in arm, shown as-is in the screen's alert). So the
change is in the agent crate's text and the CLI; the screen follows with no UI change.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/agent/src/text.rs`,
`builder/agent/src/claude.rs` (`SignIn`, `SUBSCRIPTION_OFFERED`, `starts`), `builder/cli/src/main.rs`
(`claude_check`, and the `SignIn { subscription }` subcommand), and the tests named in section 2.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. **Commit nothing; leave
every change in the working tree.**

## 1. Decisions already made (do not change them)

1. **Three cases, in this order**, decided with `claude::starts(SignIn::Console)` and
   `claude::SUBSCRIPTION_OFFERED` (never a new `cfg` attribute):
   - **Console starts** (not a Mac): the text is unchanged:
     `…you use yourself, so sign in once with an Anthropic Console account.` and the CLI adds
     `Run voice claude sign-in to sign in.`
   - **Console can't start, the subscription is offered** (a Mac, a testers' build):
     `…you use yourself, so sign in once with your Claude subscription.` and the CLI adds
     `Run voice claude sign-in --subscription to sign in.`
   - **Neither** (a Mac, the public build):
     `…you use yourself. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.`
     and the CLI adds nothing (no command that would only be refused).

   The shared start, for every case, stays exactly:
   `Claude Code {version} at {program} isn't signed in for Voice yet. Voice keeps a sign-in of its own, separate from the Claude app and from the Claude Code you use yourself`
2. **In `builder/agent/src/text.rs`:** a private `fn sign_in_once() -> &'static str` returning the
   three tails above (`", so sign in once with an Anthropic Console account."`,
   `", so sign in once with your Claude subscription."`,
   `". Signing in with a Console account doesn't work on a Mac yet. A later update adds it."`);
   `signed_out_at` appends it to the shared start. Replace `pub const SIGN_IN_CLI` with
   `pub fn sign_in_cli() -> Option<&'static str>` returning the three CLI lines (`None` in the third
   case). One doc comment line on each function, matching the file's style. Leave every other
   constant in `text.rs` alone (`SIGNED_OUT`, `NOT_INSTALLED`, `CONSOLE_NOT_AVAILABLE` and the rest
   are out of scope for this slice).
3. **In `builder/cli/src/main.rs` `claude_check`:** the error is `signed_out_at(...)`, then, when
   `text::sign_in_cli()` is `Some(line)`, a space and `line`. Nothing else in the CLI changes.
4. No change to the API, the DTOs, the generated types or the UI source.

## 2. Tests first (red, then green)

Every test that pins this text expects the case for the platform and build it runs on, using
`cfg!(target_os = "macos")` and `voice_agent::claude::SUBSCRIPTION_OFFERED` to pick, and the
**literal** expected strings (never a call to `signed_out_at` or `sign_in_cli` to build the
expected value). Write these first and run them to see them fail on a Mac, then change the code.

1. `builder/agent/tests/text.rs`: replace the `ends_with(...)` check in
   `console_sign_in_uses_the_approved_words` with a new test,
   `the_signed_out_text_names_the_sign_in_this_build_can_start`, asserting the whole
   `signed_out_at("2.1.292", "~/bin/claude")` string and `sign_in_cli()` for the case that applies.
2. `builder/cli/tests/agent.rs` (the `claude check` block near line 135): the expected stderr is
   `voice: <signed-out text>[ <cli line>]\n` for the case that applies.
3. `builder/api/tests/assistants.rs` (the `("false", …)` row near line 261): the expected message is
   the signed-out text for the case that applies.
4. Any other test that breaks because it pinned the old text or `SIGN_IN_CLI`: update it the same way.

## 3. Checks you run

Your sandbox can't bind network listeners or run every sandbox test, so run what you can and say
which ones you could not run:
- `cargo fmt --all -- --check`
- `cargo test --locked -p voice-agent --test text`
- `cargo test --locked -p voice-agent --features claude-subscription --test text`
- `cargo test --locked -p voice-cli --test agent` and the same with `--features claude-subscription`
- `cargo test --locked -p voice-api --test assistants` and the same with `--features claude-subscription`
- `cargo clippy --all-targets --locked -- -D warnings`

Report: the diff, which tests failed before your code change (with the message) and passed after,
and anything you couldn't run.
