# PB8.7b fix-up 3: nothing inside the backup folder is backed up (repeat Verifier, claim 3)

Code change on top of `35c7b02`. Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`. Read `AGENTS.md`
first. At most 2 workers for cargo and tests.

## 1. The self-copy check covers both directions (red first)

`builder/core/src/backup.rs` `back_up` refuses an item whose canonical source is `voice-backups` or an ancestor of
it (`canonical_root.starts_with(&source)`). A source **inside** `voice-backups` (an old `SKILL.md` naming
`~/.claude/voice-backups/<slug>/member.json`, so the member item is `voice-backups/<slug>`) passes, and the copy
copies itself until the path is too long. Refuse that too, with the same `HOLDS_BACKUP_END` sentence:
`canonical_root.starts_with(&source) || source.starts_with(&canonical_root)`.

Red first, a test in `backup.rs` beside `member_holding_backup_folder_refuses_before_stamp`: make
`.claude/voice-backups/ari` first, use it as the `member` source, assert the `"member holds Voice's own backup
folder, so Voice backed nothing up."` refusal and that `voice-backups/ari` still holds nothing.

## 2. `<stamp>` is 0700 whatever the umask

`new_backup` makes `<stamp>` with `DirBuilder::mode(0o700)`, which the umask can still narrow. Inside `back_up`'s
guarded block (so a failure removes `<stamp>`), set its mode to 0700 explicitly again with `fs::set_permissions`,
as the code did before fix-up 2.

## Done when

- Item 1's test was red, then green.
- `cargo fmt --check`, `cargo clippy --all-targets --locked --jobs 2 -- -D warnings` and
  `cargo test --locked --jobs 2 -p voice-core --lib backup -- --test-threads=2` pass.
- Print the red, then the green count.
