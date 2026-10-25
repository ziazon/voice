# PB3.4 brief: the interview when the history is thin

Written by Claude (lane V1), 2026-10-24. Spec: lane V's `PB3.4` entry, split from PB3 of the Persona
Builder plan (PB-D6, PB-D9). Read the repo's `AGENTS.md`, then `plans/briefs/PB3.2-claude-driver.md`
(the driver this calls) and `crew/_template/interview.md` (the shapes). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`.
Run cargo with `CARGO_NET_OFFLINE=true`. **No new dependency.**

**Built by Claude directly:** Codex was at a hard usage limit (until 2026-10-30, probed at pickup), so
Claude implemented it at the same bar.

## What this slice does

`bootstrap/PROMPT.md` step 7 without a Claude session: when the merged corpus holds fewer than 150
answered questions, one tool-less call drafts twenty questions in the round-questions shape; the
person swaps any that do not fit, answers them (the app, PB7, shows them four at a time; the CLI asks
nothing), and the answers become training records that `merge` already picks up.

## Decisions (made, do not reopen)

- **A new crate `builder/build`** (package `persona-builder-build`, library `persona_builder_build`):
  the build's steps that call a model. `steps` stays model-free and `agent` stays free of build logic;
  PB3.3's build joins this crate. Dependencies: core, ingest, steps, agent, `serde_json`
  (`preserve_order`). `.gitignore` ignores every `build/`, so it gains `!builder/build/`.
- **Modules:** `interview.rs` (everything below) and `text.rs` (every sentence a person sees).
- **Counting:** `answered(records)` counts every `kind: "question"` record, as `install.sh` does
  (`grep -c '"kind": "question"'`). `THIN_BELOW = 150`. The corpus is `--corpus` (default
  `decisions.jsonl`) in the state folder; a missing corpus counts 0 (the no-history route).
- **Weighting:** the kinds of decision are the codebook's `categories` minus `other`, deduplicated;
  none left is refused (`text::NOTHING_TO_ASK`). Category counts exist only after labelling (step 8),
  which follows the interview, so they come from an optional `--labels <file>` (an earlier build's
  combined labels for this corpus), read through `tally` (strict: it must match the corpus). With
  counts: each kind's training records (questions plus messages), thinnest first, ties in codebook
  order. Without: codebook order, no counts, "spread the rest evenly".
- **The call** (through `Claude::call`, so the cap, the per-call budget and the ledger apply): step
  `interview`, model `--model` (default `claude::DEFAULT_MODEL`). `SYSTEM` and `schema()` are fixed
  text (the argv is visible to `ps`); the prompt (stdin) holds only the areas (`codebook.domains`
  minus `other`) and the kinds, with the rules: exactly 20, `n` 1 to 20 in order, `header` is the
  kind, at least one per kind and the rest to the thinnest (or evenly), one concrete everyday situation
  per question, 2 to 4 options with distinct labels and one-line descriptions, length caps, an honest
  `recommended` letter or null, `multiSelect` only where it fits, plain words, no real person or
  organisation, no private matters. **No record goes in the prompt**: the history is thin, and
  nothing of the person's needs to leave the machine for this step.
- **The check** (`check(weights, drafted)`): `round`'s own rules through a new
  `persona_builder_steps::round::check_questions` (the same parsing `round` uses, no answers), then
  exactly 20 questions, numbered 1 to 20 in order, each `header` one of the kinds, a question at most
  400 characters, a label 100, a description 300. Answers the file rebuilt with only `round`'s keys
  (`n`, `header`, `question`, `multiSelect`, `recommended`, `options[{label, description}]`). Errors
  name a question number and a rule, never the draft's words.
- **One retry:** a draft that fails the check is asked for again with
  `\nYour last draft didn't fit the form: <problem>. Draft all 20 questions again, following every rule above.\n`
  appended; a second misfit is `text::draft_failed(problem)` and nothing is saved. A call that fails
  (the driver's errors) or that the cap refuses is never retried.
- **Saving:** `interview-questions.json` in the state folder, pretty JSON, through `write_private`.
  Waiting questions are not replaced (the person may have swapped some, and a draft costs money)
  unless `--replace`; this is checked before any call, and again once the session holds the lock
  (from the security review: two drafts started together).
- **Order of `interview draft`:** member, codebook, corpus count (≥ 150: `text::enough`, exit 0,
  no call), waiting questions, labels and weights, the cap flags (`Cap::new`), find and `check`
  Claude Code (signed out: the driver's plain line, nothing spent), the session, the draft, the save.
  Prints `text::drafted`, `saved to ~/.claude/state/<slug>/interview-questions.json`, `text::swap`
  and today's spend.
- **`interview answer <member_dir> --answers <file> [--questions <file>]`:** both files in the state
  folder (`--questions` defaults to `interview-questions.json`). `round` with name `interview` and
  never held out, so ids are `question:interview-NN`, `split` is `train`, the answers go through the
  filters and the pick field is `null`; then `project` becomes `interview`, as the template's shape
  says. The timestamp is `round`'s (the answers file's `answered` date at noon UTC, plus `n` seconds),
  not "now": the same answers give the same records. Nothing kept is refused (`text::NO_ANSWERS`, or
  `text::ALL_LEFT_OUT` when the filters dropped every answer), so an earlier file is never emptied.
  Writes `decisions-interview.jsonl` (merge's `discover` already keeps it in training). Kept
  answers are not replaced unless `--replace` (from the Verifier: a stray partial answers file
  would otherwise drop earlier answers); replacing, never appending, since ids would repeat.
- **CLI argument:** `<member_dir>`, as every other command takes, not the spec's `<member.json>`.

## Tests

- `builder/build/tests/interview.rs`: counting, weights (with and without a tally, nothing to ask),
  the prompt's content, the check (a good draft rebuilt; each refusal's words; a sentinel header never
  echoed), the call against a stub `claude` (fixed system prompt and schema in the argv, the prompt
  on stdin, the ledger line; one retry naming the problem; two misfits stop; a failed call is not
  retried; the cap refuses before the stub runs), and the answers (shape, filters, refusal).
- `builder/cli/tests/interview.rs`: not thin (no call, no file), thin (one call, a 0600 file, the
  printed lines), waiting questions kept unless `--replace`, signed out (no call), weighted by labels,
  answers to a 0600 `decisions-interview.jsonl` that `merge` reads as always training, and nothing
  answered (no file).
