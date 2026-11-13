# PB8.9b2 fix-up 1: the review's findings

Lane V, runner V1, 2026-11-13. Card `pHz6miQ5`. Builds on `plans/briefs/PB8.9b2-take-newest.md` (read it first;
its Decisions and Never still hold) and the branch head `1bbef28`. Codex implements; Claude reviews and commits.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`. Invented names only (Wren,
Rowan Vale, Ari); no real person anywhere.

Each item below is decided; don't re-decide. Red first for every behaviour change: the new or changed test fails on
`1bbef28` for the stated reason, then passes.

## 1. The lock comes after the assistant is found (security review LOW, Verifier should-fix 1)

`builder/api/src/save.rs` `take_newest_in` locks `request.slug` before `listed`, so `voice open --update wrne` (no
such assistant) leaves an empty `~/.claude/state/wrne/`. Change the order to `use_it.rs`'s: `listed(home, slug)`
first, then lock **`voice_ingest::private::lock_existing_state(home, &found.member.slug)`** (never create the state
folder). Map the lock error:
- `private::BUSY` → "Voice is already working on <Name>. Wait for it to finish, then try again." Reuse `use_it.rs`'s
  `busy` (make it `pub(crate)`), don't copy the sentence.
- the state folder doesn't exist (read `lock_existing_state` to see exactly which error that is) →
  `text::not_a_member(name)`: with no state folder there is no save record, so this is the non-member case.
- anything else → the existing fallback.
Tests (`builder/api/tests/save.rs`): a slug naming no assistant → the existing "no such assistant" failure and no
`~/.claude/state/<that slug>` afterwards; an assistant with no state folder → the `not_a_member` sentence and still
no state folder; the lock held by another `lock_state` in the test → the busy sentence. The CLI test
`updating_prints_the_refusal_and_exits_one` must keep passing unchanged.

## 2. The fetch goes only to the record's repo (Verifier should-fix 2)

`builder/save/src/flow/take.rs`: right after the record is read and the stamp checked, and before any HTTP or
fetch, require `git remote get-url -- origin` in `backup.git` to equal `https://github.com/<full>.git` exactly
(after `trim_end`), as `save_now` does at `flow.rs` ~313. A missing `backup.git` or any other URL →
`Err(Error::Invalid("saved copy origin doesn't match the record".into()))` (not an `is_said` sentence, so the API
shows its fallback). Test in `builder/save/tests/take.rs`: set B's origin to another `https://github.com/…` URL →
refused, `FakeHttp` saw nothing, the wrapper log has no `fetch`, B byte-identical, `main` and `save.json` unchanged.

## 3. A linked member folder is refused (Verifier should-fix 6)

`take_newest` refuses when `home/.claude/crew/<slug>` is not a plain folder by `symlink_metadata` (a link, a file,
missing), before any git or HTTP call, with an `Error::Invalid` that is not an `is_said` sentence. Test: move B's
member folder elsewhere and link `crew/wren` to it → refused, nothing fetched, the moved folder byte-identical.

## 4. One parent-folder helper (Claude's review)

`take.rs`'s `make_parents` repeats the folder loop in `flow.rs` `write_opened_blob`. Keep one helper in `flow.rs`
(`make_parents(root, rel, made: &mut Vec<PathBuf>)`, `pub(super)` or as the module layout needs) and call it from
`write_opened_blob` (passing a throwaway `Vec`), `take.rs`'s apply and `restore`. Behaviour unchanged; every
existing test keeps passing.

## 5. Tests the Verifier found missing

In `builder/save/tests/take.rs`:
- `listing()` records each entry's mode (`permissions().mode() & 0o7777`) next to its bytes, so a mode change shows.
  Every existing comparison keeps passing (if one doesn't, stop and say which: that is a real finding).
  **Ruling (V1, after Codex's first pass):** `apply_failure_rolls_back_prior_files_and_preserves_record` took its
  `before` listing after `chmod 0500` on `old/` and compared after setting it back to 0700; that difference is the
  test's own. Take `before` before the chmod (then chmod, take, chmod back, compare). Not a code finding; carry on.
- **Rollback of a removed file:** the new version removes `old/a.md` (its folder `old/` empties) and changes
  `SKILL.md`; obstruct the record write the way `record_write_failure_rolls_back_files_and_main` does → the error,
  B byte- and mode-identical (`old/a.md` and `old/` back), `main` and `save.json` unchanged, clean.
- `rewritten_history_is_refused` also asserts `main` and `save.json` are unchanged.
- A mutation check for each new test: remove the line it guards, see it fail, restore by `cp` backup and `cmp`.

## 6. The CLI's usage errors (Verifier nits)

`builder/cli`:
- `voice open --update` with no folder name: a usage error (exit 2) that says "Name the assistant's folder, such as
  wren." (`clap::error::ErrorKind::MissingRequiredArgument`, the same `Cli::command().error(..).exit()` path the
  `owner/repo` refusal uses). `updating_requires_a_slug` asserts that sentence, not `<REPO>`.
- `opening_requires_both_the_repo_and_the_slug` must assert the missing-argument line itself (the line clap starts
  "the following required arguments were not provided", then `<SLUG>` on the next line), not just `<SLUG>`
  anywhere: the new usage override prints `<SLUG>` in every usage error, so today's assertion can't fail.

## Not changing (decided; no code)

- A `private.json` in a new version is skipped, as PB8.9b1 skips it on open (its brief, and this slice's locked
  Decision); `incoming_private_files_are_never_materialized` is right as written.
- An interrupted take (rollback failed, or the process killed between the first rename and the `main` update)
  leaves files that the next take calls changed here. Recorded as a known limit; no crash rule in this slice.

## Gate

`cargo fmt --check` (it passes on `1bbef28`; keep it passing),
`cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save -p voice-api -p voice-cli`. Your sandbox
blocks sockets, so the Ask tests may fail there; say which. Don't commit. Report: files changed, each red you saw,
each mutation check, and anything you couldn't do.
