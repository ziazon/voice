# PB8.7a: look before making, and never reuse an older assistant's folders silently

Code change. Rust in `builder/core`, `builder/api`, `builder/app` and `builder/cli`; Vue and TypeScript in
`builder/ui`. Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md` (reading the person's Claude folder, never following a
link), `~/.codex/skills/frontend/SKILL.md` (states, light and dark), `~/.codex/skills/plain-language/SKILL.md`
(every string a person reads). Read the repo's `AGENTS.md` first: invented fixtures only, no real person
anywhere (use names like Ari, Robin, Mo Lee).

## What happened

Someone made an assistant in Voice whose name matched an assistant they had built before Voice, by hand,
with this kit's guided build. Voice silently reused the old private folder `~/.claude/state/<slug>/` by
name. Nothing told them, nothing was backed up, and:

- *Put it together* (`merge_histories_in`, `builder/api/src/history.rs`) replaces `decisions.jsonl` in that
  folder. An older assistant's `decisions.jsonl` may be its only full history (beside it, say, a single
  `decisions-prev-<date>.jsonl`), so the merge would throw most of it away.
- *Turn it on* refuses the old `<claude>/skills/<slug>/` that Voice didn't add, and the old line in
  `<claude>/CLAUDE.md` (no Voice markers) stays.

Today `assistants::make` (`builder/core/src/assistants.rs`) refuses only when `~/.claude/crew/<slug>/`
exists. It never looks at the state folder, the skill, `CLAUDE.md` or the folder the old assistant was made in.

This slice is the first of five (PB8.7a–e). It **finds and reports** an older assistant, and **refuses to
make** one over it. Backing up and bringing it in is PB8.7b; do none of that here. Write nothing anywhere
in this slice except the new crew folder on the "nothing found" path, exactly as today.

## What counts as "found"

For the slug a new name makes (`crew::slugify`), an older assistant is any of these four:

1. **Its private folder.** `<home>/.claude/state/<slug>` exists and holds at least one file (counted
   recursively, never following a link), or is a symlink (any target, dangling included), or is something
   other than a folder. An empty folder is not found.
2. **Its skill.** `<claude_home>/skills/<slug>` exists (by `symlink_metadata`, so a dangling link counts)
   and Voice's own record does not claim it. Voice claims it only when `install_record::read(claude_home)`
   returns `Ok(Some(record))` with `record.slug == slug` and an `Added::Dir { path }` equal to that path. A
   record that can't be read counts as "not claimed" (the skill is found).
3. **Its line in CLAUDE.md.** A line of `<claude_home>/CLAUDE.md` that names `/<slug>` as a word, outside
   every Voice block. Rules:
   - Read the file with `fs::read` (following a symlinked CLAUDE.md is fine: it is only read, and
     `ClaudeFile::read` already follows it). Missing file: no lines. Over 4 MiB: return
     `Error::Invalid("CLAUDE.md is too big for Voice to read")`.
   - Decode with `String::from_utf8_lossy`. Split with `str::lines` (so CRLF is handled).
   - Skip Voice blocks: a line whose trimmed text starts with `<!-- voice:begin ` starts a block, one starting
     `<!-- voice:end ` ends it; the marker lines and everything between are skipped, for any id.
   - A line matches when, comparing ASCII case-insensitively, it contains `/` + slug where the byte before
     the `/` is the line's start, whitespace, `(`, `[`, `` ` ``, `"` or `'`, and the byte after the slug is the
     line's end or anything other than an ASCII letter, digit, `_`, `-` or `/`. So `- Ask Ari (/ari) before…`
     and `` run `/ari`. `` match; `~/.claude/state/ari/x`, `/ari/`, `/ari-old` and `/arik` don't.
   - Keep the 1-based line numbers.
4. **The folder it was made in.** Only when (2) found the skill: if `<claude_home>/skills/<slug>` is a real
   folder (not a link) and its `SKILL.md` is a regular file (not a link) of at most 1 MiB, find the first
   `--member ` in it, take the next whitespace-separated token, strip one pair of surrounding `"` or `'`,
   require it to end with `/member.json`, expand a leading `~/` to `<home>/`, and require an absolute path.
   Its parent folder is the old folder only if `symlink_metadata` says it is a folder (not a link) and
   `member::read_member(<folder>/member.json)` succeeds with `member.slug == slug`. Anything else: no old
   folder. (The old guided build wrote `python3 -I <repo>/scripts/crew_decisions.py --member
   <repo>/crew/<slug>/member.json similar|record` into the skill; `<repo>` is often shown as `~/…`.)

For (1), also report whether a top-level `decisions.jsonl` and a top-level `live.jsonl` are there (by
`symlink_metadata`, any type).

## The changes

### 1. `builder/core/src/existing.rs` (new, `pub mod existing;` in `lib.rs`)

```rust
#[derive(Debug, Default, PartialEq, Eq)]
pub struct State {
    pub path: PathBuf,
    pub files: u64,      // 0 when `link` or `not_folder`
    pub link: bool,
    pub not_folder: bool,
    pub corpus: bool,    // a top-level decisions.jsonl
    pub live_log: bool,  // a top-level live.jsonl
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub slug: String,
    pub state: Option<State>,
    pub skill: Option<PathBuf>,
    pub claude_md: Option<PathBuf>,      // the CLAUDE.md path, Some only when `lines` is not empty
    pub lines: Vec<usize>,
    pub old_folder: Option<PathBuf>,
}
impl Found {
    pub fn any(&self) -> bool; // state, skill, lines or old_folder
}
pub fn find(home: &Path, claude_home: &Path, slug: &str) -> Result<Found>;
```

- Validate the slug with `assistants::assistant_dir(home, slug)` first (same `Error::Invalid`).
- Never write, create, lock or follow a link (except reading CLAUDE.md, above). Never read a file's content
  in the state folder: names and counts only.
- Count files with `forget::count_files` (make it `pub(crate)`); it already treats a link as one file and
  never descends into one.
- Unit tests in the module (`crate::test_support::Scratch`), red first: nothing found; each of the four
  found alone; an empty state folder not found; a state link (to a folder, and dangling) found with
  `link: true`; a state file (not folder) found with `not_folder: true`; `corpus` and `live_log`; a skill
  that Voice's record claims not found (write a real record by turning an invented assistant on with
  `install::turn_on` in a fixture, as `install_tests.rs` does) and an unreadable record leaving it found;
  CLAUDE.md: a matching line inside a Voice block not counted, the match/no-match examples above, CRLF,
  upper case `/ARI`, line numbers; the old folder: `~/` expanded, quoted path, a `member.json` with another
  slug ignored, a symlinked old folder ignored, a symlinked `SKILL.md` ignored, no `--member` ignored;
  an invalid slug refused.

### 2. The app (`builder/api`)

- `dto.rs`:
  ```rust
  #[derive(Debug, Deserialize, TS)] #[serde(rename_all = "camelCase", deny_unknown_fields)] #[ts(export)]
  pub struct FindExistingRequest { pub name: String }
  #[derive(Debug, Serialize, TS)] #[serde(rename_all = "camelCase")] #[ts(export)]
  pub struct ExistingAssistant { pub found: bool, pub lines: Vec<String> }
  ```
  Match the derives and attributes of the neighbouring DTOs exactly (copy `SlugRequest`'s and `Said`'s).
  `lines` are fixed sentences built in Rust; the screen shows them as a list.
- `assistants.rs`:
  - `pub fn find_existing_in(home: &Path, claude_home: &Path, request: dto::FindExistingRequest) -> Result<dto::ExistingAssistant>`
    and `pub fn find_existing(request)` (home from `home(LOOK_BEFORE)`, claude home from
    `use_it::claude_home(&home, CLAUDE_CONFIG_DIR)` as `turn_on` does). Name checks and the
    no-letter-or-digit check exactly as `new_assistant_in` does (factor that into one private function both
    call; same sentences). Any `existing::find` error is `failure(LOOK_BEFORE)`.
  - `const LOOK_BEFORE: &str = "Voice couldn't look for an older assistant with that name. Close it and open it again.";`
  - The sentences, in this order, each only when it applies (`{slug}` is the validated slug; `{n}` with
    `file`/`files` by count):
    - state, folder: `"A private folder from before, ~/.claude/state/{slug}/, holds {n} file(s)."`
    - state, `corpus`: `"One of them is a put-together history that Voice would replace."`
    - state, `live_log`: `"One of them is a log of answers you recorded."`
    - state, `link`: `"~/.claude/state/{slug} is a link to somewhere else."`
    - state, `not_folder`: `"~/.claude/state/{slug} is a file, not a folder."`
    - skill: `"Claude Code has a skill called {slug} that Voice didn't add."`
    - lines: `"Your CLAUDE.md mentions /{slug} on {k} line(s) that Voice didn't add."`
    - old folder: `"That skill points to the folder the assistant was made in, with its own files."`
    No path other than `~/.claude/state/{slug}` reaches the webview.
  - `new_assistant_in` gains `claude_home: &Path` as its second argument (update every caller and test).
    After the name checks and before `shipped_cli_word`: compute the slug; if `assistant_dir(home, slug)`
    exists by `symlink_metadata`, skip the finder (the existing `Error::Exists` sentence still answers). Else
    run `existing::find`; an error is `failure(MAKE)`; `found.any()` is
    `failure(&format!("Voice found an older {name} on this computer, so it made nothing and changed nothing. Voice can't bring an older assistant in yet: pick another name for now."))`
    with `name` = `request.name`. `new_assistant` passes the claude home the same way `turn_on` does.
  - The existing test `new_answer_does_not_reuse_orphaned_state` (`builder/api/tests/assistants.rs`) now
    expects that refusal and that nothing changed (no crew folder; the state folder's bytes untouched). Rename
    it to say so. Keep `failed_consent_clear_removes_only_the_new_assistant` working: give its state folder
    no files if it needs a state folder to exist, or adjust it so it still proves the rollback.
- `parity.rs`: `Parity { app: "find_existing", cli: &[&["existing"]] }` right after `new_assistant`.

### 3. The window's command (`builder/app`)

Add `find_existing` right after `new_assistant`, in the same order, in every list the lockdown test
compares: `command_names.rs` `COMMANDS`, `commands.rs` (`#[tauri::command(async)]\npub fn find_existing(`
calling `voice_api::find_existing`), `main.rs` `generate_handler!`, `capabilities/default.json`
(`allow-find-existing`), the generated `permissions/autogenerated/find_existing.toml` (let `build.rs` make
it), and the hard-coded list in `builder/app/tests/lockdown.rs`. Export from `voice_api`'s `lib.rs` as the
neighbours are.

### 4. The command line (`builder/cli`)

- `voice existing <name> [--claude-home <dir>]`: the claude home from `places(claude_home)` as `voice on`
  does. Runs `existing::find`. Prints one line per thing found, with real paths:
  - `Private folder: <path> (<n> file(s)[, a put-together history (decisions.jsonl)][, a log of recorded answers (live.jsonl)])`
    or `… (a link to somewhere else)` / `… (a file, not a folder)`
  - `Skill Voice didn't add: <path>`
  - `CLAUDE.md lines naming /<slug>: <path> lines 3, 17`
  - `Folder it was made in: <path>`
  then `Voice won't make <name> over these. Pick another name for now.` Nothing found: `Nothing from before
  is called <name> on this computer.` Exit 0 either way. Bad names: the same errors `voice new` gives.
- `voice new` gains `--claude-home <dir>` the same way, and refuses (exit 1, nothing made) when anything is
  found: `Voice found an older <name> on this computer, so it made nothing. See what it found: voice existing <name>`.
  The crew-folder-exists message comes first, as in the app.
- Help text for both in the style of their neighbours. Tests in `builder/cli/tests/assistants.rs` (its
  `Home` sets `HOME`; set `CLAUDE_CONFIG_DIR` to `""` as `turn_on.rs` does): `existing` with nothing and with
  each kind, `new` refused with the state folder's bytes unchanged and no crew folder, `--claude-home`
  honoured. `cli/tests/app_parity.rs` must pass with the new parity row.

### 5. The screen (`builder/ui`)

- `backend.ts`: `findExisting(request: FindExistingRequest): Promise<ExistingAssistant>` in the `Backend`
  interface; `app/tauri-backend.ts` (`invoke<ExistingAssistant>('find_existing', { request })`), its test,
  and the mock in `test-support.ts` (default `{ found: false, lines: [] }`).
- `screens/NewAssistant.vue`: on *Make it*, first `await backend.findExisting({ name })`. If `found`, don't
  call `newAssistant`; show, above the buttons, an `alert alert-warning` with role `status`, the heading
  `Voice found an older {name}` (the trimmed name), the `lines` as a `<ul>`, and the closing sentence
  `Voice made nothing and changed nothing. It can't bring an older assistant in yet, so pick another name for now.`
  Editing the name clears it. A `findExisting` failure shows in the existing danger alert through
  `failureMessage(failure, "Voice couldn't look for an older assistant with that name. Close it and open it again.")`
  and nothing is made. While checking, the button reads `Checking…` and is disabled.
- Tests in `NewAssistant.test.ts`: found → warning with each line, `newAssistant` never called; not found →
  `newAssistant` called once with the same request; failure → danger alert, nothing made; editing the name
  clears the warning. Light and dark: use Tabler's alert classes only (the theme already fixes their
  contrast, `theme.css`).
- Regenerate the TS types (`cargo test` with the repo's `.cargo/config.toml` writes
  `builder/ui/src/types/generated/`); commit-ready, `ci-local.sh` checks they match.

### 6. Records

- `.ai/DECISIONS.md`: one dated entry (2026-11-12, PB8.7a) saying making an assistant now looks for an older
  one (the four kinds, the rules above in a sentence each) and refuses to reuse it, and that PB8.7b brings
  one in after a backup. Note it replaces the VO2 line "Nothing is migrated" only for this case.
- `.ai/SECURITY.md`: in the row about the app changing the Claude folder, one sentence: making an assistant
  reads (never writes) the state folder's names, the skill folder, Voice's record and `CLAUDE.md`, follows no
  link except a symlinked `CLAUDE.md` for reading, and reads at most 1 MiB of an old `SKILL.md`.

## Done when

- Every new test was red first, then green.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked -p
  voice_core -p voice_api -p voice_app -p voice_cli` (use the crates' real package names from their
  `Cargo.toml`), and `pnpm -C builder/ui test` pass. Run tests with at most 2 workers
  (`-- --test-threads=2`, and `--jobs 2` for cargo) because the machine is loaded.
- No real person's name, handle or path in any file. No fixed path: tests use scratch homes.
