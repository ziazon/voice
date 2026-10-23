# {{NAME}} review report

Every piece of work in which {{NAME}} decided at least one question ends with one short report that
{{PRINCIPAL}} reads and marks. It is how {{PRINCIPAL}} checks {{NAME}}'s decisions, and how {{NAME}} learns.

## Write it

1. **Gather the decisions:** the lines of `~/.claude/state/{{SLUG}}/live.jsonl` whose `work` is this
   piece of work.
2. **Write** `~/.claude/state/{{SLUG}}/reports/YYYY-MM-DD-<work>.md` (private: it may name real
   documents and people), one block per decision:

   ```markdown
   ### d1 · <category> · <tier> (<accuracy>)
   Question: …
   Options: … / …
   {{NAME}} picked: …
   Why: … (precedents by date and project, never quoted)
   What it changed: …
   Verdict: [ ] agree   [ ] would have chosen otherwise: <note>
   ```

   Then the escalated questions as a list: the question, {{NAME}}'s lean, {{PRINCIPAL}}'s answer,
   matched or not (no verdict box: their answer was the verdict).
3. **Show it** to {{PRINCIPAL}} with a two-line summary (what was done, how many decisions {{NAME}}
   made alone) and ask them to tick each verdict box.

## Read the verdicts back

1. Count agree, otherwise, unmarked, per tier, and add one line to `measurements.md` → *Reports*.
2. For each *otherwise* with a note, write their correction as a rule (`~/.claude/CLAUDE.md` or a
   memory) the same turn, paraphrased, so {{NAME}}'s next answer finds it.
3. When a category's *agree* share across reports falls below 90% at High, mark it not live in
   `decisions.md` until a later backtest clears it again.
