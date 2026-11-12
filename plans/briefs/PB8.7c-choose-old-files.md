# PB8.7c: choose which of the older assistant's own files come in

Code change. Rust in `builder/core`, `builder/api`, `builder/app` and `builder/cli`; Vue and TypeScript in
`builder/ui`. Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md` (writing the person's Claude folder, never following a
link, all or nothing), `~/.codex/skills/frontend/SKILL.md` (states, light and dark),
`~/.codex/skills/plain-language/SKILL.md` (every string a person reads). Read the repo's `AGENTS.md` first:
invented fixtures only, no real person anywhere (use names like Ari, Robin, Mo Lee). No fixed paths: tests use
scratch homes (`voice_core::test_support::Scratch` and each crate's existing fixtures).

## Where this starts

PB8.7a and PB8.7b are on `main` (read `git log --oneline -20` and
`plans/briefs/PB8.7b-back-up-and-bring-in.md`). Today:

- `voice_core::existing::find` reports an older assistant's `Found.old_folder`: the folder an old, unclaimed
  skill's `SKILL.md` names (the folder the assistant was made in, holding `member.json` and its own files).
- `voice_api::assistants::bring_in(home, claude_home, new, date, stamp, cli)` (`builder/api/src/assistants.rs`)
  refuses first, takes the state lock, backs up `state`, `skill`, `CLAUDE.md` and `member` (that old folder)
  into `~/.claude/voice-backups/<slug>/<stamp>/` with `voice_core::backup::back_up` (links kept as links, every
  byte checked), renames `decisions.jsonl` to `decisions-before-voice.jsonl`, then `make_and_clear` makes the
  new crew folder `~/.claude/crew/<slug>/` from the template (`crew::new_member_embedded`) and clears consent.
  Any failure after the backup puts `decisions.jsonl` back and removes the backup.
- The new crew folder is all template: none of the older assistant's own files come in. This slice adds that.

## Decisions (made; don't change them)

- **The files offered**, by these exact names, from the old folder only, in this order:
  `IDENTITY.md`, `autonomy.md`, `interview.md`, `lens.md`, `decisions.md`, `measurements.md`, `briefs`,
  `private.json`. Only names present in the old folder are offered. `SKILL.md`, `member.json` and anything
  else are never offered and never copied: they stay Voice's.
- **All ticked by default.** The person can untick any. Nothing is copied when there is no old folder.
- **Read from the backup, not the original.** The copy reads `<stamp>/member/<name>` inside the backup that
  `bring_in` just made and checked. Links there are links (the backup keeps them), so a link is never followed.
- **What may come in** (the same check when offering and when copying):
  - a file (every name but `briefs`): a regular file, not a link; at most 1 MiB, except `decisions.md` at
    most 256 KiB (`voice_grow::version::DECISIONS_MOST`'s value; Voice reads no more of it; don't add a crate
    dependency for the constant, write `256 * 1024` with a comment naming it); valid UTF-8 with no NUL byte;
  - `autonomy.md` must also pass `voice_core::autonomy::Autonomy::parse`;
  - `private.json` must also be a JSON object whose only top-level keys are `filters` and `anonymizer` (what
    a private file is for; any other key could rename the assistant or move its private folder, because
    `read_member` lets `private.json` replace any `member.json` key but `filters`);
  - `briefs`: a real folder (not a link) holding only regular files directly inside it (no subfolder, link or
    other entry), at most 64 files, each passing the file rule above with a 1 MiB cap, except `codebook.md` at
    most 64 KiB (`voice_build::draft::CODEBOOK_MOST`'s value; same comment rule, no new dependency).
- **Copied over the template:** each ticked file replaces the template's file of that name in the new crew
  folder (or is created when the template has none: `lens.md`, `private.json`). `briefs` copies each of its
  files over `briefs/<file>`; the template's other brief files stay.
  Every file brought in is written 0600 (the template's files keep their modes). Write each target by opening
  it with `O_NOFOLLOW` (`OpenOptionsExt::custom_flags(libc::O_NOFOLLOW)`), `write(true).truncate(true)
  .create(true)`, `mode(0o600)`, then `set_permissions` 0600 (an existing template file keeps its old mode
  otherwise).
- **Then the whole folder is checked:** `voice_core::member::read_member(<crew>/<slug>/member.json)` must
  succeed (this is "checked the way `read_member` reads it" for `private.json`), its `slug`, `name`,
  `principal` and `state_dir` must equal the ones `make` wrote, and `crew::MemberDir::open` plus `.autonomy()`
  must succeed.
- **All or nothing.** Any failure while copying or checking undoes the whole bring-in exactly as a failed
  `make_and_clear` does today (crew folder removed, `decisions.jsonl` put back, backup removed), and the error
  is `Error::Invalid(<the sentence below>)`.
- **A ticked name the old folder doesn't offer** (unknown, absent, or failing the rule) refuses the whole
  bring-in before anything is written (before the backup), with the sentence below.

## What to build

### 1. `voice_core::old_files` (new module `builder/core/src/old_files.rs`, `pub mod old_files;` in `lib.rs`)

```rust
pub const NAMES: [&str; 8] = ["IDENTITY.md", "autonomy.md", "interview.md", "lens.md",
    "decisions.md", "measurements.md", "briefs", "private.json"];
pub struct Offered { pub name: &'static str, pub problem: Option<&'static str> }
/// The old folder's files Voice can offer, in NAMES order; names absent from the folder are left out.
pub fn offer(folder: &Path) -> Result<Vec<Offered>>;
/// Copy the named files from `from` (the backup's member copy) over `to` (the new crew folder), then check `to`.
pub fn bring(from: &Path, to: &Path, names: &[String]) -> Result<()>;
pub const CANT_BRING: &str = …; // see sentences
pub fn says(message: &str) -> bool; // true for any sentence this module returns, as backup::says does
```

- `offer` reads with `symlink_metadata` and opens files the way `existing.rs`'s `read_capped` does
  (`O_NONBLOCK | O_NOFOLLOW`, `fstat` regular, read at most cap + 1 bytes). Move `read_capped` to a shared
  `pub(crate)` place (or make it `pub(crate)` in `existing.rs`) rather than copying it. A missing old folder
  gives an empty list. `problem` is one of these exact sentences, first that applies:
  - `"It's a link, so Voice leaves it."`
  - `"It isn't a plain file, so Voice leaves it."` (for `briefs`: `"It isn't a plain folder of files, so Voice leaves it."`)
  - `"It's too big for Voice to read, so Voice leaves it."`
  - `"It isn't plain text, so Voice leaves it."`
  - autonomy.md: `"Voice can't read its list of who decides what, so Voice leaves it."`
  - private.json: `"It changes more than the privacy settings, so Voice leaves it."` (a key other than
    `filters`/`anonymizer`, or not a JSON object, or not JSON)
- `bring` first checks every name: in `NAMES`, present in `from`, no problem (re-run the same check on
  `from`); otherwise `Error::Invalid(format!("Voice can't bring in {name}, so it changed nothing. Untick it and try again."))`.
  Then copies, then runs the folder check above; a failed folder check is
  `Error::Invalid("Voice couldn't bring in the older assistant's own files, so it changed nothing. Untick private.json and autonomy.md, then try again.")`.
  Before writing, refuse any duplicate name in `names`.

Tests in the module, red first: each of the eight names offered from a plain old folder, in order; absent
names left out; each problem sentence (a link, a folder named `IDENTITY.md`, a FIFO named `lens.md` via
`libc::mkfifo` that doesn't block, 1 MiB + 1 byte, `decisions.md` at 256 KiB + 1, `briefs/codebook.md` at
64 KiB + 1, a NUL byte, invalid UTF-8, an `autonomy.md` with a dial twice, a `private.json` with `"name"`, a
`private.json` with `"state_dir"`, a `private.json` that is a JSON array, a `briefs` holding a subfolder, a
`briefs` holding a link, 65 brief files); `bring` copies byte-identical, 0600, over a template file of 0644,
keeps the template's other brief files; an unknown, absent or duplicate name refuses before writing; a
`private.json` with only `filters` merges (`read_member` then shows the extended filter).

### 2. `bring_in` takes the ticks (`builder/api/src/assistants.rs`)

- New last parameter `files: &[String]`. Before the state lock and the backup: when `files` is not empty,
  `found.old_folder` must be `Some`, and every name must be in `old_files::offer(old_folder)` with no
  problem, else refuse with `old_files`'s sentence for that name (nothing written, no `voice-backups`).
- After `make_and_clear` succeeds, when `files` is not empty: `old_files::bring(&backup.join("member"),
  &made.dir, files)`. On error: remove `made.dir`, then fall into the same undo as a failed `make_and_clear`
  (the existing `match result` arm). Keep one undo path: move the copy into the closure so its error reaches
  that arm, and remove `made.dir` inside the closure before returning the error.
- Return the files actually brought, in `NAMES` order: change the return type to
  `(assistants::Listed, Option<PathBuf>, Vec<String>)`, or a small named struct if that reads better; update
  both callers.
- `bring_in_assistant_in`: map `old_files::says(&sentence)` to `failure(&sentence)` beside the others.

### 3. DTOs (`builder/api/src/dto.rs`), then regenerate the TS types the way PB8.7a and PB8.7b did

- New `OldFile { name: String, problem: Option<String> }` (`camelCase`, `#[ts(export)]`).
- `ExistingAssistant` gains `files: Vec<OldFile>`: `find_existing_in` fills it from
  `old_files::offer(old_folder)` when `found.old_folder` is `Some`, else empty. An `offer` error maps to
  `failure(LOOK_BEFORE)` like the others.
- New request `BringInRequest { assistant: NewAssistantRequest, files: Vec<String> }`
  (`camelCase, deny_unknown_fields`, `#[ts(export)]`). `bring_in_assistant_in` and `bring_in_assistant` take
  it instead of `NewAssistantRequest`; the field checks stay on `request.assistant`.
- `BroughtIn` gains `files: Vec<String>` (the names brought).

### 4. The app command

`bring_in_assistant` keeps its name; only its request type changes: `builder/app/src/commands.rs`,
`builder/ui/src/backend.ts`, `app/tauri-backend.ts` and its test, `test-support.ts`. No new command, so
`capabilities` and `lockdown.rs` stay as they are (check `lockdown.rs` still passes).

### 5. `voice new --bring-in --leave <file>` and `voice existing`

- `Command::New` gains `#[arg(long = "leave")] leave: Vec<String>`, allowed only with `--bring-in` (clap
  `requires = "bring_in"`). The CLI brings every offered file with no problem except those left; a `--leave`
  name not offered refuses: "Voice found no {file} to leave out. See what it found: voice existing {name}".
- After the "Voice backed up …" line, when files came in, print
  "It brought in the older {name}'s own files: IDENTITY.md, autonomy.md." (names joined with ", ", ending ".").
- `voice existing`: after the old folder's line it prints today, print one line per offered file, exactly
  `"Can bring in {file}"` or `"Leaves out {file}: {problem}"`. Then, when `can_bring_in`, the last line becomes
  "Voice can back it up and bring it in: voice new {name} --bring-in (add --leave <file> to leave a file out)"
  when any file is offered, else today's line.

### 6. The screen (`builder/ui/src/screens/NewAssistant.vue`)

- `older` keeps `files` from `findExisting`, plus a `ticked` set holding every name with no problem.
- When `older.canBringIn` and `older.files` is not empty, the warning shows, above its last paragraph, a
  `fieldset` with `legend` "Its own files" (`form-label`) and the hint "Voice keeps its own SKILL.md and
  member.json. Untick a file to leave it out." (`form-hint`), then one Tabler checkbox per file
  (`form-check`, `form-check-input`, `form-check-label`, the file name in `<code>`): checked and enabled when it
  has no problem; unchecked and disabled with the problem shown after the name (`text-secondary`) when it has
  one. Each input has a unique `id` (`old-file-<index>`) and its label `for`.
- *Back it up and bring it in* sends `{ assistant: older.request, files: [ticked names in the order shown] }`.
- The success alert's sentence becomes "Voice brought in the older {name}'s history and its own files:
  IDENTITY.md, autonomy.md." when `files` is not empty, else today's sentence; the backup sentence follows as
  today.
- Light and dark: Tabler's classes only, no new colours.

## Tests (red first, each)

Rust:
- api: bring-in with an old folder holding all eight names (invented text; a `private.json` with only
  `filters.personal`) and `files` = all eight: the new crew folder's files are byte-identical to the old
  ones, each 0600, `SKILL.md` and `member.json` are the template's, the template's other brief files are
  there, `read_member` of the new `member.json` shows the merged filter, the returned files list all eight.
- api: `files` = `["IDENTITY.md"]` only: `IDENTITY.md` is the old one, `autonomy.md` is the template's.
- api: `files` empty with an old folder: every file is the template's (today's behaviour).
- api, refusals before writing (snapshot the scratch home as PB8.7b's tests do; no `voice-backups`): a name
  not in `NAMES`; `lens.md` ticked but absent; a ticked `private.json` with `"state_dir"`; `files` not empty
  with no old folder.
- api, rollback after the backup: an old `private.json` of `{"filters":{"personal":7}}` passes `offer`
  (only `filters`) but fails `read_member` after the copy. With the state holding a `decisions.jsonl`, bring in
  all files: the error is the folder-check sentence; the scratch home is byte-identical to before except an
  empty `voice-backups/<slug>/`, which may stay; `decisions.jsonl` is back; no crew folder.
- api: `find_existing_in` lists `files` with problems as sentences.
- cli: `voice new Ari --principal Robin --bring-in` brings every offered file and prints the files line;
  `--leave autonomy.md` leaves it; `--leave nope.md` refuses; `--leave` without `--bring-in` is a usage error;
  `voice existing` prints the per-file lines.

Screens (`NewAssistant.test.ts`): the checkboxes show only with `canBringIn` and files; all ticked by
default; a file with a problem is disabled, unticked and shows its sentence; unticking one sends `files`
without it; the request is `{ assistant, files }`; the success sentence names the files; with no files the
sentence is today's.

## Done when

- Every new test was red first, then green.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked --jobs 2 -p voice-core -p voice-api -p voice-app -p voice-cli -- --test-threads=2`
  (check the package names in each `Cargo.toml`), and `pnpm -C builder/ui test` pass. The machine is loaded:
  at most 2 workers everywhere.
- No real person's name, handle or path in any file. No fixed path.
- Print a short summary: which tests were red and how, then the green counts.
