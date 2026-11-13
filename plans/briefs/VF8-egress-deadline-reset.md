# VF8 brief: the egress header-deadline test accepts a reset

`builder/agent/src/egress.rs` has the test `header_deadline_is_absolute` in its `tests` module. It
opens a connection to the proxy, sends half a `CONNECT` line, and checks that the proxy closes the
connection about `header_time` (800 ms) after accepting it. It runs twice: once silent, and once
with a writer thread that keeps sending one byte every 50 ms (the "trickle" pass), to prove the
deadline doesn't move while bytes trickle in. Each pass ends with `eof(&mut socket)`, which asserts
the next read returns `Ok(0)`, then the two timing bounds.

On 2026-11-09 the trickle pass failed once at its `eof` read with `ConnectionReset` (os error 54)
in place of `Ok(0)`. The cause is confirmed: in the trickle pass the proxy closes a socket that
still has unread bytes, or the writer sends a byte after the close, and the operating system then
answers with a reset in place of an end of stream. Run with the writer's 50 ms changed to 200 µs,
the test fails the same way in 2 of 5 runs (`egress.rs:799`, `ConnectionReset`). Either outcome
means the proxy closed the connection on time. The proxy is right; this is a test-only fix.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/agent/src/egress.rs` (the
`header` function, the test, and the `eof` helper near the top of the `tests` module). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. **Commit nothing; leave
every change in the working tree.** Change only the `tests` module of `builder/agent/src/egress.rs`.
Do not edit any code outside it (except the temporary changes in section 3, which you undo).

## 1. Decisions already made (do not change them)

1. **A new helper, `closed`,** placed right after `eof` in the `tests` module:

   ```rust
   fn closed(socket: &mut std::net::TcpStream) {
       use std::io::Read;
       match socket.read(&mut [0]) {
           Ok(0) => {}
           Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
           other => panic!("the proxy did not close the connection: {other:?}"),
       }
   }
   ```

   Give it one short comment line saying why a reset counts: a socket closed with bytes it never
   read sends a reset, not an end of stream.
2. **`header_deadline_is_absolute` calls `closed(&mut socket)` in place of `eof(&mut socket)`.**
   That is the only call that changes. Every other test keeps `eof`, and `eof` itself is unchanged.
3. Everything else in the test stays as it is: both passes, the 50 ms writer, the first read's
   timeout check, `header_time * 4` as the read timeout before the close, both timing bounds
   (`>= header_time - 300 ms` and `< header_time * 3`), the `shutdown`, the `join`, `untouched` and
   `stop`. A timed-out read (`WouldBlock` or `TimedOut`) must still fail the test, through the
   `panic!` arm.

## 2. Tests first

The behaviour the test proves doesn't change, so "red" is the probe in section 3, step 1: it must
fail before your change and pass after it. Run the test before editing (it passes) and after with
`cargo test --locked -p voice-agent --lib egress::tests::header_deadline_is_absolute -- --exact`.

## 3. Proof (you run each step, then undo it)

For every temporary change: back up `builder/agent/src/egress.rs` with `cp` first, restore it with
`cp` after, and confirm with `cmp` that it is byte-identical to the backup. Never use `git checkout`
or `git stash`. Make each temporary change on top of your finished change.

1. **The reset is now accepted.** In the test's writer, change `Duration::from_millis(50)` to
   `Duration::from_micros(200)`. Run the test 5 times: all 5 pass. (Without your change this fails
   about 2 runs in 5 with `ConnectionReset`.) Restore.
2. **A moving deadline still fails.** In `fn header` (outside the tests), move
   `let deadline = accepted + rules.header_time;` inside the `loop`, as
   `let deadline = Instant::now() + rules.header_time;` (the deadline restarts on every read).
   Run the test: it must fail in the trickle pass. Report the panic message. Restore.
3. **An early close still fails** (a reset that comes too soon is not accepted). In `fn header`,
   change `let deadline = accepted + rules.header_time;` to
   `let deadline = accepted + rules.header_time / 4;`. Run the test: it must fail (the first read, or
   the `>= header_time - 300 ms` bound, sees the early close). Report the message. Restore.
4. Run the test 10 times in a row (a shell loop that stops on the first failure) and report 10 of 10.
5. Run `cargo fmt --all -- --check` and `cargo clippy --all-targets --locked -- -D warnings`.

Report: the diff, each step's result with its panic message, and the `cmp` confirmations.
