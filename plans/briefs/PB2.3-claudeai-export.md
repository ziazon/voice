# PB2.3 brief: mine the claude.ai export, in Rust

Written by Claude (lane V1) for Codex, 2026-10-24. Spec: lane V's `PB2.3` entry, split from PB2 of the
Persona Builder plan. Read the repo's `AGENTS.md` first, then `plans/briefs/PB2.2-mine-claude-code.md`
and `plans/briefs/PB2.4-merge-and-split.md` (the slices this one builds on: same crate, same parity-test
style, same Python-compatibility helpers in `builder/ingest/src/python.rs`). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`): each test file goes red on its assertion before its code
exists, then green. Security rules: `~/.codex/skills/security-and-hardening/SKILL.md`. Never commit;
Claude commits. Run cargo with `CARGO_NET_OFFLINE=true` (your sandbox has no network).

**Built by Claude directly:** Codex was at a hard usage limit (until 2026-10-30) when this brief was
dispatched, so Claude implemented it at the same bar; the decisions below are what was built and
tested, corrected where testing proved the first draft wrong (the zip duplicate check, `-0`, the
not-a-list message, more Python-only timestamp forms).

**The one new dependency is already added and fetched** (by Claude, with the intake answers in the
record): in `builder/ingest/Cargo.toml`,
`zip = { version = "=9.0.5", default-features = false, features = ["deflate-flate2"] }` and
`flate2 = { version = "=1.1.10", default-features = false, features = ["rust_backend"] }`. Do not
change those lines or add any other dependency. Stored and deflate entries are the only compression
methods it reads; anything else is refused.

## What this slice does

1. `persona_builder_ingest` gains a `claudeai` module: reading the export, and `inspect`, `titles`,
   `mark` and `mine`, at parity with `scripts/crew_decisions.py` lines 783–1004 (`export_files` to
   `cmd_claudeai_mine`): the same records, the same review file, the same JSONL bytes, the same
   printed lines.
2. `private.rs`: `write_private` and `read_private` also take review files (a plain name ending
   `.tsv`).
3. The CLI gains `persona-builder claudeai inspect | titles | mark | mine`.

## Decisions (made, do not reopen)

### Where Python is the oracle, and where it is not

- **Parity means:** for every input the Python accepts and finishes on, Rust gives the same records,
  the same file bytes and the same printed lines. Parity tests run the real Python on the same files
  and compare.
- **Where the Python raises** (a `Failed` or a traceback), Rust returns an error and writes nothing.
  Both exit 1. **Never put any export text in an error, a summary or `inspect`'s output** beyond what
  the Python prints: no title, message, uuid, project name or timestamp value. Errors name the file,
  the 1-based conversation number and message number, and the key only:
  `conversations.json: conversation <n> has an unexpected shape (<key>)` and
  `conversations.json: conversation <n>, message <k> has an unexpected shape (<key>)`.
- **Check a field only where the Python reads it**, in the Python's order, so a field the Python never
  touches (an unmarked conversation's messages in `mine`, a skipped message's `created_at`) never
  causes an error.
- **Accepted differences, each pinned by a Rust-only test that also asserts the Python's own
  behaviour beside it, and listed in your report:**
  1. **Timestamps.** Rust reads only the grammar below; every other form Python's `fromisoformat`
     takes is refused as "not a timestamp".
  2. **Odd types the Python only tolerates by accident are refused** with the shape error: a
     conversation `uuid` that is not a string (in `mine`, Python treats one as unmarked),
     `chat_messages` that is not a list (Python iterates a string's characters; `""` gives no
     messages), a truthy `name` that is not a string (Python: `TypeError`, same exit).
  3. **A uuid holding a tab or a line break** (`\t`, or any character `python::splitlines` splits at)
     is refused: `conversations.json: conversation <n> has a uuid Persona Builder can't list`.
     Python writes it raw and corrupts the review file.
  4. **The export's form.** A link is never followed: at the export path itself, and at
     `conversations.json` inside an export folder (Python follows both). A path that is neither a
     regular file nor a folder (a pipe, a device) is refused. The 2 GB cap applies to every form,
     not just the zip. A zip that lists any name twice is refused (Python reads the last one, and
     refuses only a repeated `conversations.json`): the `zip` crate keeps one entry per name, so a
     repeated name would hide an entry and change the file count. A compression method other than
     stored or deflate, and an encrypted entry, are refused.
  5. **Encoding and JSON.** `conversations.json` must be UTF-8; a leading UTF-8 byte-order mark is
     skipped in every form (Python skips it only inside a zip). UTF-16 and UTF-32 (which Python
     detects inside a zip) are refused. `NaN`, `Infinity` and nesting deeper than 128 are refused,
     as serde_json does. An integer outside the 64-bit range, and `-0`, are reported by `inspect` as
     `float` (Python says `int`).
  6. **The review file must exist** for `mark` and `mine`:
     `there's no review file yet: run persona-builder claudeai titles first`. Python treats a missing
     file as empty (`mark` then writes a header-only file; `mine` mines nothing).
  7. **File names, not paths,** for the review and the output: plain names in the member's state
     folder, read and written through `read_private` and `write_private`. Python takes any path.
  8. **`--title` patterns** are compiled by the `regex` crate. A pattern it can't compile (a
     look-around, a backreference, `\Z`) is refused:
     `--title is not a valid pattern, or it uses look-around or a backreference, which Persona Builder can't read`.

### Reading the export (port of `export_files`)

```rust
pub struct Export { pub files: usize, pub conversations: Value }
pub fn read_export(path: &Path) -> Result<Export>;
```

- Open `path` with `OpenOptions::new().read(true).custom_flags(O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)`
  (as `mine.rs`'s `open_session` does) and `fstat` the handle. `ELOOP` is
  `<path> is a link: Persona Builder doesn't follow links. Use the export file itself`. `ENOENT` is
  `<path> doesn't exist`. Neither a file nor a folder: `<path> is not a file or a folder`.
- **A folder:** `files` counts the entries for which `fs::metadata` (following links, as Python's
  `is_file` does) says regular file. Then open `path/conversations.json` the same way (no link; must
  be a regular file; missing is `<path> has no conversations.json`), check its length against the
  cap, and read it.
- **A file whose first four bytes are `PK\x03\x04` or `PK\x05\x06`:** a zip. Open
  `zip::ZipArchive::new` over the `File` (streamed from disk: nothing is unpacked to disk and the zip
  is never read whole). Errors are `the export zip can't be read (<zip's message>)`. Then:
  - Walk the entries by index with `by_index_raw` (no decompression). An entry whose raw name ends
    `/` is a folder and is not counted; every other entry counts toward `files` (duplicates count, as
    in Python's `namelist`).
  - An entry is `conversations.json` when its raw name's last component, splitting at `/` and
    ignoring empty and `.` components (as `PurePosixPath(n).name` does), is exactly
    `conversations.json`. Exactly one must be found, else
    `the export zip should hold one conversations.json, and it holds <n>`.
  - Its declared size (`size()`) over `2 * 1024^3` is `the export's conversations.json is bigger than the 2 GB cap`;
    a declared size over `100 *` its `compressed_size()` (compute in `u128`) is
    `the export's conversations.json is compressed more than 100:1, so Persona Builder won't open it`.
    Both checks come before any decompression. Then `validate_header_consistency()` (zip 9.0.5 has
    no public duplicate check; this one refuses a repeated name and local headers that disagree with
    the central directory), as the "can't be read" error.
  - Then `by_index` and read through `.take(size + 1)` into a `Vec` that is **not** preallocated from
    the declared size. More bytes than declared is
    `the export's conversations.json is bigger than the zip says`. `zip` checks the CRC at the end;
    its error is the "can't be read" error.
- **Any other file:** `files` is 1, and the file itself is `conversations.json` (cap checked on its
  length).
- Decode: skip one leading `EF BB BF`; the rest must be UTF-8 (`str::from_utf8`), else
  `the export's conversations.json is not UTF-8 text`. Parse with `serde_json::from_str::<Value>`;
  failure is `the export's conversations.json is not valid JSON`.
- `inspect`, `titles` and `mine` all require a list:
  `the export's conversations.json isn't a list of conversations`.

### `inspect` (port of `type_name`, `key_tree`, `missing_fields`, `cmd_claudeai_inspect`)

```rust
pub struct Inspection { pub text: String, pub missing: Vec<String> }
pub fn inspect(export: &Export) -> Result<Inspection>;
```

- Type names: object, list, str, bool, int (`is_i64() || is_u64()`), float (any other number), null.
- `key_tree(items, prefix, tree)` exactly as the Python: `objects` = the items that are objects; for
  each key in the sorted union of their keys (Rust `str` order is code-point order, as Python's):
  `values` = that key's value in each object that has it; the entry for `<prefix>.<key>` is created
  on first sight (keep insertion order: a `Vec` of entries plus an index map, or serde_json's ordered
  `Map`); add the type names of `values` to its set, `len(values)` to `count`, `len(objects)` to
  `of`; when the key is `sender`, add the string values to its value set. Then recurse into
  `<prefix>.<key>[]` with the elements of every list value, flattened, and then into
  `<prefix>.<key>` with every object value. The same path can be reached twice (a key `a.b` and an
  object `a` holding `b`); the entry is shared, as `setdefault` shares it.
- `text`: `files: <n>`, then `conversations.json: list of <n>`, then per entry in insertion order
  `  <path>: <types sorted, joined by |> (<count>/<of>)`, plus ` values: <values sorted, joined by , >`
  when the value set is not empty; each line ends `\n`.
- `missing` exactly as `missing_fields`. When it is not empty the CLI prints `text` and then fails
  with `the export is missing fields Persona Builder reads: <missing joined by , >`; else it prints
  `text` and `ok: every field the adapter reads is present`.
- A value that is not a list is the not-a-list error, before anything is printed.

### Timestamps (port of `utc`, accepted difference 1)

```rust
pub fn utc(when: &str) -> Option<String>;
```

Accept exactly: `YYYY-MM-DD`, optionally followed by `T` or one space and `HH:MM` or `HH:MM:SS`,
optionally (only after `SS`) `.` and one or more ASCII digits, optionally then `Z` or `+HH:MM` /
`-HH:MM`. All digits ASCII. Year 0001–9999; a real calendar day (leap years); hour 00–23; minutes and
seconds 00–59; offset hours 00–23, offset minutes 00–59. No time, or no zone, means UTC. Digits of the
fraction past the third are dropped (truncated, never rounded). Shift to UTC by the offset with your
own days-from-civil arithmetic (no date crate); a result outside years 1–9999 is `None` (Python raises
`OverflowError`, also exit 1). Output `YYYY-MM-DDTHH:MM:SS.mmmZ`. The error, wherever `utc` is
`None`: `a date in the export isn't one Persona Builder can read: the export's shape changed, run persona-builder claudeai inspect`
(never the value). A non-string is `None` too.

Measured at pickup on Python 3.12.14 (pin these in the tests): accepted by both, `2026-03-01T12:00:00.000000Z`
→ `2026-03-01T12:00:00.000Z`; `2026-03-01` → `…T00:00:00.000Z`; `2026-03-01T12:00` →
`…T12:00:00.000Z`; `2026-03-01 12:00:00`; `…T12:00:00.123456789Z` → `.123Z`; `…T12:00:00.9999999Z` →
`.999Z`; `…T12:00:00.123+01:00` → `2026-03-01T11:00:00.123Z`; `…T12:00:00-23:59` →
`2026-03-02T11:59:00.000Z`; `…T12:00:00-00:00`; `2024-02-29T00:00:00`. Refused by both: `…z`
(lower case), `…T24:00:00`, `2026-02-29…`, `…+24:00`, `…T12:00:60`, `2026-3-01`, ` 2026-03-01`,
`+2026-03-01`, `…ZZ`, `…+00:00Z`, `…Z+01:00`, full-width digits; `0001-01-01T00:00:00+01:00` and
`9999-12-31T23:59:59-01:00` (Python: `OverflowError`). Accepted by Python only (difference 1):
`2026-03-01T12`, `2026-03-01x12:00:00` (any separator), `20260301T120000Z`, `2026-W09-1`,
`2026-03-01t12:00:00`, `…T1200`, `…T12:00:00,5Z`, `…T12:00:00.Z`, `…T12:00:00+05`, `…+0530`,
`…+05:30:15`, `…+05:30:15.5`, `…T12:00:00 +00:00`, `…T12:00.5`, `2026-03-01Z`,
`2026-03-01+01:00`.

### The review file (port of `read_review`, `write_review`)

```rust
pub struct ReviewRow { pub mine: String, pub created: String, pub updated: String, pub messages: String,
                       pub project: String, pub title: String, pub uuid: String }
pub const REVIEW_HEADER: &str = "mine\tcreated\tupdated\tmessages\tproject\ttitle\tuuid";
pub fn read_review(name: &str, bytes: &[u8]) -> Result<Vec<ReviewRow>>;
pub fn review_text(rows: &[ReviewRow]) -> String;
```

- Strict UTF-8 (`<name> is not UTF-8 text`), then `python::splitlines` (it covers universal newlines
  too). No lines, or a first line that is not exactly the header split at tabs:
  `<name>: not a review file (its first line must be mine created updated messages project title uuid)`.
- Each later line whose characters are not all `py_space` is a row: split at `\t`; fewer than 7
  fields is `<name>:<line>: a row needs 7 columns`; fields past the seventh are dropped (as `zip`
  drops them). Values are kept exactly (no trimming).
- `review_text`: the header, `\n`, then each row's 7 fields joined by `\t`, each followed by `\n`.

### Shared text helpers (refactor, keep `mine` byte-identical)

- Move `Builder::own_message` to `pub(crate) fn own_message(filters: &Filters, text: String) -> Option<String>`
  in `mine.rs`, and `Builder::text_blocks` to a free
  `pub(crate) fn text_blocks(content: &Value) -> std::result::Result<Vec<&str>, &'static str>` whose
  error is the key (`"content"` or `"text"`); `Builder` keeps thin wrappers that map the key to its
  own shape error. Make `message_record` `pub(crate)`. `mine_parity.rs` and `mine_rules.rs` must pass
  unchanged.
- `flat(text)`: split at runs of `py_space`, drop empties, join with one space (Python's
  `re.sub(r"\s+", " ", …).strip()`; `\s` in a `str` pattern matches what `str.isspace` matches).
- `chat_text(message)`: when `text` is a string whose `py_space`-trim is not empty, that string as
  it is; otherwise `text_blocks(message.content or null)` joined by `\n` (a `text` that is not a
  string is ignored, as in Python).
- `dotted(value, path)`: walk `path.split('.')` through objects; the end value if it is a string,
  else `""`.

### `titles` (port of `opening_title`, `cmd_claudeai_titles`)

```rust
pub struct Titles { pub rows: Vec<ReviewRow>, pub untitled: usize, pub opens: usize,
                    pub filtered: usize, pub empty: usize }
pub fn titles(filters: &Filters, export: &Export, existing: &[ReviewRow],
              project_field: Option<&str>) -> Result<Titles>;
pub fn titles_summary(t: &Titles, shown: &str) -> String;
```

- Marks: a map uuid → `mine` from `existing`, the last row winning.
- First, for every conversation in order, compute its sort key: it must be an object (shape error
  `conversation`), `created_at` present (shape `created_at`) and `utc` of it, and `uuid` a string
  (difference 2) with no tab or line break (difference 3). Sort by `(utc, uuid)` (stable).
- Then per conversation: `chat_messages` must be a list (difference 2); each message must be an
  object (`message` shape error); `humans` = those whose `sender` equals the string `human`.
  `name`: not truthy (`python::truthy`) or absent is `""`, a string is `flat`ted, any other truthy
  value is the shape error `name`. When the title is empty: `opening_title` (below), `untitled += 1`,
  then `filtered` when it is `[filtered]`, `opens` when it is not empty, else `empty`.
- `opening_title`: the first human message whose `chat_text` `flat`s to non-empty; none gives `""`.
  When `filters.kind(<that chat_text, unflattened>)` is `Some`, `[filtered]`. Else the `flat`ted text
  split at `" "`; the first 8 words, each shown as `…` when longer than 24 characters
  (`chars().count()`); `opens: ` + them joined by spaces, plus `…` when there were more than 8 words.
- Row: `mine` from the marks or `""`; `created` the first 10 characters of `utc(created_at)`;
  `updated` the same of `utc(updated_at)` when `updated_at` is truthy, else of `created_at`;
  `messages` the count of humans; `project` `flat(dotted(c, field))` when `project_field` is a
  non-empty string, else `""`; `title`; `uuid`.
- `titles_summary`: `listed <rows> conversations in <shown> (<kept> already marked<more>)\n` where
  `kept` counts rows whose `mine` is not empty and `<more>` is
  `; <untitled> untitled: <opens> by opening words, <filtered> filtered` when `untitled > 0`.

### `mark` (port of `cmd_claudeai_mark`)

```rust
pub struct Selector { pub uuids: Vec<String>, pub project: Option<String>, pub title: Option<Regex>,
                      pub before: Option<String>, pub after: Option<String>, pub unmarked: bool, pub all: bool }
pub fn mark(rows: &mut [ReviewRow], selector: &Selector, mark: &str) -> usize;
```

- The CLI treats an empty `--project`, `--title`, `--before` or `--after` as absent, as Python's
  truthiness does. No selector at all (none given, `--all` false):
  `mark needs a selector: --uuid, --project, --title, --before, --after, --unmarked or --all`,
  checked before the pattern is compiled, which is checked before the review is read.
- A row matches when every given selector holds: its uuid is in `uuids`; `project ==`;
  `title.is_match(title)`; `created <= before`; `created >= after` (plain string comparison, no date
  parsing, as in Python); `unmarked` and `mine` is empty. `--all` adds no condition.
- Each matching row whose `mine` differs counts one change; set it. Return the changes. The CLI
  rewrites the review (even with no change, as Python does) and prints
  `marked <n> conversations <mark>`.

### `mine` (port of `cmd_claudeai_mine`)

```rust
pub struct MinedExport { pub records: Vec<Map<String, Value>>, pub read: usize, pub not_mine: usize,
                         pub unmarked: usize, pub personal: usize, pub secret: usize, pub cutoff: Option<String> }
pub fn mine(filters: &Filters, export: &Export, review: &[ReviewRow], split: &mine::Split) -> Result<MinedExport>;
pub fn mine_summary(m: &MinedExport) -> String;
```

- Review map uuid → row, the last row winning. Per conversation in export order: object (shape
  `conversation`), `uuid` a string (difference 2); the mark is the row's `mine`, `py_space`-trimmed and
  lower-cased; not `y`, `yes` or `mine`: `not_mine += 1` when the mark is not empty, else
  `unmarked += 1`; skip.
- A marked conversation: `read += 1`; project `claude.ai`, plus `/<row.project>` when that is not
  empty; `context = ""`; `chat_messages` a list (difference 2); per message (an object, else the
  `message` shape error): sender `assistant` sets `context` to `chat_text` trimmed when that is not
  empty; any sender other than `human` is skipped; a human message's `chat_text`, `py_space`-trimmed,
  goes through `own_message`; a kept text needs `created_at` (shape `created_at`) and its `utc`, then
  `message_record(uuid, utc, project, text, context)` and `privacy::privacy`, counting `personal` and
  `secret` drops.
- Sort by `(timestamp, id)`, `mine::mark_split`, and the records are written with `mine::to_jsonl`.
  `--holdout` and `--cutoff` are parsed and checked exactly as `mine`'s (`split_choice`).
- `mine_summary`: `read <read> conversations marked mine; skipped <not_mine> not mine and <unmarked> unmarked\n`
  then `mined <records> messages; dropped <personal> personal records, <secret> secret records\n`.

### `private.rs`

`write_private` and `read_private` accept a plain name ending `.jsonl` or `.tsv` (an internal check,
message `the file name must be a plain name ending .jsonl or .tsv`). `validate_name` keeps its
`.jsonl` rule and message for the corpus names; add `pub fn validate_review_name(name: &str) -> Result<()>`
(`the review file name must be a plain name ending .tsv`). The CLI checks names with the specific
function before anything else.

### The CLI

```
persona-builder claudeai inspect --export PATH
persona-builder claudeai titles <member_dir> --export PATH [--review NAME] [--project-field KEY] [--dry-run]
persona-builder claudeai mark <member_dir> --as y|n [--review NAME] [--uuid ID]... [--project P]
                              [--title PATTERN] [--before DATE] [--after DATE] [--unmarked] [--all]
persona-builder claudeai mine <member_dir> --export PATH [--review NAME] [--out NAME]
                              [--holdout F | --cutoff TS] [--dry-run]
```

- `--review` defaults to `claudeai-review.tsv`, `--out` to `decisions-claudeai.jsonl` (the names
  `bootstrap/install.sh` uses). `--export` takes a leading `~/` as `projects_path` does. `inspect` needs
  no member. The other three load the member, its filters and the state folder check exactly as `mine`
  does (`checked_member`).
- `titles` reads the existing review with `read_private` (a missing file means no marks; any other
  error stops), writes the new one with `write_private`, and prints
  `titles_summary(t, "~/.claude/state/<slug>/<review>")`; with `--dry-run` it writes nothing and adds
  `dry run: nothing written`.
- `mark` reads the review (difference 6), applies `mark`, writes it, prints the line.
- `mine` reads the review (difference 6), prints `mine_summary`, then
  `saved to ~/.claude/state/<slug>/<out>`, or with `--dry-run` `dry run: nothing written` and no file
  or folder created.
- Doc comments (the `--help` lines), plain words: the group `/// Read your claude.ai chats: check the
  export, list the chats, mark which are yours, and read them.`; `inspect` `/// Check the export has
  what Persona Builder reads (shows field names only, never your words).`; `titles` `/// List your
  claude.ai chats in a review file, so you can mark which are yours.`; `mark` `/// Mark chats in the
  review file as yours (y) or not (n).`; `mine` `/// Read the chats you marked as yours into your
  assistant's private folder.`

### `.ai/PROJECT.md`

The `builder/ingest/` row's description becomes "…, mining Claude Code transcripts and the claude.ai
export, merging and splitting the corpus".

## Tests (each red first)

All fixtures are invented: names like Ada, Ari, Robin, Sam, Wren; no real person, no real path, no
real export.

1. **`tests/crew_claudeai_fixtures.py`** (new, Python, standard library): `python3 -I
   tests/crew_claudeai_fixtures.py <case> <dir>` writes one named case (the export, and a review file
   when the case has one) and prints a JSON object `{"export": <relative name>, "review": <name or null>,
   "titles": [<extra args>], "marks": [[<mark args>], …], "mine": [[<extra args>], …]}`. Build
   conversations with helpers shaped like `tests/crew_sources_test.py`'s `message` and
   `conversation`. Cases:
   - `basic`: that test's `EXPORT`, as a zip with `users.json` and `projects.json`, with
     `--project-field project.name`;
   - `folder` and `plain`: `basic` as a folder and as a bare `conversations.json`;
   - `nested_zip`: `conversations.json` under `data-2026/`, a folder entry, and two other files;
   - `untitled`: that test's untitled list (opening words, filtered, long words, a link, `None` and
     blank names, an empty first human message);
   - `timestamps`: conversations and messages using every form accepted by both (above), offsets that
     cross midnight and a year end, equal `created_at` values ordered by uuid, a falsy `updated_at`;
   - `blocks`: `text` blank so `content` is used, `content` a string, an object, `null`, absent, a
     block with no `text`, non-text blocks, a numeric `text` field (ignored);
   - `messages`: pasted prefixes, the noise words, a 4000- and a 4001-character message, personal
     and secret drops, an assistant reply longer than 300 characters, an empty assistant reply that
     keeps the earlier context, a `system` sender, a skipped message with a bad `created_at` (no
     error);
   - `marks`: a review with `y`, ` Yes `, `MINE`, `n`, `no`, `maybe` and empty marks, a uuid missing
     from it, a row whose conversation is gone, a row with 9 columns, blank lines and CRLF endings;
     `titles` re-run keeps the marks;
   - `unicode`: names and texts with é, 日, 😀, U+2028, U+0085, `\x1c`, tabs and newlines in a name
     (flattened);
   - `inspect_shapes`: lists of lists, a key `a.b` beside an object `a` holding `b`, several `sender`
     values, int, float, bool and null values, and a second export missing `created_at` and
     `sender` (inspect prints its tree, then fails).
   `marks` arg lists cover the selectors of `crew_sources_test.py`'s
   `test_marks_by_project_title_date_and_uuid_and_rerun_keeps_marks`, `--unmarked`, `--all`, and an
   empty `--project ""` with `--all`. `mine` arg lists: none (holdout 0.15), `--holdout 0`,
   `--holdout 1`, `--holdout 0.5`, `--cutoff 2026-03-02`, `--cutoff 9999`, `--cutoff ""`.
2. **`builder/ingest/tests/claudeai_parity.rs`**: for each case, run
   `python3 -I scripts/crew_decisions.py --member <m> claudeai …` (absolute paths) for `inspect`,
   `titles`, each `mark` in order, and each `mine`; compare stdout with `inspect`'s `text` (+ the ok
   line), `titles_summary` (shown = the path given to Python), the `marked …` line and
   `mine_summary`; the review file's bytes with `review_text`; the JSONL bytes with `to_jsonl`; and
   exit codes. Rust's mark and mine run on the review the Rust side wrote. Members: `ADA` and `ARI`,
   as `mine_parity.rs` has them. Use `common::parity_tools_available()`.
3. **`builder/ingest/tests/claudeai_utc.rs`**: every string listed under *Timestamps*, run through
   both `utc` and the Python's `utc` (one `python3 -I -c` call over a JSON list); the shared ones
   must agree, refusals must both refuse, and the Python-only list is asserted as accepted by Python
   and refused by Rust.
4. **`builder/ingest/tests/claudeai_rules.rs`** (Rust only):
   - every error above, each checked for its exact message and that no export text appears in it
     (use a sentinel word in each bad export); for the ones the Python also refuses, assert the
     Python exits 1;
   - each accepted difference, 2 to 8, with the Python's own behaviour beside it;
   - the zip caps: a 100:1+ entry; a central directory whose declared size is forged to
     `2 * 1024^3 + 1` (the `struct.pack_into` trick of `crew_hardening_test.py`'s
     `test_zip_limits_before_reading`); a declared size smaller than the data; two and zero
     `conversations.json`; a duplicate name; a bzip2 entry (write the method id into a stored
     entry's headers with raw bytes); an encrypted flag; a CRC mismatch; an empty zip
     (`PK\x05\x06` only);
   - a link at the export path and at a folder's `conversations.json`; a pipe as the export; a
     missing path; a BOM in each form; UTF-16 in a zip;
   - `inspect` output holds no title, message text, uuid or date from the export (sentinels), only
     field names, types, counts and `sender` values.
5. **`builder/cli/tests/claudeai.rs`** (style of `builder/cli/tests/mine.rs`, a scratch `HOME`, a
   member made from `crew/_template`): `titles` → `mark` → `mine` end to end, the review and the
   corpus 0600 in `~/.claude/state/<slug>/`; `titles` re-run keeps marks; `--dry-run` on `titles`
   and `mine` creates and changes no file; name rules (`--review x.jsonl` and `--out x.tsv`
   refused, a `/` refused); `mark` and `mine` with no review (difference 6); `mark` with no
   selector; a bad `--title`; `--holdout 2`; `--holdout` with `--cutoff` (usage error); a link at
   the review name refused; `inspect` exit 0 with the ok line, and exit 1 naming the missing fields
   on a broken export.
6. **`builder/ingest/tests/private_write.rs` and `private_read.rs`**: a `.tsv` name works; `.txt` and
   `x.tsv/` are refused.

## Done when

Every test above went red on its assertion first and is green; `mine_parity.rs`, `mine_rules.rs`,
`corpus_parity.rs` and the Python suites pass unchanged. `cargo fmt --check`,
`cargo clippy --all-targets --locked -- -D warnings` and `bash scripts/ci-local.sh` pass (the sandbox
refuses local sockets, so `builder/save/tests/ureq_wire.rs` fails there; that is expected, say so).
Report: each file you added or changed, the red you saw for each test file, every case where the
engines disagreed outside the accepted differences (stop and report it; never bend a test or the
Python to make it pass), and anything in this brief you could not do as written. Leave
`.ai/handoffs/persona-builder.md` alone: Claude writes the record.
