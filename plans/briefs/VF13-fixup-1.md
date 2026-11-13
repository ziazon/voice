# VF13 fix-up 1: a setuid source file stays plain in the backup

Test only. One file: `builder/core/src/backup.rs`. Commit nothing; pass `-j 2` to cargo.
Skill: `~/.codex/skills/tdd-skill/SKILL.md`.

The security review of VF13 found that `keeps_only_the_owners_execute_bit` has no source file with a special
bit set, so its `& 0o7777` mask can't catch a regression that carries setuid into the backup (for example
`source_mode & 0o4700`).

Add one row to that test's `files` array, after `scripts/run.sh`:

```rust
("setuid-run", "invented", 0o4755, 0o700),
```

A non-root owner may set setuid on their own file on macOS and Linux. Do not add a sticky-bit row (macOS refuses
it on a regular file for non-root). Change nothing else.

Run `cargo fmt --all -- --check`, `cargo clippy -j 2 -p voice-core --all-targets --locked -- -D warnings` and
`cargo test -j 2 -p voice-core --locked --lib backup::tests`. Then show the row is load-bearing: copy
`backup.rs` aside with `cp`, change the green code's `0o700` branch to
`io(source, input.metadata())?.permissions().mode() & 0o4700`, run the test and confirm it fails on
`setuid-run` (2496 against 448), then restore with `cp` and confirm with `cmp`. Report the red line.
