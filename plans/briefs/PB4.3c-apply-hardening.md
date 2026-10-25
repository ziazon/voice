# PB4.3c brief: refresh apply, the last hardening from its review

Written by Claude (lane V2), 2026-10-25. Spec: the Persona Builder card `PB4.3c` (filed from
PB4.3b's repeat Verifier pass). Read the repo's `AGENTS.md`, then `plans/briefs/PB4.3b-refresh-apply.md`
(the slice this hardens), then `builder/grow/src/apply.rs`, `builder/grow/src/diff.rs`,
`builder/grow/src/rules.rs` (`outcome`, `check_measurement`), `builder/grow/src/store.rs`
(`check_static_measurement`) and `builder/cli/src/main.rs` (`refresh_apply`). Tests:
`builder/grow/tests/apply.rs`, `builder/grow/tests/diff.rs`. Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`): each test below must fail on its assertion (or, for
item 2, fail to compile) before its fix. Security rules: `~/.codex/skills/security-and-hardening/SKILL.md`;
review bar: `~/.codex/skills/code-quality/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true`.
**No new dependency.** Keep the surrounding style (dense, few comments). No real person in any
fixture. Do not edit `.ai/**` or any Markdown file; Claude writes those.

All four items are about one threat: the private records in `~/.claude/state/<slug>/growth/` may
have been edited by hand, so everything read back from them is untrusted.

## 1. A stray `\r` on an added line must not pass the hidden-character check

Today `diff::added_lines` splits with `str::lines`, which drops one trailing `\r`, and
`apply::check_candidate` trims each line before the hidden check, while `diff::apply` keeps the
`\r` in the file it builds. So a hand-edited diff row `+- ADJUST the first step.\r` on a `\n`
file puts a `\r` into `decisions.md` unchecked.

Decisions:

- `diff::added_lines(diff: &str) -> Vec<&str>` keeps its signature and doc line's meaning
  ("added content rows, excluding the two file headers"), but:
  - It splits with `diff.split_terminator('\n')`, exactly as `apply` does, then skips the two
    header rows.
  - It tracks, per hunk, whether the **base line the added row follows** ends `\r\n`: the most
    recent `' '` (context) or `'-'` (removed) row in the same hunk whose content ends with `'\r'`
    and is **not** immediately followed by the `\ No newline at end of file` row. Reset that
    state at every row starting `@@ `. An added row before any context or removed row in its
    hunk follows no base line.
  - For each `'+'` row: when its content ends with `'\r'` and the base line it follows ends
    `\r\n`, strip exactly **one** `'\r'` (so `"b\r\r"` yields `"b\r"`). Otherwise return the
    content unchanged, `\r` included.
  - Update the doc comment to say the one allowed `\r` is a CRLF file's line ending.
- `apply::check_candidate`: run the 512-character limit and the `hidden` check on every added
  line **before** trimming it, as well as on the summary and each `why` (as now). Only after that
  trim the added lines, drop empty ones, and pass them on to `refresh::check_words` as today.
  So a leading or trailing tab, `\r` or other hidden character on an added line now refuses the
  change with `text::CHANGE_DOES_NOT_FIT`. Every refusal stays `does_not_fit()`.
- `refresh.rs`'s use of `added_lines` (earlier drafts in the prompt) is unchanged in code; it
  simply gets the new rows.

Tests:

- `builder/grow/tests/diff.rs`, a new test `added_lines_allow_one_carriage_return_only_in_a_crlf_file`:
  - `unified("decisions.md", "a\r\n", "a\r\nb\r\n")` gives `["b"]`;
  - a hand-written diff on a `\n` base, `"--- a/decisions.md\n+++ b/decisions.md\n@@ -1,1 +1,2 @@\n a\n+b\r\n"`,
    gives `["b\r"]`;
  - the same with a `\r\n` base row (` a\r`) and `+b\r\r` gives `["b\r"]`;
  - an added row first in its hunk, before its context (`@@ -1,1 +1,2 @@\n+b\r\n a\r\n`), gives `["b\r"]`;
  - a base row followed by `\ No newline at end of file` does not count as `\r\n`.
  - The existing `added_lines_preserves_plus_content_and_excludes_headers` keeps passing.
- `builder/grow/tests/apply.rs`, a new test `stray_carriage_return_on_an_added_line_refuses_before_any_call`:
  build an approved proposal on the `\n` `BASE` whose diff is `diff::unified(BASE, CANDIDATE)`
  with the added row `+- ADJUST the first step.` given a trailing `\r` (string replace on the
  diff text, then `s.rewrite(&p)`); measuring it is an error whose text is
  `persona_builder_grow::text::CHANGE_DOES_NOT_FIT`, with no model call (`s.calls()`) and no file
  changed (`s.snapshot()`). Also the same with a leading tab on that added line.
  Check first that this test goes red on the current code for the reason above (it should get
  past the hidden check and reach a model call or a different error).
- `builder/grow/tests/apply.rs`, a new test `crlf_decisions_keep_their_line_endings`: a member
  whose `decisions.md` is `BASE` with every `\n` replaced by `\r\n`, a proposal from
  `diff::unified` of that and `CANDIDATE` in `\r\n` form (base version of the `\r\n` text),
  approved; measuring keeps it and `finish` writes exactly the `\r\n` candidate. Add a small
  helper on `Setup` if `proposal`'s fixed `BASE` gets in the way; keep the existing helpers'
  behaviour.

## 2. `finish` gets the proposal `measure` checked

Today the CLI re-reads the proposal from the store between `apply::measure` and `apply::finish`.

Decisions:

- Replace `pub enum Measured { Done(Measurement), Already(Measurement) }` with
  ```rust
  #[derive(Debug)]
  pub struct Measured {
      pub proposal: Proposal,
      pub measurement: Measurement,
      pub already: bool,
  }
  ```
  `measure` returns the very `Proposal` value it read and checked (move it in; no second read).
- `builder/cli/src/main.rs` `refresh_apply`: destructure `Measured { proposal, measurement, already }`
  and pass that `proposal` to `finish`; delete the second `store.proposal(&id)` read. Remove any
  import that becomes unused. Output text unchanged.
- Update every `Measured::Done` / `Measured::Already` use in `builder/grow/tests/apply.rs` to the
  struct (`Setup::measured` asserts `!already`; the resume tests assert `already` and compare
  `measurement`).
- Test: in `keep_saves_valid_measurement_before_files_and_activates_named_tests` (or a new small
  test), assert the returned `proposal` equals the stored one that was approved.

## 3. The duplicate-approval test checks its message

`duplicate_approvals_refuse_before_any_call` asserts only `is_err()`. Make it assert the error
text equals `persona_builder_grow::text::rule(&p.id, persona_builder_grow::text::ALREADY_APPROVED)`
(check how the error displays; match the full string). Prove it red by temporarily removing the
`rules::one_approval(&approvals)?;` line in `measure` (then restore it, `cmp` against a backup).

## 4. A saved undone result is checked by the keep-or-undo rule again

On a rerun (`measure` finds a saved measurement), a saved result whose outcome is `Revert` is
today checked only for its tests existing, its approval and its base. A hand edit can leave an
"undone" result whose grades say it should have been kept.

Decision: in `measure`'s saved-measurement path, after `check_static_measurement` and the
approval/base check, when `measurement.outcome == KeepOrRevert::Revert` also run
`rules::check_measurement(&measurement, &proposal, &approval, &cases)`, mapping any error to
`does_not_fit()`. Do **not** run it for `Keep`: after `finish`, the named tests are `active`, so
rule 4 recomputed on today's tests would wrongly refuse a genuine kept result (the
`full_apply_is_idempotent_even_after_named_tests_became_active` test guards that).

Why this never refuses a genuine undone result: `finish` changes no test when it undoes, and a
genuine result's tests are exactly the ones `to_measure` gave when it ran, so recomputing gives
`Revert` again (from the same grades, or from a changed test set).

Tests, in `builder/grow/tests/apply.rs`:
- `resume_rechecks_an_undone_result_by_the_keep_rule`: measure a change that is kept (do not call `finish` first, so its named tests are still
  `new`), then hand edit the saved measurement's `outcome` to `Revert` (grades untouched; write it back the way
  `resume_rechecks_saved_measurement_and_proposal` does). Measuring again is an error whose text
  is `text::CHANGE_DOES_NOT_FIT`, with no model call and no file changed.
- A genuine undone result still resumes: in the existing revert test (or a new one), after the
  first `finish`, `measure` again returns `already == true` with no new model call, and `finish`
  again writes nothing new.

## Done means

- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test -p persona-builder-grow -p persona-builder-cli` pass (`CARGO_NET_OFFLINE=true`).
- For each item, a mutation check: back up the file (`cp`), undo just that fix, run the named
  test and see it fail, restore (`cmp` the backup; never `git checkout --`). List each mutation
  and its result in your final message.
- Do not commit. Leave the changes in the working tree and end with a short summary of the files
  changed and the mutation results.
