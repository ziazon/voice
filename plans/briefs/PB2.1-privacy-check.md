# PB2.1 brief: the privacy filters and the privacy check, in Rust

Written by Claude (lane V1) for Codex, 2026-10-24. Spec: lane V's `PB2.1` entry, split from PB2 of the
Persona Builder plan. Read the repo's `AGENTS.md` first. Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`): each test file goes red on its assertion before its code
exists, then green. Never commit; Claude commits.

## What this slice does

1. A new crate, `builder/ingest` (package `persona-builder-ingest`, library `persona_builder_ingest`),
   holds a member's privacy filters, the record check `privacy`, and the folder check that
   `scripts/crew_decisions.py privacy-check` runs today, at parity with the Python.
2. The CLI gains `persona-builder privacy-check <folder>`.
3. `persona-builder save push` runs the real privacy check, over exactly the files the save would
   commit, in place of `NotBuiltYet`. `NotBuiltYet` and `text::PRIVACY_NOT_BUILT` are then deleted.

It is a new crate, not part of `builder/core`, because core reads and writes files only, and this
crate runs `git ls-files` and `git check-ignore`.

## Decisions (made, do not reopen)

- **The regex engine is the `regex` crate, pinned `=1.13.1`** (MIT OR Apache-2.0; the version
  Flightdeck's lock uses). Default features. Its own docs say it has no look-around and no
  backreferences (`regex-1.13.1/src/lib.rs`, lines 4–6). Every construct the template's filters use is
  supported: `\b`, `\d`, `\w`, `\s`, classes, `(?:…)`, counted repeats. Both engines are Unicode-aware by
  default and pick the leftmost-first match, so match starts agree on the template's patterns.
- **Compile each filter with `RegexBuilder::new(pattern).case_insensitive(true).build()`**, the
  `re.I` of `compile_pattern`. If a template pattern is over the default size limit, raise
  `size_limit` to `64 * 1024 * 1024` and say so in your report; do not change any pattern.
- **A pattern that does not compile is refused, never rewritten:**
  `member config: filters.<key> is not a valid regex, or it uses look-around or a backreference, which Persona Builder can't read`
  (`<key>` is `excluded_folders`, `personal` or `secret`). Never put the regex crate's own error text
  in the message: it is several lines long and quotes the pattern.
- **The filters come from `persona_builder::member::read_member`** (PB1), which already lays
  `private.json` over `member.json` the way the Python does (patterns joined as `(?:a)|(?:b)`, prefixes
  appended). Do not re-implement that merge.
- **The template's filters are compiled in:**
  `include_str!("../../../crew/_template/member.json")` (from `builder/ingest/src/check.rs`), parsed with
  `serde_json` (`preserve_order`), never read from disk at run time. Persona Builder ships without the
  repo.
- **Paths sort by component**, as Python's `sorted()` on `Path` objects does (`a/b` before `a-b`).
  Rust's `Path`/`PathBuf` `Ord` already compares by component: sort `PathBuf`s, never `String`s.
- **Files are opened with `O_NOFOLLOW`** (`std::os::unix::fs::OpenOptionsExt::custom_flags(libc::O_NOFOLLOW)`;
  add `libc = "=0.2.191"`, already in the lock). A file that became a link after the listing fails
  with `Error::Io`, never followed.
- **git for the CLI walk** runs as `Command::new("git")` with `env_clear()`, then only `HOME` and
  `PATH` copied in, `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, stdin null, and `-c core.fsmonitor=false`
  before the subcommand (so a repo's config can't start a program). Arguments are a list, never a
  shell string. This mirrors `builder/save/src/git.rs`'s `Env::User`.
- **Hit texts and the summary are byte-identical to the Python** (parity tests compare stdout), even
  `1 hits`. They name a file and a line and the kind, never the matched text.

## The crate

`builder/ingest/Cargo.toml`: workspace edition, version, license and publish, like `builder/save`.
Dependencies: `persona-builder = { path = "../core" }`, `regex = "=1.13.1"`, `serde_json = { version = "1",
features = ["preserve_order"] }`, `libc = "=0.2.191"`. Add `"builder/ingest"` to the root
`Cargo.toml`'s members. Errors are `persona_builder::error::{Error, Result}`.

### `src/filters.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind { Personal, Secret }
impl Kind { pub fn as_str(self) -> &'static str }        // "personal" | "secret"

pub struct Filters { /* excluded, personal, secret: Regex; pasted: Vec<String> */ }
impl Filters {
    /// compile_pattern for each of the three keys, in the order excluded_folders, personal, secret.
    pub fn compile(filters: &persona_builder::member::Filters) -> Result<Self>;
    pub fn excluded(&self, folder_name: &str) -> bool;   // m.excluded.search(name)
    pub fn pasted_prefixes(&self) -> &[String];
    /// privacy(): secret is checked first and wins.
    pub fn kind(&self, text: &str) -> Option<Kind>;
    /// privacy_lines(): every match of personal, then of secret, keyed by its 1-based line
    /// (the number of '\n' before the match start, plus one); a later write to the same line wins,
    /// so secret wins. Sorted by line.
    pub fn lines(&self, text: &str) -> Vec<(usize, Kind)>;
}
```

### `src/privacy.rs`

The record check that PB2.2 to PB2.4 call, ported from `searchable`, `filtered` and `privacy`
(`scripts/crew_decisions.py`, lines 296–348).

```rust
/// filtered(): the text the filters read, built exactly as the Python joins it.
pub fn filtered_text(record: &serde_json::Value) -> Result<String>;
pub fn privacy(filters: &Filters, record: &serde_json::Value) -> Result<Option<Kind>>;
```

`searchable`: a `"message"` record gives its `text`; any other kind gives
`" ".join((question, " ".join(option labels), " ".join(picks), own_words, notes))`. `filtered` then
joins `searchable`, `context` (default `""`) and `project` (default `""`) with single spaces, and for
a `"question"` appends `" " + header + " " + " ".join(option descriptions)`. A key the Python indexes
directly (`kind`, `text`, `question`, `options` and each option's `label`/`description`, `picks`,
`own_words`, `notes`, `header`) that is missing or not the right JSON type gives
`Error::Invalid("a record lacks <key>")`.

### `src/check.rs`

```rust
pub struct Report { pub hits: Vec<String>, pub filters_differ: bool }

/// privacy_files(): tracked files of a clone, else a walk without ignored paths and OS junk.
pub fn tracked_or_walked(folder: &Path) -> Result<Vec<PathBuf>>;   // relative paths, sorted

/// The check over a given file list (relative to folder). Used by the CLI and by save push.
pub fn scan(folder: &Path, files: &[PathBuf], filters: &Filters) -> Result<Report>;

/// cmd_privacy_check() without the printing.
pub fn privacy_check(folder: &Path) -> Result<Report>;

/// Everything the CLI prints on stdout: each hit on its own line, then ADVICE on its own line when
/// filters_differ, then "<n> hits" or "clean", each line ending in '\n'.
pub fn render(report: &Report) -> String;
pub const ADVICE: &str = "move member-specific filter patterns to the gitignored private.json and copy filters from crew/_template/member.json";
```

`tracked_or_walked` (Python lines 211–226): canonicalize `folder`. Run
`git -C <folder> ls-files -z -- .`. Exit 0: the NUL-separated names, as paths relative to `folder`.
Otherwise walk every entry under `folder` recursively (never following links), pipe the absolute
paths NUL-separated into `git -C <folder> check-ignore --no-index -z --stdin`, and drop each path it
prints (exit 0 or 1; any other exit means nothing is dropped). Also drop any path with a component
equal to `.git`, `.DS_Store`, `__MACOSX`, `Thumbs.db` or `desktop.ini`, or starting with `._`.
In both branches keep only regular files that are not links (`symlink_metadata`), sorted.
A name that is not UTF-8 is kept as an `OsStr` path (Unix `OsStrExt`), never lossy-converted.

`scan(folder, files, filters)` (Python lines 229–270), in this order:
1. For each file in sorted order, except the top-level `member.json` and `private.json`: read its bytes
   with `O_NOFOLLOW`. Not UTF-8: hit `"<name> (not text)"`. Else one hit per entry of
   `filters.lines(text)`: `"<name>:<line> (<kind>)"`. `<name>` is the relative path with `/`.
2. If `member.json` is in `files`: parse it raw (not through `read_member`; `preserve_order`), drop its
   `filters` key, and walk every string as `string_paths` does (lines 188–199): an object key is
   reported at its parent's path (`(top level)` at the top) and flagged as a key; a value at
   `parent.key`, or just `key` at the top; a list item at `path[i]`. For each string, take only the
   first entry of `filters.lines(string)` and add `"member.json:<path> (<kind>)"`, or
   `"member.json:<path> (a key, <kind>)"` for a key. Then compare the raw `filters` object with the
   template's, key by key over the sorted union of both key sets, as JSON values; each that differs
   adds `"member.json:filters.<key> (differs from crew/_template)"` and sets `filters_differ`.
   If `member.json` is not in `files`, neither step runs.
3. Return the hits in that order.

`privacy_check(folder)`: canonicalize `folder`; list `tracked_or_walked(folder)`; if
`folder/member.json` is not a regular file, `Error::Invalid("<folder>/member.json: no member.json to check")`;
parse it raw, and if it is not a JSON object, `Error::Invalid("<folder>/member.json: member.json must be a JSON object")`;
then `read_member`, `Filters::compile(&member.filters)`, and `scan`.

## The CLI (`builder/cli`)

Add `persona-builder-ingest = { path = "../ingest" }`.

- **`persona-builder privacy-check <folder>`** (a new top-level subcommand, doc comment
  `Check an assistant's folder for anything private before it is shared.`): print `render(&report)`
  as it is. Exit 1 when there are hits, else 0. Errors print as the CLI's do today (`persona-builder: <message>`, exit 1).
- **`save push`**: a `FolderPrivacy` adapter in `main.rs` implements
  `persona_builder_save::flow::PrivacyCheck`. Its `hits(folder)`:
  `persona_builder_save::git::folder_files(folder)?` (below), then
  `read_member(&folder.join("member.json"))?`, `Filters::compile(&member.filters)?`, and
  `scan(folder, &files, &filters)?.hits`. Pass `&FolderPrivacy` to `save_now` instead of `&NotBuiltYet`.
- **In `builder/save/src/git.rs`**, add
  `pub fn folder_files(folder: &Path) -> Result<Vec<PathBuf>>`: the existing private `list(folder,
  folder, …)`, sorted, as relative `PathBuf`s. Make `commit_folder` use it, so the save and its
  privacy check read one list. This is the save's own set: every regular file at any depth except
  `private.json` and `.DS_Store`, links refused. `.gitignore` is not read, so an untracked file is
  checked too.
- **Delete** `NotBuiltYet` (`builder/save/src/flow.rs`), `text::PRIVACY_NOT_BUILT`, and the two lines
  in `builder/save/tests/flow.rs` (around 369–372) that assert it. Nothing else uses them (checked with
  `grep -rn 'NotBuiltYet\|PRIVACY_NOT_BUILT' builder/`).

## Tests (red first, each on its assertion)

Fixtures are invented only: no real person's name, handle or address (`AGENTS.md`). Use the names
already in the repo's fixtures (`Wren`, `Rowan Vale`, `example.org` addresses). Put a
`tests/common/mod.rs` in `builder/ingest` with `Scratch`, `repo()` and `parity_tools_available()` as in
`builder/core/tests/common/mod.rs`: copy them, and keep its rule that a missing `python3` or `git`
skips the parity tests unless `PERSONA_BUILDER_REQUIRE_PARITY=1`, when it fails.

1. **`builder/ingest/tests/filters.rs`**: `compile` refuses `(unclosed`, `(?=x)` and `(a)\1` with the
   exact message above, for the right key. `kind` gives `Secret` when both match. `lines` counts lines
   from 1, gives one entry per line, and secret wins on a shared line. Case-insensitive (`MY DOCTOR`
   matches `\bmy doctor\b`).
2. **`builder/ingest/tests/privacy_parity.rs`**: run the real Python as the oracle, through
   `python3 -I -c` loading `scripts/crew_decisions.py` with `importlib`, as
   `builder/core/tests/python_parity.rs` does. For about 12 invented records (messages and questions,
   with and without `context`/`project`, hitting personal, secret, both, none, and non-ASCII text),
   `filtered_text` equals Python's `filtered(record)`, and `privacy` equals `privacy(m, record)`,
   using a member built from the template's filters (`load_member`). Plus: a record lacking `kind` is
   `Error::Invalid("a record lacks kind")`.
3. **`builder/ingest/tests/check_parity.rs`**: build a crew folder from the template with
   `persona_builder::crew::new_member` (as `builder/cli/tests/save.rs` does), then compare
   `render(&privacy_check(folder)?)` with the stdout of
   `python3 -I scripts/crew_decisions.py --member <folder>/member.json privacy-check --dir <folder>`
   (run with `HOME` set to the scratch folder), and the Python's exit code with 1 when there are hits
   and 0 when clean. They must be equal for each
   case of `tests/crew_decisions_test.py`'s `PrivacyCheck` class (lines 273–386): the clean template;
   a personal hit and a secret hit in two files; a codebook string that trips secret; a filter that
   differs from the template (with the advice line); a `private.json` pattern that applies while
   `private.json` is not scanned; hits at any depth in `.md`, `.html`, `.txt`, `.json`; a file that is
   not text; a key that trips a filter. Run each case twice: once in a plain folder (the walk) and once
   after `git init` and `git add -A` in it (tracked files), plus one case with a `.gitignore` that names a
   file with a hit, in each of the two folders (the engines must agree on whether it is reported). Also: a folder
   whose `member.json` is `[1, 2]`, and one with no `member.json`, give the two error texts above.
   Also: a path sort case, `a-b.md` and `a/b.md` both with a hit, in Python's order.
4. **`builder/cli/tests/privacy_check.rs`**: the binary prints `clean` and exits 0 on the template
   folder; prints the hit lines, then `1 hits`, and exits 1 on a folder with one hit; prints the advice
   line before the summary when filters differ; and prints `persona-builder: <folder>/member.json: no
   member.json to check` on stderr, exit 1, for an empty folder. No matched text in stdout or stderr.
5. **`builder/cli/tests/save.rs`**: replace `github_record_refuses_privacy_before_sign_in_or_network`
   with two tests. (a) A clean template folder with a GitHub `save.json`: `save push` fails with
   `text::NOT_SET_UP` (the privacy check ran and passed, and nothing reached the network, since no
   client id is built in). (b) The same, after appending `We moved it because my doctor called.` to
   the folder's `decisions.md`: it fails with `text::privacy_hits("decisions.md:<line> (personal)")`.
   (c) The same as (b), with the folder inside a `git init` repo where `decisions.md` is untracked and
   listed in a `.gitignore`: still refused with the same text (the save's file set, not git's).
6. **`builder/save/tests/git.rs`**: `folder_files` lists nested files sorted by component, skips
   `private.json` and `.DS_Store` at any depth, and refuses a link with `text::links_in_folder`.

## Done when

All of the above went red on their assertions first and are green. `cargo fmt --check`,
`cargo clippy --all-targets --locked -- -D warnings` and `bash scripts/ci-local.sh` pass (the lock file
gains `regex`, `regex-automata`, `regex-syntax` and `aho-corasick`; run `cargo build` once so
`--locked` passes). If a test needs a socket and your sandbox refuses it, say which and move on.
Report: each file you added or changed, the red you saw for each test file, any parity case where the
two engines disagreed (stop and report it, never bend a test to pass), and anything in this brief you
could not do as written.

## Review fixes (Claude's review, 2026-10-24)

1. **A plain crate root.** Add `builder/ingest/src/lib.rs` holding only `pub mod check;`,
   `pub mod filters;` and `pub mod privacy;`. Remove the `[lib] path` from
   `builder/ingest/Cargo.toml`, and the root-module lines and `pub use crate as filters;` from
   `src/filters.rs`. Public paths stay `persona_builder_ingest::{filters, privacy, check}`.
2. **`Filters::lines` in one pass per pattern.** It counts every `'\n'` from the start of the text for
   each match, so a large file with many matches is quadratic. Keep a cursor per pattern (the byte
   offset and line reached so far) and count only the newlines between the previous match start and
   this one; `find_iter` yields matches in increasing order. Same result, same tests; add one test
   that two matches on lines 1 and 3, then a third on line 3, still give `[(1, …), (3, …)]`.
3. **Two parity cases the mutation checks showed missing**, in `builder/ingest/tests/check_parity.rs`,
   each compared with the Python oracle as the other folder cases are:
   - a `member.json` codebook string holding two lines, `my doctor` then `robin@example.org` (a personal
     hit on line 1, a secret hit on line 2): only the first, `(personal)`, is reported for it;
   - the walk with a working ignore list: `git init` in the folder, then overwrite `.git/index` with
     the bytes `not an index` so `git ls-files` fails while `git check-ignore --no-index` still works,
     and a `.gitignore` naming `notes.txt`, which holds `my doctor again`. Neither engine reports
     `notes.txt`; assert that explicitly as well as the equality, and that `.gitignore` is scanned.
   Both must fail when their line is mutated (Claude checks this).
