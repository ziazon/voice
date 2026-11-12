# PB8.7c fix-up 2: keep test helpers out of the library

Code change on top of the uncommitted fix-up 1 in this worktree (`plans/briefs/PB8.7c-fixup-1.md`); keep all of
fix-up 1's work. Commit nothing. Skills: `~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/tdd-skill/SKILL.md`.

Fix-up 1 turned `voice_core::test_support` into `#[doc(hidden)] pub mod test_support`, so the release library
(the app and the CLI) now carries test-only code (`Scratch`, `snapshot`). The repo keeps test helpers out of
release builds.

- Put `builder/core/src/lib.rs` back to `#[cfg(test)] mod test_support;`, and remove the `Default` impl and
  `snapshot` that fix-up 1 added to `builder/core/src/test_support.rs` (it must equal `origin/main`'s file).
- `builder/api/tests/assistants.rs`: keep the file-local `snapshot` it had on `origin/main` instead of
  importing it.
- `builder/api/src/assistants.rs`'s `#[cfg(test)] mod bring_in_tests`: give it its own scratch home and
  snapshot inside that module (a small `Scratch`-like type that makes a fresh folder under
  `std::env::temp_dir()` with a unique name and removes it on drop, as `voice_core::test_support::Scratch`
  does, plus the same `snapshot` walk). Look first for an existing in-crate test helper in `builder/api/src`
  (`grep -rn 'cfg(test)' builder/api/src`) and reuse it if there is one.
- No behaviour changes. The two tests in `bring_in_tests` still pass, and still fail if the consent restore
  in `bring_in_checked`'s undo is removed (check with a `cp` backup and `cmp`, never `git checkout`).

Done when `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, and
`cargo test --locked --jobs 2 -p voice-core -p voice-api -- --test-threads=2` pass (socket-bind failures your
sandbox causes are expected; name them). Print a one-paragraph summary.
