# PB4.2 brief: kept answers, the growth loop's records and store

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB4.2` entry, split from PB4 of the Persona
Builder plan (PB-D5, PB-D13). Read the repo's `AGENTS.md`, then `docs/spec/growth-loop.md` (the spec
this implements, version 1) and its schemas in `docs/spec/growth-loop/`, then
`plans/briefs/PB3.4-interview.md` (the closest earlier slice: a CLI command that reads a file the
app put in the state folder, runs it through the filters and keeps it privately). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`;
every sentence a person sees: `~/.codex/skills/plain-language/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true`. **No new dependency**
(`serde` with `derive`, `serde_json`, `regex = "=1.13.1"` and `libc` are all in the lockfile already).

## What this slice does

Every answer the person gives their assistant (a pick, a correction, a mark on a report, a rule they
state) is kept on their own computer as a growth-loop **signal** and turned at once into a **case**
(a test the assistant must pass), in the files `docs/spec/growth-loop.md` sets. It also gives PB4.3
(refresh, approve, re-measure) the typed records and every rule check it needs, so PB4.3 adds only
the model calls and the member writes. Nothing here calls a model or writes in the member's folder.

## Decisions (made, do not reopen)

### The crate

- **A new crate `builder/grow`** (package `persona-builder-grow`, library `persona_builder_grow`),
  added to the workspace `members` in the root `Cargo.toml`. Dependencies: `persona-builder` (core:
  `error`, `member`), `persona-builder-ingest` (the private readers and writers, `lock_state`, the
  filters), `persona-builder-steps` (`book::book` for the member's categories), `serde = { version =
  "1", features = ["derive"] }`, `serde_json = { version = "1", features = ["preserve_order"] }`,
  `regex = "=1.13.1"`. `builder/cli` depends on it.
- **Modules:** `record.rs` (the five records, parsing and the one-record checks), `rules.rs` (the
  rules that span records), `store.rs` (the private files), `keep.rs` (keeping an answer), `text.rs`
  (every sentence a person sees). `lib.rs` only declares them and documents the crate in one line.
- `.ai/PROJECT.md`'s table gains a row for `builder/grow/` ("Kept answers (persona_builder_grow): the
  growth loop's records, its rule checks and the private store of signals and cases"; tests
  `builder/grow/tests/`, `builder/cli/tests/keep.rs`), and the `docs/spec/growth-loop.md` row's test
  column also names `builder/grow/tests/`.

### `record.rs`: the five records

- One struct per record: `Signal`, `Case`, `Proposal` (with `Change { file, diff, why }`), `Approval`,
  `Measurement` (with `Outcome` entries `{ case, new, before, after }`; name the entry struct
  `CaseResult`). Fields **in the spec table's order**, so serialising an example gives back its keys
  in the file's order. `#[serde(deny_unknown_fields)]` on every struct. Optional fields are
  `Option<…>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- Enums for every `enum` in the schemas, `#[serde(rename_all = "lowercase")]`: `GivenBy { Subject,
  Staff }` (also used for an approval's `role`), `SignalKind { Pick, Correction, Verdict, Instruction }`,
  `Verdict { Agree, Disagree }`, `Match { Choice, Rubric }`, `Status { New, Active, Retired }`,
  `PersonaKind { Person, Product }`, `Decision { Approved, Rejected }`, `Grade { Pass, Fail }`,
  `KeepOrRevert { Keep, Revert }` (serialised `keep` / `revert`). `schema` is a `String` field.
- **Patterns:** `pub const` strings copied verbatim from the schema files: `TIMESTAMP`, `PERSONA`
  (`^[a-z0-9]+(-[a-z0-9]+)*$`), `FILE` (the proposal's `changes[].file` pattern), and one id pattern per
  prefix built by a `fn id_pattern(prefix) -> String` that returns `^<prefix>-[a-z0-9]+(-[a-z0-9]+)*$`.
  Compile each once (`std::sync::LazyLock<Regex>`). The schema version strings are `pub const`s too
  (`SIGNAL_V1 = "growth-loop/signal/v1"`, and so on).
- **Parsing:** `pub fn parse<T: Record>(bytes: &[u8]) -> Result<T>`: `serde_json::from_slice`, then
  `T::check()`. A `Record` trait has `fn check(&self) -> Result<()>` and `const KIND: &str`.
  `check` makes every schema rule serde cannot: `schema` equals the version constant, every pattern,
  every `minLength: 1` (no empty string), every `minItems` (`from_signals` ≥ 1, `options` ≥ 2,
  `changes` ≥ 1, `cases` ≥ 1, `results` ≥ 1), every `uniqueItems` (`from_signals`, `options`,
  `cases`); then the spec's *Checks the schemas cannot make* that live in one record: a signal has
  `verdict` exactly when `kind` is `verdict`; a case with `match: choice` has `options` and its
  `expected` is one of them; a measurement names each case once in `results`.
- **Errors never quote a value.** A record can hold what a person actually said, so every message
  names the record kind, the field and the broken rule only, e.g. *"the case's options must hold at
  least two choices"*, *"the signal's id must be sig- followed by lowercase letters, digits and
  dashes"*. A serde error is reported as *"the <kind> isn't in the growth loop's <kind> shape (line
  L, column C)"* with no serde text (serde quotes unknown field names and values). `Error::Invalid`.
- **Writing:** `pub fn to_line<T: Serialize>(record) -> String` (compact JSON plus `\n`) and
  `pub fn to_pretty<T: Serialize>(record) -> String` (`serde_json::to_string_pretty` plus `\n`).

### `rules.rs`: the rules that span records

Pure functions over records already parsed; PB4.3 calls them before it acts. Each returns
`Result<()>` with a plain `Error::Invalid` naming ids and the rule (ids are safe to show), except
where a value is answered.

- `pub fn approver(kind: PersonaKind) -> GivenBy`: `Person → Subject`, `Product → Staff`.
- `pub fn may_apply(proposal, approval) -> Result<()>` (rule 2): the approval names this proposal,
  says `approved`, and its role is `approver(proposal.persona_kind)`.
- `pub fn to_measure(proposal, cases: &CaseBook) -> Result<Vec<(String, bool)>>` (rule 3): the
  `(case id, new)` list a measurement must hold: each of the proposal's `cases` (`true`; each must
  exist and be `new`, else refused), then every `active` case in id order (`false`). `retired` cases
  and other `new` cases are left out.
- `pub fn outcome(measurement, proposal, approval, cases_at_start: &CaseBook) -> KeepOrRevert`
  (rule 4): `Keep` only when the approval is `approved` (rule 2 holds), the results are exactly
  `to_measure`'s list (same ids, same `new` flags, any order), every `new: true` result has
  `after: pass`, and no `new: false` result went `before: pass` to `after: fail`. Else `Revert`.
  `pub fn check_measurement(…) -> Result<()>` refuses a measurement whose `outcome` differs from
  `outcome(…)`, whose `proposal` or `approval` ids do not match, or whose `base` is not the
  proposal's `base`.
- `pub fn check_references(store contents) -> Result<()>` (rule 5): every case's `from_signals`
  exist; every proposal's `cases` exist; every approval's `proposal` exists; every measurement's
  `proposal` and `approval` exist and the approval approved that proposal; every record's `persona`
  (signal, case, proposal) is the store's persona.
- **`CaseBook`** (rule 7 and 8 for cases): built from the case lines in file order. The first line
  for an id is the case; a later line for the same id must equal the first in every field except
  `status` (else refused: *"case <id> was appended again with a change other than its status"*), and
  its status move must be allowed: `new → active`, `new → retired`, `active → retired`; nothing
  leaves `retired` and nothing goes back to `new` (refused naming the id and the two statuses). A
  repeat with the same status is allowed and changes nothing. `CaseBook::get(id)` and
  `CaseBook::iter()` give the last state, in id order.
- `pub fn one_approval(approvals) -> Result<()>` (rule 8): at most one approval per proposal; and
  `pub fn no_measurement_when_rejected(approvals, measurements) -> Result<()>`.
- `pub fn base_is_current(proposal, current: &str) -> Result<()>` (rule 9): refused with
  `text::base_moved` when the proposal's `base` is not `current`.

### `store.rs`: the private files

- Everything lives in the state folder's `growth/` folder (`~/.claude/state/<slug>/growth/`), never in
  the member's folder (spec rule 6). Use ingest's private readers and writers with `Some("growth")`
  as the folder: 0600 files in 0700 folders, links refused, staged and renamed (`write_private_at`,
  `read_private_at`, `private_folder`).
- **Files:** `growth/signals.jsonl` and `growth/cases.jsonl` (one record per line); one pretty JSON
  file per proposal, approval and measurement, named `<id>.json` (`prop-…`, `appr-…`, `meas-…`; the
  id patterns make these plain names).
- **Add `read_private_optional_at(home, slug, folder: Option<&str>, name)` to `builder/ingest/src/private.rs`**:
  `read_private_optional` generalised to a folder (a missing state folder, sub-folder or file is
  `Ok(None)`; a link is refused as `read_private_at` refuses it). `read_private_optional` becomes a
  one-line call to it. Test both in ingest's existing private tests (missing folder, missing file, a
  linked sub-folder refused).
- **`pub struct Store { home, slug }`**, `Store::open(home, member) -> Result<Store>`: the persona id is
  the member's `slug`, which must match `PERSONA` (else `text::SLUG_SHAPE`). Every write takes
  `lock_state(home, slug)` for its whole read, check and write, so two writers never interleave.
- **Appending a line** (`signals.jsonl`, `cases.jsonl`): under the lock, read the file (missing is
  empty), parse every line (a line that does not parse or check is refused as
  `text::damaged(file, line_number)`, and nothing is written), add the new lines, and write the whole
  file back with `write_private_at`. The old bytes are kept exactly; the file only ever grows.
- **A JSON file** (proposal, approval, measurement): written only when no file of that name exists
  (`read_private_optional_at` under the lock); an existing one is refused (`text::already_kept(id)`):
  records are never rewritten (rule 8).
- **Ids:** `Store::next_id(prefix, day: &str) -> String` gives `<prefix>-<day>-<NNN>`, `day` the
  `YYYY-MM-DD` of the record's time, `NNN` one more than the highest number already used for that
  prefix and day in that file (zero-padded to 3 digits, more digits past 999), starting at `001`. So
  the first signal on 2026-03-04 is `sig-2026-03-04-001`, as in the spec's example.
- **Readers:** `signals()`, `case_lines()`, `cases() -> CaseBook`, `proposal(id)`, `approval(id)`,
  `measurement(id)`, and `proposals()`, `approvals()`, `measurements()` (every `*.json` in `growth/`
  with the matching prefix, parsed and checked; any other file name in the folder is ignored).
  `Store::check_all() -> Result<()>` runs every rule in `rules.rs` over everything on file.
- **Times:** `pub fn utc_now() -> String` in `store.rs`, `YYYY-MM-DDTHH:MM:SSZ`, from
  `SystemTime::now()` without `libc` (days-from-civil arithmetic on the Unix seconds; the build
  crate's `ledger::utc_stamp` is private and uses `gmtime_r`, so write a safe one here and test it on
  fixed instants: the epoch, 2000-02-29, 2026-03-04T15:20:00Z). Every function that stamps a time
  takes `now: &str` as an argument; only the CLI calls `utc_now()`, so tests pass a fixed time.

### `keep.rs`: keeping an answer

- `pub fn keep(store: &Store, member: &Member, filters: &Filters, input: &[u8], now: &str) -> Result<Kept>`
  with `pub struct Kept { pub signal: Signal, pub case: Case }`.
- **The input** is the JSON object the app writes: `kind`, `area`, `situation`, `wanted` (required),
  `persona_said`, `verdict`, `source`, `given_by` (optional; `given_by` defaults to `subject`).
  `schema`, `id`, `persona` and `at` are refused (`text::FILLED_IN`: Persona Builder fills those in);
  any other key is refused (`text::unknown_key(key)`; a key name is the app's, not the person's
  words, but cut it to 40 characters). The input is at most 64 KiB (`text::TOO_BIG`).
- **Checks before anything is written,** each a plain refusal that never quotes the person's words:
  - `area` is one of the member's categories (`persona_builder_steps::book::book(member)?.categories`),
    else `text::unknown_area` listing the categories (these are the member's own settings, safe to
    show);
  - `situation`, `wanted`, `persona_said` at most 2,000 characters each, `source` at most 100, none
    empty or only white space; no control character other than `\n` and `\t`, and none of the
    invisible characters the build refuses in a model's points (reuse the same list: make
    `builder/build/src/build.rs`'s hidden-character predicate a `pub fn` in `persona_builder_steps`
    (new module `text_check.rs`, `pub fn hidden(c: char) -> bool`) and call it from both crates, so
    there is one list; move its tests with it);
  - the filters: `filters.kind(text)` on each of `area`, `situation`, `wanted`, `persona_said` and
    `source`; a hit refuses the whole answer with `text::left_out(field, kind)` (*"That answer
    looks personal (in what was being decided), so it wasn't kept. Leave out anything private and
    try again."*), naming the field in plain words and `kind.as_str()`, never the text;
  - the built signal and case each pass `check()`.
- **The signal:** `schema` `SIGNAL_V1`, `id` `next_id("sig", day of now)`, `persona` the slug, `at`
  `now`, then the input's fields.
- **The case** (no model): `schema` `CASE_V1`, `id` `next_id("case", …)`, `persona`, `created` `now`,
  `from_signals` `[signal id]`, `area` and `situation` from the signal, no `options`, `expected` the
  signal's `wanted`, `match: rubric`, `status: new`. Every signal makes exactly one case.
- **Writing:** both under one hold of the lock: the signal line first, then the case line. If the case
  write fails after the signal was written, the next `keep` still works (the signal has no case; that
  is allowed by the spec) and the error is returned.

### The CLI

- `persona-builder keep <member_dir> --signal <file>`: `<file>` is `NAME` or `FOLDER/NAME` in the
  state folder, ending `.json`, read with `json_at` exactly as `interview answer` reads `--answers`.
  Prints `text::kept(&kept)` (*"Kept your answer. It's now one of the tests your assistant is
  checked on at its next refresh."*) and `saved to ~/.claude/state/<slug>/growth/`.
- `persona-builder cases <member_dir>`: prints `text::cases_summary` from `Store::cases()`: one line
  overall (*"Tests made from your answers: 3 waiting for a refresh, 5 in use, 1 retired."*, or
  `text::NO_CASES` when there are none: *"You haven't given your assistant any answers to learn
  from yet."*), then one line per area that has any, in the member's category order. Counts and
  area names only, never a case's words.
- Register both in `Command` beside `Interview`, with help text in plain words (*"Keep an answer you
  gave your assistant, so it learns from it"*, *"Show how many tests your answers have made"*).

### Parity with the spec

- `builder/grow/tests/records.rs` parses each file in `docs/spec/growth-loop/examples/` (path from
  `env!("CARGO_MANIFEST_DIR")`), checks it, serialises it with `to_pretty`, and asserts the result
  parses to a `serde_json::Value` equal to the file's, keys in the same order.
- The same test reads each `*.schema.json` and asserts every `pattern` in it equals the matching Rust
  constant, and every `enum` list equals the Rust enum's serialised values, so the two cannot drift.
- **Golden files:** `builder/grow/tests/golden/signal.json` and `case.json`, written by `keep` from a
  fixed invented input (the spec's Robin, a fictional teacher) at a fixed `now`. The Rust test asserts
  `keep`'s lines equal them (as `Value`s); `tests/growth_loop_spec_test.py` gains
  `test_rust_golden_records_validate`, validating each golden file against its schema with
  `schema_subset.validate`. So what Rust writes is checked by the same schemas the spec ships.

## Tests (red first, each a failing assertion before the code)

- `builder/grow/tests/records.rs`: the examples round-trip; every pattern and enum matches its
  schema; each one-record check's refusal (empty string, bad id, bad timestamp, too few options,
  repeated option, `expected` not in `options`, `verdict` without `kind: verdict` and the reverse, a
  measurement naming a case twice, an unknown field, a wrong `schema`), with a sentinel word in the
  broken value that never appears in the error.
- `builder/grow/tests/rules.rs`: `approver`; `may_apply` (approved by the right role; rejected; wrong
  role; another proposal's approval); `to_measure` (new cases first, active ones in id order, retired
  and others' new left out; a proposal case that is missing or not new refused); `outcome` against the
  same cases as `tests/growth_loop_spec_test.py`'s `test_rule_four_reverts_regressions_failures_gaps_and_rejections`
  (keep; a regression; a new case failing; a missing result; an extra result; a rejected approval);
  `CaseBook` (each allowed move, each refused move, a repeat changing another field refused, the
  same status repeated); references; one approval per proposal; no measurement for a rejected one;
  `base_is_current`.
- `builder/grow/tests/store.rs` (a temporary `HOME`, as the ingest and build tests make one): files
  are 0600 in a 0700 `growth/`; a linked `growth/` refused; appending keeps the old bytes and only
  grows; a damaged line refuses the append and writes nothing; `next_id` (first, next, a new day,
  past 999); a JSON record is not rewritten; the readers ignore other file names; `utc_now`'s
  arithmetic on the fixed instants.
- `builder/grow/tests/keep.rs`: a kept answer writes one signal and one case with the right fields
  and links; the golden files; each refusal (filled-in key, unknown key, unknown area, too long,
  empty, a control or hidden character, a personal hit, a secret hit, too big) writes nothing and
  never quotes a sentinel; `given_by` defaults to `subject`; two answers on one day get `-001` and
  `-002`.
- `builder/cli/tests/keep.rs`: `keep` on an invented member (made from `crew/_template` the way
  `builder/cli/tests/interview.rs` makes one) writes 0600 files under `growth/` and prints the lines;
  a refusal exits non-zero with nothing written; `cases` with none, then after two answers.
- `tests/growth_loop_spec_test.py`: the golden files validate.
- **Mutation checks:** for each claim (each check in `record.rs`, each rule, the lock, the filters
  call, append-only), break the line (`cp` a backup, edit, run, `cmp` back), confirm a test goes red,
  restore. Never `git checkout --`.

## Done when

`bash scripts/ci-local.sh` is green (fmt, clippy `-D warnings`, every Rust and Python test, the
keystore cross-target checks). Commit in coherent commits with messages in a file (`git commit -F`):
the ingest helper; the records and rules; the store and keep; the CLI; the docs rows. No real person
anywhere: invented names only (Robin, as in the spec).

## After the gates: the fix-ups (decided, do not reopen)

`/security-review` (a sub-agent): no high or medium, four lows. The Verifier (Fable): pass with four
should-fixes, no must-fix. Claude's own review added four notes. They overlap in two places (security L3 is the Verifier's 1; L4 is a review note), so ten fixes cover them all, each with a
test proved red first:

1. **`<` is allowed in a kept answer** (review). `keep` checks `hidden(c) && c != '<'` (the `<` rule
   guards Markdown the build writes; a kept answer is data, never written into the member's files).
   `text::invalid_text` reads *"<Field> must be 1 to 2,000 characters long, with no hidden
   characters."* with the field's plain name capitalised (*What was being decided*, *The answer you
   wanted*, *The assistant's answer*, *Where the answer came from*, *The area*) and the limit with a
   thousands comma.
2. **`next_id`'s unknown prefix** (review): `next_id` takes a `pub enum Prefix { Signal, Case,
   Proposal, Approval, Measurement }` (with `fn as_str`), so an unknown prefix cannot be written; the
   overflow error gets its own text (`text::TOO_MANY_IDS`).
3. **`text::kept`** (review) becomes `pub const KEPT: &str`.
4. **`json_files` reads only names that match the id pattern** (review, security L4): a name is read
   only when `<id>.json` with `<id>` matching its prefix's id pattern; any other name is skipped, so
   no planted name reaches a message. Drop the `self.bytes("signals.jsonl")` probe: check the folder
   with a new ingest `private_folder_exists(home, slug, folder) -> Result<bool>` (descriptor-relative,
   links refused, never creates) before `fs::read_dir`.
5. **Nothing is written unless both lines will be accepted** (security L1): under the lock, `keep`
   reads both files, checks the signal id is new and that `CaseBook::new(old cases + the new case)`
   passes, and only then appends the signal, then the case. A test plants a `cases.jsonl` whose
   status moves break rule 7 and asserts `keep` fails with `signals.jsonl` unchanged.
6. **Size limits before reading** (security L2): ingest gains `read_private_at_max(home, slug, folder,
   name, max: u64)` that checks the opened file's size (`metadata().len()`) and refuses over `max`
   with a plain *"<name> is too big to read (over N KiB)"* before reading; `read_private_at` is
   unchanged. The CLI's `keep` reads the signal with `max` 64 KiB; the store reads `signals.jsonl` and
   `cases.jsonl` with `max` 64 MiB and each JSON record with `max` 16 MiB.
7. **`unknown_key` shows a key only when it matches `^[a-z_]{1,40}$`** (security L3, Verifier 1);
   otherwise *"The answer has a field Persona Builder doesn't know. Leave it out and try again."*
8. **Rule 8 at write time** (Verifier 2): under the lock, `save_approval` refuses a second approval
   for a proposal (`rules::one_approval` over the stored ones plus the new one) and an approval for a
   proposal not on file; `save_measurement` refuses when the stored approval it names is missing or
   rejected (`rules::no_measurement_when_rejected`) or does not let the stored proposal apply
   (`rules::may_apply`). Nothing is written on a refusal.
9. **`save_*_unlocked` twins** (Verifier 3): `pub(crate) fn save_proposal_unlocked`,
   `save_approval_unlocked`, `save_measurement_unlocked`; the public ones take the lock and call them,
   as the append pair does. `Store::lock` stays `pub(crate)`.
10. **The lock's message** (Verifier 4): *"Persona Builder is already working on this assistant in
    another window. Wait for it to finish, then try again"*; the three tests that look for
    `already building` look for `already working on this assistant`.
