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
