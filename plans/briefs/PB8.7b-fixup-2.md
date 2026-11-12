# PB8.7b fix-up 2: the review's findings (security review LOW 1, LOW 2 and a nit; Verifier A, C, D, E, F)

Code change on top of `079b8c0`. Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first for every item marked *red first*),
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`. Read `AGENTS.md` first:
invented fixtures only, no fixed paths. The machine is loaded: at most 2 workers for cargo and tests.

Do **not** change `voice_ingest::corpus` (`merge`, `discover`): which duplicate wins is settled and mirrored in
Python; it is out of scope here.

## 1. The screen shows the backup's own refusal (Verifier A; red first)

`bring_in_assistant_in` (`builder/api/src/assistants.rs`) passes only `HISTORY_BEFORE` and `state_refusal` through;
every refusal from `voice_core::backup::back_up` becomes `MAKE` ("…Check there's space…"), which is untrue for a
FIFO or a changed file.

- In `builder/core/src/backup.rs`, give each person-facing refusal a name: `pub const FOLDER_NOT_PLAIN` (the
  voice-backups sentence), `const CHANGED` (the "changed while Voice backed them up" sentence), `const NO_FOLDER`
  (the "couldn't make a new backup folder" sentence), `const NOT_PLAIN_END: &str = " isn't a plain file, so Voice
  backed nothing up."` (used by `not_plain`), and the new sentences of items 3 and 4 below. Add
  `pub fn says(message: &str) -> bool`: true for exactly those sentences (for `not_plain`, `ends_with(NOT_PLAIN_END)`
  with a non-empty name before it). The slug / stamp / item-name checks stay internal (`says` is false for them).
- In `bring_in_assistant_in`, map `Error::Invalid(sentence)` to `failure(&sentence)` when it is one of the step-2
  refusals **or** `voice_core::backup::says(&sentence)`; everything else stays `MAKE`.
- Red first, `builder/api/tests/assistants.rs`: an old state folder holding a FIFO (`libc::mkfifo`, as
  `backup.rs`'s test does): `find_existing_in` says `can_bring_in` true, `bring_in_assistant_in` fails with
  `"pipe isn't a plain file, so Voice backed nothing up."` (or whatever name the test gives the FIFO), and the tree
  is byte-identical to before apart from an empty `voice-backups/<slug>/` (reuse `snapshot`).

## 2. A true sentence when `decisions.jsonl` isn't a plain file (Verifier C; red first)

`bring_in_refusal` gives `HISTORY_BEFORE` ("already holds a history from before Voice") when `decisions.jsonl` is
a link or folder, which is untrue. Add `const CORPUS_NOT_PLAIN: &str = "Voice can't bring this one in: the
decisions.jsonl in its private folder is a link or a folder, not a file. Pick another name for now.";`, return it
for that case, and add it to `bring_in_assistant_in`'s pass-through list. `HISTORY_BEFORE` stays for an existing
`decisions-before-voice.jsonl`. Update the API and CLI tests that assert the old sentence for the linked or folder
`decisions.jsonl` case (`bring_in_refuses_linked_corpus_without_writing`,
`bring_in_refuses_empty_corpus_folder_with_an_older_skill`, and the CLI `existing_explains_…` test if it covers
it); see them fail first.

## 3. Never back up the backup folder itself (Verifier F; red first)

An old `SKILL.md` can name `~/.claude` as its member folder, so the `member` item holds `voice-backups` and the copy
copies itself until the path is too long. In `back_up`, after making `<slug>` and before making `<stamp>`, refuse
any item whose source, resolved with `fs::canonicalize`, is the canonical `voice-backups` folder or an ancestor of
it: `"{item name} holds Voice's own backup folder, so Voice backed nothing up."` (item names are the fixed
`state`/`skill`/`CLAUDE.md`/`member`, so no path is shown; add the sentence to `says`). Nothing is written for it
(the `<slug>` folder may stay). Test in `backup.rs`: a `member` item that is the scratch home's `.claude` refuses
at once and no `<stamp>` folder exists.

## 4. Private from the first moment; existing folders tightened (security LOW 2)

- `new_backup`: make `<stamp>` with `fs::DirBuilder::new().mode(0o700).create(&path)` (keep the `AlreadyExists`
  retry), so it is never 0755.
- `private_parent`: when `voice-backups` or `<slug>` already exists as a real folder, check its owner
  (`std::os::unix::fs::MetadataExt::uid`) equals `unsafe { libc::geteuid() }` (with a `// SAFETY:` comment; it has
  no preconditions); if not, refuse with `FOLDER_NOT_PLAIN`; if so, set its mode to 0700.
- Test: a pre-made `voice-backups` at 0755 is 0700 after a backup.

## 5. Deep folders don't overflow the stack (security LOW 1; red first)

`verify_entry` recurses per folder level with two 64 KiB arrays in each frame; the Tauri command runs on a 2 MiB
thread, so about 15 nested folders abort the app. Keep the recursion but allocate the two buffers once on the heap
(`vec![0u8; 64 * 1024]` each) in a small wrapper and pass `&mut [u8]` slices down. Red first: in `backup.rs`'s
tests, build 60 nested folders with one file at the bottom and run `back_up` inside
`std::thread::Builder::new().stack_size(2 * 1024 * 1024).spawn(…).unwrap().join()`; it must return `Ok` and the
join must not fail. (Before the fix the thread overflows and the test process aborts: that is the red. Note it in
the summary.)

## 6. Keep the backup when the history can't be put back (security nit)

In `bring_in`'s error arm, if `moved` and putting `decisions.jsonl` back fails, **keep** the backup folder (it holds
the only other copy of the old history) and return the original error. Remove the backup only when nothing was
moved or the put-back succeeded.

## 7. One label while backing up (Verifier D; red first)

`NewAssistant.vue`: while `bringingIn`, the *Make it* button still reads "Making it…". Make its label
`checking ? 'Checking…' : saving && !bringingIn ? 'Making it…' : 'Make it'` (still disabled while saving). Red
first in `NewAssistant.test.ts`: in the existing bring-in test, while the request is pending, no button reads
"Making it…".

## 8. Tests the Verifier found missing (Verifier E)

In `backup.rs`'s tests:
- the check catches a changed copy: call `copy_entry` then change one byte of the copy, and assert
  `verify_entry` returns the `CHANGED` sentence; and a copy with an extra file;
- a linked `<slug>` folder (`.claude/voice-backups/ari` a link to a scratch folder) refuses with
  `FOLDER_NOT_PLAIN` and writes nothing into the link's target.

## Done when

- Each *red first* test was red, then green (item 5's red is an abort; say so).
- `cargo fmt --check`, `cargo clippy --all-targets --locked --jobs 2 -- -D warnings`,
  `cargo test --locked --jobs 2 -p voice-core --lib backup -- --test-threads=2`,
  `cargo test --locked --jobs 2 -p voice-api --test assistants -- --test-threads=2 bring_in`,
  `cargo test --locked --jobs 2 -p voice-cli --test assistants -- --test-threads=2`, and
  `pnpm -C builder/ui exec vitest run src/screens/NewAssistant.test.ts --maxWorkers=2` pass. A test that fails only
  with a loopback PermissionDenied in your sandbox is not yours: say so.
- Print a short summary: the reds, then the green counts.
