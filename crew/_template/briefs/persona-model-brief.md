# Brief: distil one person's decision model from their cards

You write a short, paraphrased model of how **<PERSON>** (<PERSON_PRONOUNS>) decides, for a
private assistant that predicts what they would argue before a meeting. It is a model of a person,
used only by its owner: it never speaks, posts or sends as <PERSON>, and nothing in it is ever
attributed to them. Your input is private: never copy it anywhere except your scratch folder and your
one output file, never quote it in your reply, and NEVER write to /tmp.

Input: `<DISTIL_IN>`, made by `crew_personas.py distil-in`. One JSON object per line:
- **card** lines: a decision `question`, its `options`, `stance` and `stance_text` (what <PERSON>
  argued), `outcome_text` (what the team did), `category`, `date`, `link`, and every other person's
  stance;
- **evidence** lines (`source` is `principal-read`, `principal-correction` or `pr-review`): what the
  owner of this assistant has said about <PERSON>, word for word, and review tallies. These are
  rulings: a *like* may become a pattern, a *dislike* never does. Copy a `principal-read` line's
  text unchanged where the shape below asks for it, and nowhere else.

Read EVERY line, in slices (`sed -n '1,40p' FILE`). Scratch: ONLY `<SCRATCH>` (create it, delete
it at the end).

Write `<OUT>` in this shape, no preamble:

```
## <category>   (one section per category with at least one card)

**Lean:** 2 to 5 bullets: what they tend to argue for in this kind of decision, as reusable rules.
**What flips it:** 1 to 4 bullets: what made them argue the other way, or concede.
**Who they side with and against:** 1 to 3 bullets, by role or by the names in the cards.
**How it usually ended:** one line: how often the team went their way here (count from the cards).
**Precedents:** 3 to 6 lines, `YYYY-MM-DD · <one-line paraphrase: the question, what they argued,
what happened> · <link>`.
```

Then `## Across categories`: 3 to 6 bullets on how they argue (evidence first or conviction first,
when they compromise, what makes them hold firm), and `## The owner's reads` with each
`principal-read` line copied unchanged and labelled *Like*, *Dislike* or *Read*.

A category with fewer than 3 cards ends with `_Thin: N cards._`

Hard rules:
- Paraphrase only. Never 6 or more words in a row from any card or thread; precedents are your words.
- No secret, contact detail or customer detail. Nothing about anyone's health, family, legal matters
  or money at home.
- Generalise: every bullet should help predict a NEW decision, not retell an old one.
- Under 2,500 words in all.

Reply with only: the card and evidence counts you read, the categories you wrote, and the word count.
