# PB8.5 fix-up 2: a record id that repeats in a part is missing once

Follow-up to `plans/briefs/PB8.5-label-misfit-reason.md` and `…-fixup1-review-should-fixes.md` (both on
this branch). Code and tests only: `builder/build/src/build.rs`, `builder/build/tests/build.rs`. Commit
nothing. Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.
Your sandbox cannot bind loopback sockets: the unit-level check tests below need none, so run them.

## The fault

`label_problems` (`builder/steps/src/labels.rs`) reads a part's records through `ById`, so a record id
that appears twice counts once. `check_label_reply` builds `missing` from `part.records` directly, so the
same id is listed twice, `count_problem` no longer equals `label_problems`' line, and that line is
reported as a wrong label. The Verifier's probe (a part holding one id twice, no label for it):
told `these records have no label: question:t000, question:t000; these labels are wrong (the record id,
then the field): ids differ: missing 1, extra 0`, shown `2 records had no label, 1 label didn't fit the
codebook`.

## The change

- In `check_label_reply`, build `missing` from the part's record ids in first-appearance order, each id
  once (a `HashSet` of ids seen), so it matches `ById`.
- In `check_backtest_reply`, build `missing` the same way from the batch's question ids.
- Nothing else.

## Tests (red first)

In `builder/build/tests/build.rs`, beside the existing label and backtest check tests:
1. A train part whose records hold `question:t000` twice (the rest as the existing `part(Mode::Train)`
   fixture), with a reply that labels every other record correctly and not `question:t000`: the misfit's
   told is exactly `these records have no label: question:t000`, and shown exactly
   `1 record had no label`.
2. A backtest batch holding `question:h000` twice and `question:h001` once, predicting only
   `question:h001` (correctly): told exactly `these questions have no prediction: question:h000`, shown
   exactly `1 question had no prediction`.

## Proof (report each)

1. Both tests fail before the change (say on which assertion) and pass after.
2. A mutation per test (`cp` backup, dedupe removed, run, `cp` back, `cmp`; never `git checkout`).
3. `cargo test --locked -p voice-build --test build label_replies backtest_replies` and the two new tests
   (whatever runs without loopback), `cargo fmt --all --check`,
   `cargo clippy --all-targets --locked -- -D warnings`.
4. `git diff --stat`: only the two files.
