# VF7b fix-up 1: the review's findings

VF7b (`plans/briefs/VF7b-subscription-limit.md`, built in `41c3a27`) drops the daily limit under a
Claude subscription. The security review and the Verifier found no must-fix. This fix-up answers the
findings V1 accepted. Read the VF7b brief first; every decision there stands unless this one changes it.

Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Public repo:
invented names only, no fixed paths. **Commit nothing; leave every change in the working tree.** Pass
`-j 2` to cargo and `--maxWorkers=2` to vitest. If your sandbox blocks sockets or pnpm, say which checks
didn't run; V1 runs them outside it.

## 1. Decisions (do not change them)

1. **`--daily-cap-usd` is ignored under a subscription (CLI).** Today `cap_for` runs before the Claude
   check, so a daily flag still clamps the step (`step.min(daily)`) and `--daily-cap-usd 2
   --call-cap-usd 5` is refused with the "can't be more than the daily limit" sentence, under a
   subscription too. Give `cap_for` a last argument `subscription: bool`. When it is `true`, the daily
   amount is taken as `0.0` whatever the flag or the saved file says (so the step is never clamped by a
   daily amount and the pair is never refused for it), and the result is passed through
   `for_sign_in(true)`. When `false`, `cap_for` behaves exactly as today. In `build`, `interview_draft`,
   `refresh_propose` and `refresh_apply`, call `cap_for` **after** `claude.check(&home)?`, with
   `ready.subscription`, and drop the separate `for_sign_in` calls there. `spend` passes its own
   `subscription`. If a test pins the old order (a bad limit refused before Claude Code is found or
   checked), keep that test's assertion and report it instead of changing the order for that command.
2. **The step stop is always there under a subscription.** In `Cap::for_sign_in(true)`, a `call_usd` of
   0 becomes `Cap::DEFAULT_CALL_USD` (5.00); any other step amount is kept. Update its doc comment: "A
   step limit of 0 becomes the $5 default, so a call can never run without a stop under a subscription."
   *Keep going* (`no_limit`, `Cap::new(0.0, 0.0)` through `open_with`) does not go through
   `for_sign_in` and is unchanged. Update the two `cap.rs` assertions this changes
   (`Cap::new(50.0, 0.0)?.for_sign_in(true).budget_for(49.999)` is now `Ok(Some(5.0))`, and
   `Cap::new(0.0, 0.0)?.for_sign_in(true)` now has `call_limit() == Some(5.0)`). The `--call-cap-usd`
   helps (all five) end with ` Under a Claude subscription, 0 means $5.`
3. **`voice limit set` releases the state lock before it checks the sign-in:** `drop` the lock right
   after `cap.save(...)?`, then decide which sentence to print.
4. **Tests at the call sites no test covers.** Dropping `for_sign_in` (or, after decision 1, passing
   `false` to `cap_for`) currently survives every test at: CLI `build`, `refresh propose`,
   `refresh apply` and `interview draft`'s new-session branch (`Session::open`); API
   `draft_interview_in` (`building.rs`) and `apply_refresh_in` (`review.rs`). Add, for each, the same
   pair the existing subscription tests use (a `claude.ai` stub on a ledger past a saved $1.00 daily
   limit runs; an `api_key` stub is refused with the limit sentence), reusing the stub harness its test
   file already has. Where a file has no harness that can run that command against a stub, say so in
   your report instead of building one.
5. **Literal expected values.** In `builder/agent/tests/cap.rs` (`subscription_session_runs_past_daily_limit_with_step_budget`)
   and `builder/cli/tests/agent.rs` (`subscription_interview_ignores_daily_limit_keeps_step_budget`),
   replace `text::cap_reached(2.0, 1.0)` in the expected value with the literal sentence
   `Today's spending limit for this assistant is reached: about $2.00 of $1.00, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit.`
6. **Pin the two unpinned UI conditions** in `ConnectClaude.test.ts`: (a) a status with
   `ready: false, subscription: true` still shows the dollar fields (the sentence needs `ready`); (b) while
   a sign-in is in progress (the existing sign-in test's pending `claudeSignIn`), no limit field is shown,
   and after it resolves to a ready non-subscription status the fields are back.
7. **A CLI test for decision 1:** under the feature with a `claude.ai` stub and 50/5 saved,
   `interview draft --daily-cap-usd 2` passes `--max-budget-usd 5.00`, and
   `--daily-cap-usd 2 --call-cap-usd 5` runs (is not refused). With an `api_key` stub the second is
   refused with the existing sentence, as today.

## 2. Test first

Write or change each test, see it fail on its assertion, then make it pass. Mutations (`cp` backup,
change, run the one test, see it fail, restore with `cp`, `cmp`; never `git checkout --`):
- `cap_for` ignores `subscription` (daily from the flag again);
- `for_sign_in(true)` keeps a 0 step;
- each call site in decision 4 passes `false`;
- `ConnectClaude.vue`: drop `ready &&` from the subscription branch; drop `!signingIn` from the fields' gate.

## 3. Checks before you stop

The same as the VF7b brief's section 3. Report the files changed, each test added or changed, each
mutation's result, and every check's last lines.
