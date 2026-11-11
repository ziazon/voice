# VF9 brief: the sign-in stop test waits for its files

`builder/agent/tests/profile.rs` has the test `stopped_sign_in_kills_the_process_group_within_a_second`.
It starts a stand-in `claude` (a shell stub) that writes its own pid to `$here/parent`, starts
`sleep 30 &`, writes that child's pid to `$here/pid`, and waits. A second thread sets the stop flag
after a fixed 200 ms. The test then checks that the sign-in reports `SIGN_IN_STOPPED`, that it took
under a second from the start, and that both pids are gone.

On 2026-11-10, at load 21 on about 8 cores, one `ci-local.sh` run failed it: `Stub::file` panicked
with `NotFound` (`builder/agent/tests/common/mod.rs`, `fs::read_to_string(...).unwrap()`). It passed
3 of 3 runs alone. On a busy computer the 200 ms can end before the stub's shell has written its
files, so the group is killed first and a file never exists. The one-second bound, timed from the
start, has the same weakness. This is a test-only fix: the code under test is right.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/agent/tests/profile.rs` (the
test and its neighbours), `builder/agent/tests/common/mod.rs` (`Stub`, `Scratch`, `RECORD`),
`builder/agent/src/child.rs` (`kill_group`). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. **Commit nothing; leave
every change in the working tree.** Change only this one test in `builder/agent/tests/profile.rs`.
Do not edit `builder/agent/src/` (except the temporary mutation in section 3, which you undo) or
`tests/common/mod.rs`.

## 1. Decisions already made (do not change them)

1. **The stub writes `pid` atomically.** Change the stub body to
   `echo $$ > "$here/parent"\nsleep 30 &\necho $! > "$here/pid.tmp"\nmv "$here/pid.tmp" "$here/pid"\nwait`.
   `parent` is written before `pid` by the same shell, so once `pid` exists both files are whole.
2. **The stop waits for `pid`.** In place of the fixed 200 ms sleep, the stopping thread polls
   `stub.folder.0.join("pid")` every 10 ms until it exists, for at most 10 seconds. Then it takes
   `let stopped_at = Instant::now();`, sets the stop flag, and returns `stopped_at` from the scoped
   thread (`scope.spawn(...)` returns a handle; `join()` it inside the scope after `sign_in_with`
   returns, and hand both the error and `stopped_at` out of `thread::scope`). If the 10 seconds pass
   with no `pid`, it still sets the flag and returns, so the test fails on the missing file below
   rather than hanging.
3. **The one-second bound is timed from the stop.** Take `let returned = Instant::now();` right after
   `sign_in_with` returns (inside the scope, before the `join`), and assert
   `returned.duration_since(stopped_at) < Duration::from_secs(1)`, with a message naming the
   measured duration. Drop the `begun` timer.
4. Everything else stays: the `SIGN_IN_STOPPED` check, and the loop that waits up to 2 seconds for
   each pid to be gone and asserts `kill(pid, 0) == -1` with the `"{name} {pid} survived"` message.
   Keep the `#[cfg(...)]` attributes and the test's name.

## 2. Tests first

The behaviour the test proves doesn't change, so "red" here is the mutation in section 3: it must
fail before and after your change, and your change must pass. Run the test with
`cargo test --locked -p voice-agent --features claude-subscription --test profile stopped_sign_in -- --test-threads=2`
before editing (it passes), then after.

## 3. Proof (you run this, then undo it)

1. Back up `builder/agent/src/child.rs` with `cp`. In `kill_group`, replace `libc::killpg(group, libc::SIGKILL)`
   with `libc::kill(group, libc::SIGKILL)` (only the parent dies). Run the test: it must fail with
   `pid <n> survived`. Restore with `cp` and confirm with `cmp` that the file is byte-identical.
   Never use `git checkout`.
2. Run the test 10 times in a row (a shell loop, stop on the first failure) and report 10 of 10.
3. Run `cargo fmt --all -- --check` and
   `cargo clippy --locked -p voice-agent --features claude-subscription --all-targets -- -D warnings`.

Report: the diff, the pass before and after, the mutation failure text, the `cmp` result, the 10
runs, fmt and clippy.
