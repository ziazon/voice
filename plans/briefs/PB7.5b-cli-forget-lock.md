# PB7.5b brief: the command-line forget takes the state lock

Written by Claude (lane V1), 2026-10-25. Spec: the PB7.5b card (filed by V2 from PB7.5a's security
review). Read the repo's `AGENTS.md` first, then `builder/cli/src/main.rs` (`fn forget`),
`builder/core/src/forget.rs`, `builder/api/src/use_it.rs` (`forget_in`, `lock_error`, `busy`),
`builder/ingest/src/private.rs` (`lock_state`, `lock_existing_state`, `BUSY`) and
`builder/cli/tests/forget.rs`. Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security
rules: `~/.codex/skills/security-and-hardening/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true` and `-j 2`. Do not commit.

## The bug

`persona-builder forget <member> --yes` deletes `~/.claude/state/<slug>/` with no lock. If the app
(or another CLI command) is building or reading history for the same assistant at that moment, its
later writes can recreate part of the folder, partly undoing the forget. The app's Forget already
deletes under the state lock.

## Decisions (made, do not reopen)

1. **Only `--yes` locks.** The preview (no `--yes`) stays as it is: read-only, no lock.
2. **Order in the CLI's `fn forget` with `--yes`:**
   1. Run `persona_builder::forget::forget(&member_dir, &home, false)` first, as the preview does. It
      keeps refusing a `state_dir` outside `~/.claude/state/<slug>`, a symlink or a non-folder, before
      any lock. If it says the folder isn't there, print the existing "Nothing to forget" line.
   2. Take `persona_builder_ingest::private::lock_existing_state(&home, &member.slug)` and hold it until
      the delete is done. It never creates the folder. If it fails with NotFound (the folder went away
      in between), print the "Nothing to forget" line. If it fails with `private::BUSY`, return that
      error as it is (its text already says another window is working on the assistant; the CLI's
      error printing shows it, exit non-zero). Any other error goes back as it is.
   3. Under the lock, run `forget(&member_dir, &home, true)` and print the existing "Deleted …" line
      from its result.
   No other text changes. The CLI already depends on `persona-builder-ingest`.
3. **No change** to `builder/core/src/forget.rs` or to the app's `forget_in`.
4. **`.ai/SECURITY.md`:** in the row *The app changing the Claude folder or deleting history*, change
   `Forget deletes only` to `Forget, in the app and the CLI, deletes only`, and add
   `builder/cli/tests/forget.rs` to its *Proven how* column.

## Tests, in `builder/cli/tests/forget.rs` (red first)

Run the built binary as the existing test does, with `HOME` set to the scratch home. The state
folders a test makes must pass the lock's own checks (make `~/.claude`, `~/.claude/state` and the
state folder 0700, as the app makes them); the existing test may need the same so it keeps passing.

1. **Busy:** take `persona_builder_ingest::private::lock_state(&home, "ari")` in the test (add
   `persona-builder-ingest` to the CLI's dev-dependencies if needed), run `forget <member> --yes`:
   exit non-zero, stderr holds the `BUSY` text, and both invented files are still there with their
   words. Drop the lock, run it again: exit 0, the folder is gone.
2. **Never creates:** with no state folder, `forget <member> --yes` prints the "Nothing to forget" line,
   exits 0, and `~/.claude/state/ari` still doesn't exist.
3. The existing preview, delete and missing texts are unchanged.

## Done means

- The busy test fails before the change (deletes while locked) and passes after; mutation check:
  drop the lock line (`cp` backup, restore with `cp`, `cmp`), the busy test fails.
- `cargo test -j 2 -p persona-builder-cli` (or the CLI package's real name) and
  `cargo clippy -p <it> --all-targets -- -D warnings` pass; `cargo fmt --check` passes.
- Invented names only (Ari, Robin). Report the files you changed and the commands you ran.
