# PB2.7 brief: the state lock never reads busy just because another thread started a program

Persona Builder holds one lock per assistant, so two builds (or a build and a forget) never run on
the same assistant at once. `lock_state_folder_after` in `builder/ingest/src/private.rs` takes it:
it opens the state folder (`O_CLOEXEC`) and calls `flock(fd, LOCK_EX | LOCK_NB)`; `EWOULDBLOCK`
answers `Error::Invalid(BUSY)`. Every caller goes through it: `lock_state`, `lock_existing_state`,
and through them the CLI, the API, `builder/grow` and `builder/agent`'s ledger.

**The bug.** A `flock` belongs to the open file description. When another thread of the same process
starts a child (`std::process::Command`: git, claude, `open`), the child shares every descriptor
from fork until exec; `O_CLOEXEC` closes it only at exec. So if thread A holds the lock while thread
B starts a child, then A drops its lock and someone takes it again before that child has exec'd, the
lock still looks held and the answer is BUSY ("already working on this assistant in another
window"), although nothing is working. The desktop app runs commands on several threads, so a user
can see that refusal.

**Proven (2026-10-26, on `main` `28fa5ea`).** A scratch program calling `lock_state` and dropping it
20,000 times read busy 0 times on its own, and 1,402 and 1,487 times while a second thread ran
`Command::new("true").status()` in a loop. `cargo test -p persona-builder-api --test use_it` failed 4
of 30 parallel runs (`pack_clears_leftover_and_busy_refuses`, `pack_private_sorted_and_replaced_whole`)
and 0 of 30 with `--test-threads=1`. The same cause is behind the 2026-10-25 flake that moved
`one_session_per_assistant_at_a_time` into `builder/agent/tests/lock.rs`.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

Work test-first: write the test, run it and see it fail on its assertion, then write the code. This
is a public repo: invented names only (Ari, Robin, Wren, as the existing tests use), no real person,
no fixed paths. Commit nothing; leave the changes in the worktree.

## Sources (checked 2026-10-26)

- `flock(2)` (macOS man page): "file descriptors duplicated through dup(2) or fork(2) do not result
  in multiple instances of a lock, but rather multiple references to a single lock". So the lock
  stays held while the child still has its copy of the descriptor.
- `open(2)` `O_CLOEXEC`: the descriptor is closed on a successful `execve`, not at fork.
- Rust `std::time::Instant` and `std::thread::sleep`.

## The change

In `builder/ingest/src/private.rs` only:

1. Add a private constant beside `BUSY`, with a one-line doc comment saying why it exists:
   ```rust
   /// How long a refused lock is tried again: a child another thread is starting shares the lock until it execs.
   const INHERITED_LOCK_GRACE: Duration = Duration::from_millis(300);
   ```
   (`Duration` and `Instant` from `std::time`, added to the existing `time::{…}` import.)
2. In `lock_state_folder_after`, replace the single `flock` call with a loop, keeping everything
   around it as it is:
   - take `let started = Instant::now();` just before the first try (after `between()`; `between`
     is still called exactly once);
   - try `flock(directory.as_raw_fd(), LOCK_EX | LOCK_NB)`; on success leave the loop;
   - on an error other than `EWOULDBLOCK`, return `io(&path, source)` at once, as today;
   - on `EWOULDBLOCK`: if `started.elapsed() >= INHERITED_LOCK_GRACE`, return
     `Err(Error::Invalid(BUSY.into()))` as today; otherwise `std::thread::sleep(Duration::from_millis(2))`
     and try again on the **same** descriptor (never reopen the folder in the loop).
   - The `same_folder` check after the lock, and both of its refusals, stay unchanged and are not
     retried.
   Keep the existing `// SAFETY:` comment on the `flock` call.
3. Update the doc comment on `lock_state` (or the `StateLock` comment) with one sentence: a lock
   that is refused is tried again for a moment, so a child another thread is starting doesn't make it
   look held.

In `builder/agent/tests/lock.rs`, replace the first two lines of the module comment with the proven
cause, in one or two lines: a child started on another thread shares the lock until it execs; the
state lock now waits that out (PB2.7). Leave the tests in that file as they are.

Do not change `append_private` (it already blocks with `LOCK_EX`, so a child only delays it), the
`BUSY` text, or any caller.

## Tests (in the `#[cfg(test)]` module of `builder/ingest/src/private.rs`)

1. **New, red first:** `a_lock_dropped_beside_a_starting_child_never_reads_busy`. Make a `Home`, then
   start a thread that runs `std::process::Command::new("true").status().unwrap()` in a loop until an
   `Arc<AtomicBool>` stop flag is set. On the test thread, 2,000 times: `lock_state(&home.0, "wren")`,
   which must be `Ok`, then drop it. Count the failures; set the flag, join the thread, and assert the
   count is 0 with a message naming how many of the 2,000 read busy. Run it before the change and see
   it fail with a non-zero count (expect around 100 to 150), then after the change and see it pass.
2. **New:** `a_held_lock_still_reads_busy_after_the_grace`. Hold `lock_state(&home.0, "wren")`; take
   `Instant::now()`; `lock_existing_state(&home.0, "wren")` must be `Err(Error::Invalid(BUSY))`, and
   the elapsed time must be at least `INHERITED_LOCK_GRACE`, and under 5 seconds.
3. The existing lock tests (`creating_lock_refuses_…`, `existing_lock_refuses_…`, the held-lock BUSY
   loop near them, `a_started_child_keeps_no_lock_once_it_runs`) must pass unchanged.

## Check before you stop

- `cargo test -p persona-builder-ingest` green, the new test seen red before the change.
- `for i in $(seq 1 30); do cargo test -q -p persona-builder-api --test use_it || echo FAIL; done`
  prints no `FAIL`.
- `cargo test -p persona-builder-agent` green.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean (run from
  `builder/`, or wherever `scripts/ci-local.sh` runs them; read it).

Report: the red output of test 1 (its count), the green runs, and the diff.
