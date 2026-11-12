# PB8.7b: back an older assistant up, then bring its history in

Code change. Rust in `builder/core`, `builder/api`, `builder/app` and `builder/cli`; Vue and TypeScript in
`builder/ui`. Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md` (writing the person's Claude folder, never following a
link, all or nothing), `~/.codex/skills/frontend/SKILL.md` (states, light and dark),
`~/.codex/skills/plain-language/SKILL.md` (every string a person reads). Read the repo's `AGENTS.md` first:
invented fixtures only, no real person anywhere (use names like Ari, Robin, Mo Lee). No fixed paths: tests use
scratch homes (`voice_core::test_support::Scratch` and each crate's existing fixtures).

## Where this starts

PB8.7a (on `main`) added `voice_core::existing::find(home, claude_home, slug) -> Found`
(`builder/core/src/existing.rs`): an older assistant's private folder `~/.claude/state/<slug>/` holding a file
(`Found.state`, with `link`, `not_folder`, `files`, `corpus` = a top-level `decisions.jsonl`, `live_log`), a
skill Voice didn't add (`Found.skill`, the `<claude>/skills/<slug>` path), `CLAUDE.md` lines naming `/<slug>`
(`Found.claude_md`, `Found.lines`), and the folder an old `SKILL.md` names (`Found.old_folder`). `voice new`
(`builder/cli/src/assistants.rs`) and `new_assistant_in` (`builder/api/src/assistants.rs`) refuse while
`found.any()`; `find_existing_in` turns `Found` into plain lines for the screen; *Make a new assistant*
(`builder/ui/src/screens/NewAssistant.vue`) shows them in a warning. Read PB8.7a's commits
(`git log --oneline -12`, the `find_existing` command is the model for wiring a new app command end to end).

The hazard this slice fixes: *Put it together* (`merge_histories_in`, `builder/api/src/history.rs`) reads
every `decisions-*.jsonl` in the private folder except `decisions.jsonl` (`voice_ingest::corpus::discover`) and
**replaces** `decisions.jsonl`. An older assistant's `decisions.jsonl` may be its only full history, so it is
renamed `decisions-before-voice.jsonl` first: then it is a history input and is merged, never replaced.

## What to build

### 1. `voice_core::backup` (new module `builder/core/src/backup.rs`, `pub mod backup;` in `lib.rs`)

`pub fn back_up(home: &Path, slug: &str, stamp: &str, items: &[Item]) -> Result<PathBuf>` where
`pub struct Item<'a> { pub name: &'a str, pub source: &'a Path, pub follow: bool }`.

- The backup folder is `home/.claude/voice-backups/<slug>/<stamp>/`. `slug` must pass
  `assistants::assistant_dir`'s check (reuse `crew::slugify(slug) == Some(slug)`); `stamp` is
  `YYYY-MM-DD-HHMMSS` (local time; the caller makes it). Make `voice-backups` and `<slug>` with mode 0700 if
  missing; if either exists and is not a real folder (`symlink_metadata`: a link, a file), refuse
  (`Error::Invalid("Voice's backup folder ~/.claude/voice-backups isn't a plain folder, so it backed nothing up.")`).
  Make `<stamp>` with `fs::create_dir` (never reuse one); if it exists, try `<stamp>-2` … `<stamp>-9`, then refuse.
  Set 0700 on it.
- Each item is copied to `<stamp>/<name>`. The item names the caller uses are `state`, `skill`, `CLAUDE.md`
  and `member`.
- The copy never follows a link, except the item root when `follow` is true (only `CLAUDE.md`, which the finder
  reads through its link and which is often a dotfiles link; then the final file must be a regular file and its
  bytes are copied). Walk with `fs::symlink_metadata`:
  - a folder: `create_dir` 0700, then its entries (sorted by name);
  - a regular file: open with `O_RDONLY | O_NOFOLLOW | O_NONBLOCK` (`OpenOptionsExt::custom_flags`, as
    `existing.rs`'s `read_capped` does), `fstat` must say regular file, copy through `io::copy` into a file made
    with `create_new(true)` and mode 0600;
  - a link: make a link with the same target (`fs::read_link` + `std::os::unix::fs::symlink`); never read
    through it;
  - anything else (FIFO, socket, device): refuse (`Error::Invalid("… isn't a plain file, so Voice backed nothing up.")`,
    naming only the entry's file name).
- **Then check the whole backup before returning:** walk each source and its copy side by side: the same entry
  names, the same kinds, every regular file byte-for-byte equal (read both in chunks; never whole files into
  memory), every link's target equal. Any difference or error: refuse.
- **On any error after `<stamp>` was made, remove `<stamp>` (`remove_dir_all`) before returning**, so a failed
  backup leaves nothing. Never remove `voice-backups` or `<slug>`.
- Return the `<stamp>` path.

Tests in the module, red first: a tree with nested folders, two files and a link copies byte-identical with
0700 folders and 0600 files and the link still a link (its target never read: point it at a path that doesn't
exist); `follow` copies a linked `CLAUDE.md`'s target bytes as a regular file; a FIFO anywhere (`libc::mkfifo`)
refuses at once and leaves no `<stamp>` folder; a `voice-backups` that is a link refuses and nothing is written
through it; two backups with the same stamp get `<stamp>` and `<stamp>-2`; a bad slug refuses.

### 2. `voice_api::assistants::bring_in` (in `builder/api/src/assistants.rs`)

`pub fn bring_in(home: &Path, claude_home: &Path, new: &assistants::New<'_>, date: &str, stamp: &str, cli: &str)
-> voice_core::error::Result<(assistants::Listed, Option<PathBuf>)>`. Both the CLI and the app call it, so the
order and the rollback live in one place. Errors are `Error::Invalid(<plain sentence>)` or `Error::Exists`, which
each caller maps to its own wording as it maps `assistants::make`'s today.

In this order:
1. `existing::find(home, claude_home, slug)`. Nothing found: `assistants::make`, then `clear_consent` (exactly as
   `new_assistant_in` does today, removing the folder on a failed clear), and return `(listed, None)`.
2. Refuse before writing anything when:
   - `found.state` is a link or not a folder: "Voice can't bring in ~/.claude/state/<slug> because it's a link or a
     file, not a folder. Move it somewhere else, or pick another name.";
   - the private folder holds `decisions-before-voice.jsonl` already, or its `decisions.jsonl` is not a regular
     file (`symlink_metadata`): "Voice can't bring this one in: its private folder already holds a history from
     before Voice. Pick another name for now."
3. If the private folder exists, hold `voice_ingest::private::lock_existing_state(home, slug)` until the end.
4. `voice_core::backup::back_up` with the items found: `state` ← `home/.claude/state/<slug>` (when
   `found.state`), `skill` ← `found.skill`, `CLAUDE.md` ← `found.claude_md` (`follow: true`), `member` ←
   `found.old_folder`.
5. If the private folder has `decisions.jsonl`: `fs::hard_link` it to `decisions-before-voice.jsonl` (fails if
   the name is taken, so nothing is overwritten), then `fs::remove_file` the old name (on failure, remove the new
   link and refuse).
6. `assistants::make`, then `clear_consent` (the old `consent.json`, if any, is in the backup).
7. **Any failure in 5 or 6 undoes everything before returning:** remove a crew folder this call made, put
   `decisions.jsonl` back (the reverse of step 5), remove the backup's `<stamp>` folder. A failure in 4 has
   already removed its own folder. The older assistant's skill, `CLAUDE.md` and old folder are never changed in
   this slice (taking them out is PB8.7e).
8. Return `(listed, Some(<stamp> path))`.

App side, next to `new_assistant_in`:
- `pub fn bring_in_assistant_in(home, claude_home, request: dto::NewAssistantRequest) -> Result<dto::BroughtIn>`
  and `pub fn bring_in_assistant(request)` (the same `home`/`claude_home` lookup as `new_assistant`). The same
  field checks as `new_assistant_in`; the CLI word and the date as `new_assistant_in` gets them; the stamp from
  the local time (`YYYY-MM-DD-HHMMSS`, reuse whatever `local_day` uses for the local date). Map
  `Error::Invalid(sentence)` from the refusals in step 2 to that sentence, `Error::Exists` to the same sentence
  `new_assistant_in` uses, anything else to `MAKE`.
- New DTO `BroughtIn { assistant: Assistant, backup: Option<String> }` (`#[serde(rename_all = "camelCase")]`,
  `#[ts(export)]`, `deny_unknown_fields` where the other response DTOs have it). `backup` is the folder shown
  with the home folder as `~` (`~/.claude/voice-backups/ari/2026-11-12-093000`), never the full path.
- `ExistingAssistant` gains `can_bring_in: bool`: true when something is found and the step-2 refusals would not
  apply (the state is not a link or a file, no `decisions-before-voice.jsonl`, `decisions.jsonl` absent or a
  regular file). `find_existing_in` fills it.
- `new_assistant_in`'s refusal sentence becomes: "Voice found an older {name} on this computer, so it made
  nothing and changed nothing. Choose Back it up and bring it in, or pick another name." Regenerate the TS types
  the way the repo does (look at how `ExistingAssistant.ts` was generated in PB8.7a).

### 3. The app command

`bring_in_assistant`, wired exactly as PB8.7a wired `find_existing`: `builder/app/src/commands.rs`,
`command_names.rs`, `main.rs`, `capabilities/default.json`, the autogenerated permission file,
`builder/app/tests/lockdown.rs`, `builder/ui/src/backend.ts`, `app/tauri-backend.ts` and its test,
`test-support.ts`.

### 4. `voice new --bring-in`

Add `#[arg(long)] bring_in: bool` to `Command::New`. With it, `builder/cli/src/assistants.rs` `make` calls
`voice_api::assistants::bring_in` (same date and CLI word as today) and prints, after the usual "Made …" line,
"Voice backed up the older {name} first, in {backup shown with ~}." (only when a backup was made), then the usual
"Next:" line. Without it, the refusal sentence becomes "Voice found an older {name} on this computer, so it made
nothing. See what it found: voice existing {name}. To back it up and bring it in: voice new {name} --bring-in
(with the same options)." `voice existing` adds a last line "Voice can back it up and bring it in: voice new
{name} --bring-in" when `can_bring_in`, else "Voice can't bring this one in." with the step-2 reason.

### 5. The screen (`NewAssistant.vue`)

- `older` keeps `canBringIn`. When found and `canBringIn`, the warning's last paragraph reads "Voice made nothing
  and changed nothing. It can copy everything listed here into a backup folder first, then bring this {name}'s
  history in, or you can pick another name." and the warning holds a button **Back it up and bring it in**
  (`btn btn-warning`), disabled while saving, reading *Backing it up…* while it runs. When not `canBringIn`, keep
  today's paragraph but say "Voice can't bring this one in, so pick another name." Editing the name still clears
  the warning.
- The button calls `backend.bringInAssistant(request)` with the same request the check used. On success, replace
  the form's buttons with a success alert (`alert alert-success`, `role="status"`): heading "{name} is ready",
  then "Voice brought in the older {name}'s history. A copy of everything it found is in {backup}." (leave out the
  second sentence when `backup` is null), and a primary button **Open {name}** that emits `made` with the
  assistant. On failure, show `failureMessage(failure, "Voice couldn't back up the older assistant, so it changed
  nothing. Check there's space on this computer, then try again.")` in the existing danger alert.
- Light and dark: Tabler's alert classes only, no new colours.

## Tests (red first, each)

Rust, in each crate's existing test files next to PB8.7a's:
- api, bring-in with all four found (state with `decisions.jsonl`, `live.jsonl` and one other file; an unclaimed
  skill whose `SKILL.md` names an old member folder; a `CLAUDE.md` line): the backup holds `state/`, `skill/`,
  `CLAUDE.md` and `member/` byte-identical to the sources; the private folder now has
  `decisions-before-voice.jsonl` (the old bytes) and no `decisions.jsonl`; the crew folder exists; `consent.json`
  is gone; the skill, `CLAUDE.md` and the old member folder are byte-identical to before; `find_existing_in`
  now says not found; `backup` is shown with `~`.
- api, **the hazard**: a private folder holding a `decisions.jsonl` (records with question ids q1, q2) and a
  `decisions-prev-2026-01-01.jsonl` (q3, q4), invented records shaped like the ones `builder/api/tests/history.rs`
  already uses. Bring in, agree to consent the way those tests do, `merge_histories_in`: the new
  `decisions.jsonl` holds all four questions.
- api, refusals leave the tree byte-identical (snapshot every path, kind and file's bytes under the scratch home
  before and after) and make no `voice-backups`: a linked state folder; a state that is a file; a
  `decisions-before-voice.jsonl` already there.
- api, rollback: make step 6 fail (put a plain file at `~/.claude/crew` so `assistants::make` errors): the tree is
  byte-identical to before except an empty `voice-backups/<slug>/` folder, which may stay; `decisions.jsonl` is
  back.
- api, nothing found: `bring_in_assistant_in` makes the assistant as `new_assistant_in` does and `backup` is
  null. `find_existing_in` gives `canBringIn` true for a plain old state, false for a linked one.
- cli: `voice new Ari --principal Robin --bring-in` on a scratch home with an old state prints the backup line
  and makes the assistant; without `--bring-in` the new refusal sentence; `voice existing` prints the new last
  line.
- app lockdown test lists the new command.

Screens (`NewAssistant.test.ts`): the bring-in button shows only with `canBringIn`; clicking it calls
`bringInAssistant` with the request; the success alert names the backup and **Open Ari** emits `made`; a null
backup leaves the sentence out; a failure shows the danger sentence; editing the name hides the warning;
`canBringIn` false shows the can't sentence and no button.

## Done when

- Every new test was red first, then green.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked --jobs 2 -p
  voice-core -p voice-api -p voice-app -p voice-cli -- --test-threads=2` (check the package names in each
  `Cargo.toml`), and `pnpm -C builder/ui test` pass. The machine is loaded: at most 2 workers everywhere.
- No real person's name, handle or path in any file. No fixed path.
- Print a short summary: which tests were red and how, then the green counts.
