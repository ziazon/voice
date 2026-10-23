---
model: {{PRINCIPAL}}'s decisions
member: {{NAME}} ({{PRONOUNS}})
mined_through: YYYY-MM-DD
sources: <N answered questions and M typed messages in Claude Code on <machines>, plus K messages from claude.ai conversations marked mine, YYYY-MM-DD → YYYY-MM-DD; interview answers, if any>
generated_by: scripts/crew_decisions.py (counts) and sub-agent distillation over labelled records (paraphrase only)
---

# How {{PRINCIPAL}} decides

{{NAME}}'s model of {{PRINCIPAL}}'s decisions, one section per kind of question. Read by the
`/{{SLUG}}` skill (§2). Built from {{PRINCIPAL}}'s own answers to the questions Claude asked, the
messages {{PRINCIPAL}} typed, and (when the history was thin) the calibration interview.

- **Rules win.** Where a line here and something {{PRINCIPAL}} has stated as a rule disagree, the
  rule wins and this file is stale.
- **Paraphrase only.** Precedents name a date and a project, never {{PRINCIPAL}}'s words. The raw
  answers are searchable with `crew_decisions.py similar` and stay in `~/.claude/state/{{SLUG}}/`.
- **Counts are the tally's**, over the training split; never an agent's estimate.

## Calibration (the blind backtest)

<Filled by the backtest (measurements.md, newest entry). A category is **live** when {{NAME}} made at
least 5 High-confidence picks in it and at least 90% matched {{PRINCIPAL}}. {{NAME}} decides alone
only in live categories that autonomy.md opens; elsewhere High is treated as Medium.
When there were too few held-out questions to backtest, say so here and mark no category live.>

| Category | High | Medium | Low | Live |
|---|---|---|---|---|

## How often {{PRINCIPAL}} takes the recommendation

<The `crew_decisions.py tally` table, pasted as printed.>

## By kind of question

<One section per category, in the shape the distil brief sets: ### category, then **The lean**,
**What flips it**, **When {{PRINCIPAL}} writes an own answer**, **Standing rules**, **Precedents**.>
