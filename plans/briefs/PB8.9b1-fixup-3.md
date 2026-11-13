# PB8.9b1 fix-up 3: the repeat Verifier's should-fix

Lane V, V1, 2026-11-13. On top of commit `0bdd67f`. Same skills, same Never list. Test first.

1. **An empty `~/.claude/crew` left behind** (`builder/save/src/flow.rs` `stage_open`, `builder/api/src/save.rs`
   `open_saved_in`). `stage_open` removes a `~/.claude/crew` it made only when staging itself fails. When staging
   succeeds and `open_saved_in` then refuses (an assistant already here, the state refusal, `check_brought_member`,
   or a `place_opened` error), it removes only the staging folder, so an empty `~/.claude/crew` it made stays. Add
   `pub made_crew: bool` to `Staged`; after `open_saved_in` removes the staging folder on a failed open, it also
   runs `fs::remove_dir(home/.claude/crew)` when `made_crew` (only removes it if empty). Do the same when
   `clear_consent` fails: remove the `~/.claude/state/<slug>` and `~/.claude/state` folders `place_opened` made, if
   empty (have `place_opened` report which it made, in the returned value or a small struct; keep it minimal).
   Test (`builder/api/tests/save.rs`): a home with no `~/.claude/crew` and `~/.claude/state/ari` as a file → the
   open fails and `~/.claude/crew` doesn't exist afterwards. Update `assert_no_staging` so it accepts a missing
   `~/.claude/crew` (no `unwrap` on `read_dir`).
2. **nit:** `builder/save/tests/git.rs` `fetch_checks_transferred_objects`: also assert the `-c
   transfer.fsckObjects=true` pair comes before the `fetch` argument.

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save -p voice-api --test save --test git`
(and `-p voice-save --lib`). Don't commit.
