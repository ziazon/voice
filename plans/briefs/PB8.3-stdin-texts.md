# PB8.3 brief: the record and look-up texts can't run as shell code

Persona Builder builds a person's AI assistant from their own Claude history. The assistant's
`SKILL.md` (from `crew/_template/SKILL.md`) tells Claude Code to run two commands, through the
placeholders `{{SIMILAR}}` and `{{RECORD}}`:

- the guided build fills them with `python3 -I {{REPO}}/scripts/crew_decisions.py --member … similar`
  and `… record` (`bootstrap/install.sh`, and `builder/core/src/crew.rs` with `Decisions::Python`);
- the app fills them with the shipped `"$HOME/…/persona-builder" similar <member dir>` and
  `… record <member dir>` (`Decisions::Cli`).

Today the template has Claude write each text as a double-quoted shell word:
`--query "<the question and its options>"`, `--question "<q>" --{{SLUG}}-pick "<pick>"`, and so on.
The texts are questions, picks and answers, and they can hold anything: a question about a script
can contain `$(rm …)` or backticks, and the shell runs those inside double quotes before the command
ever starts. This slice gives `similar` and `record` a way to take their texts that the shell never
reads, **one JSON object on stdin through a single-quoted heredoc**, and makes the template use it.

Why JSON in a quoted heredoc: the shell does no expansion at all inside `<<'END_OF_TEXT'`, and valid
JSON can never hold a line that is exactly `END_OF_TEXT` (a line break inside a JSON string is
written `\n`, and a bare `END_OF_TEXT` is not a JSON token), so the text can't end the heredoc early
and spill into the shell either.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`, then
`plans/briefs/PB8.1-shipped-commands.md` (the slice that made `{{SIMILAR}}`, `{{RECORD}}` and
`persona-builder record`). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`.

Work test-first: write each test, run it and see it fail on its assertion, then write the code.
This is a public repo: invented names only in tests and fixtures (Ari, Robin, Wren, as the existing
tests use), no real person, no fixed paths. Commit nothing; leave the changes in the worktree.

## Sources (checked 2026-10-26)

- POSIX Shell Command Language §2.7.4 *Here-Document*
  (<https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html#tag_19_07_04>): "If
  any part of word is quoted, … the here-document lines shall not be expanded"; the here-document
  ends at "a line containing only the delimiter".
- Python `argparse`: `add_mutually_exclusive_group(required=True)` and `ArgumentParser.error`
  (prints the usage and the message to stderr, exits 2).
- Python `json.loads`: `object_pairs_hook` sees every key in order, so a repeated key can be refused;
  `json.loads` accepts a lone surrogate escape (`"\ud800"`) that `serde_json` refuses.
- `serde`: `#[serde(deny_unknown_fields)]` refuses an unknown key; the derived visitor refuses a
  repeated field ("duplicate field").
- clap 4: `Arg::required_unless_present`, `Arg::conflicts_with`, `ArgAction::SetTrue`,
  `Command::error(..).exit()` (exit 2).

## The input: one JSON object on stdin

Both programs (`scripts/crew_decisions.py` and the Rust `persona-builder`) read it the same way, and
accept and refuse the same inputs. With `--stdin`:

1. Read stdin as bytes, at most 64 KiB (65536 bytes): read up to 65537 bytes; more than 65536 is
   refused: `--stdin takes at most 64 KiB`.
2. Decode as strict UTF-8 (refuse otherwise: `--stdin needs UTF-8 text`). Python: decode the bytes
   with `"utf-8"` yourself before `json.loads` (never `json.loads(bytes)`, which guesses UTF-16/32).
3. Parse one JSON value; it must be an object (refuse otherwise:
   `--stdin needs one JSON object: <detail>`, where `<detail>` is the parser's own message). Leading
   and trailing whitespace is fine (both parsers allow it).
4. Keys: only those listed for the command; an unknown key, a repeated key, a missing required key,
   or a value of the wrong type is refused. A required value is a JSON string; `rule` and `answer`
   (record only) are a JSON string or `null`, or left out (both mean "not given").
5. Python only: after parsing, refuse any string that can't be encoded as UTF-8
   (`s.encode("utf-8")` raising `UnicodeEncodeError`, a lone surrogate), with the same
   `--stdin needs one JSON object: <detail>` form; serde_json already refuses it.

Every refusal here is a **usage error**: exit 2, nothing written, nothing searched. Python reports
it through the subparser's `error()` (usage plus `<prog> <cmd>: error: <message>`); Rust through
`clap::Command::error(ErrorKind::ValueValidation, message).exit()`. The two programs' messages need
not match word for word; each names the problem, and a key problem names the key.

`--stdin` and the text flags are either-or: giving `--stdin` and any text flag is a usage error;
giving neither is the same usage error as today's missing flag.

### `similar --stdin`

The object is `{"query": "<text>"}`; `query` is the only key and is required. `--corpus`, `--k`
and `--json` stay flags (they are not texts). The output is byte for byte what `--query <same text>`
prints.

- Python (`parser_for`): replace `p.add_argument("--query", required=True)` with a required
  mutually exclusive group holding `--query` and `--stdin` (`action="store_true"`,
  `help="read {\"query\": …} as JSON from stdin"`). Keep `p.set_defaults(usage_error=p.error)` on
  the subparser so `cmd_similar` can report usage errors. In `cmd_similar`, when `args.stdin`, the
  query is the object's `query`.
- Rust (`builder/cli/src/main.rs`, `Command::Similar`): `query: Option<String>` with
  `#[arg(long, required_unless_present = "stdin", conflicts_with = "stdin")]`, and
  `#[arg(long)] stdin: bool`. In `similar()`, read the query from stdin when `stdin` is set.

### `record --stdin`

The object's keys are `work`, `kind`, `category`, `tier`, `question`, `pick` (required) and `rule`,
`answer` (optional). They are the flag names without the member: `pick` is what
`--<slug>-pick` gives and `answer` what `--<principal_slug>-answer` gives, because the member
folder in the command already says whose they are. `kind` must be `decided` or `escalated`
(`kind must be decided or escalated`), `tier` `high`, `medium` or `low`
(`tier must be high, medium or low`). The line written to `live.jsonl` is exactly the line the
flags would write for the same values (keys in the same order, `<slug>_pick` and
`<principal_slug>_answer` as today, `null` for a value not given), and the printed line is the same
`recorded a <kind> <category> decision (<tier>) for <work>`. The 64 KiB line limit and every
writing rule (`append_private`, the missing-state sentence) stay exactly as they are.

- Python: in `parser_for`, the six required record flags become `required=False`; add `--stdin`
  (`action="store_true"`, `help="read the texts as one JSON object from stdin"`) and
  `p.set_defaults(usage_error=p.error)`. Keep a list of `(dest, flag)` pairs in definition order
  (`--work`, `--kind`, `--category`, `--tier`, `--question`, the pick flag) beside the
  `add_argument` calls. In `cmd_record`:
  - `--stdin` with any of the eight text values not `None`: `usage_error("give the texts either on stdin (--stdin) or as flags, not both")`;
  - `--stdin`: read the object (the rules above), check `kind`/`tier`, then build `line` from it;
  - no `--stdin`: when any required flag is missing,
    `usage_error("the following arguments are required: " + ", ".join(<missing flags>))`, which is
    argparse's own message, so the flag form behaves as before. `--kind`/`--tier` keep their
    `choices`.
  Put the stdin reading in one helper, `stdin_object(usage_error, required, optional=())`, used by
  both commands, returning a dict of the keys given.
- Rust (`builder/cli/src/record.rs`): in `record_command`, add
  `Arg::new("stdin").long("stdin").action(ArgAction::SetTrue).help("Read the texts as one JSON object from stdin")`;
  each of the eight text args gets `.conflicts_with("stdin")`, and the six required ones use
  `.required_unless_present("stdin")` instead of `.required(true)`. The existing
  option-like-value loop stays for the flag form (it only sees flags given). When `stdin` is set,
  deserialize the object into

  ```rust
  #[derive(Deserialize)]
  #[serde(deny_unknown_fields)]
  struct RecordInput {
      work: String,
      kind: String,
      category: String,
      tier: String,
      question: String,
      pick: String,
      rule: Option<String>,
      answer: Option<String>,
  }
  ```

  and build the same `row` from it. Refactor so both forms fill one set of values and the row is
  built once (no copy of the row-building code).
- Shared Rust reading: a new `builder/cli/src/stdin_json.rs` with
  `pub fn read<T: DeserializeOwned>(command: &clap::Command) -> T` doing rules 1 to 4 and exiting
  through `command.error(..).exit()` on any refusal. `similar` uses a `SimilarInput { query: String }`
  with `deny_unknown_fields`; for its error, use the `similar` subcommand from `Cli::command()`
  (`find_subcommand("similar")`), so the usage line is the real one.
- Update the usage block at the top of `scripts/crew_decisions.py`:
  `similar [--corpus decisions.jsonl] (--query TEXT | --stdin) [--k 6] [--json]` and the record
  lines with `(--work … --<member>-pick P [--rule R] [--<principal>-answer A] | --stdin)`, plus one
  indented line: `--stdin reads the texts as one JSON object, so the shell never reads them.`

## The template and the brief that use it

### `crew/_template/SKILL.md`

§2 step 2 becomes (three-space indentation as now, the fence included):

````
2. **Precedents.** Find {{PRINCIPAL}}'s own past answers to questions like it:

   ```bash
   {{SIMILAR}} --stdin <<'END_OF_TEXT'
   {"query": "<the question and its options>"}
   END_OF_TEXT
   ```

   The text goes in as one line of JSON between the `END_OF_TEXT` lines, where the shell never reads
   it: write `"` as `\"`, `\` as `\\` and a line break as `\n`, and leave `$`, backticks and quotes
   as they are. Never put a text on the command line itself.
   It searches the private history in `~/.claude/state/{{SLUG}}/`. Never quote what it prints
   anywhere but this chat with {{PRINCIPAL}}.
````

§3, the decided block becomes:

````
  ```bash
  {{RECORD}} --stdin <<'END_OF_TEXT'
  {"work": "<what you are working on>", "kind": "decided", "category": "<c>", "tier": "high", "question": "<q>", "pick": "<pick>"}
  END_OF_TEXT
  ```
````

and the escalated block:

````
  ```bash
  {{RECORD}} --stdin <<'END_OF_TEXT'
  {"work": "<work>", "kind": "escalated", "category": "<c>", "tier": "<t>", "rule": "<rules>", "question": "<q>", "pick": "<pick>", "answer": "<their answer>"}
  END_OF_TEXT
  ```
````

After the escalated block, before "Say it in one line…", add the bullet:
`- Write the texts as in §2 step 2: JSON on one line between the \`END_OF_TEXT\` lines, never on the command line.`
Change nothing else in the template. `--{{SLUG}}-pick` and `--{{PRINCIPAL_SLUG}}-answer` no longer
appear in it; that is intended.

### `crew/_template/briefs/backtest-brief.md`

The guided build's backtest sub-agent runs the same look-up with a held-out question's text, so it
gets the same form. Replace the bullet

```
- the precedent search, run EXACTLY as:
  `python3 -I … similar --corpus ~/.claude/state/{{SLUG}}/decisions.jsonl --query "<question and options>"`
  (it searches the training split only, so it never shows a held-out answer)
```

with

````
- the precedent search, run EXACTLY as (the question goes in as one line of JSON, where the shell
  never reads it: `"` as `\"`, `\` as `\\`, a line break as `\n`):

  ```bash
  python3 -I {{REPO}}/scripts/crew_decisions.py --member {{REPO}}/crew/{{SLUG}}/member.json similar --corpus ~/.claude/state/{{SLUG}}/decisions.jsonl --stdin <<'END_OF_TEXT'
  {"query": "<question and options>"}
  END_OF_TEXT
  ```

  (it searches the training split only, so it never shows a held-out answer)
````

Do **not** change `crew/_template/briefs/panel-seat.md` or `persona-backtest-brief.md`: they run
`scripts/crew_personas.py`, another program, outside this slice.

## Tests

Hostile text used below (call it `HOSTILE`), as a Rust/Python string literal: it holds
`$(touch pwned-dollar)`, `` `touch pwned-tick` ``, `${HOME}`, `"double"`, `'single'`, `back\slash`,
`café`, a line break, and a line that is exactly `END_OF_TEXT` (so: `…\nEND_OF_TEXT\n…`). Filling a
template placeholder inside JSON quotes means writing the text JSON-escaped without its outer quotes
(Python `json.dumps(t, ensure_ascii=False)[1:-1]`, Rust `serde_json::to_string(t)` without the first
and last character).

1. **The app's SKILL.md runs the texts as text** (`builder/cli/tests/assistants.rs`, extend
   `skill_commands_run_similar_and_record_against_private_state`): take each fenced `bash` block of
   the made `SKILL.md` that holds `--stdin` (the similar block and both record blocks), remove the
   block's common indentation (as Claude does when it reads the markdown), replace the word with the
   test binary as today, fill the placeholders with JSON-escaped text (the query and every record
   text include `HOSTILE`; the similar query also includes `durable garden benches`), and run each
   with `/bin/sh -c` with `HOME` set and `current_dir` an empty folder made for the test. Then:
   no `pwned-dollar` or `pwned-tick` exists in that folder or in the home; similar's output contains
   the invented precedent; each record line's `question`, `work`, `ari_pick` (and in the escalated
   one `rule` and `robin_answer`) equal the texts exactly, `END_OF_TEXT` line and all. Update the
   two `contains` checks in `new_list_and_agree_use_the_claude_folders_and_final_wording` and in
   `builder/api/tests/assistants.rs` to the new lines
   (`{word} similar ~/.claude/crew/<slug> --stdin <<'END_OF_TEXT'` and
   `{word} record ~/.claude/crew/<slug> --stdin <<'END_OF_TEXT'`), and the template checks in
   `builder/core/src/crew.rs` and `builder/core/tests/install_parity.rs`.
2. **The guided build's SKILL.md does the same with Python** (`tests/crew_install_test.sh`): just
   before the final `echo "crew_install_test: all passed"`, a `python3 -I - … <<'PYSRC'` block that
   does test 1's steps on `$main/projects/voice/crew/ari/SKILL.md`, running each block with
   `HOME="$main"` and `cwd` a new empty folder under `$scratch`; the similar query includes words
   of a question known to be in `$main`'s merged corpus (pick one from the transcripts the test
   writes) and the output must contain it. Records go to `$main/.claude/state/ari/live.jsonl`
   (nothing later in the script reads it). Then the same no-`pwned-*` and exact-text checks.
   Do it the same way for `$main/projects/voice/crew/ari/briefs/backtest-brief.md`'s block.
3. **Rust and Python agree on `--stdin`** (`builder/cli/tests/record.rs`, beside
   `records_match_python_bytes_with_and_without_optional_fields`, with the same Python-missing
   rule): the same stdin object (with `HOSTILE` in every text), once with `rule`/`answer` and once
   without, gives the same line bytes (timestamp excepted) from Rust `record --stdin` and Python
   `record --log <tmp> --stdin`, and that line equals the flag form's line for the same values.
   A new `builder/cli/tests/similar_stdin.rs` (same rule, follow `record.rs` for the home): Rust
   `similar --stdin`, Rust `similar --query` and Python `similar --stdin` print the same bytes on an
   invented `decisions.jsonl`, with and without `--json`.
4. **Refusals, Rust** (`builder/cli/tests/record.rs` and `similar_stdin.rs`), each exit 2 with
   nothing written (`live.jsonl` absent or unchanged): `--stdin` with `--work`; neither `--stdin`
   nor flags; stdin not JSON; a JSON array; an unknown key (stderr names it); a repeated key; a
   missing `pick`; `pick` a number; `rule` a number; `kind` `maybe`; `tier` `top`; 65537 bytes;
   invalid UTF-8 bytes; `{"query": …}` with an extra key and with `--query` too. `rule` and
   `answer` given as `null` are fine.
5. **Refusals, Python** (`tests/crew_decisions_test.py`, beside its record and similar tests): the
   same cases, plus a lone surrogate `{"…": "\ud800"}` refused; the flag form still prints
   argparse's `the following arguments are required: --work, …` when flags are missing.
6. Every existing test keeps passing (update only lines that asserted the old template text or the
   old help text).

## Records

- `.ai/SECURITY.md`: after the PB8.1 paragraph (`persona-builder record` writes only into an
  existing state folder…), add: "The texts an assistant records or looks up never pass through the
  shell: the SKILL.md (and the guided build's backtest brief) gives them to `--stdin` as one JSON
  object inside a quoted heredoc, which the shell does not expand and valid JSON cannot end early.
  Both programs read at most 64 KiB, strict UTF-8, one object with only the known keys; anything
  else is a usage error and nothing is written. `scripts/crew_personas.py similar` (the persona
  panel briefs) still takes its query on the command line."
- `.ai/DECISIONS.md`: after the PB8.1 entry, in its format:
  `## 2026-10-26: texts go to the decision commands on stdin (PB8.3)` and the paragraph:
  "The SKILL.md's look-up and record commands take their texts as one JSON object on stdin
  (`--stdin`, a quoted heredoc), never as shell words, in the app's `persona-builder` and the
  guided build's Python alike; the flags stay for people typing the commands. JSON over one field
  per line because a text can hold line breaks and a line that matches the heredoc's end. An
  assistant made before this keeps its old SKILL.md; make it again to get the new one."

## Done means

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`PERSONA_BUILDER_REQUIRE_PARITY=1 cargo test --locked`, `python3 tests/crew_decisions_test.py`,
`bash tests/crew_install_test.sh`, and `shellcheck tests/crew_install_test.sh` all pass. Then
`bash scripts/ci-local.sh` passes. Commit nothing.
