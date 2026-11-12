# PB8.7a fix-up 2: the security review and the Verifier

From the security review (one MEDIUM, two LOW taken) and the Verifier (two should-fixes, two nits taken)
on `6c7e738`. Same skills and rules as `plans/briefs/PB8.7a-look-before-making.md`. Commit nothing. Red
test first for each change.

1. **One capped, non-blocking open for every file the finder reads** (`builder/core/src/existing.rs`).
   Add a private helper:
   ```rust
   /// Reads a regular file of at most `cap` bytes through one open: never blocks on a FIFO or device,
   /// never follows a final link unless `follow`. `Ok(None)` when it is missing, `Err` names why not.
   fn read_capped(path: &Path, cap: u64, follow: bool) -> Result<Option<Vec<u8>>>
   ```
   Open with `OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | if follow { 0 } else { libc::O_NOFOLLOW })`
   (`std::os::unix::fs::OpenOptionsExt`; `libc` is already a dependency). `NotFound` → `Ok(None)`.
   `fstat` the open handle (`file.metadata()`): not a regular file → `Err`; then read with
   `take(cap + 1)`, longer than `cap` → `Err`. Use it for:
   - **`CLAUDE.md`**: `follow: true`, cap 4 MiB. A non-regular file gives
     `Error::Invalid("CLAUDE.md isn't a plain file, so Voice can't read it")`; over the cap keeps
     `"CLAUDE.md is too big for Voice to read"`. Replace the FIFO test from fix-up 1 with one that a FIFO
     `CLAUDE.md` with no writer returns that error at once (no thread, no writer: it must not block).
   - **The old skill's `SKILL.md`**: `follow: false`, cap 1 MiB, replacing the `symlink_metadata` check and
     the separate `fs::read` (no gap between the check and the read). Any error → no old folder.
   - **The old folder's `member.json`**: `follow: false`, cap 1 MiB. Parse the bytes with
     `serde_json::from_slice`, run the same `validate_config` that `member::read_member` runs (make it
     `pub(crate)` if it is private), then `Member::from_value`, and compare the slug. Never call
     `read_member` here, so the old folder's `private.json` is never opened. Any error → no old folder.
     Tests: a FIFO `member.json` gives no old folder without blocking; one over 1 MiB gives none; a
     `private.json` beside it that is unreadable (`chmod 000`) doesn't stop the old folder being found.

2. **Never the private folder as the "old folder."** In `find_old_folder`, a candidate folder at or under
   `<home>/.claude/state` is refused (no old folder), checked on the lexical path after `~/` expansion and
   before any open. Test: `--member ~/.claude/state/ari/member.json`, with a valid `member.json` there, gives
   no old folder and opens nothing (prove it with that file at mode 000: the result is the same and no error).

3. **An unclosed Voice marker hides nothing.** In `find_lines`, a `<!-- voice:begin … -->` line only starts a
   skipped block when a `<!-- voice:end … -->` line follows it; with no end after it, the lines after it are
   checked as usual (and the begin line itself is skipped either way). Test: `<!-- voice:begin x -->` with
   no end, then `- Ask Ari (/ari)`, finds that line.

4. **The right sentence when the finder fails.** In `new_assistant_in` (`builder/api/src/assistants.rs`),
   a `existing::find` error is `failure(LOOK_BEFORE)`, not `MAKE`. Test: a `CLAUDE.md` over 4 MiB gives that
   sentence and makes nothing.

5. **The request refuses unknown fields.** Add to `builder/api/tests/assistants.rs` a test that
   `{"name":"Ari","extra":1}` doesn't deserialize as `FindExistingRequest`, beside the other DTO round trips.

6. **`.ai/SECURITY.md`**: replace the PB8.7a sentence with: making an assistant first reads, and never
   writes, the names and file count in `~/.claude/state/<slug>/` (no file there is opened), whether
   `skills/<slug>/` exists and Voice's record claims it, `CLAUDE.md` (a symlinked one is followed, at most
   4 MiB, a plain file only), and from an old skill's `SKILL.md` (at most 1 MiB, never through a link) the
   `member.json` it names (at most 1 MiB, never through a link, never under `~/.claude/state`, its
   `private.json` never opened); a linked Claude, `state` or `skills` folder is read through, as turning on
   already allows.

Run the same checks as the brief's *Done when* (2 workers) and print a short summary.
