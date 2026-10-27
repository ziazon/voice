# PB2.6 brief: reading Claude Code history takes a multi-select answer stored as a list

Written by Claude (lane V2), 2026-10-26. Spec: lane V's `PB2.6` entry (filed by the attended PB7.3
walk). Read the repo's `AGENTS.md` first, then `builder/ingest/src/mine.rs` (all of it),
`scripts/crew_decisions.py` (`parse_answer`, `outcome`, `questions_in`, `harvest`, `cmd_mine`),
`tests/crew_mine_fixtures.py`, `tests/crew_fixtures.py` (`ask`), `builder/ingest/tests/mine_parity.rs`,
`builder/ingest/tests/mine_rules.rs`, `builder/ingest/tests/common/mod.rs`, `builder/api/src/dto.rs`
(`ReadCounts`), `builder/api/src/history.rs` (`read_claude_code_in`), `builder/api/tests/history.rs`
(`code_history`, `code_read_dry_run_parity_progress_and_lock`), `builder/ui/src/screens/sources/ClaudeCode.vue`,
`builder/ui/src/screens/sources/ClaudeCode.test.ts`, `builder/ui/src/test-support.ts` and
`tests/crew_decisions_test.py` (`Harness`, `ExampleMember`).

Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar: `~/.codex/skills/code-quality/SKILL.md`;
the new sentences: `~/.codex/skills/plain-language/SKILL.md`; the screen: `~/.codex/skills/frontend/SKILL.md`.
Run cargo with `CARGO_NET_OFFLINE=true` and `-j 2`. Do not commit. No new dependency, no new app
command, no change to the claude.ai reader (`claudeai.rs`).

This is a public repo: every fixture is invented (no real person, name, handle or quote). Never read
anything under `~/.claude/projects` or `~/.claude/state`.

## The problem

Claude Code now records a multi-select answer to an `AskUserQuestion` in `toolUseResult.answers` as an
array of strings, not one comma-joined string:

```json
{"questions":[{"question":"Which parts?","multiSelect":true,"options":[{"label":"A"},{"label":"B, with a comma"}]}],
 "answers":{"Which parts?":["B, with a comma","my own words"]}}
```

Each element is either an option's exact label or the person's own words (what they typed under
"Other"). A real history (counted on this machine by shape only, no text read) holds 128 such
answers, all on multi-select questions; every element is a string; 361 elements equal a label
exactly, 27 don't (at most one per list, and not always last); 20 elements contain a comma, so
splitting a joined string on commas would be wrong.

Today `mine.rs` calls `self.string(answer, "answer")?`, which refuses the array with "a transcript
record has an unexpected shape (answer)", and that one error stops the whole read: the app's *Count
first* fails on the whole history. `crew_decisions.py` parses `str(answer)`, the list's Python repr
(`"['A', 'B']"`), so its picks and own words are wrong.

## Decisions (made, do not reopen)

1. **A list answer is the picks it names, in both readers.** When the answer is a JSON array whose
   elements are all strings (whatever `multiSelect` says):
   - `picks` = the elements that equal one of the question's labels exactly, in the list's order,
     each label at most once (a repeated element is not picked twice);
   - `own_words` = every other element, each stripped of white space the way Python's `str.strip()`
     strips (Rust: `trim_matches(py_space)`), empty results dropped, joined with `", "`;
   - an empty list gives `picks = []`, `own_words = ""`.
   `outcome`, `recommended`, the member's pick field, `notes` and every other field are computed
   exactly as today from those picks and own words. A string answer is parsed exactly as today
   (`parse_answer`, unchanged). In Rust put the list rule in its own small function beside
   `parse_answer` (for example `fn list_answer(&self, answer: &[Value], labels: &[String]) ->
   Result<(Vec<String>, String)>` returning `self.shape("answer")` when an element is not a string).
   In Python, add a sibling `def list_answer(answer, labels)` with a one-line docstring and call it
   from `questions_in` when `isinstance(answer, list)`; otherwise keep `parse_answer(str(answer),
   labels, multi)`. Python's `list_answer` does the label test first and calls `.strip()` on the
   others, so a non-string element raises there (Python stops on it; see 2).
2. **A record of an unexpected shape is skipped and counted by the app's reader, not fatal.** In
   `mine_with_progress`, when `b.harvest(...)` returns an error (every error it returns is a shape
   error from `Builder::shape`; it does no I/O), skip that whole JSON record, add one to a new
   `Mined` field `pub unreadable: usize` (doc comment: records of a shape Persona Builder can't read,
   left out whole), and carry on with the next record. Nothing from a skipped record is kept: a record
   with two questions whose second is malformed yields neither. The `context` carried between
   records is whatever it was before the skipped record. Lines that are not JSON at all are still
   skipped silently and not counted (unchanged). Folder and file errors (a link, not a file, not
   UTF-8, I/O) still stop the read: they are safety checks, not shapes. Errors from `privacy(...)`
   still propagate with `?`. Python's `crew_decisions.py` keeps stopping on a malformed record
   (unchanged): it is the reference script, and this is an accepted difference the tests state.
3. **The CLI summary says so only when it happened.** `summary` keeps its two lines byte-for-byte
   when `unreadable == 0` (so Python parity on every accepted fixture is untouched). When it is above
   zero, the second line ends `; skipped N records of an unexpected shape` before its newline, e.g.
   `dropped 1 excluded folders, 0 personal records, 0 secret records; skipped 2 records of an unexpected shape\n`.
4. **The app reports it.** `ReadCounts` (in `dto.rs`) gains `pub unreadable: u32` after
   `private_left_out`; `read_claude_code_in` sets it from `count(mined.unreadable)` and to `0` in its
   no-history early return. Let `cargo test` regenerate `builder/ui/src/types/generated/ReadCounts.ts`
   and keep the regenerated file. In `ClaudeCode.vue`'s `countsLine`, right after the
   `… that looked private were left out.` sentence, add, only when `unreadable > 0`:
   ` 1 entry it couldn't read was left out.` for one, ` N entries it couldn't read were left out.` for
   more (`toLocaleString()` on N, as the other numbers). Nothing else on the screen changes. Add
   `unreadable: 0` to the `readClaudeCode` mock in `test-support.ts` and to the `counts` fixture in
   `ClaudeCode.test.ts` (its existing expected sentence stays as it is).
5. **The old error test changes meaning.** `malformed_records_error_without_record_text` in
   `mine_parity.rs` becomes `malformed_records_are_skipped_and_counted`: for each key, `mine` returns
   `Ok`, `unreadable == 1`, no kept record's JSON contains `private record sentinel`, and the summary's
   second line ends `; skipped 1 records of an unexpected shape`. Python still exits 1 for every key,
   and writes no output for every key except `timestamp` (as today). The fixture `error_answer` in
   `tests/crew_mine_fixtures.py` changes from the list `['A']` (now a valid answer) to `['A', 7]` (a
   list with a non-string element), so Python now exits 1 on it too: remove the `key != "answer"`
   exception. `empty_options_object_is_an_explicit_accepted_difference` keeps its name; Rust now
   returns `Ok` with `unreadable == 1` and no records, Python still succeeds and writes a record.

## Tests (red first)

Each must fail before the change for the reason its name says. Expected values are literals, never
recomputed the way the code computes them.

**Fixture.** In `tests/crew_mine_fixtures.py` add the case `list_answers`: one session in `FOLDER`,
each record from `ask(...)` at its own timestamp (`2026-09-01T10:0<i>:00Z`), `recommended=None` unless
said, `multi=True` unless said, with the answer replaced by these lists:

| # | question | labels | answer |
|---|---|---|---|
| 0 | `List 0?` | `A`, `A longer`, `B` | `['A longer', 'B']` |
| 1 | `List 1?` | `A`, `B` | `['B', 'A']` |
| 2 | `List 2?` | `A`, `B` | `['A', '  own words, with a comma  ']` |
| 3 | `List 3?` | `Yes, keep it`, `No` | `['Yes, keep it']` |
| 4 | `List 4?` | `A`, `B`, `recommended=0` | `['A (Recommended)']` |
| 5 | `List 5?` | `A`, `B`, `multi=False` | `['A']` |
| 6 | `List 6?` | `A`, `B` | `[]` |
| 7 | `List 7?` | `A`, `B` | `['A', '   ', 'A']` |

Add `"list_answers"` to `all_accepted_fixtures_match_python_bytes_and_summary`'s list.

**Rust, `builder/ingest/tests/mine_rules.rs`:**

1. `a_list_answer_is_the_picks_it_names` — mine the `list_answers` fixture (write it with
   `tests/crew_mine_fixtures.py` as `mine_parity.rs`'s `fixture` does, or build the same records in
   Rust; either way not from a real history) and assert, per question in timestamp order, `picks` and
   `own_words`: 0 `["A longer","B"]`,`""` · 1 `["B","A"]`,`""` · 2 `["A"]`,`"own words, with a comma"`
   · 3 `["Yes, keep it"]`,`""` · 4 `["A (Recommended)"]`,`""` with `outcome` `took_rec` · 5 `["A"]`,`""`
   · 6 `[]`,`""` · 7 `["A"]`,`""`; `unreadable == 0`.
2. `records_of_an_unexpected_shape_are_skipped_and_counted` — one session: a typed message
   `first kept`, then four bad records (an answer `7`; an answer `["A", 7]`; `options` `{"a": 1}`; the
   JSON array `[1, 2]`), then a record with two questions whose first is fine and whose second has
   answer `7`, then a typed message `last kept`. Result: `Ok`; the kept records are exactly the two
   messages; `unreadable == 5`; `summary` ends
   `0 secret records; skipped 5 records of an unexpected shape\n`.
3. `a_skipped_record_keeps_the_reply_before_it_as_context` — an assistant reply `the reply`, a bad
   user record (answer `7`), then a typed message: the message's `context` is `the reply`.

**Rust, `builder/api/tests/history.rs`:**

4. `code_read_takes_list_answers_and_skips_unreadable_records` — `code_history(&h, 1)`, then append to
   its `session.jsonl` one user record with a multi-select list answer (invented question and labels,
   answer `["Small", "keep it tiny"]` on labels `Small`, `Large`) and one with answer `7`. Read (dry
   run and saved): it succeeds (today it fails with the read sentence); `questions` is one more than
   `code_read_dry_run_parity_progress_and_lock`'s, `unreadable == 1`; the saved file's new question has
   `picks` `["Small"]` and `own_words` `keep it tiny`. Update the `ReadCounts` literals in the
   existing tests with `unreadable: 0`.

**Python, `tests/crew_decisions_test.py`:**

5. `test_mine_reads_a_list_answer` (in `ExampleMember` or a new `ListAnswer` class) — run the real CLI
   (`h.crew(member, "mine", …)`) over a session holding rows 0, 2 and 4 of the table above (built with
   `ask`), and assert the same literal `picks`, `own_words` and `outcome` as test 1 for those rows.

**Screen, `ClaudeCode.test.ts`:**

6. `says how many entries it couldn't read` — `unreadable: 3` shows `3 entries it couldn't read were
   left out.` right after the private sentence; `unreadable: 1` shows `1 entry it couldn't read was
   left out.`; `unreadable: 0` shows neither (the existing test already pins the full sentence).

## Done means

- `bash scripts/ci-local.sh` passes (it runs the Python tests, `cargo fmt --check`, clippy with
  `-D warnings`, `PERSONA_BUILDER_REQUIRE_PARITY=1 cargo test --locked`, the generated-types diff
  check, and the screens' typecheck, tests and build).
- Report each red you saw before the change and why it failed.

## Follow-up: skip only a record of an unexpected shape (the Verifier's should-fix)

Today `mine_with_progress` turns any `Err` from `Builder::harvest` into a counted skip
(`Err(_) => { mined.unreadable += 1; continue; }`). Every error `harvest` returns today comes from
`Builder::shape`, so this is safe now, but nothing keeps it so: a check added inside `harvest` later
(an I/O error, a safety refusal) would quietly become a skip. Make the skip match the shape error only.

In `builder/ingest/src/mine.rs`:

1. Add `const UNEXPECTED_SHAPE: &str = "a transcript record has an unexpected shape";` and use it in
   `Builder::shape`'s message (the text it writes stays byte for byte the same:
   `"{path}:{line}: a transcript record has an unexpected shape ({key})"`).
2. Add a private `fn unreadable(error: &Error) -> bool`: true only for `Error::Invalid(m)` whose `m`
   contains `UNEXPECTED_SHAPE`; false for every other `Invalid`, every `Io` and every `Exists`.
3. At the skip site: `Err(e) if unreadable(&e) => { mined.unreadable += 1; continue; }` and
   `Err(e) => return Err(e)`. Nothing else changes.

Test first, in the `#[cfg(test)] mod tests` at the bottom of `mine.rs`: one test,
`only_a_shape_error_counts_as_an_unreadable_record`, asserting `unreadable` is true for an error made
by `Builder::shape` (build a `Builder` the way the module's code does, or format the same message with
the constant) and false for `Error::Invalid("x is not a file".into())`,
`Error::Io { path: "x".into(), source: std::io::Error::other("boom") }` and `Error::Exists("x".into())`.
See it fail first (the function doesn't exist, so it fails to compile; that counts as red), then make it
pass. The existing `mine_rules` and `history` skip tests must stay green unchanged.

Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Match the
module's style; no new dependency; no comments beyond one line on `unreadable` saying why it is
narrow. Run `cargo fmt`, `cargo clippy -p persona-builder-ingest --all-targets -- -D warnings` and
`cargo test -p persona-builder-ingest -- --test-threads=2` (the machine is loaded). Do not commit.
