# PB8.9b2 fix-up 2: the delta Verifier's two nits

Lane V, runner V1, 2026-11-13. Card `pHz6miQ5`. Builds on `plans/briefs/PB8.9b2-take-newest.md` and
`plans/briefs/PB8.9b2-fixup-1.md` (their Decisions and Never still hold) at the branch head `4176aed`. Codex implements;
Claude reviews and commits. Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`. Invented names only (Wren, Rowan Vale, Ari); no real person anywhere.

Each item is decided; don't re-decide. Red first: each new or changed test fails on `4176aed` for the stated reason,
then passes. Keep the change small; touch nothing else.

## 1. Missing git says so (take.rs)

`builder/save/src/flow/take.rs` `take_newest`: the origin check (fix-up 1, item 2) runs git before
`self.git.check_version()`, so on a computer with no git the person gets "saved copy origin doesn't match the record"
(not a said sentence, so the API's fallback) instead of `text::NEEDS_GIT`. Move the `let github = self.github.ok_or_else(..NOT_SET_UP..)?;`
and `self.git.check_version()?;` lines up to just after the member-folder check and before the origin check. Order
afterwards: record → stamp → member folder → `NOT_SET_UP` → `check_version` → origin → token → account →
confirm_private → … (no HTTP call moves). Test in `builder/save/tests/take.rs`: a `Git::at(<a path that doesn't
exist in the scratch home>)` with a valid member record → the error's text is `voice_save::text::NEEDS_GIT`,
`FakeHttp` saw nothing, the member folder listing, `main` and `save.json` unchanged. Every existing take test keeps
passing; if one depended on the old order, stop and say which.

## 2. The usage line names `voice open` (cli)

`builder/cli/src/main.rs`, the `Some(Command::Open { .. })` arm: the two hand-made usage errors (no folder name
with `--update`; a repo with no slug) and the existing `owner/repo`-with-`--update` refusal build their error from
`Cli::command()`, so clap prints `Usage: voice [COMMAND]`. Build each from the `open` subcommand instead, so the
usage block is the `override_usage` one (`voice open [REPO] [SLUG]` / `voice open --update <SLUG>`). One small
helper in `main.rs` (for example `fn open_usage_error(kind, message) -> !`, using
`Cli::command().find_subcommand_mut("open")` with clap's own `error(..).exit()`, keeping the `voice open` bin name
in the usage) used by all three; no copies. Exit code stays 2, messages unchanged.
Tests in `builder/cli/tests/open.rs`: `updating_requires_a_slug`, `opening_requires_both_the_repo_and_the_slug`,
and the `owner/repo`-with-`--update` test each also assert stderr contains `voice open --update <SLUG>`.

## Not changing (decided)

- The Verifier's third nit (the member-folder check uses `home/.claude/crew/<slug>` while writes go to
  `member.dir`): the API is the only caller and builds both from the same `crew_dir(home)`. Recorded, no code.

## Gate

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save --test take --test git_version` and
`-p voice-cli --test open`. Your sandbox blocks sockets; say which tests it stopped. Don't commit. Report: files
changed, each red you saw, a mutation check per new assertion (`cp` backup, `cmp` restore; never `git checkout --`),
and anything you couldn't do.
