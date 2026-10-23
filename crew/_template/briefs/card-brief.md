# Brief: turn recorded team decisions into decision cards (one batch)

You turn a batch of recorded team decisions into **decision cards**: one per decision, each a neutral
question, the options people argued, each person's stance and what happened. The cards train
private, per-person decision models and score them, so every field must be true to the thread and
nothing may leak the answer into the question. Your input is private: never copy it anywhere except
your scratch folder and your one output file, never quote it in your reply, and NEVER write to /tmp.

Input:
- `<BATCH>`: the batch, one decision per block: its id, date, link, the people in it, and the full
  thread (who said what, in order). Read EVERY block, in slices (`sed -n '1,80p' FILE`), so nothing
  is cut off.
- `<CATEGORIES>`: the category list (JSON). Use exactly one of these per card.
- `<AGENT_RULE>`: how to tell a message an AI assistant wrote for its sender (a signature line, or
  a date range plus a shape). A stance that rests only on such messages is listed in `agent_drafted`.

Scratch: ONLY `<SCRATCH>` (create it, delete it at the end).

Write `<OUT>`: one JSON object `{"cards": [...]}`, one card per block, in this shape:

```json
{"id": "<the block's id>", "date": "YYYY-MM-DD", "category": "<one category>", "source": "card",
 "question": "The decision as a neutral question, as it stood when it came up",
 "context": "What was known then. Never the outcome, never who argued what, never an option's wording",
 "options": [{"id": "a", "text": "…"}, {"id": "b", "text": "…"}],
 "stances": {"<person>": "a | b | … | own | asked-only"},
 "outcome": "a | b | … | unresolved | other",
 "asked": [{"by": "<person>", "about": "what they asked, paraphrased"}],
 "agent_drafted": [], "link": "<the block's link>"}
```

- **Options**: the 2 to 4 distinct positions really argued, each a short neutral sentence with no
  names. Two people who argued the same thing share one option. A position nobody else took and that
  is not a real alternative is `own` for that person.
- **Stances**: every person who took part. `asked-only` for someone who asked or helped but took no
  side. Leave out people who only reacted.
- **Outcome**: the option the team went with; `unresolved` if it never settled; `other` if the team
  went a way nobody argued.
- **Asked**: every substantive question someone raised (not "can you share the link").
- Use the person names exactly as the block gives them.

Hard rules:
- Paraphrase only. Never 6 or more words in a row from the thread. Never quote.
- No secret or credential, no contact detail, no financial identifier, no customer detail.
- Nothing about anyone's health, family, legal matters or money at home: skip such a block and
  list its id in your reply.
- If a block holds no real decision, skip it and list its id.

Before you reply, run `python3 -I {{REPO}}/scripts/crew_personas.py check --cards <OUT> --categories
<CATEGORIES>` and fix every problem it prints until it says 0 problems.

Reply with only: the blocks you read, the cards you wrote, the ids you skipped and why (one word
each), and the check's last line. No content from the data.
