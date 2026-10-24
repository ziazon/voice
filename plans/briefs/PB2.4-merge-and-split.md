# PB2.4 brief: merge and split the corpus, in Rust

Written by Claude (lane V1) for Codex, 2026-10-24. Spec: lane V's `PB2.4` entry, split from PB2 of the
Persona Builder plan. Read the repo's `AGENTS.md` first, then `plans/briefs/PB2.2-mine-claude-code.md`
(the slice this one builds on: same crate, same parity-test style, same Python-compatibility helpers).
Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`): each test file goes red on its assertion before
its code exists, then green. Security rules: `~/.codex/skills/security-and-hardening/SKILL.md`. Never
commit; Claude commits. Run cargo with `CARGO_NET_OFFLINE=true` (your sandbox has no network). This
slice adds **no new dependency**.

## What this slice does

1. `persona_builder_ingest` gains a `corpus` module: `merge` and `split`, at parity with
   `scripts/crew_decisions.py`'s `cmd_merge` and `cmd_split`: the same records, in the same order,
   written as the same bytes, and the same printed lines.
2. `private.rs` gains `read_private`, the reading twin of `write_private`: every corpus file is read
   from the member's state folder only, through `O_NOFOLLOW` directory descriptors.
3. The CLI gains `persona-builder merge <member_dir>` and `persona-builder split <member_dir>`, both
   with `--dry-run`.
4. One Python fix: `read_jsonl` splits lines at `\n` only. Today it uses `str.splitlines`, which also
   splits at U+2028, U+0085 and others. `json.dumps(ensure_ascii=False)` writes those characters raw,
   so a corpus holding one fails to read (proved at pickup: a record whose text holds U+2028 raises
   `JSONDecodeError`).

## Decisions (made, do not reopen)

### Where Python is the oracle, and where it is not

- **Parity means:** for every input the Python accepts and finishes on, Rust gives the same records,
  the same JSONL bytes and the same printed lines. Parity tests run the real Python on the same files
  and compare.
- **Where the Python raises**, Rust returns an error and writes nothing. Both exit 1. Rust's messages
  are the fixed ones below. **Never put any record text in an error**: name the file, the 1-based line
  and the key only.
- **Follow the Python's order of checks exactly** (listed per command below), so an error happens
  where the Python's would.
- **Accepted differences, each pinned by a Rust-only test and listed in your report:**
  1. **Numbers.** A record holding a number that is not a whole number serde_json keeps exactly
     (`as_i64()` or `as_u64()` is `Some` and it is not an `f64`) is refused:
     `<name>:<line>: a record holds a number Persona Builder can't copy exactly`. Python and serde_json
     write floats differently (`1e16` is `1e+16` in Python). Mined records hold no numbers. Pin what
     `-0` does: refused, or written `0` as Python writes it; either is fine, the test says which.
  2. **JSON serde_json refuses** (`NaN`, `Infinity`, a lone surrogate escape, nesting deeper than 128)
     is an error, `<name>:<line>: a line is not valid JSON`, as any line that does not parse is. The
     Python accepts these.
  3. **Odd types the Python only tolerates by accident are refused** with the shape error
     `<name>:<line>: a record has an unexpected shape (<key>)`: an `id` that is not a string (Python
     dedupes by any hashable value, and `1`, `1.0` and `true` collide), a `timestamp` that is not a
     string (Python sorts numbers), a `kind` that is not a string. Where `privacy::filtered_text` already
     refuses a type (picks given as a string, which Python would join letter by letter), keep its
     refusal, prefixed `<name>:<line>: `.
  4. `--holdout` must be a finite number from 0 to 1, and `--holdout` with `--cutoff` is a usage error,
     exactly as `mine` does it.
  5. **On an error Rust writes nothing.** The Python's `split` writes the questions file before it
     fails on a missing answer field.
  6. **File names, not paths.** Every file is a plain name in the state folder (below). The Python
     takes any path.

### Reading a JSONL file (`read_records`, the port of `read_jsonl`)

- The bytes must be UTF-8 (no lossy decode: `str::from_utf8`), else `<name> is not UTF-8 text`.
- Split at `\n` only (`str::split('\n')`, after the Python fix). Line numbers count the pieces from 1.
- Skip a piece whose characters are all `python::py_space` (empty included), as `line.strip()` does.
- Parse each other piece with `serde_json::from_str::<Value>`; a failure is
  `<name>:<line>: a line is not valid JSON`. A value that is not an object is
  `<name>:<line>: a line is not a JSON object`. Then the number check (difference 1), over the whole
  value, nested arrays and objects included.
- A `\r` before the `\n` is JSON whitespace: it parses on both sides. Duplicate keys in one object keep
  the first key's position and the last value, as both Python and `preserve_order` do (pin it).
- Return `Vec<(usize, Map<String, Value>)>`: the line number and the record.

### `merge` (port of `cmd_merge`, `scripts/crew_decisions.py` lines 1210–1248)

Inputs have a mode: `History` (`--in`), `Train` (`--train`) and `Round` (`--rounds`). They are read in
this order: every history input, then every train input, then every round input, each list in the
order given. A name may repeat; each occurrence counts as a file. No inputs at all is the error
`merge needs at least one input`.

For each record, in this order:
1. `Round` only: `split` must be the string `train` or `heldout`, else
   `<name>:<line>: a round record's split must be train or heldout`.
2. `id` must be present and a string, else the shape error (`id`).
3. An `id` seen before (in any input, kept or dropped) counts one duplicate and is skipped. Record the
   id as seen.
4. `privacy::privacy(filters, record)`; an error is `<name>:<line>: <its message>`. A `personal` or
   `secret` result counts one dropped record of that kind and skips it.
5. A kept record's `timestamp` must be a string, else the shape error (`timestamp`). (Python reads it
   only for kept records, in the sort; a dropped record without one is fine on both sides.)
6. Keep it. A `Train` record's fixed split is `train`; a `Round` record's is its own `split`.

Then:
- Sort the kept records by `(timestamp, id)`, comparing the strings as Rust's `str` `Ord` does (UTF-8
  byte order is code point order, as Python compares).
- Mark the split on the **history records only**, exactly as `mine`'s `mark_split` does (make it
  `pub(crate)`, or generalise it to take the records to mark; keep `mine`'s behaviour unchanged): the
  cutoff comes from the history questions alone, and every history record gets `split` (an existing
  key keeps its position; a new one goes last). Fixed records never move the cutoff.
- Set every fixed record's `split` to its fixed split (same rule: in place, or last if absent).
- The output is the sorted records, written with `mine::to_jsonl`.

```rust
// builder/ingest/src/corpus.rs
pub enum Mode { History, Train, Round }
pub struct Input<'a> { pub name: &'a str, pub mode: Mode, pub bytes: &'a [u8] }
pub struct Merged {
    pub records: Vec<Map<String, Value>>,
    pub questions: usize, pub held: usize, pub messages: usize,
    pub files: usize, pub duplicates: usize, pub cutoff: Option<String>,
    pub personal: usize, pub secret: usize,
}
pub fn read_records(name: &str, bytes: &[u8]) -> Result<Vec<(usize, Map<String, Value>)>>;
pub fn merge(filters: &Filters, inputs: &[Input], split: &mine::Split) -> Result<Merged>;
pub fn merge_summary(merged: &Merged) -> String;
pub fn discover(names: &[String], out: &str) -> Vec<(String, Mode)>;
pub struct Heldout { pub questions: Vec<Map<String, Value>>, pub answers: Vec<Map<String, Value>> }
pub fn split(member: &Member, name: &str, bytes: &[u8]) -> Result<Heldout>;
pub fn split_summary(heldout: &Heldout) -> String;
```

`merge_summary` is the Python's two lines, each ending `\n`:
`merged <q> questions (<held> held out) and <m> messages from <files> files; <dups> duplicates dropped; held out from <cutoff>`
then `dropped <p> personal records, <s> secret records`. `<q>` counts records whose `kind` is
`question`, `<held>` those of them whose `split` is `heldout` (fixed ones included), `<m>` is the rest,
and `<cutoff>` is `nothing` when the cutoff is none or empty, as in `mine::summary`.

`discover` picks the inputs the way `bootstrap/install.sh` (step 6) does, from the state folder's file
names: every name starting `decisions-` and ending `.jsonl`, except `out`;
`decisions-interview.jsonl` is `Train`, `decisions-round-*.jsonl` is `Round`, the rest `History`.
Each mode's names are sorted by byte order; the result lists history, then train, then round.

### `split` (port of `cmd_split`, lines 501–506)

- Read the corpus with `read_records`. For each record, in order: `kind` must be present (shape error
  `kind`); when it equals the string `question`, `split` must be present (shape error `split`); the
  record is held when `kind == "question"` and `split == "heldout"` (JSON equality: a non-string never
  matches, as in Python).
- Each held record gives a question, its fields in this order: `id`, `timestamp`, `project`, `header`,
  `question`, `multiSelect`, `options`, `recommended` (the Python's `QUESTION_FIELDS`), and an answer:
  `id`, `recommended`, `<slug>_pick` (from `member.slug`), `picks`, `own_words`, `notes`, `outcome`,
  `multiSelect` (`answer_fields`). A missing field is the shape error naming it. Values are copied as
  they are, any JSON type.
- `split_summary` is `split <n> held-out questions\n`.

### `read_private` (`builder/ingest/src/private.rs`)

`pub fn read_private(home: &Path, slug: &str, name: &str) -> Result<Vec<u8>>`: the same walk as
`write_private` (canonical home, then `.claude`, `state`, the slug, each `openat` with
`O_RDONLY | O_DIRECTORY | O_NOFOLLOW`), but it creates nothing. Then `openat` the file with
`O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`, wrap it in a `File`, check `metadata().is_file()`
(else `<path> is not a file`), and read it all. Errors: a link or not a folder on the way, the
existing message from `write_private`; a link at the file, the same message; a missing folder or file
(`ENOENT`), `<path> doesn't exist`. Same name rule as `write_private` (a plain name ending `.jsonl`).
Reuse `write_private`'s helpers; every descriptor an `OwnedFd`; any new `unsafe` stays in this file,
each block with a `// SAFETY:` line.

### The CLI

```
persona-builder merge <member_dir> [--in NAME...] [--train NAME...] [--rounds NAME...]
                      [--out NAME] [--holdout F | --cutoff TS] [--dry-run]
persona-builder split <member_dir> [--corpus NAME] [--questions NAME] [--answers NAME] [--dry-run]
```

- `<member_dir>`, the member, the filters and the state folder check are exactly `mine`'s (same
  `private_state_dir` message). Factor out what `mine` and these share (the name rule, the
  `--holdout`/`--cutoff` parse, the state folder check) into small functions; keep `mine`'s behaviour
  and messages unchanged.
- `--in`, `--train` and `--rounds` each take one or more names and may repeat
  (`num_args = 1..`, `ArgAction::Append`). Every name follows the plain-name rule
  (`the file name must be a plain name ending .jsonl`).
- `merge`: `--out` defaults to `decisions.jsonl`; `--holdout` to `0.15`. When no input is named, the
  inputs come from `discover` over the state folder's entries (`fs::read_dir`; names that are not UTF-8
  are skipped); when the folder is missing or nothing is found:
  `nothing to merge yet: ~/.claude/state/<slug> has no decisions-*.jsonl`. An `--out` that is also an
  input: `the output can't also be one of the inputs`. Each input is read with `read_private`.
  It prints `merging: <names>` (joined by `, `, a train name followed by ` (always training)`, a round
  name by ` (keeps its split)`), then `merge_summary`, then `saved to ~/.claude/state/<slug>/<out>`, or
  with `--dry-run` `dry run: nothing written` and no file or folder created.
- `split`: `--corpus` defaults to `decisions.jsonl`, `--questions` to `heldout-questions.jsonl`,
  `--answers` to `heldout-answers.jsonl`; the three must differ
  (`--corpus, --questions and --answers must be three different files`). It reads the corpus with
  `read_private`, computes both outputs, then writes the questions file and the answers file with
  `write_private`. It prints `split_summary`, then `saved to ~/.claude/state/<slug>/<questions> and <answers>`,
  or with `--dry-run` `dry run: nothing written`.
- Doc comments (the `--help` lines), in plain words: `/// Combine your histories into one set of
  decisions, and set aside the newest to test with.` and `/// Set aside the held-out questions and
  their answers for the test.`

### The Python fix

`read_jsonl` becomes `[json.loads(line) for line in Path(path).read_text().split("\n") if line.strip()]`.
Add a test to `tests/crew_hardening_test.py`: a corpus whose message text holds U+2028 and U+0085,
written as `write_jsonl` writes it, goes through `merge` with exit 0 and the record intact.

### `.ai/PROJECT.md`

The `builder/ingest/` row's description ends "…, mining Claude Code transcripts, merging and splitting
the corpus".

## Tests (each red first)

All fixtures are invented: names like Ada, Ari, Robin, Sam, Wren; no real person, no real path.

1. **`tests/crew_merge_fixtures.py`** (new, Python, standard library): `python3 -I
   tests/crew_merge_fixtures.py <case> <dir>` writes one named case's input files (records built with
   `crew_fixtures.py`'s helpers where they fit, written as `json.dumps(r, ensure_ascii=False) + "\n"`,
   raw bytes through `Path.write_bytes` where a case needs them) and prints the case's merge arguments
   as one JSON list (`["--in", "a.jsonl", …, "--holdout", "0.5"]`), relative names only. Cases:
   - `two_machines`: two history files sharing records, one named twice;
   - `interview_train`: history plus `decisions-interview.jsonl` given as `--train`, one interview
     record without a `split` key and one with `"split": "heldout"`;
   - `rounds_keep_split`: history plus two round files holding `train` and `heldout` records;
   - `rounds_only`: round files alone (no history questions: cutoff `nothing`);
   - `messages_only`: history with messages and no question;
   - `refilter`: records the ARI member's filters now drop, personal and secret, plus a later duplicate
     of a dropped record (it counts as a duplicate, not a drop);
   - `ties`: equal timestamps, ordered by id;
   - `unicode`: text holding U+2028, U+0085, U+001C, quotes, backslashes, é, 日, 😀 and every code point
     below U+0020;
   - `shapes`: an extra unknown key, keys in an unusual order, nested lists and objects, whole numbers,
     `true`, `false`, `null`, a duplicate key inside one object;
   - `whitespace_lines`: CRLF endings, empty lines, a line of spaces and `\x1c`, no final newline;
   - each history case also with `--holdout 0`, `--holdout 1`, `--holdout 0.5` (half-to-even on 3 and
     on 5 questions), `--cutoff 2026-09-02T10:00:00Z` and `--cutoff ""`.
2. **`builder/ingest/tests/corpus_parity.rs`**: for each case and flag set, run
   `python3 -I scripts/crew_decisions.py --member <m> merge <args> --out <abs path>` from the case
   folder, and compare its output file's bytes with `to_jsonl(&merged.records)` and its stdout with
   `merge_summary(&merged)`. Then run the Python `split` on that output (`--corpus`, `--questions`,
   `--answers` absolute) and compare both files' bytes with `to_jsonl` of `Heldout`'s lists, and the
   stdout with `split_summary`. Members: the `ADA` and `ARI` configs, as `mine_parity.rs` has them.
   Use `common::parity_tools_available()`, so `PERSONA_BUILDER_REQUIRE_PARITY=1` makes it fail rather
   than skip.
3. **`builder/ingest/tests/corpus_rules.rs`** (Rust only):
   - every error above, each checked for its exact message and that no record text appears in it (use a
     sentinel word in each bad record); for the ones the Python also refuses (not an object, not JSON,
     a bad round split, a missing `id`, a missing `kind`, a kept record with no `timestamp`, a missing
     question or answer field in `split`), assert the Python exits 1 too;
   - each accepted difference, 1 to 3, with the Python's own behaviour asserted beside it where it
     finishes;
   - `discover`: the classes, the byte-order sort, `out` left out, a non-`decisions-` name left out;
   - `merge` with no inputs; the duplicate count across modes (a history record repeated in a round file
     stays a history record).
4. **`builder/ingest/tests/private_read.rs`**: `read_private` reads a 0600 file; refuses a link at the
   file, at the slug folder, at `state` and at `.claude`; refuses a folder and a pipe named `x.jsonl`;
   reports a missing folder and a missing file; refuses names with `/`, a leading `.`, or no `.jsonl`;
   creates nothing (the state folder listing is the same after a failed read).
5. **`builder/cli/tests/merge.rs`** (style of `builder/cli/tests/mine.rs`, a scratch `HOME`, a member
   made from `crew/_template`): `merge` with discovery (history, interview and round files, and
   `decisions.jsonl` itself not read back as an input), with named inputs, the `merging:` line, the
   saved file 0600; `--dry-run` creates no file and changes no file; `--out` equal to an input refused;
   a bad name refused; no inputs found; a link in the state folder refused; `--holdout 2` refused;
   `split` with the defaults, its two files 0600, `--dry-run`, the three-names rule.
6. **`tests/crew_hardening_test.py`**: the U+2028 test above, red before the fix.

## Done when

Every test above went red on its assertion first and is green. `cargo fmt --check`,
`cargo clippy --all-targets --locked -- -D warnings` and `bash scripts/ci-local.sh` pass. Report: each
file you added or changed, the red you saw for each test file, every case where the engines disagreed
outside the accepted differences (stop and report it; never bend a test or the Python to make it pass),
and anything in this brief you could not do as written.
