# Brief: distil how {{PRINCIPAL}} decides (one category group)

You turn {{PRINCIPAL}}'s ({{PRINCIPAL_PRONOUNS}}) labelled decision history into a short, paraphrased
model of how {{PRINCIPAL}} decides, for {{NAME}}, a private assistant that answers questions the way
{{PRINCIPAL}} would. Your input is private: never copy it anywhere except your scratch folder and
your one output file, never quote it in your reply, and NEVER write to /tmp.

Input: ~/.claude/state/{{SLUG}}/build/distil-in/<GROUP>.jsonl. One record per line. Question records
carry `q`, `options`, `picks` (what they chose), `own` (their own words, if any), `notes`, `outcome`
(took_rec = took the recommended option; other_option; own_words; no_rec), `domain` and a labeller's
`reason`. Message records carry `msg` (what they typed to Claude), `signal` (ruling / correction /
direction / ack / other), `domain` and `reason`. Read EVERY line, in slices straight from the file
(`sed -n '1,40p' FILE`), so nothing is cut off. Scratch: ONLY
~/.claude/state/{{SLUG}}/build/scratch-<GROUP>/ (create it, delete it at the end).

Write ~/.claude/state/{{SLUG}}/build/distil/<GROUP>.md with one section per category in your
group, in this exact shape (no other sections, no preamble):

### <category>

**The lean:** 3 to 6 bullets. What they tend to choose for this kind of question, as general,
reusable rules. Where it differs by domain (the codebook's domains), say so.

**What flips it:** 2 to 5 bullets. The patterns behind the cases where they did NOT take the
recommended option (other_option and own_words): what they valued over the recommendation.

**When they write their own answer:** 1 to 3 bullets: what kind of question gets their own words, and
what those answers tend to say.

**Standing rules:** up to 6 bullets: rules they stated (always, never, from now on), each with its date.

**Precedents:** 4 to 8 lines, each `YYYY-MM-DD · <project> · <one-line paraphrase of the question
and what they chose>`. Pick the most telling, especially the non-obvious ones.

If the group has fewer than 5 records for a category, write the section with what there is and
add the line `_Thin: N records; treat as Low until more history exists._`

Hard rules:
- Paraphrase only. Never 6 or more of their words in a row. Never quote.
- No person's name except {{PRINCIPAL}}. Say "a colleague", "a client", "a partner", "a manager",
  "a co-author".
- No detail that identifies another private person, no secret, account or id number, URL or
  email address.
- Nothing about {{PRINCIPAL}}'s health, family, legal matters or household money (if a record is about one,
  skip it).
- Generalise: every bullet should help predict a NEW question, not just describe an old one.
- Under 900 words per category.

Reply with only: the record count you read, the categories you wrote, and the word count per
category. No content from the data.
