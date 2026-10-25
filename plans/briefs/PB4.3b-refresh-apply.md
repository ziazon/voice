# PB4.3b brief: refresh, apply, re-measure, keep or revert

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB4.3b` entry, split from PB4.3 of the
Persona Builder plan (PB-D5, PB-D13). Read the repo's `AGENTS.md`, then `docs/spec/growth-loop.md`
(version 1: rules 2, 3, 4, 7, 8 and 9 are what this slice implements), then
`plans/briefs/PB4.3a-refresh-propose.md` (the slice this finishes: `builder/grow/src/refresh.rs`,
`diff.rs`, `edit.rs`, `version.rs`), then `builder/grow/src/rules.rs` and `store.rs`, and
`builder/build/src/build.rs` (the `Asker`, `batches`, the backtest's prompt and check) and
`builder/build/src/draft.rs` (`write`, `measurements`, `check_private`, `preflight`). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`;
every sentence a person sees: `~/.codex/skills/plain-language/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true`. **No new
dependency**: everything below uses crates already in `builder/grow/Cargo.toml` and the lockfile.

## What this slice does

The second half of a refresh. `persona-builder refresh apply <member_dir> <CHANGE>` takes a change
the person approved, tests the assistant on every test from their answers before and after the
change, and keeps the change only when every new test passes and nothing got worse. Otherwise the
assistant stays as it was. Either way the result is written down, and the person sees it.

## Decisions (made, do not reopen)

### Where the code goes

- New module `builder/grow/src/apply.rs` (`pub mod apply;` in `lib.rs`). Its fixed system prompts
  are `builder/grow/src/prompts/answer.md` and `builder/grow/src/prompts/grade.md`,
  `include_str!`'d as `pub const ANSWER_SYSTEM` and `pub const GRADE_SYSTEM`. Every sentence a
  person sees goes in `builder/grow/src/text.rs`.
- `builder/build/src/build.rs`: `batches` becomes generic, behaviour unchanged:
  `pub fn batches<T>(items: &[T]) -> Vec<&[T]>`. The build's call site stays as it is.
- `builder/build/src/draft.rs`: add `pub fn write_measurements(member_dir: &Path, measurements:
  &str) -> Result<()>` and `pub fn write_decisions(member_dir: &Path, decisions: &str) ->
  Result<()>`: each `sweep`s, then stages and renames its one file, exactly as `write` does for it.
  `write` is unchanged.
- `builder/grow/src/refresh.rs`: move the word checks at the end of `check_reply` (the filters loop
  and the shingle check, from `for (at, value) in words.iter().enumerate()` to the `overlaps` hit)
  into `pub(crate) fn check_words(words: &[String], cases: &[Case], signals: &[Signal], filters:
  &Filters) -> std::result::Result<(), Misfit>`, and call it from `check_reply`. Behaviour and
  messages unchanged; `apply.rs` reuses it. Make `decisions(member_dir)` `pub(crate)`.

### The session and the lock

`apply` spends (model calls), so the CLI opens a `Session` as `refresh propose` does, and the
`Session` holds the state lock for its whole life. So nothing in `apply.rs` calls `Store::lock` or a
locked `save_*`/`append_*`: every write goes through the `_unlocked` methods, and a comment says so,
as `propose` does.

### Step 1: refusals, all before any call

`pub fn measure(claude, session, home, store, member, member_dir, filters, model, id, now: &dyn
Fn() -> String) -> Result<Measured>`, `pub enum Measured { Done(Measurement), Already(Measurement)
}`. In this order, each refusal before any model call and before any write:

1. The proposal exists (`text::no_proposal(id)`). It passes `check_stored` (make that `pub(crate)`),
   else `text::DAMAGED_PROPOSAL`.
2. Its approval: the one approval whose `proposal` is `id`. None: `text::not_answered(id)`. Rejected:
   `text::was_rejected(id)`. Then `rules::may_apply` (role and decision).
3. A measurement for this proposal already exists (`store.measurements()`, by `proposal`): return
   `Measured::Already(it)`, no call, nothing else checked here (step 4 finishes what is left).
4. `draft::preflight(member_dir)` (both files replaceable, `measurements.md` readable).
5. The current `decisions.md` (`refresh::decisions`) has the proposal's `base` as its version
   (`rules::base_is_current`; its text already says to draft a new one).
6. The candidate: apply each change's `diff` in order with `diff::apply(text, diff, "decisions.md")`,
   starting from the current text; every `change.file` must be `decisions.md`. Any failure:
   `text::CHANGE_DOES_NOT_FIT`. The candidate is at most `DECISIONS_MOST` bytes and differs from the
   base, else the same refusal.
7. **The candidate is checked again, as PB4.3a checks a draft** (the stored proposal is untrusted on
   the way back in): the words are the summary, every `why`, and every added line of every diff (a
   line starting `+` but not `+++`, without its `+` and any trailing `\r`, trimmed, skipping empty
   ones). Each word line is at most 512 characters and holds no `hidden` character; then
   `refresh::check_words(words, cases, signals, filters)` with the proposal's cases (from the case
   book) and their signals. Any failure: `text::CHANGE_DOES_NOT_FIT`, showing nothing from the text.
   Then `draft::check_private(filters, &candidate, "")` on the whole candidate (its error as it is).
8. `rules::to_measure(proposal, &cases)`: the list to run, `(case id, new)`, in that order. Its error
   (a named case no longer `new`) is told as `text::cases_moved(id)` ("Some of the tests this change
   was meant to pass have changed since you approved it. Draft a new one with `persona-builder
   refresh propose`.").

### Step 2: the answers (one call per batch per version)

- For each version, base text then candidate text, the cases to run go in `build::batches` (about 40
  each), one call per batch: `asker.ask("answer", ANSWER_SYSTEM, &answer_schema(), &prompt,
  text::TESTING, check)`, with an `Asker::new(claude, session, home, &member.slug,
  model).with_misfit(text::apply_misfit)`. One `Asker` for the whole run, so a misfit is told back
  once per call and replies are cached (a rerun after a failure reuses them and spends nothing).
- `pub fn answer_schema() -> Value`: `{"answers": [{"id": string, "answer": string}]}`,
  `additionalProperties: false` at each level, every key required.
- `pub fn answer_prompt(decisions: &str, batch: &[&Case]) -> String`: "Answer each test the way the
  person would.\n\nHow the person decides (their decisions.md):\n\n{decisions}\n\nThe tests, one JSON
  object per line:\n" then one line per case: `{"id", "area", "situation"}` plus `"options"` when the
  case has them. **Never `expected`, never `match`, never the signals, never which version this is.**
- `pub fn check_answers(batch: &[&Case], reply: &Value) -> std::result::Result<BTreeMap<String,
  String>, Misfit>`: the object has exactly the key `answers`; exactly one answer per case id in the
  batch, none missing, none extra, none twice; a case with `options` must answer exactly one of them
  (string equality, no trimming); a case without options answers a trimmed non-empty line of at most
  1,000 characters with no `hidden` character. Problems are told back as `<case id> <what>` (ids are
  ours, never the person's words); what is shown is a count only ("{n} of its answers didn't fit").
- `answer.md` (fixed text, it goes in argv; write exactly this):

  > You are one person's AI assistant, built by Persona Builder to answer the way they would. The
  > message gives how the person decides (their decisions.md), then tests, one JSON object per line.
  > Answer every test the way the person would, from how they decide. When a test has `options`,
  > answer with the exact text of one option, copied exactly. When it has none, answer in one or two
  > plain sentences in your own words. You have no tools. Answer only with the JSON the schema asks
  > for: exactly one answer for every test id, and none for an id that is not there.

### Step 3: the grades

- A case with `match: choice` passes when its answer equals `expected` exactly. No call.
- A case with `match: rubric` is graded by a second call that sees `expected` and the answer only,
  never the case id, the situation, the decisions or which version the answer came from:
  - Collect the distinct `(case id, answer)` pairs over both versions (an answer the two versions
    share is graded once, and that grade is used for both).
  - Order them by the lowercase SHA-1 hex (`sha1_smol`) of `"{case id}\n{answer}"`, then label them
    `item-1`, `item-2`, … in that order. So the order is fixed (the cache works) and says nothing of
    the version.
  - `build::batches` of the items, one call each: `asker.ask("grade", GRADE_SYSTEM,
    &grade_schema(), &prompt, text::GRADING, check)`.
  - `pub fn grade_schema() -> Value`: `{"grades": [{"id": string, "grade": "pass"|"fail"}]}`, closed.
  - `pub fn grade_prompt(items: &[(String, &str, &str)]) -> String` (label, expected, answer):
    "Grade each answer against what the right answer must do.\n\nThe items, one JSON object per
    line:\n" then `{"id", "expected", "answer"}` per line.
  - `check_grades`: exactly one grade per item label in the batch, none missing, extra or twice,
    each `pass` or `fail`; shown as a count.
- `grade.md` (fixed text; write exactly this):

  > You grade answers. Each item, one JSON object per line, has an `id`, `expected` (what the right
  > answer must do or say) and an `answer`. Grade "pass" when the answer does what `expected`
  > describes, in substance, even in other words. Grade "fail" when it misses it, contradicts it, or
  > picks something `expected` rules out. Judge each item on its own. You have no tools. Answer only
  > with the JSON the schema asks for: exactly one grade for every item id.

### Step 4: the measurement, saved first

- Re-read `decisions.md`; if its version is no longer the base, save nothing and refuse with
  `text::DECISIONS_CHANGED_TESTING` ("Your assistant's decisions changed while the change was being
  tested, so nothing was saved. Draft a new one with `persona-builder refresh propose`.").
- `let at = now();` (called after the calls: `at` is when the run finished). The measurement:
  `schema` `MEASUREMENT_V1`, `id` `store.next_id(Prefix::Measurement, &at[..10])`, `proposal`,
  `approval` (the approval's id), `at`, `base` the proposal's base, `candidate`
  `version(candidate.as_bytes())`, `results` one `CaseResult` per `to_measure` entry in that order
  (`before` the base's grade, `after` the candidate's), `outcome` from `rules::outcome(&m, proposal,
  approval, &cases)` (build `m` with a placeholder outcome, then set it), `reason` (below). Then
  `rules::check_measurement`, then `store.save_measurement_unlocked`.
- `reason`, one plain sentence (text functions):
  - keep: "Every new test now passes, and none of the others got worse."
  - revert, a new test fails after: "{n} of the {m} new tests still didn't pass, so the change was
    undone." (singular forms right: "1 of the 2 new tests still didn't pass")
  - revert, else (a regression): "{n} earlier test(s) that passed before failed with the change, so
    it was undone." (singular/plural right).

### Step 5: finish (files, then case status), and resume

`pub fn finish(store, member, member_dir, filters, proposal: &Proposal, measurement: &Measurement)
-> Result<Finished>`, `pub struct Finished { pub outcome: KeepOrRevert, pub left: Vec<Left> }`,
`pub enum Left { Decisions, Entry, Cases }` (what this call had to do; empty when nothing was left).
The CLI's `apply` calls `measure` then `finish`; a `Measured::Already` goes straight to `finish`.

- **The entry** for `measurements.md`: heading `## {day}: refresh ({proposal id})` where `day` is
  `&measurement.at[..10]`, then a blank line, then (text functions):
  - `- **Change:** {summary}`
  - `- **Tests:** {a} of {b} new tests pass; {c} of {d} earlier tests held.` (`a` = new with `after:
    pass`, `b` = new; `c` = not-new that did not go `pass` → `fail`, `d` = not-new; when `d` is 0:
    `- **Tests:** {a} of {b} new tests pass; there were no earlier tests.`)
  - `- **Outcome:** kept.` or `- **Outcome:** undone.`, then a space and the measurement's `reason`.
  Counts and the summary only. It is present when `measurements.md` holds a line equal to the
  heading.
- **Keep:**
  - The current version of `decisions.md` equals `candidate`: the file is done. It equals `base`:
    rebuild the candidate (apply the diffs as in step 1.6; its version must equal `candidate`, else
    `text::CHANGE_DOES_NOT_FIT`), `draft::check_private(filters, &candidate, &entry)`, then, when the
    entry is missing too, `draft::write(member_dir, &candidate, &measurements)` (both, `decisions.md`
    first); when the entry is already there, `draft::write_decisions(member_dir, &candidate)` (add
    it beside `write_measurements`: `sweep`, then stage and rename `decisions.md` only). Neither base nor
    candidate: refuse with `text::DECISIONS_MOVED_AFTER` ("Your assistant's decisions.md changed
    after this change was tested, so it wasn't applied. Draft a new one with `persona-builder
    refresh propose`.") and write nothing.
  - The entry missing (the file already done): `draft::check_private(filters, "", &entry)`, then
    `draft::write_measurements`. The new `measurements.md` text is `draft::measurements(existing,
    member, &entry)` on the file read with `draft::read_text(..., MEASUREMENTS_MOST)`.
  - Then for each case the proposal names whose last line is not `active`: append it again with
    `status: active` (`append_case_unlocked`, every other field as it is).
- **Revert:** `decisions.md` is never written (not even with the same bytes). The entry, when
  missing, as above through `draft::write_measurements`. The cases stay `new`; nothing appended.
- `apply` (the CLI) prints, in order: for a `Done`, `text::kept(id)` ("Kept. Your assistant now
  works this way.") or `text::undone(id)` ("Undone. Your assistant is as it was, and your answers
  stay saved for the next refresh."), then the reason; for an `Already` with something left,
  `text::finished_left(id)` ("This change was already tested; finished what was left."); with
  nothing left, `text::nothing_left(id, kept)` ("This change ({id}) was already tested and kept."
  / "…and undone."). Then the report (below), then the spend line as `propose` prints it.

### The report's growth line

- `pub fn growth(store: &Store, since: Option<&str>) -> Result<Option<String>>` in `apply.rs`:
  the measurements whose `at[..10]` is not before `since` (the report's `built` day; `None` counts
  all). None: `None`. Else `text::growth(kept, undone, passing, of)` from the newest of them (by
  `at`, then `id`): `passing` = results whose grade on the version now in place is `pass` (`after`
  when it was kept, `before` when undone), `of` = its results. Words: "Since the build: {kept}
  change(s) kept and {undone} undone. Your assistant passes {passing} of {of} tests from your
  answers." (singular/plural right; no "proposal", "case" or "measurement").
- Printed after the report's lines (`report::read` then `report::render`, `since` its `built`) by
  `refresh apply` and by the existing `persona-builder report` command. With no `report.json`,
  `refresh apply` prints the growth line alone and `report` keeps its no-report line, then the
  growth line when there is one. `report.json`'s schema is unchanged.

### The CLI (`builder/cli/src/main.rs`)

- `RefreshCommand::Apply { member_dir, proposal, --model, --daily-cap-usd, --call-cap-usd }`, doc
  line "Test an approved change on all your answers, then keep it or undo it"; the `proposal`
  argument as the other subcommands write it (`value_name = "CHANGE"`, the same doc line); the flags,
  defaults and docs exactly as `refresh propose`'s. It finds and checks Claude Code, opens the
  `Session` and the cap as `refresh_propose` does, and passes `&utc_now` as the clock.
- `.ai/PROJECT.md`: the `builder/grow/` row says it also tests an approved change and keeps or undoes
  it, and its tests column gains nothing new unless you add a CLI test file (the new tests go in the
  existing `builder/cli/tests/refresh.rs`).

### Words (`text.rs`)

Plain, warm, no tooling words in what a person reads ("change", "tests", "your answers"; never
"proposal", "case", "diff", "measurement", "candidate", "base"). Use the sentences above; adjust only
for clarity. `TESTING` "Testing the change on your answers"; `GRADING` "Checking the written
answers"; `not_answered(id)` "That change ({id}) is still waiting for your answer. Look at it with
`persona-builder refresh show`, then approve or reject it."; `was_rejected(id)` "You rejected that
change ({id}), so it can't be applied."; `CHANGE_DOES_NOT_FIT` "That saved change no longer fits your
assistant, so nothing was changed. Draft a new one with `persona-builder refresh propose`.";
`apply_misfit(what, problem)` "{what}: Claude's answers didn't fit twice, so nothing was saved
({problem}). Run `persona-builder refresh apply` again."

## Tests (red first: each a failing assertion before its code)

Copy `builder/grow/tests/fixtures/refresh_stub.py` to `builder/grow/tests/fixtures/apply_stub.py`
and change it: it tells the step from the prompt's first line ("Answer each test…" or "Grade each
answer…"). For answers: it answers every test; a choice test gets the option whose text holds the
marker word `plan["right_when"]` (default `ADJUST`) when the prompt's decisions hold that word, else
the first option; an open test gets `"Offer a smaller first step."`. For grades: `pass` when the
answer holds `smaller`, else `fail`. `plan.json` can make call N answer `bad` (one answer not among
the options), `missing` (one answer left out), `hidden` (a `​` in an open answer), `move`
(append to `plan["decisions_path"]` during the call) or `error`. It records stdin and argv per call
as the refresh stub does.

- `builder/grow/tests/apply.rs`, on a member and growth store made as `builder/grow/tests/refresh.rs`
  makes them (invented names only), a proposal saved through `refresh::propose` with the refresh stub
  or written through the store, and its approval through `refresh::answer`:
  - **keep:** a new choice test fails on the base and passes on the candidate (the candidate's added
    rule holds `ADJUST`), the active tests hold: the measurement is saved and valid (`check_all`,
    `check_measurement`), `outcome: keep`; `decisions.md` is byte-for-byte the candidate (the diffs
    applied to the base); `measurements.md` gains the entry once, before `## Reports` when there is
    one; each named test's last line is `active`; the reason is the keep sentence.
  - **revert, a new test still fails:** the candidate lacks the marker: `outcome: revert`;
    `decisions.md` is byte-identical **and its modified time unchanged**; the tests stay `new`; the
    entry says undone; afterwards `refresh::open` finds nothing and a second `propose`'s prompt lists
    the change under the "said no or undone" heading.
  - **revert, a regression:** an `active` test passes on the base and fails on the candidate (the
    candidate replaces the point the base's right answer depends on): `revert`, the regression
    reason.
  - **rubric:** a rubric test is graded by the grade call: its prompt holds `expected` and the
    answer, and never the case id, the situation, any line of `decisions.md` or the words "base" or
    "candidate"; when both versions answer the same, one item is graded. A choice-only run makes no
    grade call.
  - **blind answers:** no answer prompt holds a sentinel word put only in a rubric case's
    `expected`, nor any signal's `wanted`; for choice cases (whose `expected` is one of the options,
    so it is in the prompt by design) no answer prompt line carries an `expected` or `match` key,
    and the options appear in their stored order.
  - **refusals, each with no call made (no `stdin*` file) and nothing written** (store and member
    folder unchanged): an unknown id; no approval; rejected; a moved base; a named test already
    `active`; a stored diff that doesn't apply; a stored added line holding a filter hit, a 6-word
    run of a case's `expected`, and a `‮`; a change naming a file other than `decisions.md`.
  - **decisions changed during the calls** (`move`): nothing saved, the testing refusal.
  - **misfits:** each of `bad`, `missing`, `hidden` told back once and a good retry kept; two misfits
    refuse with the apply misfit sentence and save nothing; what is shown never holds the open
    answer's words.
  - **resume:** after `measure` alone (keep), `finish` writes both files and the statuses and returns
    `[Decisions, Entry, Cases]`; after a keep where `decisions.md` is the candidate but the entry is
    missing (write the candidate by hand), `finish` writes only the entry and the statuses; after a
    full apply, `measure` returns `Already` with no call and `finish` returns nothing left and writes
    nothing; a keep whose `decisions.md` is neither version refuses and writes nothing. A rerun of
    `measure` after a failed write reuses the cache (no new `stdin*` file).
  - **growth:** after one keep and one undo, the line counts 1 and 1 and the newest one's passing
    tests; `since` after both gives `None`.
- `builder/cli/tests/refresh.rs`: on the invented member, `propose`, `approve`, then `refresh apply`
  prints the kept line, the reason and the growth line; `persona-builder report` then shows the growth
  line; `refresh apply` again prints the nothing-left line and calls no Claude Code; `apply` on an
  unanswered change prints the waiting line.
- The build's and PB4.3a's tests stay green (`batches` and the `check_words` move are
  behaviour-free).
- **Mutation checks:** for each claim (each refusal in step 1, the blind prompt, the grade's
  blindness and dedupe, the choice exact match, each `check_answers` and `check_grades` rule, the
  re-read in step 4, measurement-before-files, revert never writing `decisions.md`, the active
  appends, each resume branch, the lock use), break the line (`cp` a backup, edit, run, `cmp` back),
  confirm a test goes red, restore. Never `git checkout --`. List them in your final message.

## Done when

`bash scripts/ci-local.sh` is green (fmt, clippy `-D warnings`, every test), each new test was seen
red first, the mutation list is in your final message, and a revert leaves `decisions.md` untouched.
The seven `ureq_wire.rs` tests can't bind a socket in your sandbox: say so and run everything else.
Don't commit: leave the changes in the worktree and say which files changed.

## Fix-up pass (decided, do not reopen)

The first pass stopped on a real contradiction in the blind-answers test: a choice case's
`expected` is one of its options, so it is in the prompt. The test bullet above is corrected; apply
it. Then:

1. Fix the clippy `collapsible_if` in `builder/grow/tests/apply_reply.rs` (and anything else
   `cargo clippy --all-targets -- -D warnings` raises).
2. Run the remaining mutation checks the brief lists for the apply flow and the CLI (each refusal in
   step 1, the re-read in step 4, measurement-before-files, revert never writing `decisions.md`, the
   active appends, each resume branch, the grade's blindness and dedupe, the lock use, the growth
   counts), each with `cp` backup, edit, run, `cmp` restore. List each with its red result.
3. Delete `.ai/handoffs/PB4.3b-refresh-apply.md`: the slice record is Claude's, in
   `.ai/handoffs/persona-builder.md`.
4. Run `bash scripts/ci-local.sh` (the `ureq_wire.rs` socket tests may fail in your sandbox: say so
   and confirm everything else passed).
