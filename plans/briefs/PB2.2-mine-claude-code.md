# PB2.2 brief: mine Claude Code transcripts, in Rust

Written by Claude (lane V1) for Codex, 2026-10-24. Spec: lane V's `PB2.2` entry, split from PB2 of the
Persona Builder plan. Read the repo's `AGENTS.md` first, then `plans/briefs/PB2.1-privacy-check.md` (the
slice this one builds on: same crate, same parity-test style). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`): each test file goes red on its assertion before its code exists,
then green. Security rules: `~/.codex/skills/security-and-hardening/SKILL.md`. Never commit; Claude
commits. Run cargo with `CARGO_NET_OFFLINE=true` (your sandbox has no network; every crate named here is
in the local registry).

## What this slice does

1. `persona_builder_ingest` gains `mine` and `folders`, at parity with `scripts/crew_decisions.py`'s
   `cmd_mine` and `cmd_folders`: the same records, in the same order, written as the same bytes, and the
   same printed lines.
2. The mined file is written into the member's state folder only, 0600 in 0700 folders, through
   `O_NOFOLLOW` directory descriptors and an atomic replace, as `private_write` does.
3. The CLI gains `persona-builder mine <member_dir>` (with `--dry-run`) and
   `persona-builder folders <member_dir>`.
4. One Python fix: `cmd_folders` sorts folders that share a project name by folder name, so its order no
   longer depends on the filesystem.

## Decisions (made, do not reopen)

### Where Python is the oracle, and where it is not

- **Parity means:** for every input the Python accepts and finishes on, Rust gives the same records, the
  same JSONL bytes and the same two summary lines. Parity tests run the real Python on the same folder
  and compare.
- **Where the Python raises an exception** (it reads a key with `.get` on something that is not an
  object, joins a non-string, compares a number with a string), Rust returns an error and writes
  nothing. Both exit 1. The messages differ; Rust's is
  `<session file>:<line>: a transcript record has an unexpected shape (<key>)`, where `<line>` is the
  1-based number of the `\n`-ended line and `<key>` is the fixed key name from this brief. **Never put
  any record text in an error.**
- **Follow the Python's order of checks exactly**, so an error happens where the Python's would
  (`typed_text` checks `isMeta` and `isCompactSummary` before it reads `origin`; `questions_in` reads
  `answers` only once it reaches a question).
- **Accepted differences, each pinned by a Rust-only test and listed in your report:**
  1. A link inside the projects folder (a folder link, or a session file link) is refused: error
     `<path> is a link: Persona Builder doesn't follow links in your Claude Code history. Move it away and try again`.
     The Python follows them. The projects folder itself may be a link (it is what the person named).
  2. A line `serde_json` cannot parse is skipped, as the Python skips a line `json.loads` cannot. So
     these, which `json.loads` accepts and `serde_json` refuses, are skipped by Rust: `NaN`, `Infinity`,
     a lone surrogate escape (`"\ud800"`), a number out of `f64` range, nesting deeper than 128.
     Claude Code writes none of them.
  3. Where the Python quietly accepts an odd type it never meant to (an `answer` that is not a string,
     which `str()` would turn into Python's repr; a truthy non-string `timestamp`; `options` given as
     an object), Rust errors with the shape error.
  4. `--holdout` must be a finite number from 0 to 1, else
     `--holdout must be a number from 0 to 1`. (Above 1 the Python wraps to a negative index.)
     `--holdout` and `--cutoff` together are a clap usage error (the Python lets `--cutoff` win).
  5. A folder or session file name that is not UTF-8 is an error naming it (`Path::display`). The
     Python fails too, but only once a record from it is kept.
  6. An entry named `*.jsonl` that is not a regular file (a folder, a pipe) is an error,
     `<path> is not a file`, in `mine` and in `folders` (the Python's `folders` counts it).

### Reading the transcripts

- **Folder listing:** `fs::read_dir(projects)`; for each entry use `entry.file_type()` (never follows).
  A link: refused (difference 1). A directory: a project folder. Anything else: skipped. Sort folders by
  name as Rust `String`s (code point order, which is Python's `Path` order for sibling names).
- **`mine` skips an excluded folder without listing it** (`filters.excluded(name)`, counted in
  `excluded`). `folders` lists every folder, excluded ones included, to count their sessions.
- **Session files:** in a folder, every entry whose name ends with `.jsonl` (case-sensitive; a hidden
  `.x.jsonl` counts, as pathlib's glob counts it), sorted by name. A link: refused. Not a regular file:
  difference 6. The session id is `Path::file_stem` (`a.b.jsonl` gives `a.b`, as Python's `stem`).
- **Open each session file with `O_NOFOLLOW`** (`OpenOptionsExt::custom_flags(libc::O_NOFOLLOW)`, as
  `check.rs` does) and read it **streaming**: `BufReader::read_until(b'\n')`, one `\n`-ended chunk at a
  time. This equals the Python's whole-file read, because `\n` is ASCII and never splits a valid UTF-8
  sequence. Then, per chunk:
  1. **Decode like `errors="ignore"`:** drop every invalid UTF-8 sequence, keep the rest. Use
     `<[u8]>::utf8_chunks()` and keep only each chunk's `valid()` part. Never `from_utf8_lossy` (it
     inserts U+FFFD, which the Python does not).
  2. **Split like `str.splitlines()`:** the chunk (without its `\n`) splits further at `\r\n`, `\r`,
     `\x0b`, `\x0c`, `\x1c`, `\x1d`, `\x1e`, `\x85`, ` ` and ` `. A raw U+2028 inside a JSON
     string therefore breaks that line, and both engines skip its halves.
  3. **Parse each non-empty piece** with `serde_json::from_str::<Value>`; on error, skip the piece.
     A piece that parses but is not an object: shape error `(record)`.
- **Python truthiness** (`truthy(&Value) -> bool`): false for `null`, `false`, a number equal to 0, `""`,
  `[]`, `{}`; true otherwise. Use it wherever the Python tests a value with `not`, `or` or `bool()`.
- **Python whitespace** (`py_space(char) -> bool`): `char::is_whitespace(c) || ('\x1c'..='\x1f').contains(&c)`.
  Python's `str.strip()` and `str.isspace()` also strip U+001C–U+001F; Rust's `trim` does not. Every
  `.strip()` in the ported code is `trim_matches(py_space)`, and every regex `\s` the Python uses is
  `[\s\x1c-\x1f]` in Rust. A test proves `py_space` equals Python's `str.isspace` over every code point.
- **Lengths and slices count code points** (`chars()`): `len(text) > MAX_MESSAGE_CHARS` (4000),
  `context[-CONTEXT_CHARS:]` (the last 300), `timestamp[:10]`.

### Porting the record builders (`scripts/crew_decisions.py`, lines 189–358)

Port these functions one for one, with the same names, in `builder/ingest/src/mine.rs`:
`project_name`, `record_id`, `marks`, `recommended_label`, `member_label`, `parse_answer`, `outcome`,
`questions_in`, `text_blocks`, `typed_text`, `own_message`, `assistant_text`, `harvest`,
`message_record`, `mark_split`, and `cmd_mine`'s loop. The rules the Python leaves implicit:

- **`project_name(folder, home: &Path)`** takes the home as a parameter (the CLI passes `$HOME`, as
  Python's `Path.home()` reads it). The four regexes are the Python's, on the `regex` crate:
  `--claude-worktrees-.*` removed; the home with every character outside `[A-Za-z0-9]` turned into `-`
  (one `-` per character), plus `-`, stripped as a prefix; else `^-(Users|home)-[A-Za-z0-9]+-` removed;
  then `^projects-`; a leading `-` becomes `.`.
- **`record_id`** is `"<kind>:" +` the first 12 hex digits of the SHA-1 of
  `"<session>\n<timestamp>\n<text>"` as UTF-8. Use **`sha1_smol = "=1.0.1"`** (BSD-3-Clause, no
  dependencies, in the local registry). SHA-1 here is only a stable id, never a security control; say
  so in a one-line comment.
- **The gate mark:** `(?i)[\s\x1c-\x1f]*\([\s\x1c-\x1f]*(recommended|<word>)(?:[\s\x1c-\x1f]*[·•/+,&-][\s\x1c-\x1f]*(recommended|<word>))?[\s\x1c-\x1f]*\)`
  with `<word>` = `regex::escape(&member.mark_word)`. `marks` collects groups 1 and 2 of every match,
  lowercased with `to_lowercase()`. `member_label` compares with `member.mark_word.to_lowercase()`.
- **`parse_answer`:** single select as the Python. Multi select: labels sorted longest first by code
  point count, a stable sort (ties keep option order); each label found in `rest` is picked and its
  first occurrence removed (`replacen(label, "", 1)`; an empty label is "found" and changes nothing, as
  in Python); then `[\s\x1c-\x1f]*,([\s\x1c-\x1f]*,)+[\s\x1c-\x1f]*` → `", "` and `trim_matches` of
  `' '` and `','` only.
- **`questions_in`** keeps the Python's tolerance exactly: `toolUseResult` not an object, or lacking
  `questions` or `answers`: no questions. `questions` not an array: shape error `(questions)`. A
  question not an object: `(question)`. Its `question` absent or null, bool or number: skipped (no JSON
  key can match it); an array or object: `(question)`. `answers` not an object (read only now):
  `(answers)`. The answer absent or null: skipped; not a string: `(answer)`. `options` absent: `[]`; an
  array of objects: each `label` and `description` absent gives `""`, a non-string is `(options)`;
  anything else: `(options)`. `header` absent: `""`; not a string: `(header)`. `multiSelect`:
  `truthy`. Notes: `annotations` falsy gives none; truthy and not an object: `(annotations)`; its entry
  for the question falsy gives none; truthy and not an object: `(annotations)`; its `notes` falsy gives
  `""`; truthy and not a string: `(notes)`. `timestamp` was checked by `harvest`.
- **`text_blocks(content)`:** a string gives `[string]` (even `""`, as `isinstance(str)` comes first);
  falsy gives `[]`; an array gives, for each element that is an object whose `type` is the string
  `"text"`, its `text` (absent gives `""`, non-string is `(text)`); a non-empty object gives `[]` (the
  Python iterates its keys and keeps none); `true` or a non-zero number: `(content)`.
- **`message`:** absent is `{}`; present and not an object: `(message)` (the Python's `.get` on it
  raises).
- **`typed_text`:** `isMeta` truthy, or `isCompactSummary` truthy: none. Then `origin`: falsy is `{}`;
  truthy and not an object: `(origin)`; its `kind` not the string `"human"`: none. Then the blocks: each
  with every `(?s)<system-reminder>.*?</system-reminder>` removed, then `trim_matches(py_space)`; keep
  those non-empty and not starting with `<`; join with `\n`; then `own_message`.
- **`own_message`:** none when empty, when it starts with any pasted prefix (an empty prefix matches
  everything, as in Python), when `to_lowercase()` is `try again`, `continue` or `retry`, or when it has
  more than 4000 code points.
- **`harvest`:** for each record in order: if `type` is the string `"assistant"`, `context` becomes
  `assistant_text(record)` trimmed with `py_space` when that is non-empty (else it stays). Then, unless
  `type` is `"user"` and `timestamp` is truthy, next record. A truthy `timestamp` that is not a string:
  `(timestamp)`. Then the record's questions, then its typed message.
- **Records are `serde_json::Map`s built in the Python's key order** (the crate already enables
  `preserve_order`). A question: `id`, `kind`, `timestamp`, `project`, `header`, `question`,
  `multiSelect`, `options` (each `{label, description}`), `recommended` (string or null),
  `<slug>_pick` (`member.pick_field()`, string or null), `picks`, `own_words`, `notes`, `outcome`. A
  message: `id`, `kind`, `timestamp`, `project`, `text`, `context`. `split` is appended last.
- **Privacy:** each harvested record goes through `persona_builder_ingest::privacy::privacy` (PB2.1);
  `secret` and `personal` are counted and dropped.
- **Order and split:** sort kept records by `(timestamp, id)` with a stable sort. `mark_split`: question
  timestamps sorted; with no cutoff and at least one question,
  `held = max(1, (n as f64 * holdout).round_ties_even() as usize)` (Python's `round` is half-to-even)
  and `cutoff = questions[n - held]`; each record is `heldout` when a cutoff exists and
  `timestamp >= cutoff` (string order), else `train`. A given cutoff is used as is, even `""`.

### Writing

- **JSONL exactly as `json.dumps(r, ensure_ascii=False) + "\n"`:** a `serde_json::ser::Formatter` of
  your own, `PythonFormatter`, that writes `", "` between object entries and array items and `": "`
  after a key, and otherwise defers to the defaults. `serde_json`'s string escaping already matches
  Python's with `ensure_ascii=False` (`\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`, other controls as
  lowercase `\u00xx`, U+007F and non-ASCII raw); a test pins it against Python over every code point
  below U+0100 plus é, 日, 😀, U+2028.
- **The state folder:** add to `builder/core/src/member.rs`
  `pub fn private_state_dir(member: &Member, home: &Path) -> Option<PathBuf>`, the check `forget` makes
  today (the path is exactly `<home>/.claude/state/<slug>`, absolute, no `.` or `..` parts, a one-part
  slug), moved there unchanged. `forget` calls it and keeps its own message and tests. `mine` calls it
  and, on `None`, fails with
  `Persona Builder keeps your history only in ~/.claude/state/<slug>. This assistant's member.json says <state_dir>: change it to ~/.claude/state/<slug> and try again`.
- **`builder/ingest/src/private.rs`:** `pub fn write_private(home: &Path, slug: &str, name: &str, bytes: &[u8]) -> Result<PathBuf>`,
  the port of `private_directory` plus `private_write`:
  1. `fs::canonicalize(home)` (the home itself may sit behind a system link, `/tmp` on macOS).
  2. Open `/` with `O_RDONLY | O_DIRECTORY | O_NOFOLLOW`, then walk every part of the canonical home,
     then `.claude`, `state` and the slug, each with `libc::openat` and those flags; a missing part of
     `.claude/state/<slug>` is made with `libc::mkdirat(…, 0o700)` first (`EEXIST` is fine). A part that
     is a link or not a folder (`ELOOP` or `ENOTDIR`) fails with
     `<path> is a link or not a folder: Persona Builder writes your history only into a real folder. Move it away and try again`.
     Existing folders keep their mode, as in Python.
  3. In the slug folder, `openat` a temporary `.<name>.<pid>-<nanos>-<counter>.tmp` with
     `O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC`, mode 0600 (retry a new name on `EEXIST`),
     wrap it in a `File` (`OwnedFd`), `set_permissions(0o600)`, write all, `sync_all`, then
     `libc::renameat` onto `name` in the same folder. On any failure, `unlinkat` the temporary.
  Own every descriptor as `OwnedFd` so each closes on every path. Keep all `unsafe` in this one file,
  each block with a `// SAFETY:` line. `name` must be a bare file name: no `/`, not starting with `.`,
  ending `.jsonl`; else `the file name must be a plain name ending .jsonl`.

### The library surface

```rust
// builder/ingest/src/mine.rs
pub enum Split { Holdout(f64), Cutoff(String) }
pub struct Mined {
    pub records: Vec<serde_json::Map<String, Value>>,
    pub questions: usize, pub held: usize, pub messages: usize,
    pub cutoff: Option<String>, pub excluded: usize, pub personal: usize, pub secret: usize,
}
pub fn mine(member: &Member, filters: &Filters, projects: &Path, home: &Path, split: &Split) -> Result<Mined>;
pub fn summary(mined: &Mined) -> String;   // cmd_mine's two printed lines, each ending "\n"
pub fn to_jsonl(records: &[Map<String, Value>]) -> String;

pub struct FolderRow { pub excluded: bool, pub sessions: usize, pub project: String, pub folder: String }
pub fn folders(filters: &Filters, projects: &Path, home: &Path) -> Result<Vec<FolderRow>>;
pub fn render_folders(rows: &[FolderRow]) -> String; // cmd_folders' lines, byte for byte
```

`summary` prints `mined <q> questions (<held> held out) and <m> messages, <span>; held out from <cutoff>`
then `dropped <e> excluded folders, <p> personal records, <s> secret records`, where `<span>` is
`<first[:10]> → <last[:10]>` of the sorted records or `none`, and `<cutoff>` is `nothing` when the cutoff
is none or empty (Python's `cutoff or 'nothing'`). `folders` sorts rows by `(project, folder)`.
Put the Python-compatibility helpers (`truthy`, `py_space`, the decode, the line split,
`PythonFormatter`) in `builder/ingest/src/python.rs`, documented as "matches CPython 3".
`lib.rs` gains `pub mod mine; pub mod private; pub mod python;`.

### The CLI

```
persona-builder mine <member_dir> [--projects DIR] [--out NAME] [--holdout F | --cutoff TS] [--dry-run]
persona-builder folders <member_dir> [--projects DIR]
```

- `<member_dir>` is the assistant's folder, as `forget`, `pack` and `on` take it; the member is
  `read_member(<member_dir>/member.json)` and the filters `Filters::compile(&member.filters)`.
- `--projects` defaults to `$HOME/.claude/projects`; a leading `~/` is expanded with `$HOME`. A missing
  folder fails with `<path> doesn't exist: there is no Claude Code history there`.
- `--out` defaults to `decisions-claude-code.jsonl`, written by `write_private` into the state folder.
  `--holdout` defaults to `0.15`.
- `mine` prints `summary`, then `saved to ~/.claude/state/<slug>/<name>`; with `--dry-run` it checks
  the state folder setting, mines, prints `summary`, then `dry run: nothing written`, and creates no file
  and no folder.
- `folders` prints `render_folders`.
- Doc comments (the `--help` lines), in plain words: `/// Read your Claude Code history into your
  assistant's private folder.` and `/// List the Claude Code project folders and which are left out.`

### The Python fix

In `cmd_folders`, sort with `key=lambda p: (project_name(p.name), p.name)`. Add a test to
`tests/crew_hardening_test.py` that patches `Path.iterdir` to yield two folders that share a project
name in reverse name order, and asserts `folders` prints them in name order.

### `.ai/PROJECT.md`

The `builder/ingest/` row's description becomes "Reading histories into the corpus
(persona_builder_ingest): the privacy filters, the privacy check, mining Claude Code transcripts".

## Tests (each red first)

All fixtures are invented: names like Ada, Ari, Robin, Sam, Wren; no real person, no real path.

1. **`builder/ingest/tests/python_compat.rs`** (each compares with `python3 -I -c …`, skipped by
   `parity_tools_available()` as PB2.1's are):
   - `py_space` equals `str.isspace` for every code point 0..=0x10FFFF (skip surrogates);
   - the decode equals `bytes.decode("utf-8", "ignore")` on: a lone `\xff`, a truncated 3-byte
     sequence, a surrogate encoding `\xed\xa0\x80`, an overlong `\xc0\xaf`, a 4-byte sequence above
     U+10FFFF, valid text mixed with each;
   - the line split equals `str.splitlines()` on a string holding every separator, `\r\n`, a trailing
     separator and empty lines (ignore empty pieces on both sides);
   - `to_jsonl` equals `json.dumps(r, ensure_ascii=False) + "\n"` for a record holding every code
     point below U+0100, é, 日, 😀, U+2028, nested lists and objects, `true`, `false`, `null`, `[]`, `{}`.
2. **`tests/crew_mine_fixtures.py`** (new, Python, standard library): `python3 -I tests/crew_mine_fixtures.py <case> <projects_dir>`
   writes one named case's session files, built with `crew_fixtures.py`'s `ask`, `said` and `reply`
   (insert its own folder on `sys.path`, as `crew_decisions_test.py` does, because `-I` drops it).
   Raw bytes go through `Path.write_bytes`. It prints nothing and exits 0.
3. **`builder/ingest/tests/mine_parity.rs`**: for each case, write the folder with the fixture script,
   run `python3 -I scripts/crew_decisions.py --member <m> mine --projects <p> --out <f> [split flags]`
   with `HOME=/Users/p`, and compare its output file's bytes with `to_jsonl(&mined.records)` and its
   stdout with `summary(&mined)`, from `mine(…, home = /Users/p, …)`. Members: the `ADA` and `ARI`
   configs of `tests/crew_decisions_test.py` (copy them; `state_dir` any absolute path), and one with a
   `private.json` overlay (`PrivateOverlay.test_its_filters_extend_the_members`). Cases:
   - `ada_fixture` (copy it into the fixture script), with no split flag, `--holdout 1`, `--holdout 0`,
     `--holdout 0.5` on 3 questions and on 5 (half-to-even), `--cutoff 2026-09-02T10:00:00Z`,
     `--cutoff 9999`, and `--cutoff ""`;
   - the `AnotherMember` sessions (another member's mark is not theirs; their filters);
   - multi select: `A, B` where one label is a substring of another, own words between picks, runs of
     commas, an empty label;
   - single select: the answer is a label; the answer is own words with surrounding spaces and `\x1c`;
   - notes: present, `annotations` absent, null, `{}`, entry null, `notes` null;
   - a question with no `question`, a number `question`, a null answer, an answer missing from
     `answers`; a `toolUseResult` that is a string, and one lacking `answers`;
   - typed text: a system reminder removed, a block starting `<` dropped, a list with an image block,
     `isMeta` and `isCompactSummary` as `1`, `"x"`, `[0]`, `0`, `""`, `[]`; `origin` absent, null,
     `{"kind":"tool"}`; `Try Again`, `CONTINUE`; a pasted prefix; exactly 4000 and 4001 code points
     with multi-byte characters;
   - context: an assistant reply over 300 code points ending in emoji; a whitespace-only reply, and a
     `\x1c`-only reply, keep the previous context; an assistant record with no timestamp still sets
     it; content a string, a list of text blocks, an object;
   - bytes: CRLF endings, a last line with no newline, blank lines, an unparsable line, invalid UTF-8
     inside a text value, a raw U+2028 and a raw U+0085 inside a JSON string, a raw `\x1c` between two
     records on one line;
   - user records with `timestamp` absent, `""` and null are skipped;
   - folder names: a worktree suffix, another Mac's `-Users-someone-projects-x`, `-home-x-y`, a
     leading `-` after the strip, `projects-` stripped, and `HOME=/Users/jane.doe` with its encoded
     folder; files `a.b.jsonl`, `.hidden.jsonl`, `notes.txt` (ignored); folders whose names sort
     differently by byte and by component (`a-b`, `a.b`, `a_b`, `B`, `a`);
   - the same message twice at the same time in one session (stable order);
   - an empty projects folder; a folder with no sessions.
   Then the error cases, asserting the Python exits 1 and Rust returns `Err` whose text contains the
   shape-error key and no record text: a line `[1, 2]`; an assistant `message` of `null`; a user
   `timestamp` of `5`; `content` of `7`; `origin` of `"web"`; `options` of `{"a": 1}`
   (Rust only: the Python accepts an object; difference 3); an `answer` of `["A"]` (Rust only).
4. **`builder/ingest/tests/mine_rules.rs`** (Rust only): each accepted difference (1, 2, 4, 5, 6) as
   its own test; `folders` parity with the Python's stdout on a fixture with an excluded folder and two
   folders sharing a project name (after the Python fix).
5. **`builder/ingest/tests/private_write.rs`**: under a scratch home, `write_private` makes
   `.claude/state/wren` 0700 and the file 0600 with the bytes; a second write replaces it and leaves no
   `.tmp`; an existing state folder keeps its mode; a link at `.claude`, at `state` and at `wren` is
   refused and nothing is written where it points; a link at the file name is replaced, never written
   through (its target keeps its bytes); `../x.jsonl`, `a/b.jsonl`, `.x.jsonl` and `x.json` are
   refused.
6. **`builder/core`**: `private_state_dir` gives the path for `~/.claude/state/<slug>` and `None` for
   each case `forget`'s test refuses; `forget`'s tests pass unchanged.
7. **`builder/cli/tests/mine.rs`**: with a member made by `new_member` (as `privacy_check.rs` does) and a
   fixture projects folder: `mine --dry-run` prints the two summary lines and `dry run: nothing written`,
   and `~/.claude/state` does not exist after; `mine` writes `~/.claude/state/wren/decisions-claude-code.jsonl`
   (0600, folder 0700) and prints `saved to ~/.claude/state/wren/decisions-claude-code.jsonl`; a
   `state_dir` elsewhere is refused with the message above and nothing is written; `--out ../x.jsonl` is
   refused; `--holdout 2` is refused; `folders` prints the rows and the totals line; a missing projects
   folder gives its message. No record text in stderr in any failing case.

## Done when

Every test above went red on its assertion first and is green. `cargo fmt --check`,
`cargo clippy --all-targets --locked -- -D warnings` and `bash scripts/ci-local.sh` pass (the lock file
gains `sha1_smol`; run `cargo build` once so `--locked` passes). Report: each file you added or changed,
the red you saw for each test file, every case where the engines disagreed outside the six accepted
differences (stop and report it; never bend a test or the Python to make it pass), and anything in this
brief you could not do as written.
