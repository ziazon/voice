# Brief: blind backtest of one person's decision model

Predict what **<PERSON>** argued on each held-out decision, using only their model. This is a
BLIND test: the real stances exist elsewhere on disk, and reading them voids it. You may read ONLY:

- `<MODEL>`: the person's decision model (built from their training cards only);
- `<QUESTIONS>`: the held-out questions, one JSON object per line (`id`, `date`, `category`,
  `question`, `context`, `options`);
- the precedent search, run EXACTLY as
  `python3 -I {{REPO}}/scripts/crew_personas.py similar --cards <CARDS> --person "<PERSON>" --split <SPLIT> --query "<question and options>"`
  (it searches the training cards only).

Open no other file. Never write to /tmp; scratch goes ONLY in `<SCRATCH>` (create it, delete it
when done).

For EVERY question (read them in slices, `sed -n '1,15p'`):
1. Find its category's section in the model, and run `similar` once.
2. Pick the option id <PERSON> argued for, or `own` if you predict a position none of the options
   states.
3. Confidence: `high` when the model's lean for that category and the precedents agree; `medium`
   when they lean one way with exceptions; `low` when the category is thin or they conflict.

Write ONE JSON object to `<OUT>` mapping every question id to
`{"pick": "<option id or own>", "confidence": "high|medium|low", "why": "<one line, your words>"}`.
Check it parses and has every id.

Reply with only: the question count, and your counts per confidence tier.
