# PB8.7c fix-up 3: the delta security review's three LOW findings and the delta Verifier's should-fix

Code change on top of `plans/briefs/PB8.7c-choose-old-files.md`, `-fixup-1.md` and `-fixup-2.md` (read them first;
every decision there stands unless this brief changes it). Commit nothing: the reviewer commits. Same skills, rules
and checks: `~/.codex/skills/tdd-skill/SKILL.md` (red first for every item), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`. Invented fixtures only. At most 2 workers everywhere: the machine
is loaded (`--jobs 2`, `--test-threads=2`).

## 1. Compile only the filters Voice reads (`builder/api/src/assistants.rs`, `private_ok`)

Today `private_ok` compiles every key under `filters`, twice. At runtime `Filters::compile`
(`builder/ingest/src/filters.rs`) compiles only `excluded_folders`, `personal` and `secret`; any other key lands in
`member::Filters::extra` and is never compiled. A 1 MiB `private.json` with thousands of junk keys each near the
regex size limit stalls `voice existing`, the screen's offer and the bring-in pre-check.

- Keep the arms for `pasted_prefixes` and `denied_paths` as they are.
- For `excluded_folders`, `personal` and `secret`: the current string check, both compiles, and section 2's rule.
- Any other key: valid only when it is a string (what `read_member` requires), **never compiled**.

## 2. Refuse a filter that matches empty text (`builder/ingest/src/filters.rs`, `check_pattern`)

Fix-up 1 refused `""` because, joined to the template's pattern, it marks all text secret (or all folders excluded).
`"(?:)"`, `"|"` and `"a*"` do the same and pass today.

- `check_pattern(key, pattern)` compiles with the runtime builder (unchanged) and then returns
  `Err(Error::Invalid(format!("member config: filters.{key} matches empty text, so it would match everything")))`
  when the compiled regex `is_match("")`.
- In `private_ok`, call it on the private pattern alone (as now) and keep the joined check. The `pattern.is_empty()`
  test can go, since an empty pattern matches empty text; keep the behaviour.
- `check_pattern` is used only by `private_ok`; `Filters::compile` and `read_member` are unchanged.

## 3. Never write through a hard link planted in the new folder (`builder/core/src/old_files.rs`, `copy_and_check`)

`openat(… O_TRUNC | O_NOFOLLOW …)` refuses a symlink at the target but truncates and writes through a hard link
planted at, say, `to/IDENTITY.md`.

- Before creating each target, `libc::unlinkat(dirfd, name, 0)` relative to the same folder handle; ignore `ENOENT`,
  any other error is the existing I/O error path.
- Then create with `O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC`, 0o600 (drop `O_TRUNC`). If something
  appears between the unlink and the create, `O_EXCL` fails and the bring-in takes its existing undo path.
- Rest of the function unchanged (fchmod 0600, write, the read-back).

## 4. A `briefs` folder Voice can list but not search is a problem, not an error (`old_files.rs`, the Verifier)

A `briefs` folder at mode 0400 passes `read_dir`, then `fs::symlink_metadata` on an entry fails with `EACCES` and
the `?` makes `offer` return `Err` (the screen shows the look-before sentence, `voice existing` fails). Fix-up 1 §2
said only the old folder itself being unreadable stays an error.

- In the `briefs` arm of `check`: an entry error while iterating `read_dir` (`io(&path, entry)`), and a
  `symlink_metadata` error in the shape loop, each give `Checked::problem(READ)` for `briefs` instead of an error.
- In `check_file`: a `symlink_metadata` error gives `Checked::problem(READ)`. (`check` already returns `Ok(None)` for
  a missing top-level name before calling it; a top-level `symlink_metadata` error other than `NotFound` in `check`
  stays an error, since that means the old folder itself can't be searched.)

## 5. Tests (red first)

Core (`builder/core/src/old_files/tests.rs`): a file outside `to`, hard-linked at `to/IDENTITY.md` before `bring`,
keeps its bytes and its link count after a bring-in of `IDENTITY.md`, and `to/IDENTITY.md` holds the old assistant's
bytes with link count 1. Same for a brief hard-linked into `to/briefs/`. A `briefs` folder at mode 0400 holding one
brief: `offer` returns `Ok` with `briefs` offered under the `READ` problem (restore the mode before the scratch
folder is dropped; skip when running as root). `fixup_private_validator_can_refuse` also asserts nothing was written
(the destination is byte-identical to before, through the existing snapshot helper).

API (`builder/api/tests/assistants.rs`, beside the fix-up 1 private-json cases): each of
`{"filters":{"secret":"(?:)"}}`, `{"filters":{"secret":"|"}}`, `{"filters":{"personal":"a*"}}` and
`{"filters":{"excluded_folders":"x?"}}` is offered by `find_existing_in` with the `PRIVATE` problem, and ticking it
refuses before writing (no `voice-backups`). `{"filters":{"future":"("}}` (an unknown key, an invalid regex Voice
never compiles) is offered with no problem and brings in.

Ingest: a unit test for `check_pattern`: `"(?:)"` and `"a*"` refused, `"secret"` accepted.

## Done when

- Every new or changed test was red first, then green (say how each was red).
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked --jobs 2 -p voice-core -p voice-ingest -p voice-api -p voice-cli -- --test-threads=2` pass.
- Print a short summary: which tests were red and how, then the green counts.
