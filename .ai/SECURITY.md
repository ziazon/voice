# Security: voice

**Last reviewed:** 2026-10-24. The repo is a kit: scripts that read a principal's local history and
write paraphrase, and Persona Builder, the app growing around them. Nothing here runs as a service or
opens a network port. The only network calls are Persona Builder's saving code (`builder/save/`),
which talks to GitHub alone. Reporting: `SECURITY.md`.

## What could go wrong

- A real person's data reaches a commit: a member folder, a name, a quote, a Slack id.
- A principal's raw history is written somewhere other than their state or work folder.
- A crafted export (oversized zip, odd file names, crafted JSON) crashes a miner or escapes its folder.
- A voice recording is committed.
- Someone's assistant is pushed to a public repo, or to a repo nobody confirmed is private.
- A GitHub sign-in token leaks: into a file, a log, a command line or an error message.

## Controls

| Risk | Control | Proven how |
| --- | --- | --- |
| Personal data committed | The repo ships no members; member-specific values come from config outside the repo; `privacy-check` scans a member's tracked files before the build commits | `tests/crew_*_test.py`, build step 11 |
| Raw history leaks | Every write is an O_EXCL, O_NOFOLLOW 0600 file in a 0700 folder outside the repo; miners refuse a work folder inside a git tree | `tests/crew_hardening_test.py`, `tests/slack_mine_test.py` |
| Principal's own words committed | `crew_decisions.py shingle` (no run of 6 of their words) | build step 11 |
| Secrets in records | Anchored token patterns in the template's and the Slack miner's secret filters drop the record | tests for each shape |
| Crafted exports | Zip size and ratio caps; names flattened; one-line errors instead of tracebacks | `tests/crew_hardening_test.py`, `tests/slack_mine_test.py` |
| Crafted documents (lens step) | Symlinks never followed; images never opened; a 50 MiB file cap; `.docx` zip size and ratio caps and any `<!DOCTYPE`/`<!ENTITY` refused before parsing; `pdftotext` run with an argument list, no shell, and a timeout; an out dir inside a git tree refused | `tests/crew_documents_test.py` |
| Mining the wrong person | `install.sh --no-history` reads no Claude Code history and no export on the Mac it runs on (the adult's, when the principal has no account) | `tests/crew_install_test.sh` |
| A kit weakening filters | `install.sh` refuses kit symlinks and kit-supplied `filters` or `state_dir` | `tests/crew_install_test.sh` |
| An assistant pushed somewhere public | Before every push: the privacy check, then every push address must be a GitHub repo that GitHub's API reads back as `private: true` and `visibility: private`; any other host, a public repo or an unconfirmed one is refused. A new repo is created private and read back before the first push. Saves to a person's own repo go to a `persona/<slug>` branch built from a scratch index, so their branch and work tree are never touched | `builder/save/tests/flow.rs`, `github.rs`, `git.rs` |
| A GitHub token leaks | The token lives only in the macOS Keychain, written through `security -i` on stdin; a `Secret` type hides it from `Debug` and zeroes it on drop; HTTP sends it only to `api.github.com` (the request type takes static header values only); git gets it as an environment header scoped to `https://github.com/` for the backup repo only, with the person's git config, helpers and prompts off; git's errors are shown with the token replaced | `builder/save/tests/keychain.rs`, `ureq_wire.rs`, `git.rs` |
| Audio committed | `.gitignore` blocks audio types and `ref/`, `out/` | `git status --ignored` |
