# PB8.7e fix-up 2: the Verifier's findings

Lane V, V1, 2026-11-12. On top of `74d0ea5` (briefs `PB8.7e-replace-old.md`, `PB8.7e-fixup-1.md`). Same skills and
Never list. Test first: each new test fails before its fix, for the reason named.

1. **F1: a failure after the change is reported as "nothing was changed".** In `builder/core/src/replace_old.rs`,
   once CLAUDE.md is rewritten (the journal committed) the replace has happened: a failure of the final
   `remove_dir_all` of the renamed-aside skill must not be an error. Return `Ok(Replaced { …, leftover:
   Some(aside) })` (new field `pub leftover: Option<PathBuf>`, `None` normally). A failed rollback rename (the skill
   cannot be renamed back) returns a new `pub const`-built sentence, `rolled_back_failed(slug)`: "Voice couldn't put
   the older <slug> skill back. It's at ~/.claude/skills/.<slug>.voice-replacing-<stamp>, and a copy is in
   ~/.claude/voice-backups/<slug>/<stamp>/." (keep the backup in that case; recognise it in `says`). In
   `builder/api/src/use_it.rs` the success sentence adds, when `leftover` is set: "Voice couldn't delete the moved
   folder ~/.claude/skills/.<slug>.voice-replacing-<stamp>; you can delete it." The CLI prints `left: <path>` for it.
   Tests: core, a skill holding a `0o555` subfolder with a file in it (restore permissions in the test's cleanup):
   `replace` returns `Ok` with `leftover` set, CLAUDE.md rewritten, backup verified; api, the same setup returns the
   success sentence naming the leftover, never the `on_failed` sentence.
2. **F2: a found last line ending in a lone `\r`.** `existing::old_lines` uses `str::lines`, which keeps a final `\r`
   with no `\n`; the fix-up 1 re-check strips any `\r`. Make the re-check strip exactly what `str::lines` strips (a
   `\r` only when it came before the `\n` that ended the line), so `keep\n/ari\r` replaces and leaves `keep\n`. Also,
   after any failed replace, remove `~/.claude/voice-backups/<slug>/` when it is empty and Voice made it in this call
   (F10). Tests: the `\r`-ended last line replaces; a failed replace leaves no empty `voice-backups/<slug>/`.
3. **F3: the bytes-equal-backup check is untested.** Add a test using the `after_copy` seam that changes a byte of
   CLAUDE.md outside the found lines (found lines stay at their numbers): `replace` fails with `CHANGED`, the skill is
   back, CLAUDE.md is as the seam left it, no backup. Show it fails with that check removed.
4. **F4: a regression on plain turn-on.** `turn_on_in` and the CLI now call `replace_old::find` before every turn-on,
   so a CLAUDE.md over 4 MiB (or another `find` read error) blocks a plain turn-on that worked before. When
   `replace_old` is false, a `find` error is ignored (turn on exactly as before; `install::turn_on` decides). The
   status call keeps reporting `old_one: None` on a `find` error instead of failing (`claude_code_status_in`); update
   `old_status_caps_unicode_text_and_find_errors_are_safe` to match. Tests: a 4 MiB + 1 byte CLAUDE.md turns on with
   `replaceOld: false` in the api and plain `voice on`; status succeeds with `oldOne: null`.
5. **F8:** `builder/ui/src/screens/TurnOn.vue` uses the "nothing in your Claude folder was changed" fallback also for
   `replaceOld: true`. For that call use "Voice couldn't replace the old one. Look in ~/.claude/voice-backups/<slug>/
   before you try again." with a `TurnOn.test.ts` case.
6. **F11:** the CLI refusal reads "… so Voice didn't turn <Name> on. Run voice on <member_dir> --replace-old to back
   it up and move it aside." (no "Choose Replace the old one" in the CLI).

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-core -p voice-api -p voice-cli` (a sandbox
denial of a local connection in `ask.rs` is the sandbox, not you: say so and run the touched suites alone), and
`pnpm -C builder/ui exec vitest run --maxWorkers=2 src/screens/TurnOn.test.ts` if it can run. Don't commit.
