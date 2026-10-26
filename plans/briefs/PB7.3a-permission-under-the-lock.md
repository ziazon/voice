# PB7.3a brief: history writers check permission under the state lock

Written by Claude (lane V2), 2026-10-25. Spec: Trello card `i2yJpwEG` (a Verifier finding from
PB7.6b's repeat pass). Read the repo's `AGENTS.md` first, then `builder/api/src/history.rs`,
`builder/api/src/use_it.rs` (`forget_preview_in`, `forget_in`), `builder/api/src/assistants.rs`
(`read_consent`, `agree_consent_in`), `builder/ingest/src/private.rs` (`state_directory`,
`lock_state`, `private_folder_exists`) and `builder/api/tests/history.rs` (its `Home` fixture).
Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true`. **No new dependency.** Public repo: invented names only
(the assistant **Ari** for **Robin**); every test works under a scratch folder and never touches
the real `~/.claude`. No new user-facing sentence: reuse the existing `PERMISSION` constant.

## The bug

Each of the six history writers in `history.rs` (`read_claude_code_in`, `use_claudeai_export_in`,
`list_claudeai_chats_in`, `mark_claudeai_chats_in`, `read_claudeai_in`, `merge_histories_in`)
calls `readable()` (which checks `consent.json` at the current version) and only then `lock()`.
Forget (`use_it::forget_in`) takes the same lock and deletes `~/.claude/state/<slug>`, permission
included. A forget between the check and the lock lets the writer take the lock, which
**recreates** the state folder (`lock_state` creates it), and write history with no permission on
record.

## Decisions (made, do not reopen)

1. **A lock that never creates the folder.** In `builder/ingest/src/private.rs` add
   `pub fn lock_existing_state(home: &Path, slug: &str) -> Result<StateLock>`: the same as
   `lock_state` but opening the state folder with `create: false`, so a missing folder is an
   `Error::Io` whose source is `NotFound`. Share the body: move today's body into a private
   `fn lock_state_folder(home, slug, create: bool)` and make both public functions one-line calls.
   Doc comment: "Lock the member's state folder only if it already exists, or refuse when another
   build holds it." `lock_state` and its callers are otherwise unchanged.
   Why: permission lives inside that folder, so a history writer never has a reason to create it,
   and a forget's "deleted" must stay deleted.
2. **One helper takes both, in order.** In `history.rs` replace `fn lock` with
   ```rust
   fn locked(home: &Path, slug: &str) -> Result<(Member, private::StateLock)> {
       locked_after(home, slug, &mut || {})
   }
   fn locked_after(
       home: &Path,
       slug: &str,
       between: &mut dyn FnMut(),
   ) -> Result<(Member, private::StateLock)>
   ```
   `locked_after` runs `readable(home, slug)?`, then `between()`, then
   `private::lock_existing_state(home, slug)`, mapping errors: `Error::Invalid(text)` with
   `text == private::BUSY` → `BUSY`; `Error::Io { source, .. }` with `source.kind() == NotFound` →
   `PERMISSION`; anything else → `SAVE`. Then, holding the lock, it checks
   `crate::read_consent(home, slug).agreed` again and answers `failure(PERMISSION)` when false
   (the lock drops on return). It returns the `Member` from `readable` and the lock.
   `between` is the test seam (a forget in the gap); production passes the no-op through `locked`.
   Give `locked_after` a one-line comment saying so.
3. **Every writer uses it.** Each of the six writers starts with
   `let (member, _lock) = locked(home, &request.slug)?;` (or `slug`; `_member` / drop the binding
   where the member is unused) in place of its `readable(...)` + `lock(...)` pair. Keep each
   writer's other checks in their current order relative to the lock; in `mark_claudeai_chats_in`
   the `NO_CHATS` check stays before the lock, so it becomes: `readable` → `NO_CHATS` check →
   `locked`. That calls `readable` twice there; accept it rather than adding a second helper.
   The read-only commands (`can_read_in`, `claude_code_folders_in`, `claudeai_export_status_in`)
   take no lock and stay as they are. After this, `lock_state` is no longer called from
   `history.rs`, and no history writer calls `readable` and a lock separately.
4. **Tests (red first),** in a `#[cfg(test)] mod tests` at the bottom of `history.rs` (unit tests,
   because the seam is private). Build a scratch home the way `tests/history.rs`'s `Home::new(true)`
   does: a unique folder under `std::env::temp_dir()` (process id + an `AtomicUsize`), then
   `crate::new_assistant_in` with Ari for Robin (they/them both), then
   `crate::agree_consent_in(&home, SlugRequest { slug: "ari".into() })`. Remove the folder at the end
   (a small `Drop` guard, as in the integration test). Two tests:
   - `a_forget_between_the_check_and_the_lock_is_refused_and_recreates_nothing`: call
     `locked_after(home, "ari", between)` where `between` runs
     `crate::forget_preview_in` then `crate::forget_in` with that preview's `files` count (assert
     the forget succeeds). Expect `Err` whose message is `PERMISSION`, and expect
     `home/.claude/state/ari` **not to exist** afterwards (`symlink_metadata` is `NotFound`).
   - `permission_withdrawn_between_the_check_and_the_lock_is_refused`: `between` removes only
     `home/.claude/state/ari/consent.json` (the folder stays). Expect `Err` with `PERMISSION`, and
     the state folder's file list unchanged apart from `consent.json` being gone (nothing written).
   - Also one happy-path assertion: `locked(home, "ari")` with consent returns `Ok`, the member's
     slug is `ari`, and while it is held `private::lock_state(home, "ari")` answers the `BUSY`
     error (proving the returned lock is the real one).

   Each test must fail on its assertion before the fix exists (write `locked_after` as the old
   check-then-creating-lock behaviour first if you need it to compile, watch the first two go red,
   then make them green). The existing integration tests in `builder/api/tests/history.rs`
   (permission, BUSY, elsewhere, round trips) must all still pass unchanged.

## Done means

- `cargo test -p persona-builder-api -p persona-builder-ingest` green, and
  `cargo clippy -p persona-builder-api -p persona-builder-ingest --all-targets -- -D warnings` and
  `cargo fmt --check` clean, from the repo root (the workspace `Cargo.toml` is there).
- `bash scripts/ci-local.sh` green from the repo root.
- Commit nothing; leave the changes in the worktree. Report the red output you saw for each new
  test before the fix, and the green run after.
