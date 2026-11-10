# VF10: the stdin refusal test fails when the command stops reading early

Test only. One file: `builder/cli/tests/similar_stdin.rs`. Commit nothing; pass `-j 2` to cargo.
Skill: `~/.codex/skills/tdd-skill/SKILL.md` (red first).

## The failure

`invalid_stdin_is_a_usage_error_before_searching` sometimes panics in the file's `run` helper at
`child.stdin.take().unwrap().write_all(input).unwrap()` with `BrokenPipe` (os error 32). It happened in 2
of 3 `scripts/ci-local.sh` runs on a busy machine (load about 19) and never alone (5 of 5 green).

The likely case is the test's last loop: with no `--stdin`, or with `--stdin` and `--query` together, clap
refuses the arguments and the command exits without reading stdin. If the test thread is slow to write,
the child has already exited and the write meets a closed pipe. The test's own assertions (exit 2, empty
stdout) are what matter; whether the write finished is not.

`builder/cli/tests/record.rs` already handles the same case in its own helper, with the comment
`// A usage error can close stdin before it has been written.` and `let _ =` on the write. Here, ignore
only `BrokenPipe`, so any other write error still fails the test.

## The changes

1. **Red first.** Add a test to `similar_stdin.rs`, `#[cfg(unix)]`, named
   `run_accepts_a_command_that_exits_without_reading_stdin`. It calls the file's `run` helper with
   `Command::new("/bin/sh").args(["-c", "exit 3"])` and an input of `vec![b'x'; 1 << 20]` (1 MiB, larger
   than any pipe buffer, so the write always meets a closed pipe), and asserts the output's
   `status.code()` is `Some(3)`. Run it and confirm it fails with the `BrokenPipe` panic before step 2.
2. **Green.** In `run`, replace the write line with:

   ```rust
   // A usage error can exit before reading stdin, which closes the pipe.
   match child.stdin.take().unwrap().write_all(input) {
       Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {}
       written => written.unwrap(),
   }
   ```

   Import `ErrorKind` with the file's existing `std::{…}` import if that reads better than the full path.

Edit no existing assertion and no other file. Run `cargo fmt --all -- --check`,
`cargo clippy -j 2 -p voice-cli --all-targets -- -D warnings` and
`cargo test -j 2 -p voice-cli --test similar_stdin` (name any test your sandbox stops). Report the
files changed and the red output from step 1.
