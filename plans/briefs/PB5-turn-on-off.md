# PB5 brief: turn it on and off, the claude.ai Project pack, forget

Persona Builder is a standalone app that builds a person's AI assistant from their own Claude
history. PB1 gave `builder/core` (library `persona_builder`) the member format: `crew/<slug>/`
with `member.json`, `IDENTITY.md`, `SKILL.md`, `autonomy.md` and `decisions.md`. This slice puts a
built assistant to work and takes it away again:

- **On and off** (part A): copy the assistant's skill into the user's Claude Code folder and add
  one import line to their `CLAUDE.md`, recorded so that turning it off removes exactly what was
  added and nothing else.
- **The Project pack** (part B): the same assistant as files the user uploads to a claude.ai
  Project by hand.
- **Forget** (part C): delete everything the assistant learned from (the private history and the
  build's drafts), never the assistant's own files.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`,
`plans/briefs/PB1-member-format.md`, `builder/core/src/{crew,member,autonomy,error,json}.rs`,
`builder/core/src/test_support.rs`, `builder/core/tests/common/mod.rs`, `builder/cli/src/main.rs`,
`builder/cli/tests/member_show.rs`, `crew/_template/{SKILL,IDENTITY,autonomy,decisions}.md`,
`bootstrap/PROMPT.md` step 13 (what the guided build does by hand today). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

Work test-first: write each test, run it and see it fail on its assertion, then write the code.
You will be dispatched once per part; do only the part your dispatch names, then stop.

## Sources (checked 2026-10-24)

- Claude Code memory docs, <https://code.claude.com/docs/en/memory>: the user's own instructions
  are `~/.claude/CLAUDE.md`; "CLAUDE.md files can import additional files using `@path/to/import`
  syntax"; "Both relative and absolute paths are allowed"; a home path is written `@~/.claude/…`;
  "To import a file whose path contains spaces, put a backslash before each space"; "Import
  parsing skips Markdown code spans and fenced code blocks"; "Block-level HTML comments … are
  stripped before the content is injected"; imports in user-scope files load without an approval
  dialog. `CLAUDE_CONFIG_DIR` moves the folder `~/.claude` stands for.
- Claude Code skills docs, <https://code.claude.com/docs/en/skills>: personal skills live at
  `~/.claude/skills/<skill-name>/SKILL.md`, are picked up without a restart, and a `name` holding
  `<` or `>` or control characters does not load (our names are slugs, so they never do).
- claude.ai help, "How can I create and manage projects?"
  (<https://support.claude.com/en/articles/9519177>): "+ New Project", "Set project instructions",
  and the "+" button to add files to the project's knowledge.
- Flightdeck's managed block (same author, MIT): `cli/src/adapters/block.rs` in
  `ziazon/flightdeck`. Part A ports its rules, quoted in A1, so you do not need that repo.

## Rules

- Public repo: no real person's name, handle, path or data anywhere. Invented names only: the
  assistant **Ari** (they/them) for the principal **Robin** (they/them), and **Mo Lee** for
  Robin's second assistant in the two-persona test. No fixed home paths: every test path is under
  a scratch folder, and every test that runs the binary sets `HOME` and `CLAUDE_CONFIG_DIR` on the
  child. Never touch the real `~/.claude`.
- Stable Rust, edition 2024, the workspace's settings. No new dependency (no `sha2`, no
  `tempfile`, no `dirs`): `std`, `serde`, `serde_json` and `clap` only. `cargo fmt`,
  `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` and
  `bash scripts/ci-local.sh` must pass. Your sandbox has no network: pass `--offline` if cargo
  tries the registry.
- The library does no network and starts no process. Only the CLI reads environment variables.
- Do not change `scripts/*.py`, `bootstrap/*` or `crew/_template/*`.
- Every directory this slice creates is mode 0700 and every file 0600 (create them with
  `std::os::unix::fs::{DirBuilderExt, OpenOptionsExt}`, `create_new(true)` for new files), except
  where A5 says to keep an existing file's mode.
- **Symlinks:** refuse (an `Error::Invalid` naming the path, ending `: symlink refused`, as
  `member::refuse_symlink` does) any symlink at a path this slice creates, removes or records. The
  only symlinks followed are the user's own folders above them (the Claude Code folder itself,
  its `skills/` folder, the home folder) and `CLAUDE.md` as A5 says. Use
  `member::refuse_symlink`; make it `pub(crate)` if it is not.
- **All or nothing:** a call that fails leaves the disk as it found it. Each writing function keeps
  a list of what it created or changed in this call and undoes it, newest first, before returning
  the error (`turn_on`, `pack`).
- User-facing text (CLI output, `RULES.md`, the pack's text) is given below word for word. Do not
  reword it. Error messages you write yourself are short, plain, and say what to do next.
- Make `crew::fill` `pub(crate)` and reuse it for every `{{KEY}}` placeholder below.

## Part A: on, off, status

### A1. `builder/core/src/block.rs`: a managed block in a file the user owns

A port of Flightdeck's `block.rs` with the marker `<!-- persona-builder:` in place of
`<!-- flightdeck:`. Pure text in, text out:

- `pub fn upsert(text: &str, id: &str, body: &str) -> Result<String>` and
  `pub fn remove(text: &str, id: &str) -> Result<String>` (errors are `Error::Invalid`), plus
  `pub fn find(text: &str, id: &str) -> Result<Option<(usize, usize)>>`.
- The block is `<!-- persona-builder:begin <id> -->`, the body's lines, then
  `<!-- persona-builder:end <id> -->`, each on its own line. Markers are matched on a line's
  trimmed text.
- `upsert` replaces the block where it is, else appends it: to an empty text as the whole text,
  else after the existing text plus one line break. So `remove` after an append gives back the
  original bytes exactly, with or without a trailing newline in the original.
- A new block in a text holding any `\r\n` uses CRLF; a block already there keeps its own line
  ends. A body's trailing empty line is dropped, and `\r` is trimmed from each body line.
- `remove` of a block that ends the text also takes the one line break before it (in the block's
  own line end); a block with text after it takes only its own lines. No block: the text as is.
- Refused: an id that is not 1 to 64 bytes of `a-z`, `0-9`, `-` starting with a letter or digit;
  a body holding `<!-- persona-builder:`; the same block twice; an end with no begin; a begin with
  no end; any other `<!-- persona-builder:` marker inside the block.

### A2. Where things go

```rust
pub struct Places { pub home: PathBuf, pub claude_home: PathBuf }
```

`home` is the user's home folder; `claude_home` is Claude Code's folder (`~/.claude` unless
`CLAUDE_CONFIG_DIR` says otherwise). For a member with slug `<s>`, turning on adds:

1. `<claude_home>/skills/<s>/SKILL.md`: the bytes of `crew/<s>/SKILL.md` (it is already filled).
2. `<claude_home>/persona-builder/<s>/RULES.md`: the text in A4.
3. A block with id `<s>` in `<claude_home>/CLAUDE.md` whose body is one line, `@<import path>`.
   The import path is the absolute path of the `RULES.md` in step 2; when it is under `home`, it
   is written `~/<the rest>` instead. Every space in it is written `\ ` (backslash, space).
4. The record, `<claude_home>/persona-builder/installed.json` (A3), written last.

plus whichever of the folders `<claude_home>/skills`, `<claude_home>/skills/<s>`,
`<claude_home>/persona-builder` and `<claude_home>/persona-builder/<s>` did not exist before.
Only one assistant is on per Claude Code folder.

### A3. The record

`installed.json`, written with `json::to_python_json` through a temporary file in the same folder
(`.installed.json.tmp-<pid>`, 0600, `create_new`) renamed over it:

```json
{
  "slug": "ari",
  "name": "Ari",
  "member_dir": "/absolute/canonical/path/crew/ari",
  "claude_md_created": false,
  "added": [
    {"kind": "dir", "path": "/…/skills/ari"},
    {"kind": "file", "path": "/…/skills/ari/SKILL.md", "text": "<the exact text written>"}
  ]
}
```

`added` lists, in creation order, every folder created and every file written by this turn-on
(never `CLAUDE.md`, never the record itself). A `file` entry carries the exact text written, so
turning off can tell whether the user changed it. Make it a `#[derive(Serialize, Deserialize)]`
struct with an `enum Added { Dir { path }, File { path, text } }` tagged by `kind` (lowercase).
Reading a record refuses a symlink and names the file when it is not valid JSON in this shape.
Every path in `added` must be under `claude_home`, or the record is refused (a hand-edited record
must not make turning off delete files elsewhere).

### A4. `RULES.md`

An embedded constant filled with the member's values (`NAME`, `SLUG`), followed by one newline:

```markdown
# {{NAME}}

Before you ask me a question (a choice between options, a go or no-go, a plan's open questions),
ask {{NAME}} first with the /{{SLUG}} skill. {{NAME}} says what I would likely pick and how sure
that answer is. Ask me only what {{NAME}} hands back, and show me {{NAME}}'s answer beside the
question.
```

### A5. `pub fn turn_on(member_dir: &Path, places: &Places) -> Result<TurnedOn>`

1. `MemberDir::open(member_dir)`. The slug must pass A1's id rule.
2. `claude_home` must be a folder (following a symlink there is fine). If it is missing:
   `Invalid("<path> is missing: install Claude Code and sign in to it first")`. Canonicalize
   `home` and `claude_home` once here and use the canonical paths everywhere after (the "under
   `home`" test in A2 and the "under `claude_home`" test in A3 compare canonical paths;
   `turn_off` and `status` canonicalize the same way).
3. Read the record if there is one. If its slug differs:
   `Invalid("<record's name> is on. Turn <record's name> off first: persona-builder off")`, with
   nothing changed. If its slug is the same, turn that one off first (A6) and carry on: turning
   on again refreshes the files.
4. Check everything before writing anything:
   - `skills/<s>` and `persona-builder/<s>` do not exist (`symlink_metadata`); otherwise
     `Invalid("<path> exists and Persona Builder did not add it. Move it away, then turn <Name> on again")`;
   - `CLAUDE.md` is missing, a regular file, or a symlink whose target is a regular file (resolve
     it with `canonicalize`); its text is UTF-8; and `block::find` with id `<s>` finds no block
     and no error. A block already there that no record names:
     `Invalid("<CLAUDE.md path> already has a Persona Builder line for <s>. Remove the lines between its persona-builder markers, then turn <Name> on again")`.
5. Write steps 1 to 3 of A2 in order, creating the missing folders, then the record. To change
   `CLAUDE.md`, write the new text to a temporary file beside the **resolved** target
   (`.CLAUDE.md.persona-builder-<pid>`, `create_new`), give it the target's permission bits, and
   rename it over the resolved target, so a symlinked `CLAUDE.md` stays a symlink. A missing
   `CLAUDE.md` is created 0600 with only the block, and `claude_md_created` is `true`.
6. On any error after the first write, undo this call's writes newest first (restore the original
   `CLAUDE.md` bytes the same way, delete created files and folders), then return the error.
7. Return `TurnedOn { name, slug, skill: PathBuf, claude_md: PathBuf, import_line: String }`.

### A6. `pub fn turn_off(places: &Places) -> Result<TurnedOff>`

1. No record: `TurnedOff { name: None, left: vec![] }`.
2. Remove the block `<s>` from `CLAUDE.md` (resolved as in A5, written the same way). If the
   record says `claude_md_created` and the text left is empty, delete the file (it is a regular
   file then; a symlink is never deleted). A `CLAUDE.md` or block that is already gone is fine.
3. For each `file` in `added`, newest first: missing, nothing to do; a symlink, leave it and note
   it; text equal to the record's `text`, delete it; otherwise leave it and note it (the user
   changed it).
4. Delete the record.
5. For each `dir` in `added`, newest first: remove it only when it is empty (`remove_dir`); a
   folder that still holds something is left and noted. Never `remove_dir_all` here.
6. Return `TurnedOff { name: Some(name), left: Vec<Left> }` where `Left { path, why }` and `why`
   is `"you changed it after it was turned on"`, `"it is a symlink"` or
   `"it still holds other files"`.

A block that is malformed in `CLAUDE.md` (A1's errors) stops turning off before anything is
deleted, with the error naming the file.

### A7. `pub fn status(places: &Places) -> Result<Option<Status>>`

`None` when there is no record. Otherwise `Status { name, slug, member_dir, line: bool, files:
Vec<(PathBuf, FileState)>, out_of_date: bool }`: `line` is whether the block is in `CLAUDE.md`;
`FileState` is `Present`, `Changed` or `Missing` against the record's text; `out_of_date` is true
when `member_dir/SKILL.md` is missing or differs from the skill's recorded text.

### A8. CLI

Add to `builder/cli/src/main.rs` (keep `member show` as it is):

- `persona-builder on <MEMBER_DIR> [--claude-home <DIR>]`
- `persona-builder off [--claude-home <DIR>]`
- `persona-builder status [--claude-home <DIR>]`

`--claude-home` defaults to `$CLAUDE_CONFIG_DIR` when set and not empty, else `$HOME/.claude`.
`home` is `$HOME`; when it is unset or empty, fail with `persona-builder: HOME is not set`.
Errors print `persona-builder: <error>` on stderr and exit 1, as today. Output on stdout, word for
word (`<…>` filled in; paths printed with `display()`):

- `on`:
  ```
  <Name> is on in Claude Code.
  Added the skill /<slug> in <skill folder>.
  Added one line to <CLAUDE.md path>:
    @<import path>
  Start a new Claude Code session to use <Name>. To turn <Name> off: persona-builder off
  ```
- `off` with nothing on: `No assistant is on in Claude Code.` Otherwise
  `<Name> is off. Everything Persona Builder added is removed.` or, when something was left,
  `<Name> is off. These were left in place:` followed by one line per item,
  `  <path>: <why>`.
- `status` with nothing on: `No assistant is on in Claude Code.` Otherwise:
  ```
  <Name> is on in Claude Code.
  The line in CLAUDE.md: there | missing
  <path>: there | changed | missing        (one line per recorded file)
  ```
  and, when `out_of_date`, a last line
  `<Name> has changed since it was turned on. To update it: persona-builder on <member_dir>`.

### A9. Tests (red first), `builder/core/src/` unit tests and `builder/cli/tests/turn_on.rs`

Build the member with `crew::new_member` from `crew/_template` (Ari for Robin) into a scratch
folder; the Claude Code folder is `<scratch>/home/.claude`.

1. `block`: append to empty, to text with and without a trailing newline, to CRLF text; replace in
   place; `remove` gives back the original bytes in each case; each refusal in A1.
2. Turning on into an empty Claude Code folder: the skill's bytes equal `crew/ari/SKILL.md`;
   `RULES.md` is exactly A4's text; `CLAUDE.md` is exactly
   `<!-- persona-builder:begin ari -->\n@~/.claude/persona-builder/ari/RULES.md\n<!-- persona-builder:end ari -->\n`;
   the record parses; every created folder is 0700 and file 0600.
3. On then off, with a `CLAUDE.md` holding user text (one case with a trailing newline, one
   without, one CRLF): `CLAUDE.md` is byte-identical to before, `skills/ari` and
   `persona-builder` are gone, and a `skills/other/SKILL.md` that was there before is untouched.
4. A `CLAUDE.md` that turning on created is deleted by turning off; if the user added a line to it
   meanwhile, it is kept with only their line.
5. Mo Lee cannot be turned on while Ari is on; nothing changes on disk (compare a listing of the
   whole Claude Code folder with its file bytes before and after).
6. An existing `skills/ari` folder that no record names is refused; nothing changes on disk.
7. A symlink at `skills/ari`, at `installed.json`, and at a recorded file: each refused or left as
   A5 and A6 say, and the symlink's target is untouched.
8. A symlinked `CLAUDE.md`: the target gains the block and the link is still a link; off restores
   the target's bytes.
9. A user-changed `SKILL.md` is left in place by off and reported with its `why`.
10. Turning Ari on twice: one block, and the skill holds the member's new `SKILL.md` after it is
    edited between the two.
11. Rollback: make `<claude_home>/persona-builder` a regular file, turn on, get an error, and find
    the disk exactly as before (no `skills/ari`, `CLAUDE.md` unchanged).
12. A hand-edited record whose `added` names a file outside `claude_home` is refused by off, and
    that file is untouched.
13. Status: on; a missing file; a changed file; the line removed by hand; an out-of-date member.
14. Import paths: a home folder with a space gives `@~/…` with `\ `; a Claude Code folder outside
    the home folder gives the absolute path.
15. CLI: `on`, `status`, `off` print A8's text exactly; `--claude-home` and `CLAUDE_CONFIG_DIR`
    each pick the folder; an unset `HOME` gives the A8 error.

Mutation checks (part A): drop the "one line break before the block" in `upsert`; drop the
content comparison in A6 step 3; skip the "under `claude_home`" check in A3. Each must turn a named
test red.

## Part B: the claude.ai Project pack

### B1. `pub fn pack(member_dir: &Path, out: &Path) -> Result<Vec<PathBuf>>`

Writes `instructions.md` and one `decisions-<category>.md` per area into `out`, and returns the
paths written in that order. The output is a pure function of the member's files: the same files
give the same bytes, with `\n` line ends and one trailing newline per file.

- `out` must not exist, or be an empty folder that is not a symlink. Create it 0700 when missing.
  On any error, delete what this call wrote (and `out` if it created it).
- **Areas.** In `decisions.md`, the section `## By kind of question` holds one `### <category>`
  section per area, each running to the next `### ` or `## ` heading or the end. An area is
  included when its body has any non-blank line. A category must be 1 to 64 bytes of `a-z`, `0-9`
  and `-`, and appear once: otherwise
  `Invalid("decisions.md: \"<heading text>\" is not a kind of question (lower-case letters, digits and -)")`
  or `Invalid("decisions.md: <category> appears twice")`. With no area:
  `Invalid("<Name> has no kinds of question yet. Build <Name> first, then make the pack")`,
  and nothing is written. An area the user deleted from `decisions.md` is simply absent.
- **`decisions-<category>.md`:** `# How <Principal> decides: <category>`, a blank line, then the
  area's body with leading and trailing blank lines trimmed.
- **`instructions.md`**, in this order, blank line between parts:
  1. `# <Name>`
  2. `IDENTITY.md` from the line after its `# ` heading to the end, with leading and trailing blank
     lines trimmed, with: any `## ` section whose heading starts `## Where ` dropped (to the next
     `## ` heading or the end); every `## ` heading turned into `### `; and the link
     `[autonomy.md](autonomy.md)` replaced by `the list under "What <Principal> decides"`.
  3. The embedded "how to answer" text below, filled (`NAME`, `PRINCIPAL`).
  4. `## What <Principal> decides`, then
     `<Principal> always decides these. Say what <Principal> would likely pick and why, then leave the choice to <Principal>:`,
     a blank line, one `` - `<key>`: <text> `` line per `[x]` dial in file order (from
     `MemberDir::autonomy()`), a blank line,
     `You may decide these yourself, but only when you are at High:`, a blank line, one line per
     `[ ]` dial. An empty list is the line `- none`.
  5. `## How often <Name> has been right`, a blank line, then the lines starting `|` in
     `decisions.md`'s `## Calibration (the blind backtest)` section. When there are fewer than
     three such lines (no rows under the header), the line
     `Not measured yet. Treat every answer as Medium at most.` instead.
  6. `## Kinds of question`, a blank line,
     `Each kind has its own file in this Project's knowledge:`, a blank line, one
     `` - `decisions-<category>.md` `` line per area in order.

The embedded "how to answer" text:

```markdown
## How to answer

1. Find the kind of question. Each kind has a file in this Project's knowledge that says how
   {{PRINCIPAL}} usually leans, what changes the answer, and past examples.
2. Pick what {{PRINCIPAL}} would pick. A rule {{PRINCIPAL}} has stated comes first, then the past
   examples, then the usual lean.
3. Say how sure you are, in one word:
   - **High:** a stated rule settles it, or the examples agree with the usual lean, and the table
     below shows a good record on that kind of question.
   - **Medium:** the examples mostly agree, or there is no good record on that kind yet.
   - **Low:** few or no examples, or they disagree.
   Never more than Medium when {{PRINCIPAL}} would likely answer in their own words rather than
   pick one of the options.
4. Hand the question back when the list below says {{PRINCIPAL}} decides that kind, or when you
   are not at High. Say what {{PRINCIPAL}} would likely pick and why, and leave the choice to
   {{PRINCIPAL}}.

Answer in this shape:

{{NAME}} · <kind of question> · <High, Medium or Low>
Pick: <the option, or what {{PRINCIPAL}} would write instead>
Why: <one or two lines>
Yours to decide: no | yes, because <the reason>

When {{PRINCIPAL}} talks to you directly, answer in the first person, short and plain.
```

### B2. CLI

`persona-builder pack <MEMBER_DIR> --out <DIR>`. Output, word for word:

```
Made <Name>'s claude.ai Project pack in <out>:
  instructions.md
  decisions-<category>.md        (one line per area)
To use it: in claude.ai, make a new Project, paste instructions.md into "Set project
instructions", and add the other files to the Project's knowledge with the + button.
```

### B3. Tests (red first)

Fixture: Ari for Robin from the template, then `decisions.md` replaced by an invented one with a
calibration table of two rows and three areas (`scope`, `sequencing`, `writing-voice`), each with
a two-line invented body.

1. The exact `instructions.md` for the fixture, as a literal in the test, and the exact
   `decisions-scope.md`.
2. Two packs of the same member are byte-identical.
3. Deleting the `sequencing` section: no `decisions-sequencing.md`, and `instructions.md` does not
   name it.
4. No areas: refused with B1's message, and `out` does not exist afterwards.
5. A calibration table with only its header: the "Not measured yet" line.
6. A heading `### Not A Kind` and a repeated category: each refused, nothing written.
7. `out` non-empty, and `out` a symlink: each refused, untouched.
8. The pack never contains the text of `private.json` (put an invented marker string in one).
9. CLI: B2's text exactly.

Mutation checks (part B): include the `## Where ` section; drop the "body has a non-blank line"
rule. Each must turn a named test red.

## Part C: forget

### C1. `pub fn forget(member_dir: &Path, home: &Path, delete: bool) -> Result<Forgotten>`

Deletes the member's private folder: the history the assistant learned from and the build's
drafts. The assistant's own files in `member_dir` are never touched.

1. `member::read_member(member_dir.join("member.json"))` (so `private.json` applies).
2. Expand `state_dir`: a leading `~/` becomes `home` joined with the rest; an absolute path is
   used as is; anything else is refused. The expanded path, compared component by component, must
   equal `<home>/.claude/state/<slug>` exactly (no `.` or `..` components anywhere). Otherwise
   `Invalid("state_dir <state_dir as written> is not ~/.claude/state/<slug>, so Persona Builder will not delete it. Delete that folder yourself if you mean to")`.
3. Missing: `Forgotten { path, files: 0, existed: false }`. A symlink: refused. Not a folder:
   refused.
4. Count every entry that is not a folder, walking with `symlink_metadata` and never following a
   symlink (a symlink counts as one entry).
5. When `delete`, `std::fs::remove_dir_all(path)` (it removes symlinks inside without following
   them). Return `Forgotten { path, files, existed: true }`.

### C2. CLI

`persona-builder forget <MEMBER_DIR> [--yes]`. Output, word for word:

- Missing folder: `Nothing to forget: <path> is not there.`
- Without `--yes`:
  ```
  This deletes <N> files in <path>: the history <Name> learned from and the drafts of each build.
  <Name>'s own files in <member_dir> stay.
  To go ahead: persona-builder forget <member_dir> --yes
  ```
  and nothing is deleted.
- With `--yes`: `Deleted <N> files in <path>. <Name>'s own files in <member_dir> are untouched.`

### C3. Tests (red first)

1. Without `delete`: the count is right and nothing is deleted. With it: the folder is gone and
   `member_dir` is byte-identical. A second run: `existed: false`.
2. `state_dir` of `~`, `~/.claude/state`, `~/.claude/state/other`, `~/.claude/state/ari/..`,
   `state/ari` and `/tmp/x`: each refused, nothing deleted. The absolute
   `<home>/.claude/state/ari`: accepted.
3. `private.json` setting `state_dir` to `~`: refused (the override is read).
4. The state folder is a symlink: refused, its target untouched.
5. A symlink inside the state folder pointing at a file outside it: counted, removed, and the
   outside file is untouched.
6. CLI: C2's text exactly, with and without `--yes`.

Mutation check (part C): drop the component-by-component comparison. A named test must turn red.

## Docs (with part C)

- `.ai/PROJECT.md` table: the `builder/core/` row's "what" becomes `The Persona Builder core
  library (persona_builder): the member format, the dial, field packs, turning an assistant on and
  off in Claude Code, the claude.ai Project pack, forget`.
- `README.md`: no change (the app's own words come in PB7).

## Done when (each part)

The part's tests went red first and are green; its mutations each turned their named test red and
were restored (back up with `cp`, restore with `cp`, confirm with `cmp`; never `git checkout --`
or `git stash`); `bash scripts/ci-local.sh` passes with all Rust steps run. Report: the files
changed, each red you saw (test name and failing assertion), each mutation's red, and the
`ci-local.sh` tail. Do not commit; leave the changes in the worktree.
