# PB7.8 brief: a short interview for the thin kinds only

Written by Claude (lane V1), 2026-10-26. Spec: lane V's `PB7.8` entry (from question V-Q7, answered
with option 1). Read the repo's `AGENTS.md` first, then `builder/build/src/interview.rs`,
`builder/build/src/report.rs`, `builder/build/src/text.rs`, `builder/cli/src/main.rs`
(`fn interview_draft`, `fn interview_answer` and their clap args), `builder/api/src/building.rs`
(everything from `fn object` to `keep_interview_answers_in`), `builder/api/src/dto.rs`
(`InterviewStatus`), `builder/ingest/src/corpus.rs` (`discover`),
`builder/ui/src/screens/build/Interview.vue` and `builder/ui/src/screens/Report.vue`, and their tests
(`builder/build/tests/interview.rs`, `builder/build/tests/report.rs`, `builder/cli/tests/interview.rs`,
`builder/api/tests/building.rs`, `Interview.test.ts`, `Report.test.ts`). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`;
review bar: `~/.codex/skills/code-quality/SKILL.md`; screens: `~/.codex/skills/frontend/SKILL.md`;
every sentence a person reads: `~/.codex/skills/plain-language/SKILL.md`. Run cargo with
`CARGO_NET_OFFLINE=true` and `-j 2`. Do not commit.

## The problem

The report says *answer the interview* for any kind of decision with fewer than `build::THIN` (5)
examples, even when the history holds 150 or more answered questions. Then the interview refuses:
the CLI's `interview draft` prints `text::enough`, and the app's `can_draft` fails with `ENOUGH`. A
suggestion that leads to a refusal is a dead end.

## Decisions (made, do not reopen)

1. **Two forms of the interview.** In `interview.rs` add

   ```rust
   pub enum Form { Full, Short }
   ```

   - `Form::for_answered(answered)`: `Full` below `THIN_BELOW` (150), `Short` at 150 or more.
   - Each form has its own files and round name, so keeping a short interview never overwrites a
     full interview kept earlier:

     | | `Full` (today's, unchanged) | `Short` |
     |---|---|---|
     | round and ledger step | `interview` | `interview-short` |
     | ids | `question:interview-NN` | `question:interview-short-NN` |
     | questions file | `interview-questions.json` | `interview-short-questions.json` |
     | answers file (app) | `interview-answers.json` | `interview-short-answers.json` |
     | kept records | `decisions-interview.jsonl` | `decisions-interview-short.jsonl` |

     Keep the existing `STEP`, `QUESTIONS_FILE`, `ANSWERS_FILE` and `OUT` constants as the `Full`
     values; add methods on `Form` (`step()`, `questions_file()`, `answers_file()`, `out()`).
   - Every record either form keeps still gets `"project": "interview"` (one source kind).
   - If the ledger or anything else allowlists step names, add `interview-short` there.
2. **Which kinds the short interview asks about.**
   - `pub fn thin_kinds(weights: &[(String, Option<usize>)]) -> Vec<(String, Option<usize>)>`: from
     the weights `weights`/`weights_from_report` already give (thinnest first), keep each kind with
     `Some(n)` where `n < build::THIN`, at most `MOST_THIN_KINDS = 5` of them, in that order.
   - Weights with no counts (`None`, no report and no labels) give no thin kinds.
3. **How many questions.** `PER_THIN = 4` questions on each thin kind: the count is
   `PER_THIN * kinds.len()` (4 to 20). `Full` stays at `COUNT` (20).
   - `schema`, `prompt`, `check` and `draft` take the count (or the form plus the weights; your
     choice, keep it small).
   - The `Short` prompt asks for exactly `PER_THIN` questions on each kind listed, and says these are
     the kinds of decision their history holds the fewest examples of. Every other rule is the same.
   - `check` for `Short` also refuses a draft that leaves a listed kind with no question
     (`question … the kinds of decision … each need a question`-style message, never the draft's
     words), so the one retry asks again.
4. **The CLI.**
   - `interview draft`: with 150 or more answered questions, work out the weights as today (from
     `--labels`'s tally, else the last report), then `thin_kinds`.
     - No thin kinds: print `text::enough(answered)` and exit 0, as today. Reword `enough` to
       *"Your history already holds {answered} answered questions, enough to build from, and no
       kind of decision is thin."*
     - Thin kinds: draft the `Short` form into its questions file (the `ALREADY_DRAFTED` and
       `--replace` rules apply to that file). Print a new `text::drafted_short(count, &kinds)`:
       *"Drafted {count} questions on the kinds of decision your history holds the fewest examples
       of: {kinds joined with ", "}."* Then the saved path, `text::swap` and the spend line as today.
   - `interview answer`: the form comes from the answered count, the same way. `--questions` becomes
     optional and defaults to the form's questions file; the `ALREADY_ANSWERED` check and the write
     use the form's kept-records file; the round name is the form's.
5. **The app** (`building.rs`).
   - One helper gives the form from `answered(home, slug)`. `questions`, `answers`, `status`,
     `can_draft`, `draft_interview_in`, `save_interview_answers_in` and `keep_interview_answers_in`
     all use the form's files and round name.
   - `InterviewStatus` gains `thin_kinds: Vec<String>`: the kinds a short interview would ask about.
     It is empty for the `Full` form. Regenerate the TypeScript type the way the repo already does
     (`builder/ui/src/types/generated/`).
   - `can_draft`: `Short` with no thin kinds fails with `ENOUGH` (reword it to match the CLI: *"Your
     history already holds enough answers to build from, and no kind of decision is thin."*).
   - `draft_interview_in` for `Short` drafts on the thin kinds only.
6. **The corpus.** `discover` trains `decisions-interview-short.jsonl` always, like
   `decisions-interview.jsonl` (`Mode::Train`).
7. **The report.**
   - The rule that adds `Strengthen::Interview` is unchanged.
   - `render`: with `report.answered >= THIN_BELOW` the interview item reads a new
     `REPORT_STRENGTHEN_SHORT`, *"answer a few questions about it (persona-builder interview
     draft)"*; below it, today's `REPORT_STRENGTHEN_INTERVIEW`.
8. **The screens.**
   - `Report.vue`: the `interview` help item says *Answer a few questions about it* when
     `report.answered >= report.thinBelow`, else *Answer the interview*. When the history is long and
     any area's help holds `interview`, a `btn btn-secondary` *Answer a few questions* under the table
     emits `build` (it leads to the interview card on Build).
   - `Interview.vue`, with `answered >= thinBelow`:
     - no `thinKinds` and no questions: today's *enough to build from* line, nothing else;
     - otherwise: *"Your history holds {n} answers to Claude's questions, enough to build from, but
       a few kinds of decision have fewer than five examples so far: {kinds joined with ", "}. A few
       questions on each fills them out."*, then the same draft button, the questions, keep and
       redraft flow as today. The *"Answering twenty questions fills it out."* line stays for the
       short history only.
   - The questions' paging (four a page) already handles any count.
9. **Out of scope:** a second short interview replaces the first one's kept answers (`--replace` in
   the CLI, the app's keep), as the full interview does today. Don't change that.
10. **Docs:** update `.ai/PROJECT.md`'s `builder/build/` row (the interview when the history is thin,
    or a short one on the thin kinds), and any doc or help text that says the interview is only for a
    thinner history (`grep -rn "thinner history" builder docs`).

## Tests (red first, each failing before its change)

Invented names only (Ari, Robin, kinds like *Scope* and *Go or no-go*). Use the fake Claude the
existing interview tests use.

1. `builder/build/tests/interview.rs`:
   - `Form::for_answered(149)` is `Full`, `(150)` is `Short`; each form's file names and round name.
   - `thin_kinds`: from a report with kinds at 0, 3, 4, 5 and 40 examples, the result is the three
     under 5, thinnest first; with seven kinds under 5, only the five thinnest; with no counts, none.
   - The short schema asks for exactly `4 * kinds` questions; `check` refuses a short draft with a
     listed kind left out, and accepts a good one; the short prompt lists only the thin kinds and
     says four on each.
   - `answer` with the `Short` round gives ids `question:interview-short-NN`, project `interview`.
2. `builder/build/tests/report.rs`: `render` at 150 answered shows the *a few questions* item; at
   149, today's.
3. `builder/ingest` (its corpus tests): `decisions-interview-short.jsonl` is discovered as `Train`.
4. `builder/cli/tests/interview.rs`:
   - With 150 answered and a report holding a thin kind, `interview draft` writes
     `interview-short-questions.json` with `4 * thin kinds` questions and prints the new line.
   - With 150 answered and no thin kind, it prints the reworded `enough` and spends nothing (no call
     on the ledger).
   - `interview answer` then writes `decisions-interview-short.jsonl` with `question:interview-short-NN`
     ids and leaves an existing `decisions-interview.jsonl` byte for byte unchanged.
5. `builder/api/tests/building.rs`:
   - `interview_status_in` gives `thin_kinds` for a long history with a report, empty for a short one.
   - `draft_interview_in` drafts the short form; save then keep writes the short kept file and
     leaves a full one unchanged.
   - `draft_interview_in` with a long history and no thin kinds fails with the reworded `ENOUGH`,
     with no call made.
6. `Interview.test.ts`: the long-history-with-thin-kinds text names the kinds and shows *Draft the
   questions*; long history with none shows only the *enough* line. `Report.test.ts`: the help label
   at long and short histories, and the *Answer a few questions* button emits `build`.

## Done means

- Each test above fails before its change and passes after. Mutation checks (copy the file to a
  backup with `cp`, change it, run the test, restore with `cp`, confirm with `cmp`): make
  `for_answered` always `Full` (the CLI and api short tests fail); drop the left-out-kind check in
  `check` (its test fails); point the short form at `decisions-interview.jsonl` (the byte-for-byte test
  fails).
- `cargo test -j 2` for `persona-builder-build`, `persona-builder-ingest`, `persona-builder-api` and
  the CLI package (use the real package names), `cargo clippy` on each `--all-targets -- -D warnings`,
  `cargo fmt --check`, and `pnpm -C builder/ui test` plus `pnpm -C builder/ui typecheck` all pass.
- Report the files you changed, the commands you ran and their results, and anything you could not do.
