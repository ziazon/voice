# PB8.7e fix-up 1: Claude's review

Lane V, V1, 2026-11-13. On top of the uncommitted PB8.7e work in this worktree (brief `PB8.7e-replace-old.md`).
Same skills, same Never list. Test first; each new test must fail before its fix.

1. **Stale line numbers** (`builder/core/src/replace_old.rs`, the CLAUDE.md rewrite). The numbers come from `find`,
   which reads CLAUDE.md before `back_up`. If the file changes between that read and the backup, the backup matches
   the new bytes, the equality check passes, and the rewrite drops whatever now sits at those numbers. Fix: before
   dropping, check that each found number's line in `claude.text` (split as the rewrite splits it, with its `\n` and
   any `\r` stripped) equals the text `find` recorded for it; any mismatch, or a number past the end, → the `CHANGED`
   error, with the same rollback as the other failures (skill renamed back, backup removed, CLAUDE.md untouched).
   Test: a `#[cfg(test)]` seam, as `after_copy` does, that rewrites CLAUDE.md after `find` but before `back_up` so a
   different line sits at the found number; `replace` must fail with `CHANGED`, leave CLAUDE.md as the seam left it,
   the skill in place, and no new backup folder. Move or add the seam as needed, keeping `replace`'s public signature.
2. **A false sentence after a replace** (`builder/api/src/use_it.rs`, `turn_on_in`). When `replace` succeeded and
   `install::turn_on` then fails, the failure says "…so nothing in your Claude folder was changed", which is false.
   Fix: in that case fail with "Voice moved <the parts> to <~/ backup path>/, but couldn't turn <Name> on. Try
   again; turning <Name> on again won't need to replace anything." using the same parts and path text as the success
   message (factor one helper for both). Test in `builder/api/tests/use_it.rs`: an old skill plus a member whose
   `turn_on` fails after the replace (for example a `voice/assistants/<slug>` folder Voice didn't add, which
   `turn_on` refuses and `find` doesn't report); assert the sentence names the backup and the old skill is in it.

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-core -p voice-api -p voice-cli`. Don't commit.
