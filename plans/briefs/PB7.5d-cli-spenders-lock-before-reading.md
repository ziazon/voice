# PB7.5d brief: the CLI's build, interview draft and keep take the state lock before they read

Written by Claude (lane V1), 2026-10-26. Spec: the Trello card *PB7.5d* (filed from PB7.5c's
security review and Verifier pass). Read the repo's `AGENTS.md` first, then
`builder/cli/src/main.rs` (`nothing_yet`, `history_lock_existing`, `fn build`, `fn interview_draft`,
`fn keep_answer`, `cap_for`), `builder/agent/src/ledger.rs` (`Session::open`,
`Session::open_existing`), `builder/build/src/run.rs` (`prepare`), `builder/grow/src/keep.rs`
(`keep`, `keep_locked`), `builder/grow/src/store.rs` (`lock`), `builder/core/src/member.rs`
(`private_state_dir`), `builder/api/src/building.rs` (`open`: the app already does this with
`open_existing`), then `builder/cli/tests/build.rs`, `builder/cli/tests/interview.rs`,
`builder/cli/tests/keep.rs` and `builder/cli/tests/history_lock.rs` (fixtures and the stub
`claude` script pattern), and the forget row of `.ai/SECURITY.md`.

Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`. Run cargo with `CARGO_NET_OFFLINE=true` and `-j 2`. Do
not commit.

## The problem

Three CLI commands read private history and only then take the creating state lock, then write
files made from what they read:
- `build`: `run::prepare` reads the corpus; `claude.check()` runs a child (seconds); then
  `Session::open` (`lock_state`); the build writes Claude's replies into `cache/` and the report.
- `interview draft`: reads the corpus, labels and report; `claude.check()`; `Session::open`; writes
  the questions file.
- `keep`: reads the signal file; then `keep::keep` takes `Store::lock` (`lock_state`) and writes
  `growth/signals.jsonl` and `cases.jsonl` with the signal's text.

A forget (app or CLI) in between succeeds, and the command then recreates
`~/.claude/state/<slug>` holding files made from the history just forgotten.

## Decisions (made, do not reopen)

1. **`build`.** Its corpus must already be in the state folder, so it opens the ledger with
   `Session::open_existing` instead of `Session::open`. Keep today's early `run::prepare` call where
   it is, as the check that fails fast before Claude Code is checked, but do not use its result:
   after the session is open (the lock is held), call `run::prepare` again and use that one for
   everything below. A `NotFound` from `open_existing` (`Error::Io` whose `source.kind()` is
   `NotFound`) becomes `nothing_yet(&member.slug)`; every other error passes through.
2. **`interview draft`.** It may run on an assistant with no state folder yet (the corpus is
   optional), so:
   - Before the first state read, note whether the state folder exists:
     `let existed = persona_builder::member::private_state_dir(&member, &home).is_some_and(|dir| dir.symlink_metadata().is_ok());`
     (`checked_member` already refused a member whose state folder isn't the private one).
   - Move the reads and the plan (records, `answered`, `form`, the labels tally or report,
     `weights`, `weighted`, the short-form thinning) into one local function, e.g.
     `fn draft_plan(home: &Path, member: &Member, book: &Book, args: &DraftArgs) -> Result<DraftPlan>`
     with a small `struct DraftPlan { answered, form, weights, weighted }`, so the code is not
     copied.
   - Call it before Claude Code is checked, as today, for the early answers (*enough* for a short
     form with nothing to ask; *already drafted*).
   - Open the ledger with `Session::open_existing` when `existed`, else `Session::open`. A
     `NotFound` from `open_existing` becomes `nothing_yet(&member.slug)`.
   - Under the lock, call `draft_plan` again and use that plan for the draft and the messages. If
     it now says *enough*, print `text::enough(answered)` and return `Ok(())` without drafting.
     Keep the existing *already drafted* re-check under the lock.
3. **`keep`.** Take `history_lock_existing(&home, &member.slug, || nothing_yet(&member.slug))`
   after `Filters::compile` and the `place` / `validate_json_name` argument checks (no file
   touched) and before `read_private_at_max`, bound to `_lock` until return; call
   `keep::keep_locked` instead of `keep::keep` (it is for a caller holding the lock; `keep` would
   take the lock again in this process and answer busy).
4. **Out of scope:** `consent agree` (it writes only a version and a date, and a person agreeing
   during a forget has agreed afresh), `refresh *`, `spend`, the app, and every other crate. No new
   dependency. Success output unchanged.
5. **`.ai/SECURITY.md`, the forget row:** delete the sentence starting `Known limit (PB7.5d):`
   through `holding files made from it.` and add after the CLI history commands' sentence:
   `The CLI's `build` and `keep` take the lock only on a state folder that already exists, and
   `interview draft` does too when the folder was there at its first look; each reads what it uses
   again once it holds the lock, so a forget while one runs leaves no state folder behind.` Add
   `builder/cli/tests/history_lock.rs` already listed; add the test files you change if they are
   not listed.

## Tests first (red, then green)

Use the stub `claude` pattern from `builder/cli/tests/build.rs` / `interview.rs`: a `#!/bin/sh`
script on `PATH` whose `auth` branch first runs `rm -rf "$HOME/.claude/state/<slug>"` and then
prints `{"loggedIn":true}` (its `--version` branch prints `2.1.292 (Claude Code)`; any other call
appends to a `calls` file next to the script, so a test can assert no Claude call was made). That
deletion is a forget landing between the first read and the lock.

1. **`build`, forgotten during the Claude Code check:** seed a valid corpus (borrow `build.rs`'s
   fixture), run `build`: exit 1, stderr exactly `persona-builder: <nothing_yet sentence>\n`,
   `~/.claude/state/<slug>` does not exist afterwards, no Claude call made, and the member folder's
   files unchanged. Red today (the folder is recreated).
2. **`interview draft`, forgotten during the check, folder there at first look:** seed a corpus;
   run `interview draft`: same assertions as 1. Red today.
3. **`interview draft` on a fresh assistant still works:** no state folder at all, a stub that
   doesn't delete anything and replies like `interview.rs`'s: it drafts and saves the questions
   file (an existing `interview.rs` test may already cover this; if so, name it and add nothing).
4. **`keep` locks before it reads:** hold `lock_state` in the test, run `keep` on a signal name
   that does **not** exist in the state folder: stderr exactly the `BUSY` sentence (a read before
   the lock would say the file doesn't exist). Drop the lock, write a valid signal (borrow
   `keep.rs`'s), run again: it keeps it.
5. **`keep` with no state folder:** exit 1, the `nothing_yet` sentence, no state folder created.

Put them in a new `builder/cli/tests/forget_race.rs`, or extend `history_lock.rs` if its fixture
fits; one file either way.

Mutation checks (cp backup, edit, run, restore, `cmp`; never `git checkout --`), report each red:
- `build` back to `Session::open`: test 1 red;
- `interview draft` always `Session::open`: test 2 red;
- `interview draft` always `Session::open_existing`: test 3 red;
- `keep` back to reading before the lock: test 4 red.

Then `cargo test -p persona-builder-cli -j 2`, `cargo clippy -p persona-builder-cli --all-targets
-j 2 -- -D warnings`, `cargo fmt --all --check`.

## Done means

The three commands lock as decided; the new tests and every existing test green; `.ai/SECURITY.md`
updated; a short list back of files changed, tests added and the four mutation results.
