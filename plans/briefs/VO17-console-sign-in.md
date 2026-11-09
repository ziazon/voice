# VO17 brief: Claude sign-in with a Console account, the subscription only in testers' builds

Voice signs its own Claude Code in with `claude auth login --claudeai` today (`builder/agent/src/claude.rs`,
`sign_in_with`). The owner ruled (VO-D10, 2026-11-08): third-party developers may not offer the
claude.ai login without Anthropic's approval, which has not come, so **the public build signs in with
an Anthropic Console account (`--console`, API billing) and the subscription sign-in exists only in
testers' builds**, behind a cargo feature `claude-subscription` that is off by default. A build
without the feature must contain no way to start a subscription sign-in, and must treat a profile
already signed in with a subscription as not signed in.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md` (the row "The app's Claude sign-in"),
`CONTRIBUTING.md`, `docs/spikes/VO11-os-sandbox.md` (the sandbox and the egress proxy). Code to read
before writing any: `builder/agent/src/claude.rs`, `builder/agent/src/sandbox.rs` (`valid_link`,
`Launch::sign_in_link`), `builder/agent/src/text.rs`, `builder/agent/src/egress.rs` (`Rules::claude_code`),
`builder/agent/tests/launch.rs` and `builder/agent/tests/find_check.rs` (the stub-script pattern under
the `test-stubs` feature), `builder/api/src/assistants.rs` (`claude_check_in`, `claude_sign_in_in`),
`builder/api/src/dto.rs` (`ClaudeStatus`), `builder/api/tests/claude_sign_in.rs`,
`builder/app/src/commands.rs`, `builder/app/tests/lockdown.rs`, `builder/cli/src/main.rs`
(`ClaudeCommand`, `claude_sign_in`, the claude check), `builder/cli/tests/agent.rs`,
`builder/cli/tests/app_parity.rs`, `builder/ui/src/screens/ConnectClaude.vue` and its test,
`builder/ui/src/backend.ts`, `builder/ui/src/app/tauri-backend.ts` and its test,
`builder/ui/src/test-support.ts`, `scripts/ci-local.sh`, `scripts/build-mac-app.sh`,
`tests/mac_install_test.sh`.

Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence a person reads).

This is a public repo: invented names only, no real person, no fixed paths, no email addresses in
fixtures beyond `example.com`. **Commit nothing; leave every change in the working tree** (your
sandbox can't write this worktree's git index).

## Facts already checked (do not re-derive; cite them in comments where the code depends on them)

Read from Claude Code 2.1.292's own bundle and `claude auth login --help` on 2026-11-08:

- `claude auth login --console` = "Use Anthropic Console (API usage billing) instead of Claude
  subscription"; `--claudeai` = "Use Claude subscription (default)".
- `claude auth status --json` writes `{loggedIn, authMethod, apiProvider, …}`. `authMethod` is one of
  `"claude.ai"` (a subscription login), `"oauth_token"` (a `CLAUDE_CODE_OAUTH_TOKEN`, which
  `claude setup-token` makes from a subscription), `"api_key"` (a Console login's key, reported with
  `apiKeySource: "/login managed key"`, or an `ANTHROPIC_API_KEY`), `"api_key_helper"`,
  `"third_party"` (Bedrock, Vertex and the like) and `"none"`.
- The Console sign-in's page is `https://platform.claude.com/oauth/authorize?…` (opened by the browser,
  never the child), with the same `client_id` (`sandbox::CLAUDE_CODE_CLIENT_ID`) and
  `redirect_uri=http://localhost:<port>/callback` as the subscription's. The child then calls
  `https://platform.claude.com/v1/oauth/token`, `https://api.anthropic.com/api/oauth/claude_cli/create_api_key`
  and `…/claude_cli/roles`. All three hosts are already in `Rules::claude_code`, so **the egress
  allowlist does not change**; only its doc comment gains the Console sign-in's two URLs.

## What to build

### 1. The feature (`Cargo.toml` of each crate)

- `builder/agent/Cargo.toml`: `[features]` gains `claude-subscription = []` beside `test-stubs`, with
  a comment: off by default; testers' builds only (VO-D10); `scripts/ci-local.sh` checks the default
  app and CLI builds lack it.
- `builder/api`, `builder/cli` and `builder/app` each gain a pass-through feature of the same name:
  api `["voice-agent/claude-subscription"]`, cli `["voice-api/claude-subscription", "voice-agent/claude-subscription"]`,
  app `["voice-api/claude-subscription"]`. No default features anywhere. Do not add the feature to
  any dev-dependency line. `Cargo.lock` must not change (features don't touch it; run with `--locked`).

### 2. `voice_agent::claude`

- A public enum `SignIn { Console, Subscription }` (derive `Debug, Clone, Copy, PartialEq, Eq`) and
  `pub const SUBSCRIPTION_OFFERED: bool = cfg!(feature = "claude-subscription");`.
- A private `fn login_args(sign_in: SignIn) -> Option<[&'static str; 3]>`: `Console` →
  `Some(["auth", "login", "--console"])`; `Subscription` → `Some(["auth", "login", "--claudeai"])`
  only under `#[cfg(feature = "claude-subscription")]`, else `None`. The string `--claudeai` must
  appear in the crate only inside that `cfg` (so a default build holds no subscription argv).
- `Claude::sign_in(&self, home, stop, sign_in: SignIn)` and `sign_in_with(&self, home, stop, sign_in, open)`:
  when `login_args` gives `None`, return `Err(Error::Invalid(text::SUBSCRIPTION_NOT_OFFERED))`
  **before** starting the proxy or any child. Otherwise run as today with those args, and pass
  `sign_in` to `launch.sign_in_link(sign_in)`.
- `Status` gains `pub subscription_refused: bool`. `status()` reads only `loggedIn` and `authMethod`
  from the JSON (never email, org or anything else; keep the doc comment saying so) and decides:
  - `loggedIn == true` and `authMethod == "api_key"` → `signed_in: true`;
  - `loggedIn == true` and `authMethod` is `"claude.ai"` or `"oauth_token"` → `signed_in: SUBSCRIPTION_OFFERED`,
    `subscription_refused: !SUBSCRIPTION_OFFERED`;
  - anything else (`loggedIn` false or missing, `authMethod` missing, not a string, `"none"`,
    `"api_key_helper"`, `"third_party"`, an unknown value) → `signed_in: false`, `subscription_refused: false`.
  Put this in one small pure function so it is tested directly with JSON values.
- `check()`: `subscription_refused` → `Err(Invalid(text::SUBSCRIPTION_REFUSED))`; not signed in →
  `SIGNED_OUT` as today.
- Update the module doc comment: the sign-in is Claude Code's own `auth login --console`, with
  `--claudeai` only in testers' builds (VO-D10), and the 2026-11-08 source line above.

### 3. `voice_agent::sandbox`

- `valid_link(link: &str, sign_in: SignIn) -> bool`: `Console` accepts only links starting
  `https://platform.claude.com/oauth/authorize?`; `Subscription` accepts only today's two prefixes
  (`https://claude.com/cai/oauth/authorize?`, `https://claude.ai/oauth/authorize?`). Every other
  rule (length, bytes, the query, `client_id`, `response_type`, the localhost callback) is unchanged
  and applies to both.
- `Launch::sign_in_link(&self, sign_in: SignIn) -> Link` passes it through (both the unix and the
  non-unix versions take the argument).

### 4. `voice_agent::text` (plain words; no tool words a person wouldn't know)

- `NOT_INSTALLED`: "Voice works through your own Claude Code, and it isn't on this computer yet.
  Install it (https://code.claude.com/docs/en/setup), then try again. Voice signs it in with an
  Anthropic Console account, which is billed for what it uses (API billing)."
- `SIGNED_OUT`: "Claude Code isn't signed in for Voice. Sign in with your Anthropic Console account
  on Connect Claude, or with `voice claude sign-in`, then try again."
- New `SUBSCRIPTION_REFUSED`: "Claude Code is signed in for Voice with a Claude subscription, which
  this copy of Voice can't use. Sign in again with an Anthropic Console account on Connect Claude, or
  with `voice claude sign-in`."
- New `SUBSCRIPTION_NOT_OFFERED`: "This copy of Voice signs in with an Anthropic Console account only."
- `SIGN_IN_WAITING`: "Finish signing in on the page that opened in your browser. Voice carries on when
  you're done."
- `SIGN_IN_FAILED`: "Claude Code didn't finish signing in. Try again. If no sign-in page opened in your
  browser, check that this computer has a default browser."
- `signed_out_at`: end with "…so sign in once with an Anthropic Console account." instead of "…with
  your Claude account."
- Leave `PLAN_STOPPED` and every other sentence unchanged. Update `builder/agent/tests/text.rs` and
  any test that pins these sentences.

### 5. `voice_api`

- `dto::ClaudeStatus` gains `pub can_sign_in_with_subscription: bool` (doc: the screen also offers the
  testers' subscription sign-in; true only when `can_sign_in` and the build has the feature). Every
  place that builds a `ClaudeStatus` sets it to `can_sign_in && claude::SUBSCRIPTION_OFFERED`.
  Regenerate `builder/ui/src/types/generated/ClaudeStatus.ts` by running the api tests (ts-rs
  writes it) and keep the generated file in the working tree.
- New `dto::ClaudeSignInRequest { pub subscription: bool }` (same derives and `ts(export)` as the
  other requests; regenerate its `.ts`).
- `claude_check_in`: a status with `subscription_refused` → `ready: false`, `message: text::SUBSCRIPTION_REFUSED`,
  `can_sign_in: true`.
- `claude_sign_in_in(home, path, fallbacks, sign_ins, request: &dto::ClaudeSignInRequest)` and
  `claude_sign_in(request: dto::ClaudeSignInRequest)`: `subscription: true` in a build without the
  feature returns `Ok(ClaudeStatus { ready: false, message: text::SUBSCRIPTION_NOT_OFFERED, note: None,
  can_sign_in: true, can_sign_in_with_subscription: false })` before taking the busy slot or finding
  Claude Code. Otherwise map to `SignIn::Subscription` / `SignIn::Console` and continue as today.

### 6. `voice-app`

- `commands::claude_sign_in(request: voice_api::dto::ClaudeSignInRequest)`, the same shape as the
  other request commands. Keep the command name; adjust `lockdown.rs` only if it pins the signature.

### 7. `voice` CLI

- `ClaudeCommand::SignIn` becomes a struct variant. Under `#[cfg(feature = "claude-subscription")]`
  only, it has `#[arg(long)] subscription: bool` with help "Sign in with a Claude subscription
  (testers' builds only)." A default build has no such flag (clap refuses it as unknown).
  `claude_sign_in` passes `SignIn::Subscription` when the flag is given, else `SignIn::Console`.
- The CLI's claude check (the function that prints `signed_out_at` + `SIGN_IN_CLI`): when
  `subscription_refused`, fail with `SUBSCRIPTION_REFUSED` instead.
- `voice claude sign-in`'s own help line says it signs in with an Anthropic Console account.

### 8. The screen (`ConnectClaude.vue`)

- `backend.claudeSignIn(request: ClaudeSignInRequest)`; `tauri-backend.ts` passes `{ request }` like
  the other request commands; update `backend.ts`, `test-support.ts` and `tauri-backend.test.ts`.
- Intro: "Voice works through your own Claude Code, signed in with your own Anthropic Console
  account, which is billed for what it uses. It checks that now, and you set how much it may use in a day."
- The sign-in button: "Sign in with Anthropic Console" → `claudeSignIn({ subscription: false })`.
  When `status.canSignInWithSubscription`, a second button beside it, `btn btn-outline-secondary ms-2`:
  "Sign in with a Claude subscription (testers)" → `claudeSignIn({ subscription: true })`.
- While signing in: "Finish signing in on the page that opened in your browser. Voice carries on when
  you're done."
- Footer: "Voice never sees your password: you sign in on Anthropic's own page. Its sign-in is
  separate from the Claude app and from the Claude Code you use yourself. Claude is the only choice
  for now. Other AI providers come later."
- Both local fallback statuses in the script set `canSignInWithSubscription: false`.

### 9. Checks and builds

- `scripts/ci-local.sh`, right after the existing `cargo test` block, with the same environment
  variables per platform: `cargo test --locked -p voice-agent -p voice-api -p voice-cli --features claude-subscription`
  (echo the command first, as the script does). After the existing `test-stubs` loop, a second loop over
  `voice-cli voice-app` with the same `cargo tree … -e features,normal,build -i voice-agent` that fails
  with `ci-local: $package builds voice-agent with claude-subscription` when
  `voice-agent feature "claude-subscription"` appears; then one positive check that
  `cargo tree --locked --target all -p voice-app --features claude-subscription -e features,normal,build -i voice-agent`
  **does** show it (failing with `ci-local: voice-app's claude-subscription feature does not reach voice-agent`).
  Add `cargo clippy --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --all-targets --features claude-subscription -- -D warnings`.
- `scripts/build-mac-app.sh`: accept `--testers` (alone or with `--universal`, either order; usage
  `bash scripts/build-mac-app.sh [--universal] [--testers]`). With it, the Tauri build gets
  `--features claude-subscription` (Tauri 2's `tauri build -f, --features`), both `cargo build … -p voice-cli`
  lines get `--features claude-subscription`, and the package name ends `-testers`
  (`voice-<version>-mac-<label>-testers`). Without it, nothing changes. Update `tests/mac_install_test.sh`
  only where it pins the usage line.
- `CONTRIBUTING.md`: a short "Testers' builds" item: the public build signs in with an Anthropic
  Console account only; a tester's copy, which also offers the Claude subscription sign-in, is built
  with `bash scripts/build-mac-app.sh --testers` (or `cargo build -p voice-cli --features claude-subscription`
  for the command alone); never hand a testers' build to anyone outside the testing group, and no
  release passes the feature.
- `.ai/SECURITY.md`, the row "The app's Claude sign-in": `claude auth login --console` (fixed words),
  `--claudeai` only in a build with the `claude-subscription` feature (testers' builds, VO-D10); a
  default build counts a subscription profile (`authMethod` `claude.ai` or `oauth_token`) as not signed
  in; the link check takes the sign-in kind (Console: `platform.claude.com` only). Add the new tests
  to the row's test column.
- `docs/spikes/VO11-os-sandbox.md`: a short "Console sign-in (VO17, 2026-11-08)" paragraph with the
  facts above (hosts unchanged, the page prefix, read from 2.1.292's bundle; not yet walked live).

## Tests first (red, then green)

Write each failing test before its code, and run it red on its assertion.

- agent: `status`'s decision function for every `authMethod` above, with `loggedIn` true and false,
  and a missing or non-string `authMethod`, with expectations that depend on `SUBSCRIPTION_OFFERED`
  (assert `cfg!(feature = "claude-subscription")` branches both ways in one test, so it passes in
  both builds and fails if the mapping is wrong in either).
- agent, through a stub `claude` script (the existing `test-stubs` pattern) that writes its argv to a
  file: the default sign-in runs exactly `auth login --console`; `SignIn::Subscription` runs
  `auth login --claudeai` with the feature, and without it returns `SUBSCRIPTION_NOT_OFFERED` with the
  stub never started (no argv file). A stub `auth status --json` per `authMethod` drives `check()`:
  `claude.ai` gives `SUBSCRIPTION_REFUSED` without the feature and `Ready` with it.
- agent `launch.rs`: `valid_link` with `Console` accepts a `platform.claude.com/oauth/authorize?` link
  and refuses both subscription prefixes; with `Subscription`, today's cases pass and the platform link
  is refused. Keep every existing case, now passing `SignIn::Subscription`.
- api `claude_sign_in.rs` / `assistants.rs`: `claude_check_in` on a `claude.ai` stub (message,
  `can_sign_in`, `can_sign_in_with_subscription == SUBSCRIPTION_OFFERED`); a `subscription: true` request
  without the feature returns `SUBSCRIPTION_NOT_OFFERED` and starts nothing.
- cli `agent.rs`: without the feature `voice claude sign-in --subscription` exits with clap's unknown
  argument error; with it, `--help` lists the flag (gate each test with `#[cfg]`).
- ui: one button with `canSignInWithSubscription: false`, calling `claudeSignIn({ subscription: false })`;
  two with it true, the second calling `claudeSignIn({ subscription: true })`; the tauri backend passes
  `{ request }`.
- Mutation check per claim: break the mapping (accept `claude.ai` always), drop the `cfg` from
  `login_args`, and swap a link prefix; each must turn a test red. Restore from a `cp` backup and
  check with `cmp`.

## Run before you stop (report the exact results)

The computer is loaded: use 2 test threads (`-- --test-threads=2` is fine; or `CARGO_BUILD_JOBS=4`).

- `cargo fmt --check`
- `cargo clippy --all-targets --locked -- -D warnings`
- `cargo clippy --locked -p voice-agent -p voice-api -p voice-cli -p voice-app --all-targets --features claude-subscription -- -D warnings`
- `cargo test --locked -p voice-agent -p voice-api -p voice-cli` and the same with `--features claude-subscription`
- `cargo test --locked -p voice-app`
- `pnpm -C builder/ui test` (if pnpm is present; `pnpm -C builder/ui install --frozen-lockfile` first)
- `shellcheck scripts/ci-local.sh scripts/build-mac-app.sh tests/mac_install_test.sh` and `bash tests/mac_install_test.sh`
- `git status --porcelain -- builder/ui/src/types/generated` lists only `ClaudeStatus.ts` and the new
  `ClaudeSignInRequest.ts`.

Out of scope: an API-key provider in the broker; OpenAI or Gemini adapters; where the login is stored
inside the sandbox (VF3, parked); `PLAN_STOPPED`'s wording; README and `bootstrap/PROMPT.md` (they
don't mention signing in today).

## Fix-up after the Verifier (2026-11-09)

The Verifier passed `e65e06e` with three should-fixes. Make exactly these, nothing else, test first.

**Never run a built `voice` binary, the app, or the real `claude` in this fix-up.** Every test uses the
existing stub `claude` scripts and disposable homes. A built binary can start a real sign-in.

1. **The sandbox probe signs in with Console.** In `scripts/claude_sandbox_probe.py` (`command()`,
   the `actions` table) change `'sign-in': ['auth', 'login', '--claudeai']` to
   `['auth', 'login', '--console']`. Update the expectation in `tests/claude_sandbox_probe_test.py`
   (`test_exact_commands`) first, so it goes red, then fix the script.
2. **Feature-build tests that the subscription request reaches `--claudeai`.** Gate each with
   `#[cfg(feature = "claude-subscription")]`.
   - api `builder/api/tests/claude_sign_in.rs`: a stub whose `auth login` writes its argv (as
     `successful_login_is_checked_again_and_releases_the_slot` does through `runs`) and touches
     `ready`; call `claude_sign_in_in` with `ClaudeSignInRequest { subscription: true }`; assert the
     answer is ready and `stub.runs()` contains `auth login --claudeai` and not `--console`.
   - cli `builder/cli/tests/agent.rs`: copy `claude_sign_in_uses_its_profile_then_checks_the_result`
     into a feature-gated test that runs `["claude", "sign-in", "--subscription"]` and asserts the
     login file reads `"auth\nlogin\n--claudeai\n"`.
3. **A sign-in that ends on the subscription account says so.** If the browser signs in with a
   subscription in a public build, the re-check after `sign_in` sees `subscription_refused`, and today
   both callers fall through to the generic failure. After the post-sign-in `claude.status(home)`:
   - api `claude_sign_in_in` (`builder/api/src/assistants.rs`): when `status.subscription_refused`,
     return the same `dto::ClaudeStatus` that `claude_check_in` returns for that case
     (`text::SUBSCRIPTION_REFUSED`, `ready: false`, `can_sign_in: true`,
     `can_sign_in_with_subscription: claude::SUBSCRIPTION_OFFERED`). Factor it into one private
     `subscription_refused_status()` used by both, like `ready_status`.
   - cli `claude_sign_in` (`builder/cli/src/main.rs`): when the re-check has `subscription_refused`,
     return `Err(Error::Invalid(text::SUBSCRIPTION_REFUSED.into()))`.
   - Tests, `#[cfg(not(feature = "claude-subscription"))]`: an api stub and a cli stub whose
     `auth login` makes the next `auth status` answer `{"loggedIn":true,"authMethod":"claude.ai"}`;
     the api answer's message is `text::SUBSCRIPTION_REFUSED` and not ready; the cli exits non-zero
     with that text on stderr.
4. **`scripts/ci-local.sh` runs the feature tests before the default ones**, on both the Darwin and
   the other branch, so `target/debug/voice` ends as the public build. Move each
   `--features claude-subscription` `cargo test` pair (echo and command) above the default
   `cargo test --locked` line. Change nothing else in the script.

Mutation checks: revert fix 3 in the api (`cp` backup, `cmp` after) and the new api test goes red; the
same for the cli. Run before you stop, with 2 test threads: `cargo fmt --check`;
`cargo test --locked -p voice-api -p voice-cli` with and without `--features claude-subscription`;
`cargo clippy --locked -p voice-api -p voice-cli --all-targets -- -D warnings` with and without the
feature; `python3 -m unittest tests.claude_sandbox_probe_test`; `shellcheck scripts/ci-local.sh`.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`.
Commit nothing; leave the changes in the worktree.
