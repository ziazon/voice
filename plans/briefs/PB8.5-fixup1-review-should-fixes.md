# PB8.5 fix-up 1: the review's should-fixes

Follow-up to `plans/briefs/PB8.5-label-misfit-reason.md` (read it first; it is landed on this branch as
`3b290d6`). Code and tests only, in `builder/build` and `builder/api`. Commit nothing. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`.

**Your sandbox cannot bind loopback sockets, so stub-driven tests fail there before any call.** Write
them, run what you can (unit tests, `cargo fmt --all --check`, `cargo clippy --all-targets --locked --
-D warnings`), and say plainly which you could not run. The reviewer runs the rest.

## 1. Undo the reviewer's stray edit

In `builder/build/tests/build.rs`, the three existing tests
`mined_mail_at_its_default_is_refused_before_a_call_or_ledger_line`,
`mined_mail_allowed_to_anthropic_is_named_in_every_build_brief` and
`merged_mbox_config_names_mail_without_a_mined_file_and_other_sources_do_not` must start again with
`let python = python().expect("python3 for the stub");` (as on `8fcf8e3`). The tests PB8.5 added keep
`let Some(python) = python() else { return };`.

## 2. Pin the two behaviours no test holds

- **Distil keeps the old retry text.** In an existing distil-retry test (or a new one: plan the first
  distil call `bad`, the retry good), assert the retry prompt ends
  `"Answer again in full, following every rule above.\n"` and does not contain `You may give only`.
- **The daily limit beats a misfit.** In `builder/api/src/building.rs`'s
  `call_failure_checks_the_next_budget_before_reporting_the_daily_limit`, also call `call_failure` with
  an `Error::Invalid(text::misfit(&text::labelling(1, 2), "1 label didn't fit the codebook"))` for each
  case: where the limit is reached the message is `cap_reached(…)`, otherwise it is the misfit text
  itself (not `BUILD_FAILED`).

Each must fail when its behaviour is broken: say how you checked (a `cp` backup, the change, the run,
`cp` back, `cmp`).

## 3. Plain wording now that distil's reasons reach the app

In `check_distil_reply`, the two `shown` texts become, with `counted` (already in `build.rs`):
- `counted(n, "point looked", "points looked") + " personal or secret"` (so `1 point looked personal or
  secret`, `2 points looked personal or secret`);
- `counted(n, "run", "runs") + " of {SHINGLE} or more of your own words"` (`your`: the app speaks to the
  person). `told` stays as it is.
Update the tests that assert the old texts (`builder/build/tests/build.rs` near the `private.shown` and
`quoted.shown` asserts, and the `contains("runs of 6 or more of their own words")` one) and any CLI test
that expects them (`grep -rn "their own words\|points looked" builder`).

## 4. The repeated-label refusal

In `check_label_reply`, when labels repeat:
- `told` parts, joined with `"; "`, only those that apply:
  `these records are labelled more than once: <record ids>` and
  `leave out the <n> repeated labels whose id is not a record here` (never starting with "and").
- `shown` parts, joined with `", "`: `counted(r, "record was", "records were") + " labelled more than
  once"` where `r` counts only record ids, and `counted(n, "label was for a record that isn't there",
  "labels were for records that aren't there")` for the repeated made-up ids.
Test: a reply where only a made-up id repeats gives exactly those texts (`told` starts `leave out`).

## 5. Drop only `label_problems`' own count line

Replace the `starts_with("ids differ: ")` filter with an exact match on the line `label_problems` writes
for this reply: `format!("ids differ: missing {}, extra {}", missing.len(), extra)` (compute `missing`
and `extra` first). Test: a part whose record id starts with `ids differ: ` and whose label has a wrong
category is refused (`told` names `<that id> category`).

## Proof (report each)

1. Red first for 2, 3, 4 and 5; green after.
2. `cargo test --locked -p voice-build` and `-p voice-api` where your sandbox allows; name the binaries
   you could not run.
3. `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings`.
4. `git diff --stat`: only `builder/build/src/build.rs`, `builder/build/tests/build.rs`,
   `builder/api/src/building.rs`, and any CLI test you had to update.
