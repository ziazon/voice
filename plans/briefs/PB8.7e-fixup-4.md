# PB8.7e fix-up 4: the security review's backup-name finding

Lane V, V1, 2026-11-13. On top of `30aa6eb` (fix-up 3). Same skills and Never list. Test first.

1. **The rolled-back-failed sentence names the wrong backup after a stamp collision.** `backup::new_backup` falls
   back to `<stamp>-2` … `<stamp>-9` when `voice-backups/<slug>/<stamp>/` already exists, but
   `replace_old::rolled_back_failed(slug, stamp)` always names `~/.claude/voice-backups/<slug>/<stamp>/`. When the
   rollback rename fails after such a collision, the one recovery pointer the person gets names a folder that holds
   some other backup. Fix: `rolled_back_failed(slug, stamp, backup_name)`, where `stamp` stays the aside folder's
   stamp (`skills/.<slug>.voice-replacing-<stamp>`, unchanged) and `backup_name` is the real backup folder's name
   (`backup.file_name()`, which is `<stamp>` or `<stamp>-<n>`). The sentence becomes "Voice couldn't put the older
   <slug> skill back. It's at ~/.claude/skills/.<slug>.voice-replacing-<stamp>, and a copy is in
   ~/.claude/voice-backups/<slug>/<backup_name>/." `says` accepts it only when `backup_name` is exactly `stamp`, or
   `stamp` followed by `-` and one digit 2 to 9 (the only names `new_backup` makes), and `message ==
   rolled_back_failed(slug, stamp, backup_name)`. Keep the `~/.claude/` wording: `backup.rs`'s own sentences use it.
   Tests: extend `rollback_failure_keeps_backup_and_aside_and_reports_real_stamp` with a variant that first creates
   `voice-backups/ari/<STAMP>/` (an empty folder, mode 0700), so the backup lands in `<STAMP>-2`: assert the exact
   sentence names `<STAMP>-2`, that `<STAMP>-2` holds the copy and the pre-made `<STAMP>/` is untouched. In
   `rollback_failure_sentence_is_recognized`, accept `-2` and `-9`, and reject `-1`, `-10`, `-x`, a different stamp in
   the backup name, and a backup name with a path separator.

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-core`. Don't commit.
