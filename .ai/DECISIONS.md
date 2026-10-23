# Decisions: voice

## 2026-10-23: a public kit with no members

- **The repo ships no member.** Only `crew/_template/` and the build. Builders keep their member in a
  private copy of their own; a fork of a public repo is public, so the README says not to fork.
- **Miners are generic.** A script that mines a person's messages takes the people, their labels,
  the workspace and any filters from a config file outside the repo, and its tests use fictional
  fixtures.
- **No fixed paths.** Scripts locate the repo from their own path; docs never name a clone folder.
- **The folder keeps its name.** `crew/` and `crew_decisions.py` stay as they are; the docs say
  "AI assistant" and "member".

## 2026-10-23: a route for a principal with no Claude history

- **Two sources replace the history:** the principal's own documents, distilled into a lens
  (`documents` plus `lens-brief.md`), and question rounds answered on a form an adult relays
  (`round`). The kit names no principal; the case behind it is anyone without a Claude account of
  their own, such as someone under 18.
- **A round trains unless imported with `--heldout`;** a held-out round keeps that split through
  `merge --rounds` and never moves the history's cutoff, so the existing backtest scores the
  model blind on it. A whole round is held out or none of it.
- **`install.sh --no-history` never mines the Mac it runs on.** That Mac's history is the adult's.
- **PDF text comes from poppler's `pdftotext` when it is installed,** not from a Python
  dependency: the kit stays standard-library only, and a missing tool is a skip that says how to
  fix it.

## Recordings

- Recordings of a real person's voice never enter git. `.gitignore` blocks audio; never weaken it.
