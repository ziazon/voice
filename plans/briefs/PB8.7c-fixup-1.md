# PB8.7c fix-up 1: the security review and the Verifier's findings

Code change on top of `plans/briefs/PB8.7c-choose-old-files.md` (read it first; every decision there stands
unless this brief changes it). Commit nothing: the reviewer commits. Same skills, rules and checks as that
brief: `~/.codex/skills/tdd-skill/SKILL.md` (red first for every item), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`,
`~/.codex/skills/frontend/SKILL.md`. Invented fixtures only. At most 2 workers everywhere: the machine is loaded.

## 1. A `private.json` that would break the assistant never comes in

Today `{"filters":{"secret":"("}}`, `{"filters":{"secret":""}}` (joined to `…|(?:)`, which marks all text
secret), `{"anonymizer":7}` and `{"anonymizer":{"bogus":1}}` all pass, and the bring-in reports success; every
later run then fails.

- **The authoritative check, after the copy** (in `voice_api::assistants::bring_in`, right after
  `old_files::bring` succeeds, inside the same closure so a failure takes the existing undo path):
  `voice_core::member::read_member(<made.dir>/member.json)`, then
  `voice_ingest::filters::Filters::compile(&member.filters)` and
  `voice_ingest::anonymize::rules::Rules::for_member(&member, home)` (check the exact public paths). Any error:
  remove `made.dir` and return `Error::Invalid(old_files::CANT_BRING)`.
- **The same rules when offering,** so the screen and `voice existing` show the problem up front instead of a
  failed bring-in. `old_files::offer` and `old_files::bring` take one more argument,
  `private_ok: &dyn Fn(&serde_json::Value) -> bool`, called on a `private.json` that already passed the key
  check; `false` gives the existing `PRIVATE` problem. Core supplies no rule of its own beyond the keys.
- In `builder/ingest/src/filters.rs`, lift the inner `compile(key, pattern)` to a module function and add
  `pub fn check_pattern(key: &str, pattern: &str) -> Result<()>` using it (same builder, same error).
- In `builder/api/src/assistants.rs` add `pub fn offer_old_files(home: &Path, folder: &Path) ->
  voice_core::error::Result<Vec<old_files::Offered>>` that calls `old_files::offer(folder, &|value| private_ok(home, value))`,
  where `private_ok` is true only when:
  - `filters`, if present, is an object; `pasted_prefixes` and `denied_paths` are arrays of non-empty strings;
    every other key is a **non-empty** string that `check_pattern` accepts;
  - `anonymizer`, if present, passes `Rules::from_value(Some(value), home)`.
  `find_existing_in`, the pre-check in `bring_in`, and the CLI (`make` and `existing`) use `offer_old_files`
  instead of calling `old_files::offer` directly; `bring_in` passes the same closure to `old_files::bring`.
- The `private.json` checkbox gets a second line under it, always shown when `private.json` is offered
  (`form-hint`): "Its privacy settings: what the older assistant hid, and any words it let through." The
  CLI's `Can bring in private.json` line gets the same sentence after a colon.

## 2. Hard links, unreadable files, `.DS_Store`, a codebook in another case

In `builder/core/src/old_files.rs`:
- A regular file whose link count is above 1 (`std::os::unix::fs::MetadataExt::nlink`) is a problem:
  `"It shares its contents with another file, so Voice leaves it."` (new constant, in `says`). Check it in
  `check_file` on the `symlink_metadata` (the original during `offer`; the backup's copies always read 1, so
  the API pre-check on the original is what refuses it).
- A file or brief Voice can't open or read (`Error::Io` from `read_capped`, e.g. mode 000) is a problem,
  `"Voice can't read it, so Voice leaves it."` (new constant, in `says`), never an error from `offer`. An
  unreadable `briefs` folder (`read_dir` fails) gives the same problem for `briefs`. Only the old folder
  itself being unreadable stays an error.
- In `briefs`, an entry named exactly `.DS_Store` is skipped: not counted, not checked, not copied.
- The 64 KiB cap applies to a brief whose name equals `codebook.md` ignoring ASCII case
  (`eq_ignore_ascii_case`).

## 3. Write through folder handles, not paths

In `copy_and_check`, open `to` once with `O_RDONLY | O_DIRECTORY | O_NOFOLLOW` and, for briefs, open (or
`mkdirat` then open) `briefs` relative to it the same way; create each target with
`libc::openat(dirfd, name, O_WRONLY | O_CREAT | O_TRUNC | O_NOFOLLOW | O_CLOEXEC, 0o600)` relative to its folder
handle, wrapping the returned fd in `std::fs::File` (`FromRawFd`, an `OwnedFd` first) so it is closed; then
`fchmod` 0600 (`set_permissions` on the file) and write. Each name is one path part (check: no `/`, not `.`
or `..`, not empty) before `openat`. Keep the rest of the function as it is.

## 4. Words that work for the CLI as well as the screen

- `CANT_BRING` becomes: "Voice couldn't bring in the older assistant's own files, so it changed nothing.
  Leave out private.json and autonomy.md, then try again."
- The refusal for one name becomes: "Voice can't bring in {name}, so it changed nothing. Leave it out and try
  again." Keep `says` matching both.

## 5. Tests that pin what the Verifier's mutations showed is unpinned (red first where the code changes)

Core (`builder/core/src/old_files/tests.rs`):
- write side: a link planted at `to/IDENTITY.md` (pointing at a file outside `to`), a link at `to/briefs`, a
  link at `to/briefs/<name>`, and `to` itself a link: each refuses with `CANT_BRING` and the link's target is
  byte-identical and no file appears through it;
- source side: `from` a link refuses before writing;
- `bring` with a name whose re-check has a problem (e.g. a NUL byte put into the backup copy after `offer`)
  refuses with the one-name sentence and writes nothing;
- exact caps are accepted: 1 MiB, `decisions.md` 256 KiB, `codebook.md` 64 KiB, and 64 brief files; one over
  each is left out (some exist; add the accepted side);
- `briefs/CODEBOOK.md` at 64 KiB + 1 is left out with `BIG`;
- a hard-linked `IDENTITY.md` is offered with the new problem; a mode-000 `lens.md` and a mode-000 brief give
  the "can't read" problem and `offer` returns `Ok` (skip the case when running as root);
- a `briefs` holding `.DS_Store` and one plain brief is offered with no problem, and `bring` copies the brief
  and not `.DS_Store`.
- `private_ok` returning false gives `PRIVATE`.

API (`builder/api/tests/assistants.rs`):
- each of `{"filters":{"secret":"("}}`, `{"filters":{"secret":""}}`, `{"anonymizer":7}`,
  `{"anonymizer":{"bogus":1}}` is offered by `find_existing_in` with the `PRIVATE` problem, and ticking it
  refuses before writing (no `voice-backups`);
- the after-copy check is a second line of defence: once section 1's offer rules hold, no `private.json` the
  offer accepts fails it. The existing rollback test (`{"filters":{"personal":7}}`) now refuses at the offer:
  change it to assert the pre-backup refusal (no `voice-backups`). To keep the undo after a late failure
  pinned (crew folder removed, `decisions.jsonl` and `consent.json` back, backup removed), split `bring_in`
  into a private `fn bring_in_checked(…same arguments…, after_copy: &dyn Fn(&Path, &Path) -> Result<()>)`
  (called with the home and the new crew folder) and a `pub fn bring_in` that passes the real check from
  section 1. Add a unit test in `builder/api/src/assistants.rs`'s own `#[cfg(test)]` module (create one at the
  end of the file if there is none) that passes an `after_copy` returning an error and asserts the scratch
  home is byte-identical to before except an empty `voice-backups/<slug>/`, with a `consent.json` and a
  `decisions.jsonl` in the old state folder. Reuse the snapshot helper the API tests use (move it to
  `voice_core::test_support` if it is only in `builder/api/tests/`);
- `BroughtIn.files` comes back in `NAMES` order when the request lists them in another order.

CLI: `voice existing` prints the private.json sentence on its line. Screen: the private.json hint shows only
when `private.json` is offered.

## Done when

- Every new or changed test was red first (or, for tests of code that already behaves, a mutation of the code
  made it fail: say which), then green.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked --jobs 2 -p voice-core -p voice-ingest -p voice-api -p voice-app -p voice-cli -- --test-threads=2`,
  and `pnpm -C builder/ui test` pass (dependencies are installed in `builder/ui/node_modules`).
- Print a short summary: which tests were red and how, then the green counts.
