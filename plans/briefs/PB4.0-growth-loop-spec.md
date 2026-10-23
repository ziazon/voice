# Brief PB4.0: the growth loop's schemas, examples and their test

You are working in a git worktree of a public repo (an open-source kit for building AI assistants).
Work only inside this worktree. Do not commit; leave the changes in the working tree.

Read first, in this order: `AGENTS.md`, `CONTRIBUTING.md`, `.ai/PROJECT.md`, then the spec this brief
implements, `docs/spec/growth-loop.md` (already written; do not change its prose). Work test-first
and match the surrounding code.

## Goal

The spec defines five record kinds (signal, case, proposal, approval, measurement), each with a
field table and one inline JSON example. Make the spec checkable:

1. one JSON Schema per record kind,
2. each inline example stored as a file,
3. a test, wired into `scripts/ci-local.sh`, that proves every example validates against its schema,
   that the files match the doc, and that the examples obey the spec's cross-record rules.

## Hard constraints

- **Python 3.9+ standard library only.** No `jsonschema` package, no pip, no new dependency. CI is
  `bash scripts/ci-local.sh` and needs only Python and shellcheck.
- **No real people.** Only the invented persona in the spec (Robin, persona id `robin`).
- **No fixed paths.** Find files from the test's own location (`Path(__file__).resolve()`).
- Match the existing tests' style: `unittest`, run as `python3 tests/<name>_test.py`, `if __name__ == "__main__": unittest.main()`.

## Files to create

### `docs/spec/growth-loop/<kind>.schema.json` (five files: signal, case, proposal, approval, measurement)

JSON Schema draft 2020-12 (`"$schema": "https://json-schema.org/draft/2020-12/schema"`), no `$id`,
a `title`, a one-line `description`. Use **only** these keywords: `$schema`, `title`, `description`,
`$defs`, `$ref` (local `#/$defs/<name>` only), `type`, `properties`, `required`,
`additionalProperties` (always `false` on every object), `items`, `minItems`, `uniqueItems`, `enum`,
`const`, `pattern`, `minLength`.

Encode exactly the doc's field tables:

- every row with *Required: yes* is in `required`; every row is in `properties`; nothing else is allowed;
- `schema`: `const` with the value the table gives (for example `"growth-loop/signal/v1"`);
- ids: `type: string`, `pattern` `^<prefix>-[a-z0-9]+(-[a-z0-9]+)*$` with the prefix from the doc
  (`sig`, `case`, `prop`, `appr`, `meas`). Fields that reference another record use that record's id
  pattern (`from_signals` items use the `sig` pattern, `cases` items the `case` pattern, `proposal`
  the `prop` pattern, `approval` the `appr` pattern, `results[].case` the `case` pattern);
- `persona`: `pattern` `^[a-z0-9]+(-[a-z0-9]+)*$`;
- times (`at`, `created`): a `$defs/timestamp` string with `pattern`
  `^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})$`;
- enums exactly as the doc lists them (`given_by`, `kind`, `verdict`, `match`, `status`,
  `persona_kind`, `role`, `decision`, `before`, `after`, `outcome`);
- free-text strings (`area`, `situation`, `wanted`, `persona_said`, `source`, `expected`, `summary`,
  `why`, `diff`, `base`, `candidate`, `note`, `reason`, each `options` item): `type: string`,
  `minLength: 1`;
- arrays: `from_signals`, `changes`, `cases`, `results` have `minItems: 1`; `options` has
  `minItems: 2`; id arrays and `options` have `uniqueItems: true`;
- `changes[].file`: a relative path inside the persona folder: `pattern`
  `^(?!/)(?!.*(^|/)\.\.(/|$))[^\\]+$` (no leading slash, no `..` segment, no backslash);
- `results[].new`: `type: boolean`.

### `docs/spec/growth-loop/examples/<kind>.json` (five files)

Each is the doc's inline example for that kind, byte-for-byte the same JSON value (formatting may
differ: 2-space indent, trailing newline).

### `tests/schema_subset.py`: a small validator for exactly the keyword list above

- `validate(instance, schema) -> list[str]`: returns error strings, empty when valid. Each error
  starts with a JSON-pointer-ish path to the failing value (`/changes/0/file: …`).
- `$ref` resolves only `#/$defs/<name>` against the root schema; any other `$ref` raises
  `UnsupportedSchema`.
- **Any keyword not in the list raises `UnsupportedSchema` naming it** (so a schema can never rely on
  a keyword this validator silently ignores). `$schema`, `title` and `description` are accepted and
  ignored.
- `type` accepts a string or a list. `integer` and `number` must reject `bool` (Python's `True` is an
  `int`). `pattern` uses `re.search`, as JSON Schema does.
- `uniqueItems` compares values as JSON (use `json.dumps(v, sort_keys=True)`).

### `tests/growth_loop_spec_test.py`

Test-first: write it before the schemas and the validator, run it, and see it fail on the missing
files before making it pass. Tests:

1. **Validator unit tests** (one small inline schema each): passes a valid value; reports a missing
   required field; reports an extra property under `additionalProperties: false`; reports a wrong
   `enum`/`const`; reports a `pattern` miss; reports `minItems` and `uniqueItems`; rejects `true` for
   `integer`; resolves a `$defs` `$ref`; raises `UnsupportedSchema` for an unknown keyword (for example
   `format`) and for a non-local `$ref`.
2. **Each example validates** against its schema (subTest per kind), with zero errors.
3. **Each schema rejects a broken copy of its example**: for each kind, (a) drop one required field,
   (b) add an unknown field `"extra": 1`, and assert at least one error each.
4. **Schemas follow the doc**: for each kind, parse the doc's field table under the
   `### <n>. <Kind>` heading (rows `| \`field\` | yes/no | … |`) and assert the schema's `properties`
   keys equal the table's fields and its `required` equals the *yes* rows.
5. **The doc's inline examples equal the files**: for each kind, find the line
   ``Example (`growth-loop/examples/<kind>.json`):``, take the next fenced ```` ```json ```` block,
   `json.loads` it, and assert it equals the file's parsed JSON.
6. **The examples obey the cross-record rules** (spec rules 2–5):
   - the case's `from_signals` contains the signal's `id`; the proposal's `cases` contains the
     case's `id`; the approval's `proposal` and the measurement's `proposal` equal the proposal's
     `id`; the measurement's `approval` equals the approval's `id`; `persona` is the same on the
     signal, case and proposal; the measurement's `base` equals the proposal's `base`;
   - the approval's `role` is the one the proposal's `persona_kind` requires (`person` → `subject`,
     `product` → `staff`);
   - the set of `results[].case` with `new: true` equals the proposal's `cases`;
   - a helper `expected_outcome(measurement, approval)` implementing rules 3 and 4 (`keep` only when the
     approval's `decision` is `approved`, every `new: true` result has `after: pass`, and no
     `new: false` result went `pass` → `fail`; else `revert`) returns the example's `outcome`; and it
     returns `revert` for three mutated copies: a regression (`before: pass, after: fail` on an old
     case), a new case still failing, and `decision: rejected`.
7. **Every schema file parses and uses only supported keywords**: validating any example already
   proves this; also assert the five schema files and five example files exist and nothing else is in
   those two folders (so a stray file is caught).

### `scripts/ci-local.sh`

Add `growth_loop_spec` to the `for t in …` list of Python tests (last in the list). Change nothing
else in it.

### `.ai/PROJECT.md`

Add one row to the table, after the `slack_*` row:

`| \`docs/spec/growth-loop.md\` | The growth loop spec any persona app implements, with its JSON Schemas | \`tests/growth_loop_spec_test.py\` |`

## Done when

- `python3 tests/growth_loop_spec_test.py` passes, and it failed before the files existed.
- `bash scripts/ci-local.sh` passes end to end.
- Mutation check, reported in your summary: temporarily change one example's `"kind": "correction"`
  to `"kind": "complaint"` and confirm test 2 fails; temporarily delete one row from the doc's signal
  table and confirm test 4 fails; restore both with a `cp` backup and confirm with `cmp`.
- Your final message lists every file created or changed, and the test output.
