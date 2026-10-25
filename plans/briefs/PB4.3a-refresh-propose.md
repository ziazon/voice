# PB4.3a brief: refresh, draft the change, then approve or reject it

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB4.3a` entry, split from PB4.3 of the
Persona Builder plan (PB-D5, PB-D13). Read the repo's `AGENTS.md`, then `docs/spec/growth-loop.md`
(version 1, the spec this implements) and its schemas, then `plans/briefs/PB4.2-kept-answers.md`
(the slice this builds on: `builder/grow`'s records, rules and store) and `builder/build/src/build.rs`
(the build's checked, cached, retry-once model calls, which this slice reuses). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`;
every sentence a person sees: `~/.codex/skills/plain-language/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true`. **No new
dependency**: `sha1_smol = "=1.0.1"` and the workspace crates are already in the lockfile.

## What this slice does

The first half of a refresh. The person's own Claude Code reads the tests their answers made (the
`new` cases) and drafts a change to `decisions.md`; Persona Builder builds the diff itself, checks
every new line, keeps the proposal privately and shows it; the person approves or rejects it. Nothing
here changes the member's folder: applying, re-measuring and keep-or-revert are PB4.3b.

## Decisions (made, do not reopen)

### Where the code goes

- `builder/grow` gains the dependencies `persona-builder-agent = { path = "../agent" }`,
  `persona-builder-build = { path = "../build" }` and `sha1_smol = "=1.0.1"` (build does not depend
  on grow, so there is no cycle). New modules: `version.rs` (the persona's version), `diff.rs`
  (unified diffs), `edit.rs` (turning edits into a new `decisions.md`), `refresh.rs` (propose,
  show, approve, reject). The fixed system prompt is `builder/grow/src/prompts/refresh.md`,
  `include_str!`'d as `pub const REFRESH_SYSTEM`. Every sentence a person sees goes in
  `builder/grow/src/text.rs`.
- **The build's asker becomes public**, unchanged in behaviour: in `builder/build/src/build.rs`,
  `struct Asker` becomes `pub struct Asker<'a>` with private fields, a `pub fn new(claude: &'a
  Claude, session: &'a Session, home: &'a Path, slug: &'a str, model: &'a str) -> Self`,
  `pub fn ask(...)` (today's signature) and `pub fn calls(&self) -> usize`. `Misfit::new` and
  `Misfit::plain` become `pub`. `build()` constructs it through `Asker::new` and reads
  `asker.calls()`/`reused` as it does now (add `pub fn reused(&self) -> usize` and use it). The
  cache stays the state folder's `build/`, keyed as today by `cache_name`, with step `"refresh"`.

### The version (`version.rs`)

- `pub fn version(decisions: &[u8]) -> String`: the git blob id of the bytes, lowercase hex, 40
  digits: SHA-1 of `blob <decimal length>\0` followed by the bytes (what `git hash-object` prints).
  A v1 proposal changes only `decisions.md`, so its version names the persona's state.
- `pub const DECISIONS_MOST: u64 = 256 * 1024`. Read `decisions.md` with
  `persona_builder_build::draft::read_text(path, DECISIONS_MOST)` (no link, a regular file, UTF-8).
  A missing file refuses with `text::NOT_BUILT` ("Build your assistant first: there's nothing to
  refresh yet.").

### The diff (`diff.rs`)

- `pub fn unified(file: &str, old: &str, new: &str) -> String`: a unified diff with headers
  `--- a/<file>` and `+++ b/<file>`, hunks `@@ -<start>,<count> +<start>,<count> @@` (the count is
  always written; a zero-count side's start is the line before, as GNU diff writes it), 3 lines of
  context, adjacent hunks merged when their context overlaps. Lines are split on `\n`; `\r` is part
  of a line. A side whose last line has no final `\n` gets `\ No newline at end of file` after that
  line. Find the changed region by trimming the common leading and trailing lines first, then a line
  LCS over what is left; refuse (return an error) when what is left is over 2,000 lines on either
  side (every v1 edit changes one or two lines, so this never happens in use).
- `pub fn apply(old: &str, diff: &str, file: &str) -> Result<String>`: applies a diff strictly:
  the headers must name `a/<file>` and `b/<file>`, hunks in order and not overlapping, every
  context and removed line must equal the file's line at that place exactly (no fuzz, no offset),
  the counts must match the hunk's lines. Any mismatch is `Error::Invalid(text::DIFF_DOES_NOT_FIT)`.
  PB4.3b applies with it; this slice uses it to prove every diff it writes (`apply(old,
  unified(old, new)) == new`, checked before a proposal is saved).

### The edits (`edit.rs`)

The model never writes a diff. It answers edits; the code makes the new file.

- `pub struct Edit { area, kind: EditKind, old: Option<String>, text, why, cases: Vec<String> }`,
  `pub enum EditKind { Add, Replace }`.
- `decisions.md`'s shape (what `persona_builder_build::draft::decisions` writes): a `## By kind of
  question` heading; under it one `### <area>` section per area; inside a section the lists
  `**The lean:**`, `**What flips it:**`, `**When they write their own answer:**` (the label text
  after `When ` varies: match any line starting `**When ` and ending `:**`), `**Standing rules:**`,
  `**Precedents:**`, each followed by `- ` items, an empty list being the single item `- None yet.`.
  A section ends at the next line starting `### ` or `## `, or the end of the file.
- `pub fn apply_edit(decisions: &str, edit: &Edit) -> Result<String>`:
  - **Add**: in the area's section, under `**Standing rules:**`: replace `- None yet.` when it is the
    only item, else insert `- <text>` after the list's last item. A section with no
    `**Standing rules:**` list gets one, `\n**Standing rules:**\n- <text>\n`, at the section's end
    (before the trailing blank lines). An area with no section gets
    `### <area>\n\n**Standing rules:**\n- <text>\n\n` at the end of `## By kind of question` (the end
    of the file when it is the last heading, as it is in the template). Refused when the section
    already holds an item equal to `text`.
  - **Replace**: `old` must equal, character for character after trimming, the text of exactly one
    `- ` item in the area's section, in any list but `**Precedents:**`, and not `None yet.`; that
    item's text becomes `text`.
  - No `## By kind of question` heading: `text::NOT_BUILT`.
- Edits apply in order, each to the file the one before left.

### The model call (`refresh.rs`)

- `pub const MOST_CASES: usize = 40`, `pub const MOST_EDITS: usize = 10`,
  `pub const MOST_LINE: usize = 400`, `pub const MOST_REJECTED: usize = 5`.
- **The cases:** every `new` case from `store.cases()`, oldest first (`created`, then `id`), at most
  `MOST_CASES`. None: `propose` answers `Proposed::Nothing` and makes no call.
- **One open proposal at a time.** Open: its base equals the current version and it has no approval,
  or an `approved` approval and no measurement. `propose` refuses while one is open, with
  `text::open_waiting(id)` (no approval yet: show it, then approve or reject it) or
  `text::open_approved(id)` (approved, not yet applied). A proposal whose base is not current is
  stale and never counts as open.
- **The schema** (`pub fn refresh_schema() -> Value`, the build's style):
  `{"summary": string, "edits": [{"area": string, "kind": "add"|"replace", "old": string|null,
  "text": string, "why": string, "cases": [string]}]}`, `additionalProperties: false` at each
  level, every key required.
- **The prompt** (`pub fn refresh_prompt(...) -> String`), fixed text first: what to do, the areas
  (the member's codebook categories, `book(member)?.categories`, exactly as written), the current
  `decisions.md` in full, then the cases, one JSON object per line, each with `id`, `area`,
  `situation`, `options` (when set), `wanted` (the case's `expected`) and, from the case's signals,
  `kind` and `persona_said` (when set). Then, when there are rejected proposals naming any of these
  cases, the `MOST_REJECTED` newest of them (by `created`), each its `summary` and the `+` lines of
  its diffs, under "The person said no to these earlier drafts; propose something else". Never the
  measurement or approval files.
- **`refresh.md`** (fixed text; it goes in the argv, so nothing from the person): write changes to
  how the person decides so the assistant would answer each case as they wanted; prefer one standing
  rule per lesson; replace a point only when a case shows it is wrong; write in plain, warm words
  and in your own words, never repeating the person's; one line per point, no Markdown, no `<`;
  name for each edit the case ids it serves; at most 10 edits; the summary is one or two plain
  sentences for the person, saying what will change.
- **The check** (`pub fn check_reply(...) -> std::result::Result<Vec<Edit>, Misfit>`), each failure
  a `Misfit` whose `shown` has no words from the reply or the cases (counts and places only):
  - `summary`, every `text`, `why` and `old`: trimmed non-empty, one line, at most `MOST_LINE`
    characters, no `persona_builder_steps::text_check::hidden` character;
  - 1 to `MOST_EDITS` edits; `area` one of the codebook's categories; `old` null exactly when
    `kind` is `add`; `cases` non-empty, no repeat, each one of the cases asked; every asked case
    need not be named;
  - the edits apply (`apply_edit` in order), and no two edits replace the same item;
  - **the filters:** `filters.kind(...)` on the summary, every `text` and every `why` (the
    member's `Filters::compile(&member.filters)`), told back as places only;
  - **paraphrase only:** `persona_builder_steps::shingle::overlaps` with `n = 6`
    (`persona_builder_build::build::SHINGLE`) of the summary, texts and whys joined by `\n`, against
    the cases asked and their signals as records: one record per case and per signal, the keys
    `overlaps` reads (`builder/steps/src/shingle.rs`'s `FIELDS`) filled as `question` ←
    `situation`, `text` ← the case's `expected` or the signal's `wanted`, `context` ←
    `persona_said` (plus whatever else `Record::all` needs to read a record; add a test that a
    6-word run from each of the three fields is caught). A hit is told back as the runs, shown as a
    count, as the build's distil check does.
- `Asker::ask("refresh", REFRESH_SYSTEM, &schema, &prompt, &text::DRAFTING, check)`: one call, a
  misfit told back once, the reply cached.

### `propose`

`pub fn propose(claude, session, home, store, member, member_dir, filters, model, now) ->
Result<Proposed>`, `pub enum Proposed { Nothing, Drafted(Proposal) }`. The `Session` holds the
state lock for its life (`builder/agent/src/ledger.rs`), so `propose` must never call
`Store::lock` or a locked `save_*`: it writes through the `_unlocked` methods only, and says so in
a comment. Steps: read `decisions.md` and its version; the open check; the cases; the call and
check; then build the proposal: `schema` `PROPOSAL_V1`, `id` `store.next_id(Prefix::Proposal,
&now[..10])`, `persona` the slug, `persona_kind: person`, `created: now`, `base` the version,
`summary`, one `Change` per edit in order (`file: "decisions.md"`, `diff` the unified diff from the
file before that edit to the file after it, `why` the edit's why), `cases` every case the edits name,
in the order first named. Before saving: `apply` each change's diff in order from the base text and
compare with `apply_edit`'s result (a mismatch is a bug: `Error::Invalid`); read `decisions.md`
again and refuse with `text::base_moved` if its version changed during the call. Save with
`store.save_proposal_unlocked`.

### `show`, `approve`, `reject`

- `pub fn open(store, member_dir) -> Result<Option<Proposal>>` and `pub fn render(proposal,
  member) -> Vec<String>`: the lines `show` prints: a heading line with the date and id, the summary,
  then for each change `<n>. <why>` and its diff lines, then how many of the person's answers it
  learned from, then the two commands (`persona-builder refresh approve <member_dir> <id>` and the
  same with `reject`). A stored proposal whose `summary` or `why` holds a hidden character, or whose
  diff holds a control character other than `\n`, is refused with `text::DAMAGED_PROPOSAL`, never
  printed (it is untrusted on the way back in too).
- `pub fn answer(store, member_dir, id, decision, now) -> Result<Approval>`: takes `store.lock()`
  itself (no session here) and then uses the `_unlocked` methods. The proposal must exist
  (`text::no_proposal(id)`); approve also needs a current base (`rules::base_is_current`; the error
  says to draft a new one) and every case it names still `new` (`rules::to_measure`); reject needs
  neither. The approval: `schema` `APPROVAL_V1`, `id` `next_id(Prefix::Approval, day)`, `proposal`,
  `at: now`, `role: subject`, the decision, no `note`. A second answer is refused by the store (rule
  8, `ALREADY_APPROVED`): keep that.

### The CLI (`builder/cli/src/main.rs`)

A `refresh` group (`#[command(subcommand)]`), doc lines in plain words:

- `persona-builder refresh propose <member_dir> [--model] [--daily-cap-usd] [--call-cap-usd]`
  ("Draft a change to how your assistant decides, from the answers you've given it"), the flags and
  defaults exactly as `build`'s. It finds and checks Claude Code as `build` does, opens the
  `Session`, prints `text::NOTHING_TO_REFRESH` for `Nothing`, else the `render` lines, then the spend
  line.
- `persona-builder refresh show <member_dir> [<proposal>]` ("Show the change waiting for your
  answer"): the named proposal, else the open one, else `text::NO_OPEN`.
- `persona-builder refresh approve <member_dir> <proposal>` and `refresh reject ...`: print
  `text::approved(id)` / `text::rejected(id)`.
- `.ai/PROJECT.md`: the `builder/grow/` row says it also drafts refreshes and takes the person's
  answer, and its tests column gains `builder/cli/tests/refresh.rs`.

### Words (`text.rs`)

Plain, warm, no tooling words (no "proposal", "case", "diff" in what a person reads; say "change",
"tests", "your answers"). Suggested, adjust only for clarity: `DRAFTING` "Drafting a change from your
answers"; `NOTHING_TO_REFRESH` "There's nothing new to learn from yet. Keep answering your assistant,
then try again."; `open_waiting` "A change is already waiting for your answer ({id}). Look at it with
`persona-builder refresh show`, then approve or reject it."; `open_approved` "You approved a change
({id}) that hasn't been tested and applied yet."; `approved` "Approved. Your assistant changes only
after the change is tested on all your answers and nothing gets worse."; `rejected` "Rejected. Nothing
changed. Your answers stay saved for the next refresh."; `NO_OPEN` "No change is waiting for your
answer."; `DAMAGED_PROPOSAL` "That saved change looks damaged, so it wasn't shown. Draft a new one
with `persona-builder refresh propose`."

## Tests (red first: each a failing assertion before its code)

- `builder/grow/tests/version.rs`: the empty blob is `e69de29bb2d1d6434b8b29ae775ad8c2e48c5391`;
  `hello\n` is `ce013625030ba8dba906f756967f9e9ca394464a`.
- `builder/grow/tests/diff.rs`: one added line, one replaced line, a change in the first and last
  lines (context clipped), two nearby changes merged into one hunk, two far changes in two hunks, no
  final newline on either side; `apply` round-trips each; `apply` refuses a changed context line, a
  wrong count, a wrong file name, overlapping hunks, and a diff against a file one line shorter.
- `builder/grow/tests/edit.rs`, on a `decisions.md` made by `draft::decisions` from an invented
  build (see `builder/build/tests/`): add to `None yet.`, add after items, add to a section with no
  rules list, add for an area with no section, add a repeat (refused), replace in the lean (ok), in
  precedents (refused), a text matching no item and two items (refused); no heading (refused).
- `builder/grow/tests/refresh.rs`, with a stub `claude` (copy `builder/build/tests/fixtures/claude_stub.py`'s
  pattern into `builder/grow/tests/fixtures/refresh_stub.py`: it answers one add per case asked, a
  rule written in fixed words naming the area, and `plan.json` can make call N answer `bad` (an area
  not in the codebook), `quote` (a rule that is a 6-word run of the first case's `wanted`),
  `private` (a rule holding an email address the default filters mark), `hidden` (a rule with a
  `​`) or `error`): a proposal is saved 0600 under `growth/`, with the base the version of the
  file, one change per edit whose diffs apply in order, `cases` the named ones; no new cases makes no
  call; an open proposal refuses a second `propose` and makes no call; a stale one does not block;
  each misfit is told back and a good retry is kept; two misfits refuse and save nothing; the prompt
  holds the cases and the rejected drafts' summaries and never an approval; `decisions.md` is
  byte-identical after every path; the shown misfit never holds a sentinel word from the cases.
  `answer`: approve and reject write the right approval; a second answer refused; approve on a moved
  base refused and reject allowed; approve with a case no longer `new` refused; an unknown id refused.
  `render`: the lines for a two-change proposal; a damaged stored proposal refused.
- `builder/cli/tests/refresh.rs`: on an invented member (as `builder/cli/tests/build.rs` makes one,
  then `keep` two answers), `propose` prints the change and the commands, `show` prints it again,
  `approve` prints the line and writes the approval, then `propose` refuses with the approved line;
  `reject` path; `propose` with nothing kept prints the nothing line and calls no Claude Code.
- The build's own tests stay green (the asker change is behaviour-free).
- **Mutation checks:** for each claim (each check in `check_reply`, the open check, the base
  re-read, the diff self-check, each `apply` refusal, the lock use, the role, the filters and the
  shingle check), break the line (`cp` a backup, edit, run, `cmp` back), confirm a test goes red,
  restore. Never `git checkout --`. List them in your final message.

## Done when

`bash scripts/ci-local.sh` is green (fmt, clippy `-D warnings`, every test), each new test was seen
red first, the mutation list is in your final message, and nothing writes to the member's folder.
Don't commit: leave the changes in the worktree and say which files changed.
