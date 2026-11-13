# PB8.9b1 fix-up 1: Claude's review

Lane V, V1, 2026-11-13. On top of the uncommitted PB8.9b1 work in this worktree (brief `PB8.9b1-open-saved.md`).
Same skills, same Never list. Test first; each new test must fail before its fix (say so for any that can't).

1. **A finished open reported as failed** (`builder/save/src/flow.rs`, `place_opened`). The last step,
   `fs::remove_dir(&staged.dir)`, returns an error after the member folder, `backup.git` and `save.json` are all in
   place; the api then rolls back its backup as if nothing was opened, while the new assistant stays. Make that
   removal best-effort (`let _ =`); `open_saved_in` already removes the staging folder at the end. Also: when
   `place_opened` itself made `~/.claude/state/<slug>` (it didn't exist before the call) and then fails, remove that
   folder again (only if it is still empty, `fs::remove_dir`). Test: a fresh home where the member move fails (make
   `~/.claude/crew/<slug>` appear between staging and placing, as the existing refusal test does) leaves no
   `~/.claude/state/<slug>`.
2. **A damaged save.json now blocks choosing again** (`flow.rs`, `refuse_member`). Before this slice,
   `choose_local`, `choose_existing` and `choose_github` overwrote an unreadable `save.json`; now they fail on it.
   For the three `choose_*` calls, read the record with `.ok().flatten()` (an unreadable record is not a member
   record); `save_now_from` keeps the strict read. Test: a `save.json` holding `{` lets `choose_local` write the
   local record.
3. **A builder name with spaces at either end** (`builder/save/src/computer.rs`). `valid_computer_name` trims
   before checking, so `" Studio "` is accepted, kept in `save.json` as is, and printed with the spaces. Accept only
   a value equal to its own trim. Test: `" Studio"`, `"Studio "` refused; `read_record` refuses a member record
   whose builder has an edge space.
4. **The opened member isn't checked as bring-in checks one** (`builder/api/src/save.rs`, `open_saved_in`). PB8.7b's
   `check_brought_member` (`builder/api/src/assistants.rs`: `read_member`, `Filters::compile`,
   `anonymize::Rules::for_member`) runs on a brought-in folder; run it on `staged.member_dir` before backing anything
   up (make it `pub(crate)`), and refuse with `text::saved_copy_refused(&full, "member.json")`. Test: a saved
   `member.json` with an anonymizer setting `Rules::for_member` rejects (read `builder/ingest/src/anonymize.rs` for
   one that `read_member` accepts) is refused and leaves the home untouched and no staging folder. If no such value
   exists, say so and skip the test.
5. **Unused parameters** (`save.rs`, `saved_on_github_in`): drop `_claude_home` and `_git`, and their arguments at
   the call sites.
6. **CLI help** (`builder/cli/src/main.rs`, `Command::Open`): match the neighbouring doc comments: "Open an assistant
   you saved on GitHub, on this computer." and a doc comment on each argument (`<owner>/<repo>` and the assistant's
   folder name). Keep the help test passing.

Gate: `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save -p voice-api -p voice-cli`. Socket tests
that the sandbox refuses are fine to report as such. Don't commit.
