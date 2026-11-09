# VF7a brief: a subscription sign-in reports calls and tokens, not dollars

When Voice is signed in with a Claude subscription (testers' builds only, the `claude-subscription`
feature, VO-D10), every report of today's use still ends in dollars, for example
*Today, about $1.28 of the $50.00 daily limit, as Claude Code estimates it.* A subscription has no
per-call bill, and usage reports under one are never in dollars. Claude Code doesn't report the plan's
share on demand (V-Q8), so the report becomes what Voice can count itself: today's calls and tokens,
from its own ledger (`runs.jsonl`).

**Out of scope (VF7b, parked on V-Q16):** the daily limit itself. Its check before each call, its
saved file, `voice limit show` and `set`, Connect Claude's limit fields and the limit-reached message
(`text::cap_reached`, `building.rs` `cap_reached`) stay exactly as they are, in dollars. A Console
sign-in, and every public build, keep today's dollar lines word for word.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/agent/src/ledger.rs`,
`builder/agent/src/text.rs`, `builder/agent/src/claude.rs` (`sign_in_status`, `Status`, `Ready`,
`status`, `check`), `builder/api/src/building.rs` (`spend`, `build_assistant_in`, `spend_today_in`,
`spend_today`), `builder/api/src/review.rs` (the two refresh flows), `builder/api/src/dto.rs` (`Spend`),
`builder/cli/src/main.rs` (`spend`, `build`, `interview_draft`, `refresh_propose`, `refresh_apply`),
`builder/ui/src/screens/build/BuildRun.vue`, `builder/ui/src/screens/review/Refresh.vue`, and their
tests (`builder/agent/tests/find_check.rs`, `builder/cli/tests/agent.rs`, `builder/api/tests/building.rs`,
`builder/api/tests/claude_sign_in.rs`, `BuildRun.test.ts`, `Refresh.test.ts`, `src/test-support.ts`).
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

This is a public repo: invented names only (the tests' existing `wren`, `example`), no real person,
no fixed paths. **Commit nothing; leave every change in the working tree.** The computer is busy:
pass `-j 2` to cargo and `--maxWorkers=2` to vitest.

## 1. Decisions already made (do not change them)

### Knowing the sign-in

- `claude::Status` gains `pub subscription: bool`: signed in with a subscription **in a build that
  offers it**. `sign_in_status` returns it alongside the other two: `true` only when `loggedIn` is
  `true`, `authMethod` is `claude.ai` or `oauth_token`, and `SUBSCRIPTION_OFFERED` is true. In a build
  without the feature it is always `false` (that case is already `subscription_refused`). Return a
  small private struct or a triple, whichever reads better; keep the existing two values' meaning.
- `claude::Ready` gains `pub subscription: bool`, copied from `Status` by `check`. Update every
  `Ready { … }` in `builder/agent/tests/find_check.rs`; extend its existing `authMethod` table test
  (the one with `#[cfg(feature = "claude-subscription")]` at about line 271) so `claude.ai` and
  `oauth_token` give `subscription: true` with the feature and `api_key` gives `false`.

### Counting today's use

- In `ledger.rs`, add
  ```rust
  /// Today's calls and tokens, for a subscription sign-in's report: no dollars.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub struct Used { pub calls: u64, pub tokens: u64 }
  ```
  and `pub fn used_today(&self) -> Result<Used>` on `Session`, holding the ledger mutex like
  `spent_today`. It reads the same lines and the same local day as `today()`. **Calls** is the
  number of today's lines, every outcome counted (a `started` line is a call in flight). **Tokens** is
  the sum over today's lines of `input_tokens + output_tokens + cache_creation_input_tokens +
  cache_read_input_tokens`, with saturating adds. A token field that is missing or not a whole
  number ≥ 0 counts as 0: tokens only feed a report, so they never stop spending. A line that fails
  today's validation (`parse_line`) still makes the ledger damaged, exactly as `spent_today` does
  (reuse `ledger_lines` and `parse_line`; read the token fields in a separate small helper). Don't
  change `parse_line`'s meaning or what any existing function returns.

### The words

- In `text.rs`, add `pub fn usage(used: crate::ledger::Used) -> String`:
  - 0 calls: `Today, no calls to Claude yet, on your Claude subscription.`
  - otherwise: `Today, {calls} {call|calls} to Claude and {tokens}, on your Claude subscription.`
  - `{tokens}`: under 1,000, the exact number and `token`/`tokens` (`1 token`, `850 tokens`, `0 tokens`);
    from 1,000 to 999,499, `about {n} thousand tokens` with n = tokens / 1000 rounded to the nearest
    whole (342,310 → `about 342 thousand tokens`; 1,000 → `about 1 thousand tokens`);
    from 999,500 up, `about {m} million tokens` with m = tokens / 1,000,000 to one decimal place,
    a trailing `.0` dropped (999,600 → `about 1 million tokens`; 1,240,000 → `about 1.2 million tokens`;
    12,000,000 → `about 12 million tokens`). Use integer arithmetic for the rounding (round half up),
    not float formatting, so a value like 1,250,000 gives `1.3` on every platform.
- The existing `text::spend` is unchanged.

### The CLI

- Add one helper in `builder/cli/src/main.rs` that returns the closing line:
  `text::usage(session.used_today()?)` when `subscription`, else today's
  `text::spend(session.spent_today()?, cap.daily_usd())`. Use it at all five sites: `spend`, `build`,
  `interview_draft`, `refresh_propose`, `refresh_apply`.
- `build`, `interview_draft`, `refresh_propose` and `refresh_apply` already call
  `claude.check(&home)?`; keep its `Ready` and pass `ready.subscription`.
- `voice spend` doesn't need Claude Code today and must still work without it. When
  `voice_agent::claude::SUBSCRIPTION_OFFERED` is false, it never looks for Claude Code (public builds
  start no new process). When it is true: `claude::find(PATH, home)`, then `status(&home)`; use
  `status.subscription`, and treat any error (not found, doesn't run) as `false`, printing the dollar
  line as today.

### The app

- `dto::Spend` gains
  ```rust
  /// Set when Voice is signed in with a Claude subscription: today's calls and tokens, in words.
  /// The screens show it instead of the dollar estimate.
  pub usage: Option<String>,
  ```
  Let `cargo test` regenerate `builder/ui/src/types/generated/Spend.ts` (ci-local fails if it is
  stale or uncommitted-looking; leave the regenerated file in the working tree).
- `building.rs` `spend(session, subscription: bool)` fills `usage` with `text::usage(...)` from
  `session.used_today()` when `subscription` (a ledger read error maps to the same `failure(LEDGER)`
  as `spent_today`), else `None`. Every caller passes the `subscription` of the `Ready` that its own
  `claude.check(home)` returned: `build_assistant_in`, and both refresh flows in `review.rs`. The
  `last_spend` literals start with `usage: None`.
- `spend_today_in` gains `path: Option<&OsStr>, fallbacks: &[PathBuf]` as its second and third
  arguments (the same order as `build_assistant_in`), and `spend_today` passes
  `std::env::var_os("PATH").as_deref()` and `&claude::fallbacks(&home)`. It decides `subscription`
  exactly as `voice spend` does (no lookup when `SUBSCRIPTION_OFFERED` is false; any error is
  `false`). Update its callers in `builder/api/tests/building.rs`.
- `BuildRun.vue` and `Refresh.vue`: when `spend.usage` is set, the paragraph shows that text and
  nothing else; otherwise today's dollar paragraph, unchanged. Same element and classes as now
  (`<p v-if="spend?.usage" …>{{ spend.usage }}</p>` then the existing `<p v-else-if="spend" …>`).
- Every TypeScript `Spend` literal in tests and `src/test-support.ts` gains `usage: null`.

## 2. Test first

Write each test, run it, and see it fail on its assertion before the code that makes it pass.

1. `ledger.rs` unit tests (or `builder/agent/tests/cap.rs`, beside the existing spend tests):
   `used_today` counts today's lines including a `started` one, sums all four token fields, skips
   another day's lines, counts a line with no token fields as 0 tokens, and still reports a damaged
   ledger for a line `parse_line` refuses.
2. `text.rs` unit test, a table over `usage`: (0, 0), (1, 1), (2, 850), (3, 1000), (11, 342_310),
   (4, 999_499), (5, 999_600), (40, 1_240_000), (41, 1_250_000) → `1.3`, (60, 12_000_000), with the exact
   sentences from §1.
3. `find_check.rs`: the `subscription` field per `authMethod`, with and without the feature.
4. `builder/cli/tests/agent.rs`, under `#[cfg(feature = "claude-subscription")]`, using the stub
   Claude Code the file already has for that feature (about lines 194 and 674) reporting
   `{"loggedIn":true,"authMethod":"claude.ai"}`: `voice spend` on a ledger with two of today's lines
   and one older line prints exactly the `usage` sentence. Add the same closing-line assertion to the
   existing CLI test that runs `refresh propose` or `apply` with a stub, if one exists, for the
   subscription case. The existing dollar tests stay as they are and must still pass in both builds.
5. `builder/api/tests/building.rs` (and the refresh tests where their stub can report `claude.ai`):
   under the feature with a `claude.ai` stub, `spend_today_in` and the build's progress `Spend` carry
   `usage: Some(…)` with the sentence; with an `api_key` stub, `usage: None`. Without the feature,
   always `None`.
6. `BuildRun.test.ts` and `Refresh.test.ts`: with `usage` set, the screen shows that sentence and no
   `$`; with `usage: null`, today's dollar sentence.

**Mutations** (each: `cp` the file to a backup, make the change, run the one test, see it fail,
restore with `cp`, then `cmp` against the backup; never `git checkout --`):
- drop `cache_read_input_tokens` from the sum in `used_today`;
- make `sign_in_status` always return `subscription: false`;
- in `BuildRun.vue`, remove the `usage` paragraph.
Report each mutation and the test that caught it.

## 3. Checks before you stop

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -j 2 -- -D warnings`, and again with
  `-p voice-agent -p voice-api -p voice-cli --features claude-subscription`
- `cargo test -j 2 -p voice-agent -p voice-api -p voice-cli`, and again with `--features claude-subscription`
- in `builder/ui`: `pnpm test -- --maxWorkers=2` and `pnpm run typecheck` (or the repo's equivalent
  scripts in `package.json`)
- `git status` shows the regenerated `Spend.ts` and no stray files.

Report: the files changed, each test added, each mutation's result, and every check's last lines.
