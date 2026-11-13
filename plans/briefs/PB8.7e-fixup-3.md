# PB8.7e fix-up 3: Claude's review of fix-up 2

Lane V, V1, 2026-11-12. On top of `3af63c6`. Same skills and Never list. Test first.

1. **The rolled-back-failed sentence shows a placeholder.** `replace_old::rolled_back_failed(slug)` prints the literal
   text `<stamp>` twice (fix-up 2's brief used `<stamp>` to mean the real one). Make it
   `rolled_back_failed(slug, stamp)` and write the real stamp in both paths; `says` recognises the sentence for any
   valid slug and any stamp of the `back_up` shape (17 characters, `YYYY-MM-DD-HHMMSS`), never by a placeholder.
   Test (none exists yet): add a `#[cfg(test)]` seam in `replace_checked` right after the skill is renamed aside;
   the test's seam makes a non-empty folder at the old skill path (so the rename back fails) and changes a byte of
   CLAUDE.md (so the step fails with `CHANGED`). Assert the exact sentence with the test's stamp, that the backup
   folder is kept, and that the aside folder is still there. A `says` unit test accepts that sentence and rejects it
   with `<stamp>` or a malformed stamp.

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-core -p voice-api -p voice-cli`. Don't commit.
