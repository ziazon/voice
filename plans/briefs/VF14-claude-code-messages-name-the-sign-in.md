# VF14 brief: the other Claude Code messages name the sign-in this build can start

VF6 made `voice claude check`'s not-signed-in text name the sign-in this build can start. Four more
fixed sentences in `builder/agent/src/text.rs` still send a Mac user to the Anthropic Console sign-in,
and since VF3 that sign-in is refused on a Mac (`claude::starts(SignIn::Console)` is false there, and
`sign_in` stops with `CONSOLE_NOT_AVAILABLE`):

- `NOT_INSTALLED`: *…Voice signs it in with an Anthropic Console account, which is billed for what it uses (API billing).*
- `SIGNED_OUT`: *Sign in with your Anthropic Console account on Connect Claude, or with `voice claude sign-in`, then try again.*
- `SUBSCRIPTION_REFUSED`: *…Sign in again with an Anthropic Console account on Connect Claude, or with `voice claude sign-in`.*
- `SUBSCRIPTION_NOT_OFFERED`: *This copy of Voice signs in with an Anthropic Console account only.*

The CLI and the Connect Claude screen (through the API's allow-list, `builder/api/src/assistants.rs`
`claude_message`) show them as-is. Make each name the sign-in this build can start, on this platform.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/agent/src/text.rs` (note how
`CONSOLE_NOT_AVAILABLE` is already chosen per build with `#[cfg]`), `builder/agent/src/claude.rs`
(`SignIn`, `SUBSCRIPTION_OFFERED`, `starts`), `builder/api/src/assistants.rs` (`claude_message`), and
VF6's tests for the expected-text pattern: `builder/agent/tests/text.rs`
(`the_signed_out_text_names_the_sign_in_this_build_can_start`), `builder/cli/tests/agent.rs`.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. **Commit nothing; leave
every change in the working tree.**

## 1. Decisions already made (do not change them)

1. **The four stay `pub const …: &str`**, each defined once per build with `#[cfg]` attributes, the
   way `CONSOLE_NOT_AVAILABLE` is. Never turn them into functions: the API's allow-list compares
   against them, and about fifteen files use them by name; none of those uses change.
2. **The builds** (`claude::starts` and `claude::SUBSCRIPTION_OFFERED` decide the same thing; the
   attributes mirror them):
   - not a Mac: `#[cfg(not(target_os = "macos"))]`
   - a Mac, testers' build: `#[cfg(all(target_os = "macos", feature = "claude-subscription"))]`
   - a Mac, public build: `#[cfg(all(target_os = "macos", not(feature = "claude-subscription")))]`
3. **The exact sentences.** Off a Mac every sentence stays exactly as it is today.
   - `NOT_INSTALLED`, shared start for every build:
     `Voice works through your own Claude Code, and it isn't on this computer yet. Install it (https://code.claude.com/docs/en/setup), then try again.`
     then, off a Mac: ` Voice signs it in with an Anthropic Console account, which is billed for what it uses (API billing).` (today's text);
     a Mac, testers' build: ` Voice signs it in with your Claude subscription, so what it does uses your plan.`;
     a Mac, public build: ` Signing in with a Console account doesn't work on a Mac yet. A later update adds it.`
   - `SIGNED_OUT`:
     off a Mac: today's text;
     a Mac, testers' build: `Claude Code isn't signed in for Voice. Sign in with your Claude subscription on Connect Claude, or with \`voice claude sign-in --subscription\`, then try again.`;
     a Mac, public build: `Claude Code isn't signed in for Voice. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.`
   - `SUBSCRIPTION_REFUSED` (two variants: `not(target_os = "macos")` and `target_os = "macos"`):
     off a Mac: today's text;
     a Mac: `Claude Code is signed in for Voice with a Claude subscription, which this copy of Voice can't use. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.`
   - `SUBSCRIPTION_NOT_OFFERED` (two variants, as above):
     off a Mac: today's text;
     a Mac: `This copy of Voice can't sign in with a Claude subscription. Signing in with a Console account doesn't work on a Mac yet. A later update adds it.`

   Each full sentence is written out as one string literal per variant (no macro, no `concat!` of
   other constants). Backticks in `SIGNED_OUT` are literal characters, as in today's text.
4. **No other change**: no call site, no allow-list, no UI file, no other constant. `builder/save`
   has its own `SIGNED_OUT` (GitHub); never touch it.

## 2. Tests, red first

1. In `builder/agent/tests/text.rs`, add one test,
   `the_claude_code_messages_name_the_sign_in_this_build_can_start`, that builds the expected text
   for each of the four with the same three-way choice VF6's test uses
   (`!cfg!(target_os = "macos")`, else `voice_agent::claude::SUBSCRIPTION_OFFERED`, else the public
   build) and `assert_eq!`s each constant against it. For the two-variant constants the testers' and
   public Mac cases expect the same Mac sentence. Run it before changing `text.rs` and see it fail on
   its assertion (on a Mac).
2. Find every test that asserts one of these sentences as a literal string, rather than by the
   constant's name: `grep -rn "Anthropic Console account" builder --include='*.rs'` and check each hit
   in a `tests/` folder or `#[cfg(test)]` module. Where it asserts one of the four, make its expected
   text follow the build the same way (VF6's `let expected = if !cfg!(target_os = "macos") { … } else if … SUBSCRIPTION_OFFERED { … } else { … };`).
   Leave tests that only use the constant by name alone.
3. Green, in both builds, from the repo root (the gate's own forms, two test threads for the
   machine's load):
   `cargo test --locked -p voice-agent -p voice-api -p voice-cli -p voice-panel-runner -- --test-threads=2`
   and the testers' build:
   `cargo test --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription -- --test-threads=2`
4. `cargo fmt --all` and `cargo clippy --all-targets --locked -- -D warnings`, and
   `cargo clippy --all-targets --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription -- -D warnings`.

## 3. Done when

- The four constants read as in §1 on each build; nothing else in `src/` changed.
- The new test fails on today's code on a Mac and passes after, in both builds.
- fmt and clippy are clean. Report the files changed and the exact commands you ran with their results.
