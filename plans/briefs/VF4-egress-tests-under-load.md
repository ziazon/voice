# VF4 brief: the gate's timing tests hold on a busy computer

Four tests in `builder/agent/src/egress.rs` (the `tests` module) check a deadline with a small
margin of wall time. When the computer is busy they fail with no change to the code:
`resolvers_in_flight_are_capped` failed 2 of 3 runs of
`cargo test -p voice-agent --lib egress::tests` at load about 20 on about 8 cores (line 1063,
`elapsed < dial_time + 300 ms`), and `header_deadline_is_absolute` stopped a `ci-local.sh` run at
load about 14. `resolution_rejects_without_dialling` (line 1110) and
`resolution_is_checked_and_bounded` (line 1135) have the same shape with `dial_time + 500 ms`.
One Python test has it too: `tests/claude_sandbox_probe_test.py`
`test_idle_tunnel_closes_but_traffic_resets_deadline` patches `TUNNEL_IDLE_TIMEOUT` to 0.3 s and
sends a byte every 0.15 s; it stopped a `ci-local.sh` run and failed 3 of 4 runs alone at load
about 11 (`b'' != b'x'`, once `BrokenPipeError`).

Each test guards a fault that takes far longer than its deadline, so the fix is to widen the gap
between "the deadline worked" and "the fault", not to drop the check:

| Test | The fault it guards | How the fault shows today |
|---|---|---|
| `header_deadline_is_absolute` | the header deadline restarts on each read, so a client that trickles one byte every 50 ms is never closed | `eof()` never sees the close; its read times out |
| `resolvers_in_flight_are_capped` | (a) the dial waits for the resolver instead of `dial_time`; (b) more than `connections` (2) resolvers run at once | (a) the 502 arrives only when `slow` returns; (b) `SLOW_RESOLVES` reaches 3 |
| `resolution_rejects_without_dialling`, `resolution_is_checked_and_bounded` | the dial waits for a slow resolver | the 502 arrives only when `slow` returns |
| `test_idle_tunnel_closes_but_traffic_resets_deadline` | traffic doesn't reset the tunnel's idle deadline (`last_byte` in `scripts/claude_sandbox_probe.py`'s tunnel loop) | the tunnel closes while bytes still flow |

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, and `builder/agent/src/egress.rs`
(`header()` near line 300, `dial()` near line 397, and the tests module from line 660),
`scripts/claude_sandbox_probe.py` (the tunnel loop near line 271) and
`tests/claude_sandbox_probe_test.py`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. **Commit nothing; leave
every change in the working tree.**

## 1. Decisions already made (do not change them)

Change only the `tests` module of `builder/agent/src/egress.rs` and the one test in
`tests/claude_sandbox_probe_test.py`. No production code, no new dependency, no new helper beyond
what is named here.

1. **`slow` waits 10 s, not 2 s.** In `fn slow`, change `Duration::from_secs(2)` to
   `Duration::from_secs(10)`. Every caller abandons it after `dial_time` (`dial()`'s helper thread is
   the one thread `stop()` does not join), so no test waits for it. This keeps the first two
   resolvers in `resolvers_in_flight_are_capped` in flight however slowly the three requests run,
   and puts the fault (waiting for the resolver) far past every bound below and past the client's
   2 s read timeout.
2. **The dial upper bounds get 1 s of slack:**
   - line 1063: `dial_time + Duration::from_millis(300)` becomes `dial_time + Duration::from_secs(1)`;
   - line 1110: `short.dial_time + Duration::from_millis(500)` becomes
     `short.dial_time + Duration::from_secs(1)`;
   - line 1135: `dial_time + Duration::from_millis(500)` becomes `dial_time + Duration::from_secs(1)`.

   Keep `assert!(started.elapsed() >= dial_time)` and `assert_eq!(SLOW_RESOLVES.load(..), 2)` as
   they are.
3. **`header_deadline_is_absolute` gets a longer wait for the close.** Keep `header_time` at 800 ms,
   the first read with `header_time / 2` and its `TimedOut | WouldBlock` assertion, and the lower
   bound `>= header_time - 300 ms`. Change:
   - the read timeout before `eof(&mut socket)` from `header_time + Duration::from_millis(500)` to
     `header_time * 4`;
   - the upper bound from `header_time + Duration::from_millis(500)` to `header_time * 3`.

   With a per-read deadline the trickling writer keeps the connection open until it is shut down,
   so `eof()`'s read still times out and the test still fails.
4. **`test_idle_tunnel_closes_but_traffic_resets_deadline` gets a 1 s window and 0.25 s gaps:**
   patch `TUNNEL_IDLE_TIMEOUT` to `1.0` instead of `0.3`, `time.sleep(0.25)` instead of `0.15`, and
   `range(6)` instead of `range(3)`. Six gaps (1.5 s) outlast one window, so a deadline that traffic
   doesn't reset still closes the tunnel inside the loop; each gap leaves 0.75 s of slack. Keep the
   final `assertEqual(child.recv(1), b'')` (the child's 2 s timeout covers the 1 s window). This adds
   about 2 s to the gate.
5. Leave `Upstream::untouched`'s 300 ms wait (line 733) alone: load makes it less sensitive, never
   flaky.

## 1a. Amendment: one more test (after the first pass)

The first pass landed §1 in the working tree's last commit. Then `ci-local.sh` failed on a fifth
test of the same shape, `builder/api/src/reader_client.rs`
`progress_resets_the_quiet_deadline` (3 of 6 runs of
`cargo test -p voice-api --lib reader_client::tests` failed at load about 14, with "Voice's reader
stopped unexpectedly"). It gives `judge_with_deadlines` a 200 ms `quiet` window and writes a
progress line every 50 ms. The fault it guards: the quiet deadline doesn't restart on each line
(set once for the whole read), so a reader that keeps reporting progress is killed.

5. **`progress_resets_the_quiet_deadline` gets a 1 s window and 0.25 s gaps.** In that test only:
   `std::thread::sleep(Duration::from_millis(50))` becomes `from_millis(250)`; the `quiet` argument
   `from_millis(200)` becomes `from_secs(1)`; the `after` argument `from_millis(100)` becomes
   `from_secs(1)` (it only waits for the end of output after the result line, which arrives at once
   when nothing is wrong). Keep the 8 progress lines and every assertion. Eight gaps (2 s) outlast
   one window, so a deadline set once still kills the reader; each gap leaves 0.75 s of slack. This
   adds about 1.6 s to the gate. Change nothing else in `reader_client.rs`.

For this amendment only, do step 1 for `reader_client::tests` (three runs before the change), make
change 5, then **M5** below, then 10 runs of `cargo test -p voice-api --lib reader_client::tests -j 2`
with the load, then `cargo clippy -p voice-api --all-targets --locked -- -D warnings` and
`cargo fmt --check`. Skip M1 to M4; they are done.

- **M5, a quiet deadline set once:** with a `cp` backup of `reader_client.rs`, in
  `judge_with_deadlines` add `let started = std::time::Instant::now();` just before `loop {`, and
  change the `quiet` branch of `let deadline = …` to `quiet.saturating_sub(started.elapsed())`.
  `progress_resets_the_quiet_deadline` must fail. Restore with `cp`, check with `cmp`.

## 2. Test first, and prove each guard still catches its fault

Run tests with `-j 2` and `RUST_TEST_THREADS=2` (the computer is busy).

1. Before changing anything, run `cargo test -p voice-agent --lib egress::tests -j 2` and
   `python3 tests/claude_sandbox_probe_test.py` three times each, and note how many runs fail and
   which tests (this is the red: expect at least one failure while
   the computer is busy; if all three pass, say so and go on).
2. Make the changes in §1.
3. **Mutations**, each with a `cp` backup of `egress.rs`, then restore with `cp` and check with `cmp`:
   - **M1, per-read header deadline:** in `fn header`, move the deadline into the loop as
     `let deadline = Instant::now() + rules.header_time;` (first line of the loop body; delete the
     line above the loop). `header_deadline_is_absolute` must fail.
   - **M2, dial ignores its deadline:** in `fn dial`, replace `receive.recv_timeout(left)` with
     `receive.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected)`.
     `resolvers_in_flight_are_capped`, `resolution_rejects_without_dialling` and
     `resolution_is_checked_and_bounded` must each fail. Expect this run to take at least 10 s.
   - **M3, no cap on resolvers in flight:** in `fn dial`, change the `fetch_update` closure to
     `|count| Some(count + 1)`. `resolvers_in_flight_are_capped` must fail.
   - **M4, traffic doesn't reset the idle deadline:** with a `cp` backup of
     `scripts/claude_sandbox_probe.py`, delete the `last_byte = time.monotonic()` line inside the
     `for source in ready:` loop (keep the one before the `while`).
     `test_idle_tunnel_closes_but_traffic_resets_deadline` must fail.
4. With every mutation restored, run `cargo test -p voice-agent --lib egress::tests -j 2` and
   `python3 tests/claude_sandbox_probe_test.py` **10 times in a row each**, and report each result and `uptime`'s load before the first and after the last. All 10
   must pass. If any fails, stop and report which test, its message and the load: do not widen any
   bound further.
5. Run `cargo test -p voice-agent -j 2`,
   `cargo clippy -p voice-agent --all-targets --locked -- -D warnings`, and `cargo fmt --check`.
   Don't run `ci-local.sh`; the reviewer runs it.

## 3. Report back

The three red-step runs, each mutation's outcome with its failing message, the 10 runs with the
load, the commands from step 5 with their results, and `git status --short`.
