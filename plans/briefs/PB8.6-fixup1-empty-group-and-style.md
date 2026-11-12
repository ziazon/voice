# PB8.6 fix-up 1: test the total dropping to the groups that have decisions; match the tests' style

Test and comment change on top of the uncommitted PB8.6 work in this worktree
(`plans/briefs/PB8.6-build-progress.md`). Commit nothing. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Invented fixtures only.

## 1. A build whose second group is empty (`builder/build/tests/build.rs`)

Nothing tests `total = parts.len() + groups.len() + batches.len()` after labelling: the stub labels a
question by its `header` (`Scope`, `Methods`, `Tooling`, lower-cased) and a message as `tooling`
(`tests/fixtures/claude_stub.py`, `label`), and `corpus()` always holds two messages and `Tooling`
headers, so both settings groups (`g1`: scope, methods; `g2`: tooling, other) always have decisions.

Add `the_total_drops_to_the_groups_that_have_decisions`:

- A corpus of 12 training and 32 held-out questions, no messages, every `header` `"Scope"` or
  `"Methods"` (build it from `question(...)` with the header overwritten, as the existing tests edit
  records; no new fixture file). So `g2` gets no decision and is dropped.
- Keep every `(line, done, total)`, as `the_build_counts_its_steps` does.
- `parts`, `batches` and `groups = the_book().groups.len()` (2) computed as that test does. Assert:
  the reading line's total is `parts + 2 + batches`; every label line keeps that total; the first
  distil line's total, and every later one, is `parts + 1 + batches`; there is exactly one distil line;
  the last line has `done == total == parts + 1 + batches`.

See it pass, then check it catches the fault: with the line `total = parts.len() + groups.len() +
batches.len();` in `build()` removed, it must fail (put the line back after; report both runs).

## 2. Match the file's style

- In the three new tests in `builder/build/tests/build.rs` and the new one above, open with
  `let Some(python) = python() else { return };`, as every other stub test in that file does (`python()`
  itself panics when `VOICE_REQUIRE_PARITY=1`), not `python().expect(...)`.
- `builder/build/src/build.rs`: wrap `build`'s doc comment at 100 columns like the lines around it,
  and make `stopped_times`'s doc comment read:
  `/// How many builds this part has stopped: its second tries' rejected replies, at least 1.`

## Done means

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` and
`cargo test --locked -p voice-build` pass (stub tests may not run in your sandbox; say so). Report the
mutation run.
