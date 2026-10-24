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
| `scripts/slack_mine.py`, `slack_select.py` | Optional Slack export miners, driven by `--config` | `tests/slack_*_test.py` |
| `scripts/crew_personas.py` | Optional decision cards, per-person models, blind backtests, panels | `tests/crew_personas_test.py` |
| `docs/spec/growth-loop.md` | The growth loop spec any persona app implements, with its JSON Schemas | `tests/growth_loop_spec_test.py` |
| `builder/core/` | The Persona Builder core library (persona_builder): the member format, the dial, field packs | `builder/core/src/`, `builder/core/tests/` |
| `crew/_fields/` | Field packs: starter decisions and words per kind of work | `builder/core/tests/` |
| `builder/cli/` | The `persona-builder` command | `builder/cli/tests/` |
| `builder/save/` | Saving an assistant (persona_builder_save): an existing repo, a private GitHub repo, or this Mac only; GitHub sign-in, the Keychain, git | `builder/save/tests/` |

All of it runs with `bash scripts/ci-local.sh` (Python 3.9+ standard library and shellcheck). The
Rust checks (fmt, clippy, tests) need stable Rust and are skipped when `cargo` is missing.

## Rules that bind every change

- No real people: see `AGENTS.md`. Member-specific values (names, filters, workspaces, ids) come
  from a member's `member.json` and its gitignored `private.json`, or a config path passed in.
- Every file a miner writes goes outside the repo, 0600 in 0700 folders.
- Never print a secret or an environment value; tokens live in the Keychain, never in config or argv.
- Test first: a change to behaviour starts with a failing test.
