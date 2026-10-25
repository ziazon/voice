# PB3.1 brief: the build's steps that need no model, in Rust

Written by Claude (lane V1), 2026-10-24. Spec: lane V's `PB3.1` entry, split from PB3 of the Persona
Builder plan. Read the repo's `AGENTS.md` first, then `plans/briefs/PB2.4-merge-and-split.md` (same
parity style, the same accepted-difference rules, the same private-folder reads and writes). Work
test-first (`~/.codex/skills/tdd-skill/SKILL.md`). Security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`. **No new dependency.**

**Built by Claude directly:** Codex was at a hard usage limit (until 2026-10-30) at pickup, so Claude
implemented this brief at the same bar.

## What this slice does

A new crate `builder/steps` (package `persona-builder-steps`, library `persona_builder_steps`) ports
eight commands of `scripts/crew_decisions.py` that need no model: `chunk`, `check-labels`, `tally`,
`distil-in`, `score`, `similar`, `shingle` and `round`. Each gives the same records, the same bytes and
the same printed lines as the Python on every input the Python accepts and finishes on, and each is a
`persona-builder` command. PB3.3 (the build) and PB3.4 (the interview) call the library directly.
`similar` is here because the tool-less backtest (PB3.3) runs it in Rust and puts the precedents in the
prompt.

## Decisions (made, do not reopen)

### Parity, and where the Python is not the oracle

PB2.4's rules hold: the Python is the oracle where it finishes; where it raises, Rust returns an error,
writes nothing, and both exit 1. **No error carries record text**: it names the file, a line number, a
key or a record id (ids are hashes or names the person chose). Accepted differences, each pinned by a
Rust-only test:

1. **Files, not paths.** Every corpus, label, prediction and round file is `NAME` or `FOLDER/NAME` in
   the member's state folder (`~/.claude/state/<slug>/`), read and written through descriptors that
   never follow a link. `--doc` (shingle) is the one path: the draft may sit in the crew folder.
2. **Every JSON input is checked whole before use.** UTF-8 only; serde_json's grammar (`NaN`,
   `Infinity` refused); a number that is not a whole number serde_json keeps exactly is refused, as
   `read_records` does. A labels or predictions file must be an object whose values are objects. A
   label's `category` must be a string where it is read.
3. **Odd types the Python only tolerates by accident are refused:** a corpus `outcome` that is not one
   of `took_rec`, `other_option`, `own_words`, `no_rec` (the Python adds 1 to any key of the tally row
   with that name); a `timestamp`, `id`, `text`, option `label` or `description`, and in `similar` a
   `project`, `question` or `outcome`, that is not a string, and in `score` a `picks` given as text
   rather than a list (the Python reads the text's letters); `--size` or `--n` below 1 and a negative
   `--k` (the Python crashes on 0 and slices from the end for a negative `--k`); `--k 0` prints
   nothing, as the Python does.
4. **`check-labels` reports, never crashes.** It is the gate the model's labels pass, and its
   problems feed a retry. A label that is not an object is the problem `<id> label`; an `escalates`
   that is not a list of strings is `<id> escalates` (the Python turns a string into its letters and
   crashes on a number).
5. **The codebook.** An empty `categories`, `escalates`, `signals` or `groups` counts as missing (the
   member format cannot tell empty from absent). A group whose value is not a list of strings, or
   whose name does not make a plain file name (`<group>.jsonl`), is a member config error.
6. **`similar`'s weights** are summed over the shared words in byte order, with CPython 3.12's
   compensated `sum()` (Neumaier's). The Python sums in set order, which changes with the hash seed,
   so two records whose weights differ only in the last bit can swap between Python runs; an older
   Python (before 3.12) sums without compensation. Parity tests avoid near-ties and run the Python
   with `PYTHONHASHSEED=0`.
7. **`round`'s date takes ASCII digits only.** The Python's `\d` and `strptime` also take other
   scripts' digits in the year and a day's second digit (`٢٠٢٦-10-1٢` imports there).
8. **`chunk` leaves older chunk files** beyond the ones it writes, as the Python does. PB3.3 reads
   the chunk list from the library, never from the folder.

### Reading and writing (`persona_builder_ingest::private`)

- `.json` joins `.jsonl` and `.tsv` as a private file ending; `validate_json_name` checks a plain name
  ending `.json`.
- `read_private_at(home, slug, folder, name)` and `write_private_at(home, slug, folder, name, bytes)`
  read and write `FOLDER/NAME`: the folder made as `private_folder` makes it (0700, never a link,
  refused unless only this user can open it), the file as `read_private` / `write_private` treat it
  (0600, replaced atomically, never a link). `write_private` and `write_private_at` share one write.
- `place(arg)` splits a CLI argument into an optional folder and a name: at most one `/`, both parts
  plain (no leading `.`, no `\0`, not empty).

### Shared helpers made public in `persona_builder_ingest`

`privacy::searchable(record)` (the Python's `searchable`, factored out of `filtered_text`);
`mine::outcome`; `mine::Mark` (the gate-mark pattern, with `bare(label)`: the marks removed, then
Python's `strip()`); `corpus::exact_numbers`; the civil-date helpers move to `dates.rs`.

### JSON output

- `tally --json` and `similar --json`: `json.dumps(indent=2, ensure_ascii=False)` is serde_json's
  pretty printer (same escapes, same `[]` and `{}`) plus a newline.
- `score --json`: `json.dumps(indent=2)` escapes everything outside printable ASCII:
  `persona_builder::json::to_python_json`.
- Shares are `round(n / total, 4)`: `format!("{:.4}")` parsed back (both round the exact binary value
  half to even), written by serde_json, which writes `0.0`, `1.0` and four-place values as Python's
  `repr` does. `{:.0%}` is `format!("{:.0}%", x * 100.0)`: Python multiplies first, too. Unit tests pin
  `1/32` (a tie: `0.0312`), `1/8` (`12%`), `3/8` (`38%`), `1/200` (`0%`).

### The commands (library in `builder/steps/src/`, one module each)

Each follows the Python line for line in the order it reads fields, so an error happens where the
Python's would, and reads a field only where the Python reads it (a held-out record without `text`
is fine in `similar`).

- **`chunk`** (`cmd_chunk`): train records whole; held-out questions as `kind` then the eight
  question fields; parts of `--size` named `train-NN.jsonl` and `heldout-NN.jsonl`; the summary line.
- **`check-labels`** (`label_problems`): the problems in the Python's order and words, the first 40
  printed, exit 1 when any. Records by id, the last of a repeated id winning.
- **`tally`** (`check_labels` then `tally`): the three mismatch lists, then the rows sorted by records
  (most first) then name; the table or the JSON.
- **`distil-in`** (`cmd_distil_in`): one file per group, in the codebook's order, even when empty; a
  category in two groups goes to the later; `<group> <rows>` printed per group.
- **`score`** (`score`, `score_markdown`): missing predictions first; tiers in the order high,
  medium, low; categories by name; the `<slug>_led` row and `took_<slug>`.
- **`similar`** (`similar`, `precedent`, `cmd_similar`): training records only, messages of at least 8
  words; words of 3 or more characters, lowercased, without the stop words; at least 2 shared words;
  sorted by weight (highest first) then timestamp, stable; the first `--k`.
- **`shingle`** (`cmd_shingle`): n-word shingles of the doc against every corpus's text, own words,
  notes, question, context and option text; the count, the first 60 overlaps, exit 1 when any.
- **`round`** (`cmd_round`): every check of the Python's, in its order; labels gain
  ` (Recommended)`; one record per answered question, timestamped at noon UTC on `answered` plus `n`
  seconds; the privacy filters; the two summary lines.

### The CLI

```
persona-builder chunk <member_dir> [--corpus NAME] [--out-dir FOLDER] [--size N]
persona-builder check-labels <member_dir> --chunk PLACE --labels PLACE --mode train|heldout
persona-builder tally <member_dir> [--corpus PLACE] --labels PLACE [--json]
persona-builder distil-in <member_dir> [--corpus PLACE] --labels PLACE [--out-dir FOLDER]
persona-builder score <member_dir> [--answers PLACE] --labels PLACE --predictions PLACE [--json]
persona-builder similar <member_dir> [--corpus PLACE] --query TEXT [--k N] [--json]
persona-builder shingle <member_dir> --doc PATH --corpus PLACE... [--n N]
persona-builder round <member_dir> --name NAME --questions PLACE --answers PLACE [--heldout] [--out NAME]
```

Defaults: `--corpus decisions.jsonl`, `--answers heldout-answers.jsonl`, `--out-dir chunks` (chunk) and
`distil` (distil-in), `--size 300`, `--k 6`, `--n 6`, `--out decisions-round-<name>.jsonl`. The member
and its state folder check are `mine`'s. A command that writes prints the Python's lines, then
`saved to ~/.claude/state/<slug>/<where>`. Help lines in plain words.

## Tests

As built (Claude wrote the code and the tests together, so each test file was proven by mutation, not
by a red before its code):

1. **`tests/crew_steps_fixtures.py`** (standard library): `python3 -I tests/crew_steps_fixtures.py
   <case> <member> <state dir>` writes an invented member (ADA or ARI, with a codebook) and the case's
   files (`base`, `empty`, `shares`), and prints the runs to compare.
2. **`builder/cli/tests/steps_parity.rs`**: every run through the real Python and the
   `persona-builder` binary on the same files: the printed lines, the exit codes and every written
   file byte for byte, the files 0600 in 0700 folders. Both members, all three cases.
3. **`builder/steps/tests/rules.rs`**: each accepted difference, each error's words and that no record
   text appears in it, the Python's own exit code asserted beside each refusal.
4. **`builder/ingest/tests/private_at.rs`**: the folder reads and writes refuse links at the folder and
   the file, set 0700 and 0600, refuse a folder others can open, and create nothing on a failed read.
5. **`builder/steps/src/share.rs`**: the rounding edges, against values CPython printed.

## Done when

Every test above went red on its assertion first and is green; `bash scripts/ci-local.sh` passes.

## After the gates (2026-10-24)

`/security-review` (a sub-agent): no high or medium finding; three lows. The Verifier (Fable): pass with
should-fixes, no must-fix. Applied, items 1 to 3 and 5 each with a test that went red first; item 4 pins behaviour that was
already there:

1. **The state folder itself is checked** (security low 1): a state folder others can change is
   refused on every read and write, as a sub-folder already was (`private_at.rs`).
2. **No panic on an unchecked file** (security low 2, Verifier 4): `score` returns an error, where it
   panicked, for a prediction that is not an object, and `distil_in` names a label that is not an
   object (it already refused one, as having no category), for PB3.3 and PB3.4, which call the
   library directly.
3. **`similar` reads only the option labels** (Verifier 2), as the Python does: a record whose
   options have no description is a precedent.
4. **Two more accepted differences** (Verifier 1 and 3): `round`'s ASCII-only date (difference 7;
   the earlier note that the Python refuses other digits held only when every digit is another
   script's) and `score`'s
   `picks` as text, each pinned beside the Python's exit 0.
5. **A link at a file says so** (Verifier 5): "… is a link: Persona Builder reads only real files,
   never links", in place of the folder message.
6. **Fewer copies** (Verifier 6): `chunk` moves records into parts, `similar_json` takes the hits.

Kept, as the Python does: a label key or category the model wrote can appear in an error (security
low 3, recorded in `.ai/SECURITY.md`); `shingle --doc` reads any path the person names.

The delta Verifier pass (Opus): pass with should-fixes, no must-fix, all applied: difference 3's
wording matched to the code, one label helper for `distil_in` and `category`, the test name and these
notes made exact, the `place_directory` comment and the link message reworded.
