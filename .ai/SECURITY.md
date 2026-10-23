# Security: voice

**Last reviewed:** 2026-10-23. The repo is a kit: scripts that read a principal's local history and
write paraphrase. Nothing here runs as a service or opens a network port. Reporting: `SECURITY.md`.

## What could go wrong

- A real person's data reaches a commit: a member folder, a name, a quote, a Slack id.
- A principal's raw history is written somewhere other than their state or work folder.
- A crafted export (oversized zip, odd file names, crafted JSON) crashes a miner or escapes its folder.
- A voice recording is committed.

## Controls

| Risk | Control | Proven how |
| --- | --- | --- |
| Personal data committed | The repo ships no members; member-specific values come from config outside the repo; `privacy-check` scans a member's tracked files before the build commits | `tests/crew_*_test.py`, build step 11 |
| Raw history leaks | Every write is an O_EXCL, O_NOFOLLOW 0600 file in a 0700 folder outside the repo; miners refuse a work folder inside a git tree | `tests/crew_hardening_test.py`, `tests/slack_mine_test.py` |
| Principal's own words committed | `crew_decisions.py shingle` (no run of 6 of their words) | build step 11 |
| Secrets in records | Anchored token patterns in the template's and the Slack miner's secret filters drop the record | tests for each shape |
| Crafted exports | Zip size and ratio caps; names flattened; one-line errors instead of tracebacks | `tests/crew_hardening_test.py`, `tests/slack_mine_test.py` |
| A kit weakening filters | `install.sh` refuses kit symlinks and kit-supplied `filters` or `state_dir` | `tests/crew_install_test.sh` |
| Audio committed | `.gitignore` blocks audio types and `ref/`, `out/` | `git status --ignored` |
