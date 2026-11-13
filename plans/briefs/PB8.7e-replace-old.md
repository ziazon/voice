# PB8.7e brief: replace the old one when turning an assistant on

Lane V, runner V1, 2026-11-13. Card `NMgIuF67`. Codex implements; Claude reviews.

Read first: `AGENTS.md` (no real person in any file, fixture or string: invented names only, such as Ari),
`.ai/PROJECT.md`, `.ai/SECURITY.md`. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (every sentence the
person sees), `~/.codex/skills/project-config/SKILL.md`.

## Why

An assistant made before Voice (an old kit) left `~/.claude/skills/<slug>/` and a free-text line in
`~/.claude/CLAUDE.md` such as "Ask Ari (/ari) before asking me any question…". After a build, *Turn Ari on* refuses
in `voice_core::install::turn_on` (`builder/core/src/install.rs:114`, "… exists and Voice did not add it"), and the
app hides that behind its generic sentence ("Voice couldn't turn Ari on, so nothing in your Claude folder was
changed."). An old line alone doesn't block turning on, but then Claude Code reads two sets of instructions.

## What to build

### 1. Core: `builder/core/src/replace_old.rs` (new module, `pub mod replace_old;` in `lib.rs`)

```rust
pub struct OldOne { pub skill: Option<PathBuf>, pub claude_md: PathBuf, pub lines: Vec<(usize, String)> }
impl OldOne { pub fn any(&self) -> bool { self.skill.is_some() || !self.lines.is_empty() } }
pub struct Replaced { pub backup: PathBuf, pub skill: bool, pub lines: usize }
pub fn find(places: &install::Places, slug: &str) -> Result<OldOne>;
pub fn replace(places: &install::Places, slug: &str, stamp: &str) -> Result<Replaced>;
```

- `find` is read-only. `skill` is `<claude_home>/skills/<slug>` when it exists (by `symlink_metadata`) **and** the
  install record doesn't claim it as an `Added::Dir` for this slug: move the `claimed` closure in
  `existing::find` into a `pub(crate) fn skill_claimed(claude_home, slug, skill_path) -> bool` in `existing.rs` and
  use it from both. `lines` are the CLAUDE.md lines outside every Voice marker block that mention `/<slug>`, with
  their 1-based numbers and their text: refactor `existing::find_lines` into
  `pub(crate) fn old_lines(path, slug) -> Result<Vec<(usize, String)>>` (same rules, same 4 MiB cap, same
  `read_capped`) and make `find_lines` map it to numbers. `places` is canonicalized the way `install` does (make
  `Places::canonical` `pub(crate)`); `validate` the slug with `block::validate_id` first.
- `replace` does, in this order, and on any error before step 6 leaves the person's files as they were and deletes
  the new backup folder:
  1. `find` again; if nothing is found, `Err(Invalid("Voice found no older <slug> to replace."))`.
  2. Refuse a skill path that is a symlink or not a directory: `Err(Invalid(SKILL_NOT_FOLDER))`, a const sentence
     "The older skill isn't a plain folder, so Voice left it. Move it away yourself, then turn the assistant on." Refuse
     CLAUDE.md that is not a regular file the same way (follow a final symlink only as `ClaudeFile::read` does today).
  3. `backup::back_up(&places.home, slug, stamp, items)` with item `skill` (`follow: false`) when a skill was found
     and item `CLAUDE.md` (`follow: true`) when lines were found. That copies and byte-verifies them.
  4. Rename the skill folder aside inside its own parent: `skills/.<slug>.voice-replacing-<stamp>` (one
     `fs::rename`, same folder, so atomic). Then verify the renamed tree against `<backup>/skill` with the backup
     module's verify (make `verify_entry` `pub(crate)`). A mismatch: rename it back, then the `CHANGED` sentence.
  5. Rewrite CLAUDE.md through an `install_fs::Journal` (`ClaudeFile::read`, then `journal.replace(path, new_text,
     claude.mode, false)`): first check the bytes read now equal `<backup>/CLAUDE.md` byte for byte (else rename the
     skill back, the `CHANGED` sentence); drop exactly the found line numbers, keep every other line and its line
     ending exactly (`\n` or `\r\n`), and the file's trailing-newline state. Commit the journal.
  6. `fs::remove_dir_all` the renamed-aside skill folder (the backup holds the verified copy).
  Return the backup folder, whether a skill moved, and how many lines came out.
- Add `"Voice found no older … to replace."`-style sentences and `SKILL_NOT_FOLDER` to `backup::says`-style
  recognition only if the api needs them (see §2): export them as `pub const`s from `replace_old`.

### 2. Api: `builder/api`

- `dto::ClaudeCodeStatus` gains `pub old_one: Option<OldOneFound>`, with
  `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)] #[serde(rename_all = "camelCase")] #[ts(export)]
  pub struct OldOneFound { pub skill: bool, pub lines: Vec<String> }` (each line's text, trimmed of its line ending,
  each cut to 200 characters with "…"). Fill it in `claude_code_status_in` only when the state is `Off` and
  `replace_old::find(...).any()`; a `find` error maps to the existing `STATUS` failure. Every other state: `None`.
- New `dto::TurnOnRequest { slug: String, #[serde(default)] replace_old: bool }` (camelCase,
  `deny_unknown_fields`, `ts(export)`). `turn_on_in` and `turn_on` take it instead of `SlugRequest`; update
  `builder/app/src/commands.rs` and the UI backend types (`builder/ui/src/backend.ts`, `app/tauri-backend.ts`,
  `test-support.ts`), and regenerate `builder/ui/src/types/generated/` the way the repo does today (find the ts-rs
  export test or script; don't hand-write generated files).
- `turn_on_in`, after the existing other-assistant check and before `install::turn_on`:
  - `find` the old one (an error → `on_failed`).
  - Found and `replace_old` false and a skill was found → `Err(failure(&old_skill(name, slug)))`:
    "Claude Code already has a skill called <slug> that Voice didn't add, so Voice didn't turn <Name> on. Choose
    Replace the old one, and Voice backs it up and moves it aside first."
  - Found and `replace_old` true → `replace_old::replace(&places, slug, &voice_agent::ledger::local_stamp(now))`
    (the same stamp PB8.7b's `bring_in_assistant_in` uses; take `now` the same way). An error → `on_failed`, except
    the `replace_old` and `backup::says` sentences, which pass through as they are. Then `install::turn_on`.
  - Success message with a replace: "<Name> is on. Voice moved the older <slug> skill and <n> CLAUDE.md line(s) to
    ~/.claude/voice-backups/<slug>/<stamp>/ (only the parts it found). Turning <Name> off won't put them back. Start
    a new Claude Code session to use <Name>." Write the parts with correct singular/plural; show the backup path
    relative to home with `~/`, as `bring_in_assistant_in` does.
  - Found, lines only, `replace_old` false: turn on as today (the screen offered the choice).
- `replace_old` true with nothing found: turn on as today (the screen may be stale).
- `builder/api/src/parity.rs`: the `turn_on` row gains the CLI flag as the table's convention needs.

### 3. CLI: `builder/cli/src/main.rs`

`Command::On` gains `#[arg(long)] replace_old: bool`. Without it and an old skill found, fail with the core
sentence plus "Run voice on <member_dir> --replace-old to back it up and move it aside." With it: `replace` then
`turn_on`, and print the backup folder on its own line (`backup: <path>`) before the existing output. Keep the
existing JSON/plain output shape otherwise.

### 4. UI: `builder/ui/src/screens/TurnOn.vue`

In the `off` state, when `status.oldOne` is set, above the button: a `.alert.alert-warning` headed
"Claude Code already has an older {name}", listing "A skill called {slug} that Voice didn't add" when `skill`, and
"These lines in your CLAUDE.md:" followed by each line in its own `<code>` in a list. Under it: "Replace the old one
and Voice backs these up in ~/.claude/voice-backups/{slug}/ first, then turns {name} on. Turning {name} off won't put
them back."
- Buttons: primary "Replace the old one" (busy text "Replacing…") calls `backend.turnOn({ slug, replaceOld: true })`.
  When `skill` is false, a secondary `btn-outline-secondary` "Turn {name} on and keep those lines" calls
  `turnOn({ slug, replaceOld: false })`. When `skill` is true, no plain turn-on button.
- No old one: the screen stays as today, calling `turnOn({ slug, replaceOld: false })`.
- The error alert shows the backend's sentence (`failureMessage` already does when the failure carries one).

## Tests (red first; each must fail on today's code for the right reason)

Core (`builder/core/src/replace_old_tests.rs` or the repo's pattern; scratch homes in temp dirs, never a real one):
- `find`: an unclaimed skill alone; lines alone (one inside Voice markers not counted, `/aria` not matched for slug
  `ari`); both; a skill the install record claims → not found; nothing.
- `replace`: the skill is gone from `skills/`, `<backup>/skill` holds it byte for byte, CLAUDE.md lost exactly the
  found lines and kept `\r\n` endings and other lines; `<backup>/CLAUDE.md` equals the old bytes; mode kept.
- `replace` failure paths leave everything unchanged and no new backup folder: CLAUDE.md changed between backup and
  rewrite (inject with a test hook the way `bring_in_checked`'s `after_copy` does, or a `#[cfg(test)]` seam), a
  symlinked skill folder, a skill that is a file.
- After `replace` then `turn_on`, `turn_off` leaves no old skill and no old lines (it never restores them).

Api (`builder/api/tests/use_it.rs`): status reports `oldOne` with the skill and line text; turning on without
`replaceOld` and an old skill returns the old-skill sentence (not the generic one) and changes nothing; with
`replaceOld` it turns on and the message names the backup; lines-only without `replaceOld` turns on and keeps the
lines. UI (`TurnOn.test.ts`): the warning and the right buttons per case; the request carries `replaceOld`.

Mutation-check each claim (`cp` backup, `cmp`; never `git checkout --`).

## Never

- No real person's name, handle or text anywhere (invented: Ari, "Ask Ari (/ari) before…").
- Never delete the old skill before its copy is verified; never touch a CLAUDE.md line Voice didn't find.
- No new dependency. No shell, network or MCP call added.

## Gate

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked -p voice-core -p
voice-api -p voice-cli` (2 test threads: `RUST_TEST_THREADS=2`), `pnpm -C builder/ui test` and `typecheck`. Don't
commit; Claude reviews and commits.
