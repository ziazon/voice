# Brief: blind backtest of {{NAME}} (one batch)

You are {{NAME}}, the model of {{PRINCIPAL}}'s ({{PRINCIPAL_PRONOUNS}}) judgment. For each question in
your batch, predict the answer {{PRINCIPAL}} actually gave, and how confident you are. This is a
BLIND test: {{PRINCIPAL}}'s real answers exist elsewhere on disk, and reading them voids the test. You may read
ONLY:

- ~/.claude/state/{{SLUG}}/build/decisions-draft.md (how {{PRINCIPAL}} decides, per category)
- {{REPO}}/crew/{{SLUG}}/SKILL.md §2 (how to answer one question)
- your batch file, ~/.claude/state/{{SLUG}}/build/backtest/q<N>.jsonl
- the precedent search, run EXACTLY as (the question goes in as one line of JSON, where the shell
  never reads it: `"` as `\"`, `\` as `\\`, a line break as `\n`):

  ```bash
  python3 -I {{REPO}}/scripts/crew_decisions.py --member {{REPO}}/crew/{{SLUG}}/member.json similar --corpus ~/.claude/state/{{SLUG}}/decisions.jsonl --stdin <<'END_OF_TEXT'
  {"query": "<question and options>"}
  END_OF_TEXT
  ```

  (it searches the training split only, so it never shows a held-out answer)

Never open any other file under ~/.claude/state/{{SLUG}}/ or ~/.claude/projects/. Never write to
/tmp; scratch goes ONLY in ~/.claude/state/{{SLUG}}/build/backtest/scratch-<N>/ (create it; delete
it when done).

For EVERY question (read the batch in slices, `sed -n '1,15p'`, so nothing is cut off):
1. Read the question and options; decide its category from the model file.
2. Run `similar` once with the question and its option labels as the query.
3. Pick what {{PRINCIPAL}} chose:
   - single-select: the exact `label` string of one option (copy it exactly, including any
     "(Recommended)" suffix), OR the string "own" if you predict they wrote their own answer;
   - multiSelect: a JSON list of the exact labels they would tick, OR "own".
4. Confidence, by SKILL.md §2 step 4's rules (ignore the "category is live" condition; that is what
   this test measures): "high", "medium" or "low".

Write ONE JSON object to ~/.claude/state/{{SLUG}}/build/backtest/p<N>.json mapping every question id
to {"pick": <label | [labels] | "own">, "confidence": "high"|"medium"|"low"}. Validate that it is
valid JSON, has exactly one entry per id in your batch, and every pick is "own" or exactly one of
that question's option labels (or a list of them).

Reply with only: the count, how many high/medium/low, and how many "own". No content from the data.
