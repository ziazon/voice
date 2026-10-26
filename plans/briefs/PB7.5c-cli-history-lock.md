# PB7.5c brief: the command-line history commands take the state lock

Written by Claude (lane V1), 2026-10-26. Spec: the Trello card *PB7.5c* (filed from PB7.5b's
security review and Verifier pass). Read the repo's `AGENTS.md` first, then
`builder/ingest/src/private.rs` (`BUSY`, `StateLock`, `lock_state`, `lock_existing_state`,
`lock_state_folder_after`, `write_private`, `private_folder`), `builder/cli/src/main.rs` (`fn mine`
to `fn distil_in`, `fn round`, `fn interview_answer`, `fn limit_set` and `fn forget`: the last two
already take `lock_existing_state` and are the pattern), `builder/cli/tests/forget.rs` (how a test
holds the lock with `lock_state` while it runs the binary), `builder/cli/tests/mine.rs`,
`builder/cli/tests/claudeai.rs`, `builder/cli/tests/merge.rs`, `builder/cli/tests/interview.rs` and
`builder/cli/tests/steps_parity.rs`, then the forget row of `.ai/SECURITY.md`.

Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`; the one new sentence:
`~/.codex/skills/plain-language/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true` and `-j 2`.
Do not commit.

## The problem

Ten CLI history commands write into `~/.claude/state/<slug>/` through `write_private` /
`private_folder` / `write_private_at`, which create the state folder, and none takes the state
lock: `mine`, `claudeai titles`, `claudeai mark`, `claudeai mine`, `merge`, `split`, `chunk`,
`distil-in`, `round` and `interview answer`. A forget (the app's, or the CLI's `forget --yes`, both
under the state lock) run while one of them is going deletes the folder, and then the command
writes part of the history back. Every other writer (the app, the CLI's `forget`, `limit set`, and
the spending commands through `Session::open`) already holds the lock.

## Decisions (made, do not reopen)

1. **Each of the ten commands holds the state lock from before its first read of the state folder
   until it returns**, bound as `let _lock = ...;` in the command's own function, so a forget either
   waits its turn (it gets `BUSY`) or has already finished (the command then sees no folder).
   Take it right after `checked_member` / `member_and_home` (and after argument checks that touch
   no files, such as `validate_name`), before `existing_review`, `read_private*`, `list_private`,
   `corpus_at`, `json_at` or any write. Mining `~/.claude/projects` or reading an export may sit
   inside the lock; that is fine.
2. **Which lock.**
   - `mine` and `claudeai titles` may be the first commands to make the state folder (they work on
     a fresh home today, and their tests prove it): they take `lock_state` (creating). Its own
     same-folder check already answers `BUSY` when a forget removes the folder between the open and
     the lock.
   - The other eight (`claudeai mark`, `claudeai mine`, `merge`, `split`, `chunk`, `distil-in`,
     `round`, `interview answer`) only read what an earlier command wrote: they take
     `lock_existing_state` (creates nothing).
3. **A missing state folder on the eight** (`lock_existing_state` returns `Error::Io` whose
   `source.kind()` is `NotFound`; match it as `limit_set` does) becomes a fixed `Error::Invalid`:
   - `claudeai mark` and `claudeai mine`: exactly today's sentence for a missing review,
     `there's no review file yet: run persona-builder claudeai titles first`.
   - `merge`, `split`, `chunk`, `distil-in`, `round`, `interview answer`: a new sentence, with the
     slug filled in:
     `~/.claude/state/<slug> isn't there yet, so there's nothing to read. Start with persona-builder mine, persona-builder claudeai titles or persona-builder interview draft.`
     Put it in one CLI helper, e.g. `fn nothing_yet(slug: &str) -> Error`, next to
     `checked_member`. Nothing is created in either case.
   Every other lock error (`BUSY` included) passes through unchanged, so the person sees the shared
   `BUSY` sentence.
4. **One helper, no copies.** Add next to `checked_member` in `main.rs`:
   `fn history_lock(home: &Path, slug: &str, create: bool, missing: impl FnOnce() -> Error) -> Result<StateLock>`
   (or the equivalent two small helpers): `create` picks `lock_state` / `lock_existing_state`;
   `missing` builds decision 3's error for a `NotFound` from the non-creating lock. Use it in all
   ten commands. Do not change `limit_set` or `forget`.
5. **Dry runs take no lock** and stay as they are (they write nothing and create nothing; the
   existing `dry_run_prints_summary_without_creating_state` test pins it). This applies to `mine`,
   `claudeai titles`, `claudeai mine`, `merge` and `split`, whichever has `--dry-run`. Take the
   lock only when the run will write: `let _lock = if dry_run { None } else { Some(history_lock(...)?) };`.
   `claudeai mark`, `chunk`, `distil-in`, `round` and `interview answer` have no dry run and always
   lock.
6. **No change to the ingest crate**, the app, or any other command's behaviour. No new
   dependency. The CLI's output on success is unchanged.
7. **`.ai/SECURITY.md`:** in the forget row (*The app changing the Claude folder or deleting
   history*), delete the sentence starting `Known limit (PB7.5c):` through `write part of the
   history back`, and say instead, after the sentence about Forget: `The CLI's history commands
   (`mine`, `claudeai titles|mark|mine`, `merge`, `split`, `chunk`, `distil-in`, `round`,
   `interview answer`) hold the same lock while they read and write, so a forget waits for them or
   they find nothing (only `mine` and `claudeai titles` may make the state folder; a dry run takes
   no lock and writes nothing).` Add `builder/cli/tests/history_lock.rs` to the row's tests column.

## Tests first (red, then green)

A new file `builder/cli/tests/history_lock.rs`, using the `forget.rs` / `mine.rs` fixture pattern
(invented member, temp home, `env_clear` plus `HOME` and `PATH`, `Drop` cleanup). For each of the
ten commands, with the smallest valid arguments its existing tests use (borrow their fixtures;
write the input files the command reads into the state folder with `fs::write` and 0600/0700
modes, or with `persona_builder_ingest::private::write_private`):

1. **Busy while the lock is held.** The test takes `lock_state(&home, slug)` and runs the command:
   exit code 1, stderr exactly `persona-builder: <BUSY>\n` (use `private::BUSY`), stdout empty, and
   the command's output file(s) not written (absent, or byte-for-byte unchanged where they existed).
   Drop the lock and run it again: it succeeds. One test per command (ten tests), or one
   table-driven test that names the failing command in its assert message.
2. **Nothing recreated on the eight.** With no `~/.claude/state/<slug>` at all, each of the eight
   exits 1 with decision 3's exact sentence and `~/.claude/state/<slug>` still does not exist.
3. **The two creating commands still work on a fresh home** (already covered by `mine.rs` and
   `claudeai.rs`; confirm they still pass, add nothing).
4. **A dry run is not refused while the lock is held**, for `mine --dry-run` (it prints its dry-run
   summary and creates nothing).

Mutation checks (cp backup, edit, run, restore, `cmp` against the backup; never `git checkout --`),
report each red result:
- remove the lock from `merge`: its busy test goes red;
- make `history_lock` always creating: a test from item 2 goes red (the folder exists);
- take the lock in `mine` even for a dry run: test 4 goes red.

Then `cargo test -p persona-builder-cli -j 2`, `cargo clippy -p persona-builder-cli --all-targets -j 2 -- -D warnings`,
`cargo fmt --all --check`, and finally `bash scripts/ci-local.sh` from the repo root.

## Done means

All ten commands take the lock as decided; the new tests and every existing test green;
`.ai/SECURITY.md` updated as decision 7 says; a short list back of files changed, tests added, the
three mutation results and the gate output's last lines.

## Fix-up pass (after the security review and the Verifier, 2026-10-26)

Both passed with no must-fix. Make exactly these changes, test-first where a test is named; do not
commit.

1. **No empty state folder after a failed `mine` or `claudeai titles`.** In `fn mine`, move
   `projects_path(...)` and `Filters::compile(...)` above the lock; in `fn claudeai_titles`, move
   `Filters::compile(...)` and `claudeai::read_export(...)` above the lock. Neither reads the state
   folder. Red first: a test in `history_lock.rs` that runs `mine --projects <a missing folder>` and
   `claudeai titles --export <a missing file>` on a fresh home, expects exit 1, and asserts
   `~/.claude/state/wren` does not exist.
2. **Prove the claudeai commands lock before they read.** In the busy test, for `claudeai titles`,
   `claudeai mark` and `claudeai mine`, seed a `claudeai-review.tsv` that `read_review` refuses
   (e.g. one line of junk) before taking the lock, and still expect exactly the `BUSY` sentence (a
   read before the lock would give the parse error instead). Keep the second, unlocked run's
   success check for the other seven, and for the three claudeai ones re-seed the good review before
   the unlocked run. Mutation: move `claudeai_titles`'s lock below `existing_review`; this test
   must go red. Restore with `cmp`.
3. **Two helpers, not one with a dead argument.** Replace `history_lock(home, slug, create,
   missing)` with `fn history_lock_new(home: &Path, slug: &str) -> Result<StateLock>` (creating:
   `lock_state`) and `fn history_lock_existing(home: &Path, slug: &str, missing: impl FnOnce() ->
   Error) -> Result<StateLock>` (non-creating, maps `NotFound` to `missing()`). `mine` and
   `claudeai titles` use the first; the eight the second.
4. **A comment** above `mine_dry_run_ignores_busy_state_and_writes_nothing`: the test's own lock
   makes the folder, so "creates nothing" is pinned by `mine.rs`'s
   `dry_run_prints_summary_without_creating_state`; this test pins only that a dry run is not
   refused while the lock is held.
5. **`.ai/SECURITY.md`, the forget row:** replace the sentence added by this slice with:
   `The CLI's history commands (`mine`, `claudeai titles\|mark\|mine`, `merge`, `split`, `chunk`, `distil-in`, `round`, `interview answer`) hold the same lock while they read and write, so a forget is refused as busy until they finish, or they find nothing (of these, only `mine` and `claudeai titles` may make the state folder; a dry run takes no lock and writes nothing). Known limit (PB7.5d): the CLI's `build`, `interview draft` and `keep` read history before they take the creating lock, so a forget in between can leave a recreated state folder holding files made from it.`

Then `cargo test -p persona-builder-cli -j 2`, `cargo clippy -p persona-builder-cli --all-targets
-j 2 -- -D warnings`, `cargo fmt --all --check`. Report the mutation result for item 2 and the red
result for item 1.
