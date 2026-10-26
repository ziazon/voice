# PB7.5a brief: app commands that spend open the ledger only on an existing state folder

Written by Claude (lane V2), 2026-10-25. Spec: Trello card `cjtJmnI6` (filed by V1 from PB7.5's
Verifier pass and security review, inherited from PB7.4). Read the repo's `AGENTS.md` first, then
`builder/agent/src/ledger.rs` (`Session::open`, `Session::open_with_clock`),
`builder/ingest/src/private.rs` (`lock_state`, `lock_existing_state`, `state_directory`),
`builder/api/src/building.rs` (`open`, `build_assistant_in`, `draft_interview_in`),
`builder/api/src/history.rs` (`locked_after`: the same fix, made for the history writers in
PB7.3a) and `builder/api/tests/building.rs` (its `Home` fixture, `revoke_consent_stub`,
`build_rechecks_permission_after_claude_check`, `draft_rechecks_permission_after_claude_check`).
Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo from `builder/` with `CARGO_NET_OFFLINE=true`. **No new dependency.** Public repo:
invented names only (the fixtures' assistant **Wren** for **Robin**, as they are); every test
works under a scratch folder and never touches the real `~/.claude`. No new user-facing sentence:
reuse the existing `PERMISSION` constant.

## The bug

The app commands that run Claude Code check permission (`readable`), check Claude Code
(`claude.check(home)`, which runs `claude --version`), then call `building::open`, which calls
`Session::open`. `Session::open` takes `lock_state`, which **creates** `~/.claude/state/<slug>/`
when it is missing, and then makes its work folder (`agent/`, the `WORK_FOLDER` constant). Forget (`use_it::forget_in`) deletes the
whole state folder, permission included. A forget landing between the first check and the open
leaves a new, empty state folder behind: no data is written and no call is made (the permission
re-check after the open refuses), but a forget's "deleted" must stay deleted.

`building::open` is the one place the app opens a session. Today it serves `build_assistant_in`
and `draft_interview_in`; PB7.5 (not on `main` yet) makes it `pub(crate)` and uses it for
`draft_refresh_in` and `apply_refresh_in` too, so fixing it fixes all four.

## Decisions (made, do not reopen)

1. **A session that never creates the state folder.** In `builder/agent/src/ledger.rs` add
   ```rust
   /// [`Session::open`], but only on a state folder that already exists: a missing one is an
   /// `Error::Io` whose source is `NotFound`, and nothing is created.
   pub fn open_existing(home: &Path, slug: &str, cap: Cap) -> Result<Session>
   ```
   Share the body: move today's `open_with_clock` body into a private
   `fn open_at(home, slug, cap, clock, lock: fn(&Path, &str) -> Result<StateLock>)` (or a
   `create: bool`, whichever reads cleaner), call it with `lock_state` from `open_with_clock` and
   with `lock_existing_state` from `open_existing` (clock `Box::new(SystemTime::now)`). Import
   `lock_existing_state` beside `lock_state`. `open` and `open_with_clock` keep their behaviour:
   the CLI and every existing caller are unchanged. Do not add an `open_existing_with_clock`.
   The work folder (`agent/`) is still made with `private_folder` after the lock, inside the folder the
   lock now proves exists.
2. **The app's one opener uses it.** In `builder/api/src/building.rs`, `fn open` calls
   `Session::open_existing(home, slug, Cap::default())` and maps the errors:
   - `Error::Invalid(message)` where `message == private::BUSY` → `failure(BUSY)` (as today);
   - `Error::Io { source, .. }` where `source.kind() == std::io::ErrorKind::NotFound` →
     `failure(PERMISSION)` (the same arm `history::locked_after` has);
   - anything else → `failure(BUILD_SAVE)` (as today).
   Keep its signature and visibility exactly as they are (`fn open(home: &Path, slug: &str)
   -> Result<Session>`), so PB7.5's `pub(crate)` rebases onto it cleanly. Keep the permission
   re-check after the open in both callers as it is.
3. **Nothing else changes.** No change to `cli/`, `grow/`, `build/`, `history.rs`, `use_it.rs` or
   any screen. Do not touch `lock_state` or `lock_existing_state`.

## Tests (red first, then green)

Write these first, run them, and confirm each fails **on its assertion** against today's code
before changing `ledger.rs` or `building.rs`.

1. `builder/api/tests/building.rs`: a fixture helper `forget_stub(h: &Home)` beside
   `revoke_consent_stub`, built the same way, whose `--version` branch first removes the whole
   state folder, as Forget does:
   `import shutil; shutil.rmtree(os.path.join(os.getcwd(), '.claude/state/wren'))`.
   Then two tests, each taking the `CASES` lock like their neighbours:
   - `build_forgotten_after_claude_check_leaves_no_state_folder`: `Home::new(true)`,
     `h.corpus(12, 32)`, `forget_stub(&h)`, snapshot the member files, run `h.build(...)`; assert
     `!h.path.join(".claude/state/wren").exists()` (the red: today it is recreated), no model
     call (`h.calls() == 0`), the snapshots unchanged, and `safe(&h, result, PERMISSION)`.
   - `draft_forgotten_after_claude_check_leaves_no_state_folder`: `Home::new(true)`,
     `forget_stub(&h)`, `h.draft(false)`; assert the state folder does not exist, `h.calls() == 0`,
     and `safe(&h, result, PERMISSION)`.
   Check `h.calls()` still reads the stub's per-call files from `bin/` (outside the state folder),
   so the count is real after the delete.
2. `builder/agent/tests/lock.rs`: `open_existing_creates_nothing_and_shares_the_lock`, on its own
   `Scratch`: with no state folder, `Session::open_existing(&home, "wren", Cap::default())` is an
   `Err` whose `persona_builder::error::Error::Io` source kind is `NotFound`, and
   `home/.claude/state/wren` still does not exist; then `Session::open(...)` for `wren` succeeds
   and, while it is held, `open_existing` for `wren` is refused with the busy sentence
   ("already working on this assistant"); after dropping it, `open_existing` succeeds and
   `home/.claude/state/wren/agent` exists. Match the file's existing style (`common::*`).

Then make them green with decisions 1 and 2. Then, for each claim separately, a mutation check
(back the file up with `cp`, mutate, run only that test, see red, restore with `cp`, confirm with
`cmp -s`; **never** `git checkout --`):
- `building::open` back on `Session::open` → both api tests red;
- `open_existing` passing the creating lock → the agent test red;
- the `NotFound` arm removed from `building::open` → both api tests red on the message
  (`BUILD_SAVE` instead of `PERMISSION`).
Report each mutation and the failing assertion you saw.

## Records

- `.ai/SECURITY.md` row "The app spending the person's Claude plan, or a stop leaving a call
  running": after its first sentence, add: "Every app command that spends opens the ledger only
  on a state folder that already exists, so a forget's delete is never undone; a missing one is
  refused as needing permission." Add `builder/agent/tests/lock.rs` to that row's test list.
- Do not edit `.ai/handoffs/`; Claude writes the record.

## Done means

- From `builder/`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test -p persona-builder-agent` and `cargo test -p persona-builder-api` green
  (read the exit codes, not the summary). Then `bash scripts/ci-local.sh` from the repo root.
- One commit, message in a file (`git commit -F`):
  `builder: PB7.5a app commands that spend open the ledger only on an existing state folder`,
  with a short body saying why. No AI attribution line.
- Report: the files changed, the red you saw for each new test (the assertion text), the three
  mutation results, and the final exit codes.

## Fix-up pass (after the security review and the Verifier)

Both reviews found the same gap. `lock_state_folder` (`builder/ingest/src/private.rs`) opens the
state folder, then `flock`s it in a second step. A Forget that locks, deletes and unlocks between
those two steps leaves this lock on a deleted folder; everything after it walks the path again,
so `Session::open_at`'s `private_folder(.., WORK_FOLDER)` recreates `~/.claude/state/<slug>/agent/`
(empty: the permission re-check then refuses). PB7.3a's history writers have the same split. The
lock must guard the folder that is at the path, not just the one that was opened.

Decisions (made, do not reopen):

1. **Check the locked folder is still the one at the path.** In `lock_state_folder`, after the
   `flock` succeeds, open the path again with `state_directory(home, slug, false)` and compare the
   two descriptors' device and inode (`File::metadata` on a `try_clone`, then
   `std::os::unix::fs::MetadataExt::{dev, ino}`). Drop the second descriptor straight after.
   - Missing now (the reopen is `NotFound`), or a different folder: with `create == false`, return
     `Err(io(&path, std::io::Error::from(std::io::ErrorKind::NotFound)))`, the same error a missing
     folder gives today, so every caller's existing NotFound arm (the permission sentence) applies.
   - With `create == true` (`lock_state`: Forget and the CLI), return `Err(Error::Invalid(BUSY.into()))`:
     another command changed the folder under it, so "already working on this assistant, try
     again" is the honest answer. Do not retry.
   - Any other error from the reopen or `metadata` is returned as it is.
   Comment it in one line: the lock must hold the folder at the path, not one a forget removed
   between the open and the lock.
2. **A test seam, not a test-only build.** Split `lock_state_folder` into
   `fn lock_state_folder(home, slug, create)` calling a private
   `fn lock_state_folder_after(home: &Path, slug: &str, create: bool, between: &mut dyn FnMut())`
   that runs `between()` after opening the folder and before `flock` (the same pattern as
   `history::locked_after`). Nothing outside this file calls the seam.
3. No other change to `lock_state`, `lock_existing_state` or their callers.

Tests, red first (in the existing `#[cfg(test)] mod tests` of `private.rs`, each on its own
scratch home under `std::env::temp_dir()` with a unique name, removed at the end; make the state
folder with `lock_state` once and drop that lock first):
- `existing_lock_refuses_a_folder_forgotten_before_the_lock`: `between` runs
  `fs::remove_dir_all(<home>/.claude/state/wren)`; `lock_state_folder_after(.., false, ..)` is an
  `Error::Io` with source kind NotFound, and the state folder still does not exist.
- `existing_lock_refuses_a_folder_replaced_before_the_lock`: `between` removes the folder and
  makes a new empty one (`fs::create_dir` then mode 0700); the result is `Error::Io` NotFound and
  the new folder still exists and is empty.
- `creating_lock_refuses_a_folder_replaced_before_the_lock`: the same replacement with
  `create == true`; the result is `Error::Invalid` whose text equals `BUSY`.
- `lock_with_nothing_between_still_locks`: `between` does nothing, both modes return `Ok`, and a
  second `lock_existing_state` while the first is held is `BUSY`.
Before the fix the first three must fail on their assertions (today the lock succeeds). Then make
them green. Mutation checks (cp backup, `cmp -s` restore, never `git checkout --`): the identity
comparison removed (always equal) → the three refusal tests red; the `create == true` branch
returning NotFound instead of BUSY → the creating test red. Report each.

Records: in `.ai/SECURITY.md`, the row "Reading history without permission": after "take it only on
a state folder that already exists (so a forget's delete is never undone)" add ", and check after
locking that the folder they locked is still the one at its path". Add
`builder/ingest/src/private.rs` (tests) to that row's test list.

Commit separately on top (never amend):
`builder: PB7.5a the state lock checks it holds the folder at the path`, message in a file.
If your sandbox refuses the commit, leave the change uncommitted and say so. Run from `builder/`:
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p persona-builder-ingest`, `cargo test -p persona-builder-agent`,
`cargo test -p persona-builder-api` (the api's wire tests may not bind sockets in your sandbox:
say so, Claude runs the full gate).

## Rebase pass (PB7.5 landed on `main`)

PB7.5 is now on `main`: `draft_refresh_in` and `apply_refresh_in` (`builder/api/src/review.rs`)
open the ledger through `building::open`, so they already use `Session::open_existing` after the
rebase. Its two forget tests in `builder/api/tests/review.rs`,
`apply_refuses_forgotten_state_between_check_and_open_without_calls` and
`draft_refuses_forgotten_state_between_check_and_open_without_calls`, assert only
`!h.state("growth").exists()`, the known limit this card removes.

Decisions (made, do not reopen):

1. In both tests replace `assert!(!h.state("growth").exists());` with
   `assert!(!h.path.join(".claude/state/wren").exists());` (use the `Home` fixture's own field and
   slug; read the fixture to confirm the names, and if they differ use its exact equivalents).
   No other test change, no product code change.
2. Red proof: back up `builder/api/src/building.rs` with `cp`, put `fn open` back on
   `Session::open(home, slug, Cap::default())`, run only those two tests, and confirm both fail on
   the new assertion; restore with `cp` and confirm with `cmp -s`. Then run them green.

Commit separately on top (never amend), message in a file:
`builder: PB7.5a the refresh forget tests check no state folder is left`. If your sandbox refuses
the commit, leave it uncommitted and say so. Run from `builder/` with `CARGO_BUILD_JOBS=2`:
`cargo fmt --all -- --check` and `cargo test -p persona-builder-api --test review -- --test-threads=2`.
