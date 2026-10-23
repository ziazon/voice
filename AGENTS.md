# AGENTS.md: voice

Read in this order: `README.md`, `.ai/PROJECT.md`, `.ai/DECISIONS.md`, `.ai/SECURITY.md`, then
`CONTRIBUTING.md`.

This is a public kit for building AI assistants. It ships `crew/_template/`, the guided build in
`bootstrap/PROMPT.md`, and the shared `scripts/` and `tests/`. It ships no members: a member someone
builds lives in their own private copy. Run `bash scripts/ci-local.sh` before anything lands on
`main` (Python 3.9+ and shellcheck).

Gotchas, before anything else:

- **No real people in this repo.** No member folder, no persona, no name, handle, Slack id or
  workspace of a real person, no quote from anyone's messages. Scripts that mine a person's
  history read who to mine and what to call them from a config file outside the repo.
- **Never commit audio.** `.gitignore` blocks audio files; run `git status --ignored` if in doubt.
- **Raw history stays in `~/.claude/state/<slug>/`.** Only paraphrase is ever committed, and only
  in a member's own private copy.
- **No fixed paths.** Scripts find the repo from their own location; never assume where someone cloned it.
