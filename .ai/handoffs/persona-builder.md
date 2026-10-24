# Persona Builder: slice records

One record per slice, newest first. Paraphrase only; no real person.

## PB3.2: the Claude driver (2026-10-24, lane: V1)

- **What:** a new crate `builder/agent` (`persona_builder_agent`), the one road every model call
  takes. The agent list (one entry, `claude-code`); finding Claude Code (absolute `PATH` folders,
  then the native installer's and Homebrew's places, since an app opened from the Finder gets a
  short PATH); a check that it runs and is signed in (only `loggedIn` is read from
  `claude auth status --json`); one tool-less `claude -p` call (argv modelled on Flightdeck's
  `draft`: safe mode, no tools, no MCP servers, no session saved, a JSON schema, `--max-budget-usd`)
  with the person's records on stdin only, run in an empty 0700 folder of its own, killed with its
  whole process group at its time limit; a ledger line per call in `runs.jsonl` (tokens, the CLI's
  own cost estimate, the outcome, never any text); a daily cap in estimated dollars (default 50, 5
  per call), each call on the ledger at its whole budget before it starts; one build per assistant at a time
  (an `flock` on the state folder). `private.rs` gains `lock_state`, `private_folder` and
  `read_private_optional`. The CLI gains `persona-builder agents`, `claude check` and
  `spend <member-dir>`. PB3 was split into PB3.1 to PB3.4 at this pickup. Brief:
  `plans/briefs/PB3.2-claude-driver.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed again
  at dispatch).
- **Decisions made at pickup:** `--bare` is not used, because it never reads the person's own login
  (the headless docs); the work folder must be empty before every call, since without `--bare` a
  `-p` run loads the working folder's `.claude/settings.json` and `.mcp.json`; the session markers
  `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT` are removed from the child's environment and nothing is
  added; a call that is cut off or answers unreadably is recorded at its whole budget, so the cap
  never under-counts; a damaged ledger stops all spending; the cap's defaults (50 a day, 5 a call)
  are this slice's choice and can be changed by flag.
- **Verified:** 40 new tests, each red on its assertion first (30 in the crate, 7 in `ingest`, 3 in
  the CLI) against stub `claude` scripts: the exact argv, the prompt on stdin and nowhere in the
  argv, the work folder as the current folder, the environment, every reply shape (ok, error, exit
  1, signed out, no output, unreadable, over the output cap), the cut-off killing a grandchild, every
  refusal running nothing and writing no line, the cap (full, exact, yesterday, two calls racing), a
  damaged ledger, the lock. 20 mutations, each restored and checked with `cmp`: 18 failed a test at
  once; one pattern did not apply and one survived (a missing test, added), and both then failed.
  Restored files must be touched before the next build, or cargo reuses the mutated one.
  Live: `persona-builder claude check` against the real Claude Code 2.1.292 on Mimi said "isn't
  signed in" (its own `auth status` reads `loggedIn: false` there), and the exact call argv run by
  hand against the real CLI was accepted flag for flag and answered `is_error: true` with "Failed to
  authenticate…", the shape the driver reads as signed out; the work folder stayed empty.
- **Gates:** `/security-review` (a sub-agent): nothing exploitable; one medium (a build killed
  mid-call left that call's spend uncounted) and four lows (planted settings and an open assistant
  folder, the argv readable by other users, an API key in the environment, a group kill after a
  failed `waitid`). The Verifier (Fable): pass with no must-fix; three should-fixes (a released
  budget untested, the same mid-call crash, a prompt opening with `/` read as a command). All fixed
  in the brief's *After the gates*, each with a test; 11 more mutations, all failed a test. The delta
  Verifier pass: no must-fix; her two should-fixes (an invisible character before the `/`, and the
  time limit after a failed `waitid`) are in, the first with a test; the second has none (a failing
  `waitid` on a live child could not be produced).
- **Not verified:** a successful live call (no signed-in Claude Code CLI on Mimi); owed to an
  attended walk (PB8 covers it). `total_cost_usd` is the CLI's own estimate.
- **Known limits:** the ledger is rewritten whole on each call (like the Python `record`); the cap
  is per assistant, not per person.

## PB2.3: mine the claude.ai export, in Rust (2026-10-24, lane: V1)

- **What:** `persona_builder_ingest` gains `claudeai`, ported from `crew_decisions.py`'s
  `export_files` to `cmd_claudeai_mine`: reading the export (a zip, a folder or a bare
  `conversations.json`, in memory, never unpacked), `inspect` (field names, types and counts),
  `titles` (the review file of every chat, keeping marks already made, with opening words or
  `[filtered]` for untitled chats), `mark` and `mine`, giving the same printed lines, review TSV
  and JSONL bytes as the Python. The CLI gains `persona-builder claudeai inspect | titles | mark |
  mine`; the review (`claudeai-review.tsv`) and the corpus (`decisions-claudeai.jsonl`, the names
  `install.sh` uses) are plain names in `~/.claude/state/<slug>/`, so `write_private` and
  `read_private` now take `.tsv` names too. `text_blocks` and `own_message` moved out of the Claude
  Code miner so both share them. Brief: `plans/briefs/PB2.3-claudeai-export.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit (until 2026-10-30) when the
  brief was dispatched, so the brief was implemented at the same bar and then corrected to match
  what testing proved.
- **Decisions made at pickup:** ten accepted differences, each with a test that also shows what the
  Python does (the brief lists them): a stricter timestamp grammar (16 forms only Python reads,
  measured on 3.12.14); odd types refused; a uuid holding a tab or line break refused (Python
  writes a broken review file); links, pipes, repeated zip names, other compression methods,
  encryption, a zip over 2 GB or over 100,000 entries refused; UTF-8 only; `mark` and `mine` need
  an existing review; plain file names; `--title` read by the `regex` crate; `--holdout` checked as
  `mine` checks it; `inspect` shows only the message senders' values, control characters escaped.
  The `zip` crate keeps one entry per name, so `validate_header_consistency` runs before any read
  (zip 9.0.5 has no public duplicate check).
- **Dependency intake:** `zip` =9.0.5 (zip-rs/zip2, MIT; crate first published 2014; 9.0.5 is a
  week old, chosen over 9.1.0, published hours before pickup), default features off plus
  `deflate-flate2`, and `flate2` =1.1.10 with `rust_backend` (miniz_oxide, pure Rust). Why: nothing
  in the lock read zip archives. The lock adds adler2, bitflags, cfg_aliases, crc32fast,
  miniz_oxide, nix, simd-adler32 and typed-path. Build scripts: crc32fast's (runs
  `rustc --version`) and nix's (`cfg_aliases`), both harmless. RustSec has four `zip` advisories,
  all about extraction (fixed in 9.0.0; nothing here extracts). `cargo audit` is not installed, so
  no advisory scan of the new tree was run.
- **Verified:** parity runs the real Python on 12 invented cases (zip, folder and bare file; a
  nested zip; untitled chats; every shared timestamp form; text and content blocks; the filters
  for both test members; a hand-edited review with odd marks, extra columns and CRLF; Unicode;
  odd shapes for `inspect`), through `inspect`, `titles`, a run of `mark` selectors and 7 `mine`
  split settings, comparing stdout, review bytes and JSONL bytes. The timestamp grammar is checked
  against the Python's own `utc` on 59 strings. 24 mutations, each restored and checked with `cmp`,
  all failed a test; the two review-fix tests went red on the code before the fix.
  `bash scripts/ci-local.sh` passed (151 Python tests, 365 Rust tests; after the review fixes 151 and 367).
- **Known limits:** the export is held in memory (up to 2 GB of JSON plus its parsed tree). The
  100:1 ratio can be beaten by a central directory that overstates the compressed size; the 2 GB
  read cap still holds. The 100,000-entry cap bounds the walk, not memory: the `zip` crate parses
  the whole central directory first (about 350 bytes per entry, measured by the Verifier), so a
  crafted zip under 2 GB listing tens of millions of entries can use many gigabytes before it is
  refused. `inspect` prints key names, which a crafted export could fill with words (the Python does
  too); only control characters are escaped, so Unicode format characters print as they are.
- **Gates:** `/security-review` (a sub-agent): nothing exploitable; two low findings, both fixed
  (`ec58dac`): `sender` values at any depth, and no cap on the zip itself. The Verifier (Fable):
  passed `25ff91e` with no must-fix, after about 70 two-engine round trips and a 538-string
  timestamp fuzz (every mismatch was a form only Python reads); her three should-fixes were gaps in
  the brief's list of differences, now listed. A delta pass on `ec58dac` passed with no must-fix; her two should-fixes are in
  `1301d04`: the sender check is by place, since a key named `chat_messages[].sender` forged the
  path, and the entry cap's limit is stated above.
- **Not verified:** a real claude.ai export, and a multi-gigabyte one (the attended walk in PB8
  covers a real export).
- **Skills Review (PB2 complete: PB2.1 to PB2.4 landed):**
  - ✅ tdd-skill: each test file red first; 24 mutations, each caught.
  - ✅ security-and-hardening: zip caps, links and pipes refused, errors that name places only;
    both review findings fixed.
  - ✅ source-driven-development: the `zip` crate's source, RustSec and Python 3.12's own behaviour
    were read or measured before each decision. The first draft assumed `zip` refuses repeated
    names; the source showed it merges them.
  - ⚠️ plain-language: the messages Persona Builder writes are plain, but the `zip` crate's own
    error text still reaches the person inside "can't be read (…)".
  - ⚠️ codex division: Codex was at a hard limit, so Claude built PB2.3 directly, as the rule allows.
  - ✅ project-config: `scripts/ci-local.sh` with parity required, run with at most 2 test workers
    while the machine was loaded.

## PB2.4: merge and split the corpus, in Rust (2026-10-24, lane: V1)

- **What:** `persona_builder_ingest` gains `corpus` (`merge`, `split`, `read_records`, `discover`),
  ported from `crew_decisions.py`'s `cmd_merge`, `cmd_split` and `read_jsonl`: the same records in the
  same order, the same JSONL bytes, the same printed lines. `private.rs` gains `read_private`, which
  reads a corpus file through the same `O_NOFOLLOW` folder walk as `write_private` and creates
  nothing. The CLI gains `persona-builder merge <member_dir>` (named inputs, or the state folder's
  `decisions-*.jsonl` sorted into history, interview and rounds as `install.sh` does) and
  `persona-builder split <member_dir>`, both with `--dry-run`. Every file is a plain name in
  `~/.claude/state/<slug>/`. The Python's `read_jsonl` now splits at `\n` only: with `splitlines` a
  record holding U+2028 or U+0085 (which `json.dumps` writes raw) failed to read. Brief:
  `plans/briefs/PB2.4-merge-and-split.md`.
- **Who wrote it:** Codex, from the brief and one fix round; Claude wrote the brief and this record
  and reviewed every line.
- **Decisions made at pickup:** the Python's order of checks is kept, so an error lands where the
  Python's would. Python reads in text mode, which turns `\r\n` and a lone `\r` into `\n`; Rust does
  the same (Codex found this one and stopped, as the brief asked). Three differences are deliberate,
  each with its own test: only whole numbers are copied (Python and serde_json write floats
  differently; `-0` is refused); JSON serde_json refuses (`NaN`, a lone surrogate, deep nesting) is
  an error; an `id`, `timestamp` or `kind` that is not a string is refused. On any error Rust writes
  nothing (the Python's `split` can leave a questions file behind).
- **Dependency intake:** none.
- **Verified:** every new test file went red on its assertion first. Parity runs the real Python on
  11 invented cases (two machines, the interview, rounds, filters tightened since mining, ties, Unicode
  line separators, odd key orders and duplicate keys, CRLF and bare CR) with both test members and 6
  split settings, then runs the Python `split` on each result, comparing every file's bytes and the
  printed lines. Four mutations, each restored and checked with `cmp`, each failing a test: no bare-CR
  translation, `read_private` following a file link, interview and round records moving the cutoff,
  no duplicate check. `bash scripts/ci-local.sh` passed (151 Python tests, 330 Rust tests).
- **Known limits:** `split` writes its two files one after the other, so a failure between them can
  leave a new questions file beside an old answers file. Discovery skips file names that aren't
  UTF-8. Reading follows the same walk as writing, so a link anywhere under the home folder is
  refused.
- **Gates:** `/security-review` (a sub-agent): no findings; its two notes (fields the filters don't
  read, and `split` not re-filtering) match the Python, and the files stay in the state folder.
  The Verifier (Fable): passed on `ea7ac86` with no must-fix or should-fix findings. Besides
  checking each claim, she ran 124 checks of her own through both engines (ties, holdout rounding,
  escapes, whole-number extremes, duplicates across modes, mixed line ends), all identical. Her
  notes: Rust reports a bad timestamp before a later file's error (both exit 1, nothing written);
  discovered names pass the name rule by construction; discovery sorts by bytes where `install.sh`
  uses the shell's order (it only decides which copy of a duplicate wins).
- **Not verified:** a merge of real corpora (the attended walk in PB8 covers it).

## PB2.2: mine Claude Code transcripts, in Rust (2026-10-24, lane: V1)

- **What:** `persona_builder_ingest` gains `mine` and `folders`, ported from `crew_decisions.py`'s
  `cmd_mine` and `cmd_folders`: the same records in the same order, the same JSONL bytes
  (`json.dumps(ensure_ascii=False)`), the same printed lines. The corpus goes only into
  `~/.claude/state/<slug>/` (default `decisions-claude-code.jsonl`), 0600 in 0700 folders, through
  `O_NOFOLLOW` directory descriptors and an atomic `renameat` (`src/private.rs`, the crate's only
  `unsafe`). The CLI gains `persona-builder mine <member_dir>` (with `--dry-run`, which writes
  nothing) and `persona-builder folders <member_dir>`. `forget`'s state-folder check moved to
  `member::private_state_dir`, shared with `mine`. The Python's `folders` now breaks project-name ties
  by folder name. Brief: `plans/briefs/PB2.2-mine-claude-code.md`.
- **Who wrote it:** Codex, from the brief and two fix rounds; Claude wrote the brief and this record
  and reviewed every line. Codex also made the one-line fix to PB6's flaky `ureq_wire` test (an
  accepted socket inherits the listener's non-blocking flag on macOS, so a loaded full run sometimes
  failed with `WouldBlock`).
- **Decisions made at pickup:** CPython's own rules are ported where Rust's differ: truthiness,
  `str.isspace` (it also strips U+001C to U+001F), `splitlines` (it also splits at U+2028, U+0085
  and the rest), `errors="ignore"` decoding (drop, never U+FFFD), half-to-even rounding. Where the
  Python raises on a malformed record, Rust stops with `<file>:<line>: a transcript record has an
  unexpected shape (<key>)`, never quoting record text, and writes nothing. Six differences are
  deliberate and each has its own test: links in the history refused; lines `serde_json` can't parse
  (`NaN`, a lone surrogate, deeper than 128) skipped; odd types the Python only tolerates by accident
  refused; `--holdout` limited to 0 to 1; names that aren't UTF-8 refused; a `*.jsonl` entry that
  isn't a file refused.
- **Dependency intake:** `sha1_smol` 1.0.1 (BSD-3-Clause, no dependencies, no build script, no
  network, files or processes), for the record ids the Python makes with `hashlib.sha1`; an id only,
  never a security control. No `cargo audit` scan: it is not installed.
- **Verified:** every test file went red on its assertion first. Parity runs the real Python as the
  oracle on 17 fixture cases built with `tests/crew_fixtures.py` (new `tests/crew_mine_fixtures.py`)
  and 7 split settings, comparing the JSONL bytes and the printed lines; the helpers are checked
  against CPython over every code point (`isspace`), invalid UTF-8 sequences, every line separator and
  every code point below U+0100 (JSON escaping). One disagreement surfaced (an empty `mark_word` kept
  an empty mark) and was fixed. Five mutations, each restored and checked with `cmp`, each failing a
  test: `isspace` without U+001C to U+001F, `splitlines` without U+2028, ordinary rounding, the folder
  walk without `O_NOFOLLOW`, empty marks kept. `bash scripts/ci-local.sh` passed (150 Python tests,
  312 Rust tests).
- **Known limits:** an existing `~/.claude/state` or slug folder keeps its mode (as in the Python);
  the file is still 0600. A project folder swapped for a link between listing and reading is not
  caught (same user only). The summary prints timestamp text from the transcripts.
- **Gates:** `/security-review` (a sub-agent): no findings; its three notes below the bar are in
  Known limits. The Verifier (Fable): her first pass on `1d35cf0` ran out of run time before reporting;
  her second pass, on `a306628`, passed with no must-fix or should-fix findings. She proved the `unsafe`
  file code, the dry run writing nothing, the state folder check, error text, live parity and the
  Python tie-break test, and found no real person in the diff. Her four notes: the parity tests skip
  without Python unless `PERSONA_BUILDER_REQUIRE_PARITY=1` (`ci-local.sh` sets it); an existing
  folder keeps its mode (Known limits); no folder fsync after the rename (durability only); a slug
  may hold a line break, and still stays inside the state folder.
- **Not verified:** a run on a real Claude Code history (the attended walk in PB8 covers it).

## PB2.1: the privacy filters and the privacy check, in Rust (2026-10-24, lane: V1)

- **What:** a new crate, `builder/ingest` (`persona_builder_ingest`), since core keeps to the
  filesystem and this one runs `git ls-files` and `git check-ignore`. It compiles a member's filters
  (`filters`), checks a mined record (`privacy`, for PB2.2 to PB2.4), and checks a folder (`check`),
  ported from `crew_decisions.py`. The CLI gains `persona-builder privacy-check <folder>`, and
  `save push` now runs the real check in place of the "can't run the privacy check yet" stop.
  The save and its check read one file list (`persona_builder_save::git::folder_files`): every file
  the save commits, at any depth, untracked and git-ignored files included. Brief:
  `plans/briefs/PB2.1-privacy-check.md`. PB2 was split into PB2.1 to PB2.4 at pickup.
- **Who wrote it:** Codex, from the brief and three review rounds; Claude wrote the brief and the
  docs, and reviewed every line.
- **Decisions made at pickup:** the `regex` crate, case-insensitive, as the engine. It has no
  look-around and no backreferences (its own docs), so a filter using one is refused in plain words,
  never rewritten; the template's filters use neither. The template's filters are compiled into the
  binary. Hit lines and the summary match the Python byte for byte, `1 hits` included.
- **Dependency intake:** `regex` 1.13.1 (MIT or Apache-2.0, the version Flightdeck pins) with
  `regex-automata` 0.4.18, `regex-syntax` 0.8.11 (both MIT or Apache-2.0) and `aho-corasick` 1.1.5
  (Unlicense or MIT). None has a build script; none touches the network, files or processes. `libc`
  0.2.191 was already in the lock. No `cargo audit` scan: it is not installed.
- **Verified:** every test file went red on its assertion first. Parity runs the real Python as the
  oracle: 12 invented records through `filtered` and `privacy`, and every case of the Python
  `PrivacyCheck` tests, each in a plain folder (the walk) and a git folder (tracked files), plus a
  path-order case, a two-line codebook string, and the walk's ignore list (a repo whose index is
  corrupt, so `ls-files` fails while `check-ignore --no-index` works). No case disagreed. Twelve
  mutations, each restored and checked with `cmp`, each failing a named test: secret winning a
  shared line, secret checked first, case-insensitive, `private.json` skipped, `O_NOFOLLOW`, the `._`
  junk rule, the template comparison, the first hit only for a `member.json` string, ignored paths
  dropped, a missing record key, `save push` using the save's file list (an untracked, ignored file
  with a hit is still refused), and `folder_files` skipping `private.json`. Two missed at first and
  got the last two parity cases. `bash scripts/ci-local.sh` passed.
- **Known limits:** the two engines differ on some Unicode text, proven by the Verifier: Rust's `\w`
  and `\b` count combining marks as word characters and Python's do not, so a number or word followed
  by a combining accent in decomposed text (`1234567` then U+0301, `hoa` then U+0301) is a hit in
  Python and not in Rust; Python's `\s` matches U+001C to U+001F and Rust's does not; an email whose
  first letter carries a combining accent is a hit in Rust only. Not fixed in this slice. The save
  lists the folder twice, once for the check and once for the commit, and `hash-object` reads the
  files again, so a file written between them by the same user is pushed unchecked. File names are
  not matched against the filters. A `member.json` key that trips a filter is named by its parent,
  but a child hit's path still holds that key (as in the Python). Python's `$` also matches before a
  final newline; Rust's does not (no template pattern uses `$`). The CLI's check, like the Python,
  lists only tracked files in a clone, so an untracked file there is not checked; `save push` does
  not have that gap. A file swapped for a link inside a listed folder between listing and reading is
  refused at the last step only (`O_NOFOLLOW` covers the file, not its parent folders; same user).
  `core.fsmonitor` is turned off for the two git calls, but that is not tested.
- **Gates:** `/security-review` (a sub-agent): no high-confidence findings; its three low notes are
  in Known limits. The Verifier (Fable) passed `2a00fde`; her one should-fix, to record the proven
  engine differences, is the Known limits text above.
- **Not verified:** a real `save push` past the check (it needs V-Q1's client ID: PB6.1).

## PB5: turn it on and off, the claude.ai Project pack, forget (2026-10-24, lane: V2)

- **What:** `persona-builder on <member dir>` copies the assistant's `SKILL.md` to
  `<claude home>/skills/<slug>/`, writes `<claude home>/persona-builder/<slug>/RULES.md`, and adds
  one `@` import of it to `CLAUDE.md` inside `<!-- persona-builder:begin/end <slug> -->` markers.
  A record (`persona-builder/installed.json`) lists every folder and file added with the exact text
  written, so `off` removes exactly that, leaves anything the user changed (and says so), and gives
  back `CLAUDE.md` byte for byte. One assistant on per Claude Code folder; a failed `on` rolls back.
  `status` reports the line, each file and whether the member changed since. `pack <member dir>
  --out <dir>` writes `instructions.md` plus one `decisions-<kind>.md` per kind of question in
  `decisions.md` (deterministic). `forget <member dir> [--yes]` deletes only
  `<home>/.claude/state/<slug>`, after a preview. Brief: `plans/briefs/PB5-turn-on-off.md`.
- **Design calls (within the plan, not asked):** "confirmed area" means a kind of question that has
  a section in the committed `decisions.md` (PB4's review edits that same file, so a dropped area
  drops out of the pack). The `CLAUDE.md` import points at a generated `RULES.md` rather than a
  member file, so moving the repo does not break it. `--claude-home` defaults to
  `$CLAUDE_CONFIG_DIR`, else `~/.claude`. Sources (Claude Code memory and skills docs, the claude.ai
  Projects help page) are cited in the brief.
- **Who wrote it:** Codex, from the brief, in three parts plus four fix rounds; Claude reviewed it.
  No new dependency.
- **Rebased onto PB6:** `builder/cli/src/main.rs` was merged by hand (PB5's commands became one
  function each, as PB6's are); a Verifier pass on the resolution found every PB5 line and every
  PB6 line unchanged. `ci-local.sh` green on the rebased head.
- **Verified:** every test went red on its assertion first; mutations on the line break before an
  appended block, the changed-file comparison, the record's containment check, the dropped `Where`
  section, blank areas and `forget`'s path comparison each turned a named test red.
  `bash scripts/ci-local.sh` passed with all Rust steps. Live, with the built binary in a scratch
  home: `on`, a second assistant refused, `status`, `off` (CLAUDE.md byte-identical, nothing left),
  `pack`, `forget` preview, delete, and a second run.
- **Gates:** the security review found a name holding a line break and `@~/…` could put any file's
  import into every session through `RULES.md` (high), and that a shared `SKILL.md` could carry
  hooks or shell lines (medium). Both are fixed: names are one plain line without `@ < > { } [ ]
  ( ) # \` or backticks, and a skill must be named for its slug and ask for no hooks,
  `allowed-tools` or `` !` `` lines. The Verifier (Fable) passed the rest and found a symlinked
  `skills/` folder was refused (fixed), plus nits: a refresh's message after a user edit (fixed), a
  record listing `CLAUDE.md` (refused now), and a test named for rollback that tested an early
  refusal (renamed). A delta Verifier pass confirmed those six fixes and found three more
  (brief items 7 to 9), fixed in `a6006a5`: a `skills` link that points back into the Claude Code
  folder is refused, front-matter keys are checked against an allowlist, and a refresh names why a
  file was left. A second security review and Verifier on that commit both found that a lone
  carriage return hid a second key from the allowlist (YAML breaks a line there, Rust's line split
  does not; Claude Code parses front matter with Bun's YAML parser, read from its binary). Fixed in
  `ff82b8f` (items 10 to 12): a `SKILL.md` holding a lone carriage return, a Unicode line break or
  another control character is refused, so is a tab-indented front-matter line, and the overlap
  refusal has its own message. A third Verifier pass on `ff82b8f`: PASS, no findings in the diff.
  Every fix in both rounds was red first, and a mutation of each turned its test red.
- **Known limits:** `on` with `skills` linked back into the Claude Code folder creates
  `skills/<slug>/` through the link before it is refused, then removes it again (the disk ends as
  it began); the CLI reads `HOME` two ways since PB6 landed beside it (an empty `HOME` is refused
  by `on`, `off`, `status` and `forget` but not by `save`), for a later cleanup; `off` refuses rather than reports when the user replaced a recorded folder with
  a symlink; a dangling-symlink `CLAUDE.md` and `pack --out` under a missing folder give the
  system's own error. The installed skill still calls the Python `similar` and `record` commands;
  a later slice moves it to `persona-builder`.
- **Not verified:** that a real Claude Code session loads the import and the skill (owed to the
  first attended run, PB8), and a real claude.ai Project built from the pack.

## PB6: saving, to an existing repo, a private GitHub repo, or this Mac only (2026-10-24, lane: V1)

- **What:** a new crate, `builder/save` (`persona_builder_save`), because the core library keeps
  to the filesystem. It holds every sentence a person sees while saving (`text`), a `Secret` type
  that hides a token from `Debug` and zeroes it on drop, GitHub's device-flow sign-in with the
  granted-scope check, creating a repo private and reading it back as private, GitHub's address
  rules, the token store in the macOS Keychain, git plumbing that commits an assistant's folder
  without touching anyone's work tree, the saved choice in `<state>/save.json`, and the flow
  (`detect`, `options`, `choose_*`, `save_now`). The CLI gains `persona-builder save options |
  sign-in | sign-out | choose (local | here | github) | push`. Brief: `plans/briefs/PB6-saving.md`.
- **Who wrote it:** Codex, in separate test and implementation passes for each half; Claude wrote
  the brief, the person-facing texts and the docs, and reviewed every test and line.
- **Decisions made at pickup:** the network, git and Keychain code is a crate of its own, so core's
  "filesystem only" rule stays true. Sign-in is an OAuth app's device flow with the `repo` scope,
  the only scope that can make a private repo; the client ID is unset until V-Q1 is answered, and
  sign-in says it is not set up. A save to a person's own repo goes to a `persona/<slug>` branch
  of its own (the guided build's step 14 also saves to a branch, `crew/<slug>`), built from a
  scratch index, so their index and work tree never change; if that branch is checked out
  anywhere, the save is refused rather than moving it. The GitHub backup is a bare repo in the state folder, pushed with the
  token as an environment header scoped to `https://github.com/`, the person's git config off.
  Every push address must read back private from GitHub's API before every push; any other host
  is refused, since nothing else can be confirmed. The privacy check is a seam: the CLI's `push`
  refuses with "can't run the privacy check yet" until PB2 plugs the Rust check in.
- **Dependency intake:** `ureq` 3.4.2 (rustls, no default features; MIT or Apache-2.0; the same
  pin Flightdeck uses), and `base64` 0.23.1, `libc` 0.2.191 and `zeroize` 1.9.1, which were
  already in ureq's tree. New in the lock: ureq's tree (rustls, ring, webpki-roots,
  rustls-pki-types, ureq-proto, http, httparse, percent-encoding, untrusted, subtle, log,
  getrandom, cc, utf8-zero). `ring` builds C and assembly through `cc` at build time. `cargo
  audit` is not installed, so no advisory scan was run.
- **Sources:** GitHub's docs on the device flow, OAuth scopes, the repos REST API and API
  versions, and actions/checkout's git auth header, all read 2026-10-24 and cited in the brief
  and the code.
- **Verified:** every test file went red before its code existed (part 1: 65 tests on stubs;
  part 2: 51 tests on stubs plus the review fixes' new files red on their assertions), then green:
  135 Rust tests in the two packages, and `bash scripts/ci-local.sh` passed. Mutation checks, each
  anchored, restored from a backup and confirmed with `cmp`, and each failing a named test, not a
  build: 26 in all (bearer only on API calls; both private fields; the exact host; the slow-down
  step; the exact `repo` scope; no redirects; the expiry stop; the device page's host; the
  Keychain alphabet; the read-back after create; the git config off for the backup repo, both
  variables; the credential helper off; the cleared environment; `private.json` skipped; links
  refused; the credential hidden in errors; `--no-gpg-sign`; the privacy check run; the remote
  check run; the backup's exact origin; every push URL on GitHub; a repo `detect` did not offer;
  the remote-name rule; the 0700 folder; a local choice sending nothing). Two missed at first; two
  tests were added, and both now fail when their line is removed. The live Keychain test (a throwaway
  `persona-builder.test` item) passed and left nothing behind. The CLI was driven by hand in a
  scratch home: the options with and without a repo around `~/.claude`, every refusal, and a
  `save.json` carrying `--upload-pack=…` as its remote refused before git ran. A build with an
  invented client ID reached github.com over rustls and turned GitHub's real `404 {"error":"Not
  Found"}` into the plain "sign-in didn't work" line.
- **Test harness, fixed on review:** on this Mac `python3` is a pyenv shim, which macOS will not
  run as a `#!` interpreter, so the git wrapper now resolves the real interpreter; APFS refuses
  non-UTF-8 file names, so that case is skipped where the filesystem refuses it; one test deleted
  the folder holding its own record. Codex stopped on that one rather than bend the code.
- **Gates:** `/security-review` (run as a sub-agent): no high-confidence findings; two low notes
  kept below. The Verifier (Fable) failed the first pass on one must-fix, that the token alphabet had
  no `_` and so would refuse real `gho_…` tokens. That was a misread: the alphabet's literal is
  `._~:+/=-`, whose second character is `_`. Realistic `gho_` fixtures now pin it. Her
  should-fixes, all fixed test-first:
  a save moved the person's `persona/<slug>` branch when it was checked out (now refused); a poll
  reply with status 400, as RFC 8628 sends, was treated as unexpected; a relative `state_dir` was
  accepted. Her second pass, on those fixes, passed with one should-fix, also fixed test-first: a
  400 reply carrying `access_token` was taken as the token; a token is now read only from a 200
  reply (one test, which fails when the status check is removed).
- **Known limits:** an existing state folder keeps its mode (only one Persona Builder creates gets
  0700), and `backup.git` takes the umask's modes inside it. The folder is committed whole except
  `private.json` and `.DS_Store`: `.gitignore` is not read, so PB2's privacy check must cover any
  other file kept there. Copies of the token outside `Secret` (form fields, the git header, the
  child's environment, visible to the same user while git runs) are not zeroed. The Keychain item
  can be read by any process of the same user through `/usr/bin/security`. A lost `backup.git` on
  a second Mac makes the next push a non-fast-forward, refused, with no recovery yet; a git failure
  after GitHub made the repo leaves the repo, so a retry under the same name is refused. A file swapped for a link between listing and hashing would be read through the link
  (same-user only). The person's own `pre-push` hook runs on a save to their repo, with Persona
  Builder's cleared environment. A non-GitHub remote cannot be used at all. The check that the
  `persona/<slug>` branch is not checked out runs before the branch moves, so a checkout in
  between still moves it (same-user only). A worktree whose folder was deleted but not pruned
  still counts as having the branch checked out, so the save is refused until `git worktree prune`
  runs; the message does not say so yet.
- **Not verified:** a real sign-in, repo creation and push (they need V-Q1's client ID and PB2's
  privacy check: PB6.1, card `51dkJ0YE`, an attended walk); Linux (the Keychain is macOS only).

## PB1: the member format in Rust, the neutral base, one field pack (2026-10-24, lane: V1)

- **What:** `builder/core` reads and writes an assistant's files the way the Python tooling does:
  `member.json` with `private.json` laid over (`member`), JSON written byte for byte as Python's
  `json.dumps(indent=2)` (`json`), a new `crew/<slug>/` folder filled from the template as
  `install.sh` fills it, without a kit, and removed again if filling it fails (`crew`), the dial in `autonomy.md` (`autonomy`) and field
  packs (`fields`). The CLI gains `persona-builder member show <member.json>`. The template's
  `IDENTITY.md` and `autonomy.md` are rewritten in plain words, keeping every dial key and its
  state; `crew/_fields/general.md` is the first field pack. Brief:
  `plans/briefs/PB1-member-format.md`.
- **Who wrote it:** Codex, from the brief; Claude reviewed it and wrote the user-facing text.
- **Dependency intake:** `serde` 1 (derive) and `serde_json` 1 (`preserve_order`), both MIT or
  Apache-2.0 and the standard Rust JSON pair. They add `serde_core`, `serde_derive`, `itoa`,
  `memchr`, `zmij` (serde_json's float formatter, same author), and `indexmap` with `hashbrown`,
  `equivalent` and `foldhash`. None of them touches the network, files or processes. No
  `cargo audit` scan was run because it is not installed.
- **Verified:** every test went red on its assertion first. The parity tests run the real Python:
  Rust's `read_member` matches `crew_decisions.py`'s byte for byte on three invented members and
  gives the same error on two invalid ones. `new_member` matches `install.sh --no-history` file for
  file and byte for byte on four invented cases, one of them non-ASCII. Five mutations each turned
  their test red: the ASCII escaping, the private-filter join, the placeholder order, the skipped
  `README.md` and the cleanup on failure. `bash scripts/ci-local.sh` passed with all Rust steps; it
  sets `PERSONA_BUILDER_REQUIRE_PARITY=1`, so a missing `python3`, `git` or `bash` fails the run
  instead of skipping the parity tests.
- **Gates:** `/security-review` found three low issues; the two cheap ones are fixed (the target
  folder is created with `create_dir`, and entry types come from `file_type()`). The Verifier
  (Fable) found that parity holds for configs in the template's shape but not beyond it. Fixed: a
  nested `member.json` is now copied unfilled, as `install.sh` does, and a non-string filter in
  `member.json` is refused instead of being replaced. Two `IDENTITY.md` sentences overclaimed: one
  said a change is kept only once approved (that step comes in PB4), and one promised no backup.
  Both now say only what is true today.
- **A miss caught by the gate:** the first `autonomy.md` text said "passwords", which the
  template's own secret filter flags, so the privacy-check tests failed. It now says "logins".
- **Known limits (documented in the code):** the writer puts keys in the struct's order with
  unknown keys last, drops `null` optional fields and empty codebook lists, and reads numbers as
  `f64` or `i64`. Python keeps all of these as they were. `slugify` matches `install.sh` under
  `LC_ALL=C`; in a UTF-8 locale `tr` may lowercase some non-ASCII letters differently. Rust keeps
  CRLF line endings and does not copy a file's executable bit, while `install.sh` normalizes the
  first and keeps the second. The template has none of these cases.
- **Not verified:** a kit (`install.sh --kit`), which `new_member` does not cover yet.

## PB0: the Rust workspace and its checks (2026-10-23, lane: V2)

- **What:** a virtual Cargo workspace at the root (edition 2024, stable, no toolchain file) with
  `builder/core` (package `persona-builder`, library `persona_builder`, one `VERSION` constant) and
  `builder/cli` (binary `persona-builder`, clap derive, `--version` and `--help` only).
  `scripts/ci-local.sh` runs `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D
  warnings` and `cargo test --locked` after the existing steps, and prints a skip line when `cargo`
  is missing. `.ai/PROJECT.md`, `README.md` and `CONTRIBUTING.md` name the new folders and the Rust
  requirement. `target/` was already ignored. Brief: `plans/briefs/PB0-rust-workspace.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit (until 2026-10-29), so the
  brief was implemented by hand at the same bar.
- **Dependency intake:** `clap` 4.6 with `derive` (MIT or Apache-2.0, the standard Rust argument
  parser); the lock adds only clap's own tree (anstream, anstyle, clap_lex, heck, strsim, syn,
  proc-macro2, quote, windows-sys on Windows). No network, file or process access of its own.
  `cargo audit` is not installed here, so no advisory scan was run.
- **Verified:** red first (`builder/cli/tests/version.rs` failed on its assertion against an empty
  `main`), then green; a mutation (core `VERSION` set to `9.9.9`) turned it red and was restored with
  `cmp`. `bash scripts/ci-local.sh` passed with cargo on `PATH` (all three Rust steps ran) and with
  `~/.cargo/bin` removed from `PATH` (it printed the skip line and passed).
- **Not verified:** a Linux or Windows build; a contributor machine with an older stable Rust than
  1.98 (edition 2024 needs 1.85 or newer).
- **Landed by V1 (2026-10-24):** rebased onto `main` (one README table conflict, both rows kept),
  re-authored to the project's noreply identity, and pushed to `main` as a fast-forward so the
  committer stays noreply too. `bash scripts/ci-local.sh` passed again with and without `cargo` on
  `PATH`.

## PB4.0: the growth loop spec (2026-10-23)

- **What landed:** `docs/spec/growth-loop.md`, five JSON Schemas and five invented examples in
  `docs/spec/growth-loop/`, a standard-library validator for exactly the keywords the schemas use
  (`tests/schema_subset.py`, which refuses any other keyword anywhere in a schema), and
  `tests/growth_loop_spec_test.py`, wired into `scripts/ci-local.sh`. Brief:
  `plans/briefs/PB4.0-growth-loop-spec.md`.
- **Who built it:** implemented directly from the brief; the usual implementer was at a usage limit.
- **Decisions made at pickup:** no new dependency (a small validator instead of `jsonschema`, so the
  kit still needs only Python and shellcheck); `additionalProperties: false` everywhere, so any shape
  change is a new version; the file-path pattern avoids lookaround so a Rust implementation can compile
  it; a case's status changes by appending the whole case again (last line wins).
- **Review:** an independent review passed it with should-fixes, all applied: rule 3 now requires the
  results to cover every case the proposal names (an omitted new case can no longer pass), versioning
  matches the closed schemas, rejected proposals get no measurement, a stale `base` is never applied,
  and the doc lists the checks the schemas cannot make. Security review: no findings.
- **Verified:** red first (validator assertions failed, spec files missing), then green; seven mutation
  checks each failed the test and were restored (`cmp`); the reference `jsonschema` 4.x library, in a
  throwaway environment, agreed on all five schemas and examples and on the path probes;
  `bash scripts/ci-local.sh` green.
- **Not verified:** the patterns under a Rust or JavaScript regex engine (they use only portable
  syntax; PB4 will run them in Rust).
