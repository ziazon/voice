# voice: a kit for building AI assistants

An AI assistant built with this kit answers a decision the way its principal would, says how sure it
is, and hands the question back when it is the principal's. This repo is the kit: the member
template, the guided build, and the shared miners and their tests. It is public, so it holds no
member and no real person's data.

| Path | What | Test |
| --- | --- | --- |
| `crew/_template/` | Member template, filled by `bootstrap/install.sh --name <Name>` | `tests/crew_install_test.sh` |
| `bootstrap/` | The guided build (`PROMPT.md`) and its installer | `tests/crew_install_test.sh` |
| `scripts/crew_decisions.py` | Mine, split, tally, label, score, record, privacy-check | `tests/crew_*_test.py` |
| `scripts/crew_documents.py` | Read text, Word and PDF inputs for the lens | `tests/crew_documents_test.py` |
| `scripts/crew_mail.py`, `crew_decisions.py sources` | Extra sources listed in `private.json`, each read only with a consent date: sent mail from an mbox, the live log, project folders reviewed file by file for the lens | `tests/crew_extra_sources_test.py` |
| `scripts/slack_mine.py`, `slack_select.py` | Optional Slack export miners, driven by `--config` | `tests/slack_*_test.py` |
| `scripts/crew_personas.py` | Optional decision cards, per-person models, blind backtests, panels | `tests/crew_personas_test.py` |
| `docs/spec/growth-loop.md` | The growth loop spec any persona app implements, with its JSON Schemas | `tests/growth_loop_spec_test.py`, `builder/grow/tests/` |
| `builder/core/` | The Persona Builder core library (persona_builder): the member format, the dial, field packs, turning an assistant on and off in Claude Code, the claude.ai Project pack, forget, the template compiled in, and where the app keeps assistants (`~/.claude/crew/<slug>/`) | `builder/core/src/`, `builder/core/tests/` |
| `crew/_fields/` | Field packs: starter decisions and words per kind of work | `builder/core/tests/` |
| `builder/cli/` | The `persona-builder` command | `builder/cli/tests/` |
| `builder/save/` | Saving an assistant (persona_builder_save): an existing repo, a private GitHub repo, or this computer only; GitHub sign-in, the computer's password store, git | `builder/save/tests/` |
| `builder/keystore/` | The password store off a Mac (persona_builder_keystore): Windows' Credential Manager, the Secret Service on Linux; its own crate so both targets are checked from a Mac | `builder/keystore/tests/`, the cross-target checks in `ci-local.sh` |
| `builder/ingest/` | Reading histories into the corpus (persona_builder_ingest): the privacy filters, the privacy check, mining Claude Code transcripts and the claude.ai export, merging and splitting the corpus | `builder/ingest/tests/` |
| `builder/agent/` | The model calls (persona_builder_agent): the agent list, the person's own Claude Code checked and called with no tools, the ledger of what each call cost, the daily cap | `builder/agent/tests/` |
| `builder/steps/` | The build's steps that need no model (persona_builder_steps): chunking the corpus, checking labels, the tally, the distiller's input, scoring the backtest, finding similar past decisions, the shingle check, importing a question round | `builder/steps/tests/`, `builder/cli/tests/steps_parity.rs` |
| `builder/build/` | The build's steps that call a model (persona_builder_build): the interview when the history is thin, drafted through the person's own Claude Code and answered into training records; the shared `run.rs` path for the CLI and app, and the build itself (`persona-builder build`: label, distil, the blind backtest, then the member's `decisions.md` and `measurements.md`) | `builder/build/tests/`, `builder/cli/tests/interview.rs`, `builder/cli/tests/build.rs` |
| `builder/grow/` | Kept answers (persona_builder_grow): the growth loop's records, its rule checks and the private store of signals and cases; drafts refreshes, takes the person's answer, then tests an approved change and keeps or undoes it | `builder/grow/tests/`, `builder/cli/tests/keep.rs`, `builder/cli/tests/refresh.rs` |
| `builder/api/` | The app's commands, without Tauri, `history.rs` (reading the histories), `building.rs` (the build, its stop, spend and the interview), `review.rs` (the report, kept answers and the refresh), and the CLI-parity table | `builder/api/tests/`, `builder/api/tests/assistants.rs`, `builder/api/tests/history.rs`, `builder/api/tests/building.rs`, `builder/api/tests/review.rs`, `builder/api/tests/save.rs`, `builder/api/tests/use_it.rs`, `builder/cli/tests/app_parity.rs` |
| `builder/app/` | The Persona Builder app's window, its locked-down bridge, and `rfd` as the native file window | `builder/app/tests/lockdown.rs` |
| `builder/ui/` | The app's screens as a package another app can mount | `pnpm -C builder/ui test` |

All of it runs with `bash scripts/ci-local.sh` (Python 3.9+ standard library and shellcheck). The
Rust checks (fmt, clippy, tests) need stable Rust and are skipped when `cargo` is missing. The
screens need Node and pnpm; their checks are skipped with a printed line when pnpm is missing. The
Windows and Linux checks of `builder/keystore` need those targets (`rustup target add
x86_64-pc-windows-msvc x86_64-unknown-linux-gnu`) and are skipped, each with a printed line, without.

## Rules that bind every change

- No real people: see `AGENTS.md`. Member-specific values (names, filters, workspaces, ids) come
  from a member's `member.json` and its gitignored `private.json`, or a config path passed in.
- Every file a miner writes goes outside the repo, 0600 in 0700 folders.
- Never print a secret or an environment value; tokens live in the Keychain, never in config or argv.
- Test first: a change to behaviour starts with a failing test.
