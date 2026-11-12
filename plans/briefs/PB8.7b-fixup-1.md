# PB8.7b fix-up 1: one clear "can't" line, one local clock, the bring-in code in its place

Code change on top of `feca5e7` (PB8.7b's build). Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first for item 1), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`. Read the repo's `AGENTS.md` first: invented fixtures only, no fixed
paths. The machine is loaded: at most 2 workers for cargo and tests.

## 1. `voice existing` says "can't" once (behavior; red first)

`builder/cli/src/assistants.rs` `existing` prints `"Voice can't bring this one in. {reason}"`, but both reasons
(`HISTORY_BEFORE` and `state_refusal` in `builder/api/src/assistants.rs`) already start "Voice can't bring …", so
the person reads it twice. Print the reason alone: `println!("{reason}")`.

Red first: in `builder/cli/tests/assistants.rs`
(`existing_explains_why_a_link_or_saved_history_cannot_be_brought_in`) change the assertion near line 480 to
`text.ends_with(&format!("{reason}\n"))` **and** `!text.contains("Voice can't bring this one in. ")`; run it and
see it fail on the second, then fix.

## 2. One local clock: `local_stamp` beside `local_day` (refactor)

`builder/api/src/assistants.rs` `local_stamp` copies `voice_agent::ledger::local_day`
(`builder/agent/src/ledger.rs`) almost line for line, with a second `unsafe extern "C" { fn tzset(); }`.

- In `builder/agent/src/ledger.rs`, add a private `fn local_tm(time: SystemTime) -> Option<libc::tm>` holding the
  existing `seconds`, `tzset` and `localtime_r` call (keep the two `// SAFETY:` comments), make `local_day` use it
  (same output, same UTC fallback), and add `pub fn local_stamp(time: SystemTime) -> String` beside it:
  `YYYY-MM-DD-HHMMSS` from `local_tm`, falling back to `utc_stamp` reshaped as the current code does. Doc comment:
  `/// The person's local time to the second, `YYYY-MM-DD-HHMMSS`.`
- Test in ledger's existing test module style (a new `#[cfg(test)] mod local_stamp_tests` next to the others): for
  a fixed `UNIX_EPOCH + Duration::from_secs(1_794_480_000)`, `local_stamp` is 17 characters, dashes at 4, 7 and 10,
  digits elsewhere, and its first 10 characters equal `local_day` of the same time. (Time-zone independent; never
  set `TZ` in a test.)
- Delete `local_stamp` and its `tzset` extern from `builder/api/src/assistants.rs`; `bring_in_assistant_in` and
  `builder/cli/src/assistants.rs` call `voice_agent::ledger::local_stamp`.
- `builder/api/Cargo.toml`: put `libc = "=0.2.191"` back under `[dev-dependencies]` where it was before PB8.7b,
  if nothing outside `#[cfg(test)]` in `builder/api/src` uses `libc` any more (`grep -n libc builder/api/src`).
  `Cargo.lock` must not change in a way `--locked` refuses.

## 3. The bring-in code sits with the code it belongs to (move only)

In `builder/api/src/assistants.rs` the PB8.7b additions (`HISTORY_BEFORE`, `state_refusal`, `bring_in_refusal`,
`make_and_clear`, `bring_in`, `bring_in_assistant_in`, `bring_in_assistant`) come after
`#[cfg(test)] mod server_message_tests`. Move them, unchanged, to just after `pub fn new_assistant` (before
`consent_text`). Production code stays above every test module.

## 4. One set of request checks (refactor)

`bring_in_assistant_in` repeats `new_assistant_in`'s field checks, `checked_name` call and `shipped_cli_word` error
mapping line for line. Add two private helpers next to `check_field`, and use them in both functions:

- `fn checked_request(request: &dto::NewAssistantRequest) -> Result<String>`: the four `check_field` calls in the
  current order, then `checked_name(&request.name)`; returns the slug.
- `fn cli_word(home: &Path) -> Result<String>`: `shipped_cli_word(home)` with the existing `CLI_FOLDER` /
  `CLI_PLACE` / `MAKE` mapping.

Keep each function's own order of steps (in `new_assistant_in` the older-assistant check still comes before the
CLI word). The `Exists` sentence ("You already have an assistant called {name}. Pick another name, or open that
one.") appears in both: make it one `fn already_have(name: &str) -> dto::Failure`.

## Done when

- Item 1's test was red, then green.
- `cargo fmt --check`, `cargo clippy --all-targets --locked --jobs 2 -- -D warnings`, and
  `cargo test --locked --jobs 2 -p voice-agent --lib ledger -- --test-threads=2`,
  `cargo test --locked --jobs 2 -p voice-api --test assistants -- --test-threads=2 bring_in`,
  `cargo test --locked --jobs 2 -p voice-cli --test assistants -- --test-threads=2` pass (check package and test
  target names in each `Cargo.toml`; a test that needs a loopback socket and fails only with PermissionDenied in
  your sandbox is not yours: say so and move on).
- Print a short summary: the red, then the green counts.
