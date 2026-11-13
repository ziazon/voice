# VF13: a backup keeps each file's execute bit

Code change. One file: `builder/core/src/backup.rs` (crate `voice-core`). Commit nothing; pass `-j 2` to
cargo. Skills: `~/.codex/skills/tdd-skill/SKILL.md` (red first) and `~/.codex/skills/code-quality/SKILL.md`.

## Why

`copy_entry` copies every regular file as 0600 and every folder as 0700. A script in a backed-up skill loses
its execute bit. Since PB8.7e (*Replace the old one*), the backup in `~/.claude/voice-backups/<slug>/<stamp>/`
is the only copy of an older skill once Voice deletes the moved-aside folder, so restoring it by hand gives
scripts that don't run.

## The rule

A copied regular file gets mode **0700 when the source's owner execute bit (`0o100`) is set, else 0600**.
Nothing else from the source mode carries over: no group or other bits, no setuid, setgid or sticky bit.
Folders stay 0700, links stay links, and byte verification (`verify_entry`, `verify_entry_with_buffers`)
stays exactly as it is: it does not compare modes.

Read the mode from the **already-opened** source file (`input.metadata()` on the `File` that `open_regular`
returned), never from a second `fs::metadata(source)` by path, so the mode and the bytes come from the same
file.

## The changes

1. **Red first.** Add one test to the `tests` module in `backup.rs`, named
   `keeps_only_the_owners_execute_bit`. Build `scratch.path().join("old")` holding four invented files, set
   each one's mode with `fs::set_permissions(…, fs::Permissions::from_mode(…))` after writing it:

   | file | contents | source mode | expected backup mode |
   |---|---|---|---|
   | `run.sh` | `#!/bin/sh\necho invented\n` | `0o755` | `0o700` |
   | `owner-only` | `invented` | `0o500` | `0o700` |
   | `group-run` | `invented` | `0o654` | `0o600` |
   | `notes.md` | `invented notes` | `0o644` | `0o600` |

   Put `run.sh` inside a nested folder (`old/scripts/run.sh`) so the recursion is covered. Back up with
   `back_up(scratch.path(), "ari", STAMP, &[item(&source)])`, then for each file assert
   `fs::metadata(…).permissions().mode() & 0o7777` equals the expected mode (`0o7777`, not `0o777`, so a
   leaked special bit fails), and that the bytes equal the source's. Run
   `cargo test -j 2 -p voice-core --lib backup::tests::keeps_only_the_owners_execute_bit` and confirm it fails on
   the `run.sh` mode assertion (0o600 against 0o700) before step 2.
2. **Green.** In `copy_entry`'s `kind.is_file()` branch, after `let mut input = open_regular(source)?;`, compute

   ```rust
   // Keep a script runnable for its owner; nothing else from the source mode carries over.
   let mode = if io(source, input.metadata())?.permissions().mode() & 0o100 == 0 {
       0o600
   } else {
       0o700
   };
   ```

   and use `mode` in place of both `0o600` literals in that branch (the `OpenOptions::mode` and the
   `set_permissions`). Change nothing else in the function.

Edit no existing test or assertion and no other file. `copies_nested_tree_bytes_permissions_and_dangling_link`
must still pass unchanged (its files are written 0644, so they stay 0600).

## Check

Run `cargo fmt --all -- --check`, `cargo clippy -j 2 -p voice-core --all-targets --locked -- -D warnings` and
`cargo test -j 2 -p voice-core --locked` (name any test your sandbox stops). Report the files changed and the
red output from step 1.
