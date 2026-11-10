# VF11: the build stop test gives its stand-in five seconds to start

Test only. One file: `builder/api/tests/building.rs`. Commit nothing; pass `-j 2` to cargo.
Skill: `~/.codex/skills/tdd-skill/SKILL.md` (red first).

## The failure

`stop_second_call_is_busy_then_reuses_first_kept_reply` sometimes panics in the file's `wait_for` helper
with `stub never reached …/stdin1` (line 215), then a second panic at line 520 (`SendError`, the spawned
build thread sending to a receiver the failed test already dropped). It stopped one `scripts/ci-local.sh`
run at load 20 to 25 and failed 1 of 3 runs alone at load 20.

`wait_for` polls every 10 ms for a file the Python stub writes when a call starts, and gives up after 5 s.
On a busy computer, starting the build and the stub can take longer than that. `wait_for` only proves the
stub started; it checks no behaviour. In this test the stub call that writes `stdin1` then
sleeps 30 s (the `time.sleep(30)` the test injects), so a longer wait still lands the stop while that call
runs. The behaviour bounds (the stopped build answers within 10 s, `rx.recv_timeout(Duration::from_secs(10))`
and `assert!(start.elapsed() < Duration::from_secs(10))`) stay exactly as they are.

## The changes

1. **Red first.** Add a test to `building.rs`, named `wait_for_allows_a_slow_stub_to_start`. It takes the `CASES` lock as the other tests do, then a fixture with `Home::new(false)` (its `Drop` removes
   the folder; add no dependency), spawns a thread inside `thread::scope` that sleeps 6 s and then writes a file
   `started` in `h.path`, and calls `wait_for` on that path from the test thread; the scope joins the thread. Run it
   and confirm it fails with the `stub never reached` panic before step 2.
2. **Green.** In `wait_for`, change the bound from `Duration::from_secs(5)` to `Duration::from_secs(30)`,
   and put this comment on the line above the `assert!`:

   ```rust
   // Only proves the stub started; a busy computer can take seconds to start it.
   ```

Edit no existing assertion, no other bound and no other file. Run `cargo fmt --all -- --check`,
`cargo clippy -j 2 -p voice-api --all-targets -- -D warnings` and
`cargo test -j 2 -p voice-api --test building` (name any test your sandbox stops). Report the files
changed and the red output from step 1.
