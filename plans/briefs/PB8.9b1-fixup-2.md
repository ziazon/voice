# PB8.9b1 fix-up 2: the Verifier's and the security review's findings

Lane V, V1, 2026-11-13. On top of commit `aeb3074` (brief `PB8.9b1-open-saved.md`, fix-up 1 in it). Same skills, same
Never list. Test first; each new test must fail on `aeb3074` for the right reason.

1. **must-fix: a `private.json` by another spelling** (`builder/save/src/flow.rs`, `opening_entries` and
   `stage_open`). The skip compares the exact bytes `private.json`, so a saved `crew/<slug>/Private.json` (or
   `private.jſon`, U+017F) is written, and on a case-insensitive disk `read_member` then reads it as
   `private.json` and merges it: the saved copy supplies settings `member.json` may not carry. Fix in two places:
   (a) the name rule in item 2 refuses any non-ASCII name; (b) after every blob is written and before
   `read_member`, refuse when `symlink_metadata(<staging>/member/private.json)` finds anything (the disk's own
   answer, whatever the spelling): `text::saved_copy_refused(&full, "private.json")`. Tests: a saved
   `crew/wren/Private.json` is refused (on a case-sensitive disk (b) can't fire, so assert the refusal comes from
   whichever check fires and say which in the test); `private.jſon` is refused by the name rule.
2. **MEDIUM: a saved copy can plant a working `.git` folder** (`opening_entries`). Replace the per-part checks with
   one strict rule: every path part is 1 to 255 bytes of ASCII letters, digits, `.`, `_`, `-` or space, doesn't
   start with `.` or a space, and doesn't end with `.` or a space. Keep skipping (never writing) exact
   `.DS_Store` before the rule. That refuses `.git`, `.GIT`, `..`, any dot-file, and the HFS/NTFS look-alikes (they
   are non-ASCII or end in a dot or space). Also pass `-c transfer.fsckObjects=true` to the fetch in
   `git::fetch_main` (git's own tree checks: https://git-scm.com/docs/git-config#Documentation/git-config.txt-transferfsckObjects).
   Tests: `crew/wren/.git/config`, `crew/wren/.GIT/HEAD`, `crew/wren/notes/.hidden`, `crew/wren/a./b`,
   `crew/wren/é.md` are each refused with the home untouched and no staging folder. Make the test repo's tree with
   git plumbing (`mktree`/`commit-tree` and a push to the stand-in bare repo) since a working tree can't hold `.git`.
   With fsck on, the fetch itself refuses the `.git` trees: for those two cases assert the refusal (either
   `fetch_failed` or `saved_copy_refused`) and that nothing was written; then prove the name rule alone with a unit
   test on the part check (make it a `pub(crate) fn` with a `#[cfg(test)]` test in `flow.rs`).
3. **should-fix: raw file names in a sentence** (`opening_entries`). A refused name goes into
   `saved_copy_refused` as is, so a name holding a newline or an escape sequence reaches the terminal. Put the name
   in the sentence only when it passes item 2's rule (for example a denied `id_rsa`, or a link); otherwise use the
   words `a file with an unusual name`. Test: a refused name with `\n` and `\x1b` gives a sentence with neither.
4. **should-fix: a false sentence for the commonest mistake** (`flow.rs` `stage_open`, `builder/api/src/save.rs`
   `open_saved_in`). No `crew/<slug>/member.json` (a mistyped slug) says the copy "holds member.json". Add
   `text::saved_copy_missing(full, slug)`: "Voice found no assistant called <slug> in <full>, so it opened
   nothing." for that case, and `text::saved_copy_unusable(full)`: "The saved copy in <full> has a member.json Voice
   can't use, so it opened nothing." for a slug or private-folder mismatch, a `read_member` failure and a
   `check_brought_member` failure. Both recognised by `is_said`. Update the tests that pin the old wording.
5. **LOW: invisible characters in the builder's name** (`builder/save/src/computer.rs`). Also refuse U+061C,
   U+200B to U+200F, U+202A to U+202E, U+2060 to U+2069 and U+FEFF. Test with U+202E.
6. **nit: empty folders left behind.** When `stage_open` made `~/.claude/crew` itself and then fails, remove it if
   empty (`fs::remove_dir`); the same for `~/.claude/state` in `place_opened`. Tests: a fresh home with no
   `~/.claude/crew` or `~/.claude/state` before a refused open has neither afterwards (the existing refusal test makes
   `.claude/crew` first; add a case that doesn't).
7. **nit: sentences that don't fit.** `open_saved_in`'s "holds files it can't back up. Move that folder" sentence:
   when `~/.claude/state/<slug>` is a file or a link, say "~/.claude/state/<slug> is a file or a link, not a
   folder, so Voice opened nothing. Move it somewhere else, then try again." `text::state_in_use`: say "Move that
   folder somewhere else, then try again." instead of "Forget that assistant first" (no assistant may be listed).
   Update the pinned tests.
8. **A test that can't fail.** `builder/save/tests/open.rs` (the `!private.json` assertion in
   `stages_then_places_without_private_files`) can't fail because Save never pushes that file; remove it there (the
   dedicated test covers it), and add to the token test coverage: a failed `fetch_main` whose stderr contains the
   token and the encoded header yields a sentence with neither (reuse the wrapper's failing mode, extended to
   `fetch`).

Not in scope (recorded, not fixed here): the fetch takes all of `main`'s history (PB8.9b2 needs it for the
fast-forward check); `member.json` keys only the Python kit reads.

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save -p voice-api -p voice-cli`. Don't commit.
