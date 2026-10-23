# Calibration interview

Run when the mined history holds fewer than about 150 answered questions in total, or fewer than 10
in a category the principal cares about. Twenty structured questions, asked with AskUserQuestion,
four at a time, each with 2 to 4 concrete options and room for their own words. Draft each question
from a real kind of situation in {{PRINCIPAL}}'s domains (`member.json` → `codebook.domains`); never
from another person's private details.

Each answer becomes one training record appended to `~/.claude/state/{{SLUG}}/decisions-interview.jsonl`
(the merge step picks up every `decisions-*.jsonl`), in the shape `crew_decisions.py mine` writes,
so it is labelled, tallied and distilled like any other record:

```json
{"id": "question:interview-01", "kind": "question", "timestamp": "<now, UTC, …Z>", "project": "interview",
 "header": "<short tag>", "question": "…", "multiSelect": false,
 "options": [{"label": "… (Recommended)", "description": "…"}, {"label": "…", "description": "…"}],
 "recommended": "… (Recommended)", "{{SLUG}}_pick": null, "picks": ["<the label they chose>"],
 "own_words": "<their own words, or empty>", "notes": "", "outcome": "took_rec|other_option|own_words|no_rec",
 "split": "train"}
```

Mark a recommendation only where there is an honest default. Interview answers are never held out:
they train the model, they do not test it. `install.sh` merges this file with `merge --train`, which
keeps its records in training and out of the cutoff.

Cover each category at least once, weighted to the thin ones. Draft the twenty rows before asking,
one per line in this shape, and show them to {{PRINCIPAL}} first so they can swap any that do not fit
their work:

| # | Category | Domain | Situation to ask about |
|---|---|---|---|
| 1 | scope | <a domain> | A draft is 20% over its limit: what goes first? |
| 2 | sequencing | <a domain> | Two pieces of work are both nearly ready: which goes out first, and why? |
| 3 | methods | <a domain> | Two defensible approaches give different results: which is primary? |
| 4 | verification | <a domain> | How much checking before a draft goes to a colleague versus out of the door? |
| 5 | go-no-go | <a domain> | A deadline is in 4 days and the work is at 70%: ship, or wait for the next round? |
| … | … | … | (fifteen more, drafted the same way) |

After the interview, ask one open question: "Which kinds of decisions would you most like {{NAME}}
to take off your plate?" and note the answer (paraphrased) for the autonomy gate.

## Question rounds, when {{PRINCIPAL}} does not use Claude

When {{PRINCIPAL}} cannot be asked in a Claude session (no account of their own, or under 18, which
Anthropic's consumer terms require), the same questions go out as a **round**: a form they fill in
on paper or as a fillable PDF, whose answers an adult relays. Draft a round from the table above,
then write two files in `~/.claude/state/{{SLUG}}/rounds/`:

- `<round>-questions.json`, exactly what the form shows. Options are lettered A to D in order;
  `recommended` is a letter, or null where there is no honest default.

  ```json
  {"questions": [{"n": 1, "header": "<short tag>", "question": "…", "multiSelect": false,
                  "recommended": "A", "options": [{"label": "…", "description": "…"}, {"label": "…", "description": "…"}]}]}
  ```

- `<round>-answers.json`, typed from the returned form: the letter they ticked (a list of letters
  where several may be ticked), their own words when they wrote some, and the date they answered.
  A question left blank is simply missing.

  ```json
  {"answered": "YYYY-MM-DD", "answers": {"1": {"pick": "B"}, "2": {"own_words": "…", "notes": "…"}}}
  ```

Import each round into the corpus folder, then re-run the install command so it is merged:

```bash
$C round --name 1 --questions ~/.claude/state/{{SLUG}}/rounds/1-questions.json --answers ~/.claude/state/{{SLUG}}/rounds/1-answers.json
$C round --name 2 --questions … --answers … --heldout
```

A round trains by default, like the interview. Pass `--heldout` for a round the model must be
scored on blind: new questions, never shown to the sub-agents that draft the model, so the
backtest (PROMPT.md step 10) tests it on every one of that round's questions. Hold out a whole round or nothing; never mix.
