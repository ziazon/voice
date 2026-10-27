# PB7.9a brief: 0 means no limit, and *Save the limit* is greyed out only when a value is invalid

Written by Claude (lane V1), 2026-10-26. Spec: lane V's `PB7.9a` entry (split from PB7.9, an attended
app review; F1 and F2). Read the repo's `AGENTS.md` first, then `builder/agent/src/ledger.rs` (all of
it), `builder/agent/src/text.rs`, `builder/agent/src/claude.rs` (`fn call`, around the
`--max-budget-usd` argument and the `None => (budget, "budget")` fallback), `builder/agent/tests/cap.rs`,
`builder/agent/tests/call.rs`, `builder/agent/tests/common/mod.rs`, `builder/api/src/building.rs`
(`LIMIT_BAD`, `fn open`, `fn spend`, `fn call_failure`, `spending_limit_in`,
`set_spending_limit_after`), `builder/api/src/review.rs` (the `Spend { … }` it builds),
`builder/api/src/dto.rs` (`Spend`, `SpendingLimit`, `SpendingLimitRequest`), `builder/api/tests/`
(whatever covers the limit and spend), `builder/cli/src/main.rs` (the `daily_cap_usd`/`call_cap_usd`
args, `Limit`/`LimitCommand`, `fn cap_for`, `fn limit_show`, `fn limit_set`, `fn spend`),
`builder/cli/tests/agent.rs`, then `builder/ui/src/screens/ConnectClaude.vue` and its test,
`builder/ui/src/screens/build/BuildRun.vue` and its test, `builder/ui/src/screens/review/Refresh.vue`
and its test, and `.ai/SECURITY.md` (the row *The app spending the person's Claude plan*).

Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar: `~/.codex/skills/code-quality/SKILL.md`;
the screen: `~/.codex/skills/frontend/SKILL.md` and `~/.codex/skills/design-guide/SKILL.md`; every
sentence a person reads: `~/.codex/skills/plain-language/SKILL.md`. Run cargo with
`CARGO_NET_OFFLINE=true` and `-j 2` (and `-- --test-threads=2`); the machine is loaded. Do not commit.

## The problem

1. The person can't say "no limit". `Cap::new` refuses anything under $0.01 and a daily limit over
   $1,000, and the screen gives no way out.
2. *Save the limit* is greyed out while the values are unchanged (`unchangedLimit`), so with the valid
   defaults ($50.00 and $5.00) the button looks broken, and an invalid value is only reported after a
   click.

## Decisions (made, do not reopen)

1. **0 means no limit, in both fields.** The saved file, the DTOs and the CLI flags keep their shape
   (`f64`); `0` in `daily_usd` means no daily limit, `0` in `call_usd` means no per-step limit. A
   `limit.json` written before this slice still loads as it did.
2. **`Cap::new(daily_usd, call_usd)`** accepts exactly when: both are finite; each is `0` or between
   `0.01` and `1000` inclusive (`MOST_USD` now bounds a typed step amount as well as a typed daily
   one; "no limit" is never refused for being over $1,000); each is whole cents (the existing check);
   and, **only when both are above 0**, `call_usd <= daily_usd`. A negative amount, `NaN` or an
   infinity is refused. Store `-0.0` as `0.0` (so the file never says `-0.0`). Update the doc comment.
3. **New accessors** on `Cap`: `pub fn daily_limit(&self) -> Option<f64>` and
   `pub fn call_limit(&self) -> Option<f64>` (`None` for 0). Keep `daily_usd()`/`call_usd()` (they
   return the raw value, 0 for none) since the DTOs carry the raw value.
4. **Each call's budget**, one function used by both the ledger and the API's `call_failure`:

   ```rust
   /// The daily limit is reached: no call may start.
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub struct Reached;

   impl Cap {
       /// The budget for the next call, given what today has spent (calls still running counted at
       /// their budget): `Ok(Some(usd))` is passed to Claude Code as `--max-budget-usd`, `Ok(None)`
       /// means no limit at all and no budget is passed, `Err(Reached)` means the daily limit is reached.
       pub fn budget_for(&self, spent_usd: f64) -> std::result::Result<Option<f64>, Reached>
   }
   ```

   - step set, daily set: `Some(call)` unless `spent + call > daily + 1e-9`, then `Reached` (today's rule).
   - step set, no daily: `Some(call)`, never `Reached`.
   - **no step, daily set: `Some(left)`, where `left` is what is left of today's limit, rounded down to
     whole cents** (`((daily - spent) * 100.0 + 1e-6).floor() / 100.0`); `Reached` when `left < 0.01`.
     Why: the screen promises "Persona Builder checks the limit before every step, so it stops before it
     goes over", so a call with no step limit still never gets more than the day has left. Calls for
     one assistant never run at the same time (the state lock), so this costs nothing in parallelism.
   - neither set: `Ok(None)`, never `Reached`.
5. **`Session::start`** uses `budget_for`. `Started { budget }` becomes `Option<f64>`;
   `Started::budget()` returns `Option<f64>`. The `started` ledger line's `cost_usd` is the budget, or
   `0.0` when there is none (keep `cost_from: "budget"`). `cap_reached` is only ever shown when a daily
   limit is set, with that limit.
6. **In `claude.rs`:** pass `--max-budget-usd <budget:.2>` only when the budget is `Some`; with `None`
   the argument and its value are left out entirely. When Claude Code reports no cost: with a budget,
   count the whole budget as today (`cost_from: "budget"`); with none, record `0.0` with
   `cost_from: "unknown"` (the ledger reader accepts any `cost_from` and any finite cost >= 0; check
   that stays true and say so in a test). Nothing else in the argv changes.
7. **`builder/api/src/building.rs`:** `call_failure` reports `cap_reached` exactly when
   `session.cap().budget_for(spent)` is `Err(Reached)`. `LIMIT_BAD` becomes:
   "Each amount is 0 for no limit, or from $0.01 to $1,000 in whole cents, like 2.50. When both are set,
   the most one step may use can't be more than the daily limit." `Spend.daily_cap_usd` and
   `SpendingLimit` keep their `f64` and carry `0` for no limit: add a `///` comment on each field saying
   so, and regenerate the TypeScript types the way the repo already does (check how
   `builder/ui/src/types/generated/` is produced, `ts-rs` export in the tests; never hand-edit them).
   Same message rule in `ledger.rs`'s `Cap::new` error (an `Error::Invalid` sentence in the same words,
   for the CLI).
8. **Texts in `builder/agent/src/text.rs`** (plain language, one sentence each, "as Claude Code
   estimates it" kept):
   - `limit(cap)`, four cases:
     - both set: unchanged.
     - no daily, step set: "No daily limit, and at most $Y for one step, as Claude Code estimates it."
     - daily set, no step: "Daily limit: about $X a day, and no separate limit for one step, as Claude
       Code estimates it."
     - neither: "No limit: Persona Builder doesn't stop for the day or for one step."
   - `spend(spent, cap)` with `cap == 0`: "Today, about $S, with no daily limit, as Claude Code estimates it."
9. **CLI (`builder/cli/src/main.rs`):** every `daily_cap_usd`/`call_cap_usd`/`limit set` doc comment
   gains "0 for no limit". `cap_for`: when only the daily flag is given, the step is the saved step,
   lowered to the new daily only when both are above 0 (today it is `saved.min(daily)`, which would turn
   a saved $5 step into "no limit" when the daily flag is 0). `fn spend` and `limit show` print the
   texts above.
10. **The screen (`ConnectClaude.vue`):**
    - Delete `unchangedLimit` and every use of it.
    - Two computed reasons, `dayProblem` and `stepProblem` (`''` when valid), using the backend's rules
      exactly, checked as the person types (from the field values, no click needed):
      - not an amount (`dollars()` gives `undefined`): "Enter an amount like 50.00, or 0 for no limit."
      - day above 1000: "The daily limit can be at most $1,000. Enter 0 for no limit."
      - step above 1000: "One step can use at most $1,000. Enter 0 for no limit."
      - both above 0 and step above day: on the step field, "One step can't use more than the daily limit."
    - Each input gets `:class="{ 'is-invalid': … }"`, `:aria-invalid`, and `aria-describedby` pointing
      at a `<div class="invalid-feedback" :id=…>` holding its reason, placed inside the `input-group`
      after the input (Bootstrap/Tabler shows it only beside an `.is-invalid` input).
    - *Save the limit* is `:disabled="loadingLimit || savingLimit || !!dayProblem || !!stepProblem"`,
      so it is enabled with the unchanged valid defaults. `saveLimit` keeps its own guard on the same
      condition (no request with an invalid value) and saves the unchanged values when clicked.
    - A help line under the two fields, before the "If the build reaches the limit…" paragraph:
      "Enter 0 in either box for no limit."
    - The saved message follows the four cases: both set, today's sentence; otherwise "Saved. Persona
      Builder doesn't stop for the day" / "has no limit for one step" in the same plain style, e.g.
      "Saved. There's no daily limit, and one step may use at most $5.00." / "Saved. Persona Builder
      stops for the day at about $50.00, with no separate limit for one step." / "Saved. There's no
      limit." Show amounts with `toFixed(2)` as today.
11. **The spend line on `BuildRun.vue` and `Refresh.vue`:** when `spend.dailyCapUsd === 0`, "Today,
    about $S, with no daily limit, as Claude Code estimates it."; otherwise unchanged.
12. **`.ai/SECURITY.md`**, the row *The app spending the person's Claude plan*: replace "inside
    `Cap::new`'s bounds, each step at least $0.01 since Claude Code gets its budget to the cent" with
    the new rule in one clause: 0 for no limit in either field; a step with no limit but a daily one is
    given what is left of the day, so the limit still holds; with no limit at all nothing is passed to
    Claude Code and a call that reports no cost is counted at $0. Add to its Known limits: "with no
    limit at all, a call that reports no cost is counted at $0, so a daily limit set later the same day
    under-counts that call".

## Tests, red first

Write each failing test before its code, and see it fail on its assertion.

- `builder/agent/tests/cap.rs`: `Cap::new` accepts `(0,0)`, `(0,5)`, `(50,0)`, `(0,1000)`,
  `(1000,0)`, `(-0.0, 0)` (loads back as `0.0`); refuses `(0,1000.01)`, `(1000.01,0)`, `(-0.01,0)`,
  `(0,-0.01)`, `(10,20)` (still), `(0,0.001)`, `NaN`/infinities. Save and load round-trip each
  accepted case. `budget_for` for every case in decision 4, including `left` rounding down
  (daily 20, spent 19.995 → `Reached`; daily 20, spent 12.344 → `Some(7.65)`).
- `builder/agent/tests/call.rs` (with the fake `claude` in `tests/common`): with `(0,0)` the argv has
  no `--max-budget-usd` at all; with `(20,0)` and nothing spent it has `--max-budget-usd 20.00`; with
  `(0,5)` it has `5.00` and a ledger already over $1,000 today still lets the call start; a no-budget
  call whose stream reports no cost leaves a ledger line with `cost_usd` 0 and `cost_from` `unknown`,
  and `spent_today` still reads.
- `builder/api/tests/`: `set_spending_limit` with `0`/`0` saves and `spending_limit` reads back
  `0`/`0`; `1000.01` daily is refused with the new `LIMIT_BAD`; `spend` shows `daily_cap_usd` 0 for a
  no-daily-limit assistant; `call_failure` says the limit is reached only by decision 4.
- `builder/cli/tests/agent.rs`: `limit set <dir> --daily-usd 0 --step-usd 0` (use the real flag names)
  prints the "No limit" sentence and `limit show` prints it again; `cap_for`'s case in decision 9.
- `builder/ui/src/screens/ConnectClaude.test.ts`: replace the tests that pin the disabled-while-
  unchanged button. New: the button is enabled with the loaded defaults and clicking it calls
  `setSpendingLimit` with them; typing `abc` in either field disables it and shows that field's
  reason (and `aria-invalid="true"`) without a click; `0` and `0` save and show "Saved. There's no
  limit."; day `0` with step `2000` shows the step's $1,000 reason; day `10` with step `20` shows
  "One step can't use more than the daily limit." on the step field; fixing the value clears the
  reason and enables the button. The help line is on the screen.
- `BuildRun.test.ts` and `Refresh.test.ts`: the spend line with `dailyCapUsd: 0`.
- Mutation checks to run and report (back up with `cp`, restore with `cp`, compare with `cmp`; never
  `git checkout --`): drop the `left` branch in `budget_for` (return `None` for no step); always pass
  `--max-budget-usd`; restore `unchangedLimit` in the button's `:disabled`; drop the "both above 0"
  condition on `call <= daily`. Each must turn a test red.

## Done when

`CARGO_NET_OFFLINE=true cargo test -j 2 -p persona-builder-agent -p persona-builder-api -p
persona-builder-cli -- --test-threads=2` (check the crate names in each `Cargo.toml`), `cargo clippy`
on those crates with `--all-targets -- -D warnings`, `cargo fmt --check`, and
`pnpm -C builder/ui test` and `pnpm -C builder/ui build` pass. Report what you changed by file, the
red you saw for each test, and the mutation results. No real person, handle or path in any fixture
(invented names only, as the existing tests use).
