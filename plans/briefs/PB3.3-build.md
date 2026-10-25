# PB3.3 brief: the build (label, distil, backtest, write the member)

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB3.3` entry, split from PB3 of the Persona
Builder plan (PB-D6, PB-D9). Read the repo's `AGENTS.md`, then `plans/briefs/PB3.2-claude-driver.md`
(the driver every call goes through), `plans/briefs/PB3.4-interview.md` (the crate this joins) and
`bootstrap/PROMPT.md` steps 8 to 11 with the template briefs they name. Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`.
Run cargo with `CARGO_NET_OFFLINE=true`. **No new dependency** (`sha1_smol` is already in the lockfile,
through `ingest`).

**Built by Claude directly:** Codex was at a hard usage limit (until 2026-10-30, probed at pickup),
so Claude implemented it at the same bar. The branch is stacked on `lane-v1/PB3.4` (the crate it
joins), so it lands after PB3.4.

## What this slice does

`persona-builder build <member_dir>`: `bootstrap/PROMPT.md` steps 8 to 11 without a Claude session,
sub-agents or tools. Every model call goes through `Claude::call` (the cap, the per-call budget, the
ledger), one at a time, inside one `Session` (the state lock) held for the whole build.

## Decisions (made, do not reopen)

- **Where:** `builder/build/src/build.rs` (the steps and their checks), `builder/build/src/draft.rs`
  (the `decisions.md` text and the `measurements.md` entry, and writing them), the prompts as fixed
  files in `builder/build/src/prompts/` (`label.md`, `distil.md`, `backtest.md`, `include_str!`), every
  sentence a person sees in `text.rs`.
- **Fixed argv.** Each step's system prompt is the template brief rewritten for a tool-less call and
  holds no name, no codebook and nothing of the person's; each schema is fixed. The person's
  codebook, records, draft and precedents go only in the prompt (stdin), which opens with fixed text.
- **Inputs:** the merged corpus (`--corpus`, default `decisions.jsonl`, in the state folder) and the
  member's own `briefs/codebook.md` (a real file, not a link, at most 64 KiB, UTF-8; missing is a
  plain refusal). No training record is a plain refusal (`text::NOTHING_TO_BUILD`).
- **Label.** The training records, and the held-out questions projected to `chunk`'s
  `QUESTION_FIELDS` (never their answers), packed in order into parts of at most 300 records and at
  most 400,000 bytes of JSONL (a single larger record goes alone), so no part overruns a model's
  context. Names `train-01`, `heldout-01`, …. One call per part, the codebook text and the exact
  lists in the prompt. The reply is `{"labels": [{id, category, domain, escalates, signal, reason}]}`
  (nullable fields); the label object is rebuilt per kind and mode (question: category, domain,
  escalates, and reason in train; message: category, domain, signal, reason; held-out never a reason),
  then `label_problems` checks it. Combined, then `tally` (it must pass) gives the table.
- **Distil.** `distil_in` gives the groups; one call per group with rows, the group's categories
  (those with rows, codebook order) and the rows' JSONL in the prompt. The reply is structured
  (`{"sections": [{category, lean, flips, own_answer, rules, precedents}]}`, lists of one-line
  strings), not Markdown, so the shape is checked rather than trusted: each category with rows exactly
  once and no other, the lean 1 to 6 points, flips up to 5, own answer up to 3, rules up to 6,
  precedents up to 8 each starting `YYYY-MM-DD · `, every point one line of at most 400 characters,
  at most 900 words per category. Rendered into the distil brief's Markdown, with the `_Thin: N
  records…_` line written by the code below 5 rows. The model's points (never the fixed headings) must
  share no run of 6 words with the corpus (`shingle`) and hit no filter line.
- **Backtest.** `split`'s held-out questions: below 30, no call, the honest line, nothing live. Else
  batches of near-equal size, about 40 (`ceil(n / 40)` batches). Each question goes with the draft
  sections and the tally, and its 6 closest training precedents (`similar`, query = the question and
  its option labels), never the answers file. The reply is `{"predictions": [{id, pick, confidence}]}`;
  every id once, `pick` `"own"` or an exact option label (a list of distinct labels for a
  multiSelect), confidence `high|medium|low`. Then `score`. A category is **live** at 5 or more High
  picks with at least 90% right (`hits * 10 >= n * 9`).
- **One retry per call:** a reply that fails its check is asked for again with
  `\nYour last answer didn't fit: <problem>. Answer again in full, following every rule above.\n`
  appended; a second misfit stops the build with `text::misfit(<what>, <shown>)`. A problem has two
  forms: the one sent to Claude (may name ids and the overlapping runs, which are the person's own
  words and already in that prompt) and the one shown (counts and places only, never words). A call
  that fails, or that the cap refuses, is never retried.
- **The cache:** each reply that passed its check is kept as `build/<step>-<sha1>.json` in the state
  folder, the hash over the step, the model, the system prompt, the schema and the first prompt (no
  retry text). A rerun re-checks a kept reply before using it (a failing one is called again), so a
  build stopped by the cap or a failure pays again only for what it had not finished.
- **The draft:** `decisions.md` in the template's shape, from the template's own lines with the
  names filled (a test pins every fixed line to `crew/_template/decisions.md`): the front matter
  (`mined_through` the newest training date; `sources` the real counts: Claude Code questions and
  messages, claude.ai messages, interview answers, the date range), the calibration section and table
  (codebook order, `hits/n` or `—`, live yes or no; the honest line instead when skipped), the tally
  as printed, every group's sections in codebook category order.
- **The measurements entry:** `## <today>: build backtest` (Set, Method, Result, Read), or
  `## <today>: build (no backtest)` when skipped, plus `## <today>: calibration interview` when the
  corpus has interview answers. The template's two placeholder entries are removed; new entries go
  before `## Reports` (else at the end); a missing file starts with the template's heading lines.
- **Writing:** the new `decisions.md` text and the new entry pass the filters (any hit stops the build,
  places only), then both files are staged beside themselves (`.<name>.persona-builder-<pid>`, created
  new, never through a link, the old file's mode or 0644, synced) and renamed last, `decisions.md`
  first. A link in the place of either file is refused. Any failure before the renames leaves the
  member as it was.
- **Progress lines** (for the app, PB7): through a `&mut dyn FnMut(&str)` callback, in `text.rs`:
  reading, each part, each group, each batch (said "already done" when reused), checking, written.
- **CLI:** `persona-builder build <member_dir> [--corpus] [--model] [--daily-cap-usd] [--call-cap-usd]`.
  Order: member, codebook, corpus, the cap flags, find and check Claude Code (signed out: the plain
  line, nothing spent), the session, the build, the write, the summary and today's spend. A thin
  history (under 150 answered questions) gets one line pointing at `interview draft`.

## Tests

- `builder/build/tests/build.rs`: packing; each check's good case and every refusal, a sentinel word
  never in a shown problem; the full build against a sequenced stub `claude` (fixed system prompts and
  schemas in the argv, no held-out answer in any prompt, the files written, the ledger steps); a retry;
  two misfits stop with the member untouched; a failed call stops, and the rerun reuses what was paid
  for; the cap refuses with nothing written; under 30 held-out questions no backtest call; the live
  rule at its edges; a filter hit in the draft writes nothing; the measurements placeholders and the
  `## Reports` order; the template lines.
- `builder/cli/tests/build.rs`: a whole build on an invented member writes both files and prints the
  progress; signed out spends nothing and writes nothing.

## After the gates

`/security-review` (a sub-agent): no high or medium; four lows. The Verifier (Fable): pass with three
should-fixes, no must-fix. All seven are fixed, each with a test proved by a mutation:

- **Pre-flight before spending** (Verifier): `draft::preflight` checks both target files (a real
  file or none) and that `measurements.md` can be read, before Claude Code is checked or any call is
  made; `measurements.md` is read again just before the write, so an edit during the build is kept.
- **No held-out labels without a test** (Verifier): below 30 held-out questions the held-out parts
  are not labelled (they serve only the score).
- **Every category in a group** (Verifier): checked before any call (`text::no_group`).
- **Hidden characters** (security L1): a point holding a control character, a line or paragraph
  separator, an invisible format character (zero-width, bidi, soft hyphen, BOM) or `<` is a misfit.
- **Stale staged files** (security L2): staged names carry the pid, the time and a counter, and each
  write first removes `.decisions.md.persona-builder-*` and `.measurements.md.persona-builder-*`
  regular files (never a link).
- **A distil prompt's size** (security L3): a record's `msg`, `own` and `notes` are cut to 600
  characters in the distil prompt, and a group whose prompt is still over 700,000 bytes stops the
  build before it is sent (`text::too_big`). Splitting a large group is left for later.
- **A linked `briefs/`** (security L4): `draft::read_codebook` refuses it.
- Also: `label_schema`'s `domain` is nullable (a codebook may name no domains), and only
  `YYYY-MM-DD` dates reach the front matter (the security review's hardening note).
