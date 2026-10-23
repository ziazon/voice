# Brief: a member for a principal with no Claude history

Two generic features for a principal who never uses Claude (for example someone under 18, whom
Anthropic's consumer terms exclude), so the kit cannot mine their transcripts:

1. **`documents`**: turn a folder of the principal's own work into private sub-agent inputs for a
   *lens* (how they build, what they value, what they reject). Raw text stays in the state dir.
2. **`round`** plus **`merge --rounds`**: import a question round answered off-Claude (a fillable
   form an adult relays) into corpus records, with a flag that holds a whole round out so the
   existing backtest scores the model blind on it.

Plus an installer route, `install.sh --no-history`, that never mines the Mac it runs on (the Mac
belongs to the adult, not the principal).

Read first: `AGENTS.md`, `CONTRIBUTING.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`,
`scripts/crew_decisions.py` (the whole file), `tests/crew_decisions_test.py` (its `Harness`, `ARI`,
`ADA`), `tests/crew_fixtures.py`, `tests/crew_sources_test.py` (class `Merge`),
`tests/crew_labels_test.py`, `tests/crew_install_test.sh`, `bootstrap/install.sh`,
`crew/_template/interview.md` (the record shape).

Skills: `~/.codex/skills/tdd-skill/SKILL.md` and `~/.codex/skills/code-quality/SKILL.md`.

## Rules that bind every line

- **Public repo, no real people.** Fixtures use fictional people and worlds only (the repo's Sam,
  Robin, Ada, Ari, Alex, Jo; `example.com`). No real name, place, school or quote anywhere.
- **Python 3.9+ standard library only.** No new dependency. PDF text comes from the external
  `pdftotext` program (poppler) when present; nothing is vendored.
- **Every file written goes through `private_write` / `private_directory`** (0600 files, 0700
  folders). Never write to `/tmp` from product code.
- **Errors are one line** (`crew_decisions: <message>`), raised as `Failed`, never a traceback.
  Tests check them with `assert_cli_error`.
- **Match the file's idiom:** short docstrings, `cmd_<name>(m, args)` functions registered in
  `COMMANDS`, options in `parser_for`, the docstring at the top lists every command.
- Tests run the real CLI through `Harness` (subprocess), never import product functions.

## The contract

### A. `documents`

```
crew_decisions.py --member M documents --dir FOLDER --out-dir DIR [--size 6000] [--pdftotext PATH]
```

New module **`scripts/crew_documents.py`** holds the file reading (pure: no member, no filters, no
writes). `crew_decisions.py` imports it the way `crew_personas.py` imports `crew_decisions`
(`sys.path.insert(0, str(Path(__file__).resolve().parent))`), and `cmd_documents` in
`crew_decisions.py` applies the member's filters and writes. `crew_documents.py` never imports
`crew_decisions` (a cycle would give two `Failed` classes); it raises its own
`Unreadable(reason)` that `cmd_documents` turns into a skipped inventory row.

**Walk.** Every file under `--dir`, recursively, in sorted order of its POSIX path relative to
`--dir`. Not listed at all: any path part starting with `.`, `__MACOSX`, `Thumbs.db`,
`desktop.ini`, and Word lock files (name starting `~$`). A symlink (file or folder) is never
followed: it is listed as skipped with reason `symlink`.

**Kinds**, by lower-cased suffix:

| Suffix | Kind | Read how |
|---|---|---|
| `.md`, `.markdown`, `.txt` | `text` | bytes decoded UTF-8 with `errors="ignore"` |
| `.docx` | `docx` | `zipfile`: only `word/document.xml`; caps below; `xml.etree.ElementTree` |
| `.pdf` | `pdf` | `subprocess.run([pdftotext, "-q", "-enc", "UTF-8", str(path), "-"], capture_output=True, timeout=120)` |
| `.png .jpg .jpeg .gif .webp .heic .svg .bmp .tif .tiff` | `image` | never opened: listed by name only |
| anything else | skipped | reason `unsupported type` |

- Any file over 50 MiB on disk: skipped, reason `too large`.
- **docx caps and guard**, each a skip with reason `unreadable`: not a zip, no
  `word/document.xml`, its uncompressed size over 50 MiB, its compression ratio over 100:1 (check
  `ZipInfo` before reading, like `export_files`), the XML containing `<!DOCTYPE` or `<!ENTITY`
  (refused before parsing), or a parse error. Paragraphs are each `w:p` element
  (`{http://schemas.openxmlformats.org/wordprocessingml/2006/main}p`, found with `.iter`, so table
  cells count) whose text is its `w:t` texts joined with no separator.
- **pdf:** `--pdftotext` defaults to `shutil.which("pdftotext")`. Not found (None, or the run
  raises `FileNotFoundError`/`PermissionError`): skipped, reason
  `no pdftotext (brew install poppler, or export it as .docx or .txt)`. Non-zero exit or timeout:
  skipped, reason `unreadable`. Form feeds (`\f`) in its output count as paragraph breaks.
- **text and pdf paragraphs:** split on blank lines (`re.split(r"\n\s*\n", text)`).
- Every paragraph has its whitespace collapsed (the existing `flat`); empty ones are dropped.

**Filters.** A file whose relative path matches the member's `personal` or `secret` pattern is
skipped whole, listed with `"source": "[filtered]"` and reason `filtered`, and never opened. Each
paragraph matching `secret` is dropped and counted `secret`; else matching `personal`, dropped and
counted `personal` (secret wins, as in `privacy`). A dropped paragraph's text appears in no output.

**Parts.** A document's kept paragraphs are packed in order into parts of at most 500 words
(`PART_WORDS`; words are `len(text.split())`): a paragraph joins the current part unless that would
pass 500 and the part already holds something; a single paragraph over 500 words is a part of its
own, never cut. A part's text is its paragraphs joined with `"\n\n"`. Each part is one record:

```json
{"id": "document:<record_id(\"document\", source, str(part), text) suffix>", "kind": "document",
 "source": "maps/notes.md", "part": 1, "text": "First paragraph.\n\nSecond paragraph."}
```

(`id` is exactly `record_id("document", source, str(part), text)`; `part` counts from 1 within its
document.)

**Chunks.** All parts, in walk order, are packed into `DIR/documents-NN.jsonl` (`NN` from `01`):
a part joins the current chunk unless that would pass `--size` words and the chunk already holds a
part. Before writing, every existing `DIR/documents-*.jsonl` is deleted, so a rerun with fewer
chunks leaves no stale file. A folder with no parts writes no chunk file.

**Inventory.** `DIR/inventory.jsonl`, one line per listed file, in walk order:

- read: `{"source": s, "kind": "text|docx|pdf", "words": kept words, "parts": n, "dropped": {"personal": a, "secret": b}}`
- image: `{"source": s, "kind": "image"}`
- skipped: `{"source": s, "kind": "skipped", "reason": r}` (`s` is `[filtered]` for a filtered name)

**Out dir.** Created with `private_directory`. Refused (`Failed`, nothing written) when `DIR` or
any parent of its resolved path holds a `.git` entry: `out dir must not be inside a git work tree`.
The same check refuses nothing else in the script.

**Stdout**, exactly two lines (numbers only, never a file name):

```
read 3 documents into 4 parts (812 words) in 1 chunks; listed 1 images; skipped 2 files
dropped 1 personal and 1 secret paragraphs
```

`documents` counts text, docx and pdf files read without a skip (even if every paragraph dropped).

**`shingle`** needs no change: a document record's `text` is already read. A test pins that.

### B. `round`

```
crew_decisions.py --member M round --name NAME --questions Q.json --answers A.json [--heldout] [--out PATH]
```

`NAME` must match `^[a-z0-9][a-z0-9-]{0,39}$` (else `Failed`). `--out` defaults to
`<state_dir>/decisions-round-<NAME>.jsonl`. The output is rewritten whole each run (an import is
idempotent).

**Questions file** (drafted by the build; the form shows exactly this):

```json
{"questions": [
  {"n": 1, "header": "Coast", "question": "A new player lands on the coast. What do they see first?",
   "multiSelect": false, "recommended": "A",
   "options": [{"label": "The harbour", "description": "ships and a market"},
               {"label": "The cliffs", "description": "a long climb"}]}]}
```

- Options are lettered `A`, `B`, `C`, `D` in order; 2 to 4 options, labels non-empty and distinct.
- `n`: a positive int, distinct. `question`: non-empty string. `header`, `description`: strings,
  default `""`. `multiSelect`: bool, default false. `recommended`: an option letter, or null/absent.

**Answers file** (what the adult relays):

```json
{"answered": "2026-11-02",
 "answers": {"1": {"pick": "A"}, "2": {"pick": ["A", "C"], "notes": "only in winter"},
             "3": {"own_words": "the lighthouse, always"}}}
```

- `answered`: `YYYY-MM-DD`, required.
- Keys are question numbers as strings. A key with no question: `Failed`. A question with no key,
  or with neither a pick nor non-empty `own_words`: not imported, counted `unanswered`.
- `pick`: a letter, or a list of letters (only when `multiSelect`; a list on a single-select
  question with more than one letter is `Failed`; a one-letter list is accepted). An unknown
  letter is `Failed`. `own_words`, `notes`: strings, default `""`.

Every message for a bad file names the question number and the field, e.g.
`round: question 2: pick "E" is not an option`.

**Record**, the shape `crew/_template/interview.md` gives and `mine` writes:

```json
{"id": "question:<NAME>-<n:02d>", "kind": "question",
 "timestamp": "<answered>T12:00:00.000Z plus n seconds, as …T12:00:01.000Z for n=1",
 "project": "round-<NAME>", "header": "…", "question": "…", "multiSelect": false,
 "options": [{"label": "The harbour (Recommended)", "description": "ships and a market"}, …],
 "recommended": "The harbour (Recommended)", "<slug>_pick": null,
 "picks": ["The harbour (Recommended)"], "own_words": "", "notes": "",
 "outcome": "took_rec", "split": "train"}
```

- The recommended option's label gains ` (Recommended)`; `recommended` is that label or null.
- `picks` are the picked options' labels as shown (with the mark), in option order.
- `outcome` is the existing `outcome(picks, own_words, recommended)`.
- `split` is `"heldout"` with `--heldout`, else `"train"`. (The backtest's held-out value is
  `heldout`: `split`, `chunk` and `check-labels` read it.)
- Every record passes `privacy(m, r)`; a dropped one is counted, never written.

**Stdout:**

```
imported 4 answers from round spring (train); 1 unanswered
dropped 0 personal records, 0 secret records
```

### C. `merge --rounds`

- `--in` becomes optional (`nargs="*"`, default `[]`). New `--rounds FILE...` (`nargs="*"`,
  default `[]`): *question-round files: each record keeps the split it was imported with*.
- No file at all across `--in`, `--train` and `--rounds`: `Failed("merge needs at least one input")`.
- A round record whose `split` is not `train` or `heldout`: `Failed` naming the file.
- Round records, like `--train` records, never move the cutoff: the cutoff is computed only over
  `--in` records. `--train` records end `train`; `--rounds` records keep their own split.
  Duplicates and the privacy re-check behave as today for every input.
- The summary's file count includes `--rounds` files. The rest of the output is unchanged.

### D. `install.sh --no-history`

- New flag `--no-history` (no value). With `--role extra`: `die "--no-history is for the main role only"`.
- Step 4 prints its header, then `skipped (--no-history): nothing is read from this Mac's Claude Code history`,
  and neither mines nor creates `decisions-<host>.jsonl`. Step 5 prints its header, then
  `skipped (--no-history)`, and reads no export.
- Step 6 (both routes): `decisions-interview.jsonl` goes to `--train` (as today), every
  `decisions-round-*.jsonl` to `--rounds`, every other `decisions-*.jsonl` to `--in`. With nothing
  in any of the three, print
  `nothing to merge yet: import a question round first (PROMPT.md, the route without a Claude history)`
  and exit 0. Build the argument arrays so an empty one never reaches the command line and never
  trips `set -u` on macOS bash 3.2 (`${arr[@]+"${arr[@]}"}`, or a length check).
- The "That is thin" hint prints only without `--no-history`.
- Add the flag to the header comment, and keep `--help` printing the whole comment block (the
  `sed -n '2,20p'` range must grow with it).

### E. Wiring

- Docstring at the top of `crew_decisions.py`: add `documents`, `round` and `merge`'s `--rounds`
  in the same style as the other commands.
- `scripts/ci-local.sh`: add `crew_documents` and `crew_rounds` to the test loop.
- `.ai/PROJECT.md` table: add `scripts/crew_documents.py` (its test `tests/crew_documents_test.py`).

## Seams the tests may use (and only these)

| Seam | Catches | Misses |
|---|---|---|
| The `crew_decisions.py` CLI through `Harness.crew` | every command's output files, stdout, exit code, error line | nothing in scope |
| `--pdftotext PATH` pointing at a fake script the test writes | the argv contract and output parsing | the real poppler (the reviewer runs it live) |
| `tests/crew_install_test.sh` running `bootstrap/install.sh` in scratch HOMEs | the `--no-history` route end to end | nothing in scope |

## Dispatch A: write the failing tests ONLY

Write `tests/crew_documents_test.py` and `tests/crew_rounds_test.py` (unittest, reusing `Harness`,
`ARI`, `ADA` from `crew_decisions_test.py` and helpers from `crew_fixtures.py`), add the
`--no-history` cases to `tests/crew_install_test.sh`, and add both modules to
`scripts/ci-local.sh`. Do **NOT** create `scripts/crew_documents.py`, do **NOT** edit
`scripts/crew_decisions.py` or `bootstrap/install.sh`, and do not touch any other file. The new
tests are **EXPECTED to fail**: do not make them pass. Existing tests must still pass.

Cover at least:

- **documents:** md, txt and a docx the test builds with `zipfile` (namespaced `w:p`/`w:t`, one
  paragraph inside a table) read into the exact records and inventory rows above, with exact
  stdout; an image listed and its bytes absent from every output; a `personal` paragraph, a
  `secret` paragraph and a filtered file name, none of whose text appears in any output file
  (read every file in the out dir); hidden files, `__MACOSX`, `~$` lock files absent from the
  inventory; a symlink skipped and its target's text absent; not-a-zip, `<!DOCTYPE`/`<!ENTITY`,
  and a >100:1 `document.xml` each `unreadable`; an `.mp3` `unsupported type`; a fake `pdftotext`
  (a shell script printing two paragraphs and a `\f`) read into parts, and a missing one a skip
  whose reason names `pdftotext`; parts never over 500 words except a single long paragraph;
  `--size` splitting into several chunks and a rerun with a bigger size removing the stale
  `documents-02.jsonl`; modes 0700 and 0600; an out dir inside a fake git tree refused with one
  error line and nothing written; `shingle` flagging six words copied from a document chunk and
  passing a paraphrase.
- **round:** one full record compared field by field with a literal (not recomputed); each
  outcome (`took_rec`, `other_option`, `own_words`, `no_rec` on a multi-select with no
  recommendation); an unanswered question counted; `--heldout`; default `--out` path and mode
  0600; a rerun giving the identical file; a `personal` question dropped and counted; every bad
  input above as a one-line error (bad name, 1 and 5 options, duplicate labels, duplicate `n`,
  unknown recommended letter, unknown pick letter, a two-letter list on a single-select, an
  answer for a missing question, a missing or malformed `answered`).
- **merge:** only `--rounds` (no `--in`) works; no input at all fails; `--in` mined history
  plus a later train round plus a later held-out round, where the mined split ignores both rounds'
  timestamps and the rounds keep theirs; a bad round `split` fails; then `split` emits exactly the
  held-out round's questions and `score` scores predictions for them (an end-to-end blind test).
- **install.sh:** `--no-history` with a transcript in the scratch HOME's projects: no
  `decisions-<host>.jsonl`, the skip lines and the `nothing to merge yet` line, exit 0; after a
  round file is imported into the state dir, a rerun merges it and the merged corpus holds the round
  records and no transcript text; `--no-history --role extra` refused with its message.

Then run `bash scripts/ci-local.sh` and report which tests fail and why (they must fail on the
missing command or behaviour, not on a typo or an import error in the test itself).

## Dispatch B: implement

The tests in `tests/crew_documents_test.py`, `tests/crew_rounds_test.py` and the `--no-history`
cases in `tests/crew_install_test.sh` already exist and FAIL. **They are the specification. Do NOT
edit them.** If you believe a test is wrong, STOP and report which assertion and why.

Implement sections A to E above: `scripts/crew_documents.py` (new), `scripts/crew_decisions.py`,
`bootstrap/install.sh`, `.ai/PROJECT.md`. Keep `crew_documents.py` to file reading; filters, ids,
packing into chunks and writing live in `cmd_documents`. Keep each function at one level of
abstraction, early returns over nesting, names that say what they hold. Run
`bash scripts/ci-local.sh` until it passes, and report its tail.

## Review fixes (dispatch C)

Two defects the review found, each test first:

1. **A special file hangs `documents`.** A FIFO (or any non-regular file) under `--dir` blocks
   `read_bytes` forever. `crew_documents.read` must check the `lstat` result first: anything that is
   not a regular file is skipped with reason `unreadable` and never opened, whatever its suffix.
   Test: `os.mkfifo(self.source / "pipe.txt")` beside a normal `a.txt`; the run exits 0 and the
   inventory row for `pipe.txt` is `{"source": "pipe.txt", "kind": "skipped", "reason": "unreadable"}`.
2. **`--no-history` merges a stale history corpus.** A `decisions-<host>.jsonl` or
   `decisions-claudeai.jsonl` left from an earlier run without the flag still reaches `--in`, which
   brings the running Mac's own history into the member. With `--no-history`, step 6 passes no
   `--in` at all: each such file prints `ignored (--no-history): <file name>` and is left on disk.
   Test (in `tests/crew_install_test.sh`, in the no-history case after the round import): write
   `decisions-old.jsonl` into the relay state dir holding one message record whose text is
   `STALE_HISTORY_MUST_NOT_MERGE`, rerun with `--no-history`, check the `ignored` line, and assert
   the merged corpus still equals the imported round and lacks that text.

Also wrap the `COMMANDS` dict in `crew_decisions.py` to the file's usual width.

## Verifier fixes (dispatch D)

The Verifier found two inputs that end in a traceback instead of one `crew_decisions:` line. Test first:

1. **Deeply nested JSON.** A `--questions` or `--answers` file holding `"[" * 100000` raises
   `RecursionError` from `json.loads`. `read_round_file` must turn it into a one-line `Failed`
   (`round: questions: …`), and `main`'s final `except` must also catch `RecursionError`, so a
   nested line in any JSONL a command reads (for example `merge --in`) ends in one line too.
   Tests: `round` with a nested questions file; `merge --in` with a nested line. Each: exit 1,
   `assert_cli_error`, no traceback.
2. **A merge input line that is not an object.** `[1, 2]` as a line of any `merge` input raises
   `AttributeError` or `TypeError`. `cmd_merge` refuses it before anything else reads the record:
   `Failed(f"{path}: a line is not a JSON object")`. Test: a `--rounds` file with that line, and an
   `--in` file with that line: exit 1, one line naming the file.
