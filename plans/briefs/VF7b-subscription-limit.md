# VF7b brief: no daily limit under a Claude subscription

When Voice is signed in with a Claude subscription (testers' builds only, the `claude-subscription`
feature, VO-D10), the $50 daily limit still applies, in dollars, and Connect Claude and
`voice limit show` still talk about it in dollars. The answer to V-Q16 (2026-11-10): **no daily
limit under a subscription.** Under a subscription Voice stops for the day only when Claude Code says
the plan is nearly used up (V-Q8's plan guard, `text::PLAN_STOPPED`, unchanged). The step amount
(default $5) stays as the hidden stop for a runaway call, because `--max-budget-usd` is the only stop
inside a call. Connect Claude and `voice limit show` say the daily limit applies to a Console sign-in
only, and nothing is shown in dollars under a subscription. A Console sign-in, and every public
build, keep the $50 day and the $5 step exactly as today, word for word.

VF7a (`plans/briefs/VF7a-subscription-usage.md`, landed) already knows the sign-in
(`claude::Status::subscription`, `claude::Ready::subscription`) and already reports calls and tokens
instead of dollars. Reuse what it built.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `plans/briefs/VF7a-subscription-usage.md`,
`builder/agent/src/ledger.rs` (`Cap`, `Session::start`), `builder/agent/src/text.rs`,
`builder/agent/src/claude.rs` (`Status`, `Ready`, `status`, `check`),
`builder/cli/src/main.rs` (`cap_for`, `limit_show`, `limit_set`, `spend`, `build`, `interview_draft`,
`refresh_propose`, `refresh_apply`, and the `--daily-cap-usd` help strings),
`builder/api/src/building.rs` (`open`, `open_with`, `spend`, `build_assistant_in`, `spend_today_in`,
the interview draft near line 433), `builder/api/src/review.rs` (the two refresh flows),
`builder/api/src/ask.rs`, `builder/api/src/assistants.rs` (`ready_status` and every
`dto::ClaudeStatus { … }`), `builder/api/src/dto.rs` (`ClaudeStatus`),
`builder/panel-runner/src/turns.rs` (`ClaudeTurns::new`), `builder/ui/src/screens/ConnectClaude.vue`,
and their tests (`builder/agent/tests/cap.rs`, `builder/agent/tests/find_check.rs`,
`builder/cli/tests/agent.rs`, `builder/api/tests/building.rs`, `builder/api/tests/claude_sign_in.rs`,
`builder/ui/src/screens/ConnectClaude.test.ts`, `builder/ui/src/test-support.ts`).
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

This is a public repo: invented names only (the tests' existing `wren`, `example`), no real person,
no fixed paths. **Commit nothing; leave every change in the working tree.** The computer is busy:
pass `-j 2` to cargo and `--maxWorkers=2` to vitest.

## 1. Decisions already made (do not change them)

### The check before each call

- In `ledger.rs`, add to `impl Cap`:
  ```rust
  /// The cap for this sign-in. Under a Claude subscription there is no daily limit (V-Q16): Voice
  /// stops for the day only when Claude Code says the plan is nearly used up. The step limit stays,
  /// as the only stop inside a call.
  pub fn for_sign_in(self, subscription: bool) -> Cap
  ```
  `subscription == false` returns `self` unchanged. `true` returns a `Cap` with `daily_usd: 0.0` and
  the same `call_usd`. It never fails and never saves anything: `limit.json` keeps the person's
  Console amounts.
- `Session::start`, `budget_for`, `cap_reached` and the plan guard are **unchanged**. With the daily
  part at 0, `budget_for` already gives the step budget (`(None, Some(call))`) or no budget
  (`(None, None)`), and never `Reached`, so the limit-reached sentence can't appear under a
  subscription. The plan guard (`stopped` → `PLAN_STOPPED`) still runs first.
- Apply `for_sign_in` to the `Cap` at **every place a session is opened for calls after
  `claude.check`**, passing that check's `Ready::subscription`:
  - CLI `build`, `interview_draft`, `refresh_propose`, `refresh_apply`: the `cap_for(…)?` result,
    applied after `claude.check(&home)?` (move the `cap_for` call below the check, or apply
    `for_sign_in` at `Session::open`/`open_existing`; keep `cap_for`'s own validation and its
    order of errors otherwise). Under a subscription an explicit `--daily-cap-usd` is ignored.
  - API: give `building::open` a third argument `subscription: bool` and apply it there
    (`Cap::load(..)?.for_sign_in(subscription)`). Callers pass their own `ready.subscription`:
    `build_assistant_in`, the interview draft, both refresh flows in `review.rs`. `ask.rs` passes
    `ready.subscription` when it checked Claude (the provider is Claude), else `false`. Keep the
    `request.no_limit` branch (*Keep going*) exactly as it is: `Cap::new(0.0, 0.0)`.
  - `spend_today_in` and CLI `voice spend` only report; they pass the `subscription` they already
    compute.
  - Panels: `ClaudeTurns::new` keeps the `Ready` from `claude.check(home)` and applies
    `for_sign_in(ready.subscription)` to each seat's `Cap::load`. (`seats.rs` only holds a session as
    a lock; leave it.)
- A place that forgets `for_sign_in` keeps the daily limit, the safer side. Don't add the flag to
  `Session` or to `Cap::load`.

### The words

- In `text.rs` add:
  ```rust
  pub const LIMIT_SUBSCRIPTION: &str = "Signed in with your Claude subscription, Voice has no daily limit: it stops for the day when Claude Code says your plan is nearly used up, and leaves the rest for you. The daily limit is for an Anthropic Console sign-in only.";
  pub const LIMIT_SAVED_SUBSCRIPTION: &str = "Saved for an Anthropic Console sign-in. Signed in with your Claude subscription, Voice has no daily limit: it stops for the day when Claude Code says your plan is nearly used up, and leaves the rest for you.";
  ```
  `text::limit`, `text::spend`, `text::cap_reached` and `building.rs`'s `cap_reached` are unchanged.
- CLI: one helper, `fn signed_in_with_subscription(home: &Path) -> bool`, holding the expression
  `voice spend` uses today (`claude::SUBSCRIPTION_OFFERED && claude::find(PATH, home)
  .and_then(|c| c.status(home)).is_ok_and(|s| s.subscription)`); `spend` uses it instead of its
  inline copy. When `SUBSCRIPTION_OFFERED` is false it starts nothing (the `&&` already does that).
  - `voice limit show`: under a subscription prints `LIMIT_SUBSCRIPTION`, else `text::limit(…)` as
    today. A damaged `limit.json` still fails first, as today (load it before the sign-in check).
  - `voice limit set`: validates and saves exactly as today, then prints `LIMIT_SAVED_SUBSCRIPTION`
    under a subscription, else `text::limit(cap)` as today.
  - Help strings: the five `--daily-cap-usd` helps and `limit set`'s `daily_usd` help end with
    ` Anthropic Console sign-in only.` (keep the rest of each string as it is; `limit show`'s and
    `limit set`'s own one-line helps are unchanged).
- API and app: `dto::ClaudeStatus` gains
  ```rust
  /// Claude Code is ready and signed in with a Claude subscription (testers' builds only): the
  /// screen shows no daily limit and no dollars.
  pub subscription: bool,
  ```
  `ready_status` sets it from `status.subscription`; every other `ClaudeStatus { … }` sets `false`.
  Let `cargo test` regenerate `builder/ui/src/types/generated/ClaudeStatus.ts` (leave it in the
  working tree). Every TypeScript `ClaudeStatus` literal (the screen's own fallbacks in
  `ConnectClaude.vue`, tests, `src/test-support.ts`) gains `subscription: false`.
- `ConnectClaude.vue`, the *How much it may use* card: keep the heading. When
  `status?.ready && status.subscription`, the card shows only one paragraph, exactly the
  `LIMIT_SUBSCRIPTION` sentence above, and none of: the "The amounts are Claude Code's own estimates."
  line, the two fields, the two help paragraphs, the save button, the saved message. Otherwise the card
  is as today, except that the fields wait for the Claude check: render the existing
  `<template v-if="limit">` content only once `status` is set (not while `loading` or `signingIn`), so a
  subscription sign-in never shows dollar fields for a moment. A failed check (the fallback status)
  shows the fields as today. The limit still loads on mount as today. Same classes and elements; no
  new component.

### Out of scope (leave as they are; V1 files a follow-up card)

Connect Claude's first paragraph, the Build screen's "up to the daily limit you set on Connect Claude"
sentence, the panels note about the daily limit, the consent text in `builder/core/src/consent.rs`,
*Keep going* (`no_limit`), the saved file's format, and the API's `spending_limit` /
`set_spending_limit` commands.

## 2. Test first

Write each test, run it, and see it fail on its assertion before the code that makes it pass.

1. `ledger.rs` unit tests (or `builder/agent/tests/cap.rs`): `Cap::new(50.0, 5.0)?.for_sign_in(true)`
   has `daily_limit() == None`, `call_limit() == Some(5.0)` and `budget_for(1_000.0) == Ok(Some(5.0))`;
   `for_sign_in(false)` equals the original; `Cap::new(50.0, 0.0)?.for_sign_in(true).budget_for(49.999)`
   is `Ok(None)`; `Cap::new(0.0, 0.0)?.for_sign_in(true)` equals the original. A `Session` opened with
   `Cap::new(1.0, 0.5)?.for_sign_in(true)` on a ledger already holding $2.00 of today's spend starts a
   call (the test-stubs provider, as `cap.rs` already does) at a 0.50 budget, while the same ledger
   with `for_sign_in(false)` refuses with `text::cap_reached`.
2. `text.rs`: no new table needed; the two constants are pinned by the CLI tests below.
3. `builder/cli/tests/agent.rs`, under `#[cfg(feature = "claude-subscription")]` with the stub Claude
   Code the file already has for that feature (`{"loggedIn":true,"authMethod":"claude.ai"}`):
   - `voice limit show` prints exactly `LIMIT_SUBSCRIPTION` and no `$`; `voice limit set <dir> --daily-usd 20 --step-usd 2` saves
     (a following `limit.json` read holds 20 and 2) and prints exactly `LIMIT_SAVED_SUBSCRIPTION`.
   - With an `api_key` stub, both print today's dollar lines exactly.
   - A command that calls Claude (`voice build`, or whichever of `build`, `interview draft`,
     `refresh propose`/`apply` the file can already run against a stub most cheaply) on a ledger whose
     today's spend is past a saved $1.00 daily limit: with the `claude.ai` stub it runs and the stub
     receives `--max-budget-usd` with the step amount; with the `api_key` stub it fails with the
     limit-reached sentence as today.
   - Without the feature, the existing dollar tests stay as they are and pass.
4. `builder/api/tests/building.rs` (and `review.rs` where its stub can report `claude.ai`): under the
   feature, `build_assistant_in` with a `claude.ai` stub on a ledger past a $1.00 daily limit is not
   refused with the limit sentence; with an `api_key` stub it is, as today. Add the same pair for one
   refresh flow and for `ask` if their test files can already stub a `claude.ai` sign-in; where a file
   can't, say so in your report instead of building a new harness.
5. `builder/api/tests/claude_sign_in.rs` (or wherever `claude_check_in` is tested): `subscription` is
   `true` for a ready `claude.ai` stub under the feature, `false` for `api_key`, and `false` for every
   not-ready status.
6. `ConnectClaude.test.ts`: with `claudeCheck` resolving `{ ready: true, subscription: true, … }`, the
   card shows the `LIMIT_SUBSCRIPTION` sentence, no input, no `$` in the card and no *Save the limit*
   button; with `subscription: false`, the fields and button as today; while `claudeCheck` is still
   pending, no limit field is shown.
7. Panels: `voice-panel-runner` has no `claude-subscription` feature, so its subscription path can't be
   tested there; don't add one. Its existing tests must still pass.

**Mutations** (each: `cp` the file to a backup, make the change, run the one test, see it fail,
restore with `cp`, then `cmp` against the backup; never `git checkout --`):
- `for_sign_in` returns `self` for `true`;
- `for_sign_in(true)` also sets `call_usd` to 0;
- in CLI `build` (or the command test 3 uses), drop the `for_sign_in` call;
- in `building::open`, ignore `subscription`;
- `ready_status` sets `subscription: false`;
- in `ConnectClaude.vue`, drop the subscription branch.
Report each mutation and the test that caught it.

## 3. Checks before you stop

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -j 2 -- -D warnings`, and again with
  `-p voice-agent -p voice-api -p voice-cli --features claude-subscription`
- `cargo test -j 2 -p voice-agent -p voice-api -p voice-cli -p voice-panel-runner`, and
  `cargo test -j 2 -p voice-agent -p voice-api -p voice-cli --features claude-subscription`
- in `builder/ui`: `pnpm test -- --maxWorkers=2` and `pnpm run typecheck` (or the repo's equivalent
  scripts in `package.json`)
- `git status` shows the regenerated `ClaudeStatus.ts` and no stray files.

If your sandbox blocks sockets or pnpm, say which checks didn't run; V1 runs them outside it.

Report: the files changed, each test added, each mutation's result, and every check's last lines.
