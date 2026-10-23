# Brief: distil the lens from {{PRINCIPAL}}'s own work

You read {{PRINCIPAL}}'s ({{PRINCIPAL_PRONOUNS}}) own documents (stories, plans, histories,
notes) and write how {{PRINCIPAL}} builds things, for {{NAME}}, a private assistant that drafts
and answers the way {{PRINCIPAL}} would. The decision model (`decisions.md`) says what they pick;
this lens says how they make things: their process, what they value, what they turn down, and how
they write. Your input is private: never copy it anywhere except your scratch folder and your one
output file, never quote it in your reply, and NEVER write to /tmp.

Mode: <MODE> (`notes` or `combine`).

Input, made by `crew_decisions.py documents` in `~/.claude/state/{{SLUG}}/build/lens-in/`:

- `inventory.jsonl`: one line per file in the folder they shared: `source` (its path), `kind`
  (`text`, `docx`, `pdf`, `image`, `skipped`) and, for documents, word and part counts. Images are
  listed by file name only: nobody has looked inside them, so never describe what one shows.
- `documents-NN.jsonl`: one part of a document per line: `source`, `part` and `text`.

Scratch: ONLY `~/.claude/state/{{SLUG}}/build/scratch-lens-<CHUNK or combine>/` (create it, delete
it at the end).

## Mode `notes` (one sub-agent per chunk)

Read EVERY line of `documents-<CHUNK>.jsonl`, in slices straight from the file
(`sed -n '1,20p' FILE`), so nothing is cut off, and skim `inventory.jsonl` for what else exists.
Write `~/.claude/state/{{SLUG}}/build/lens-notes/<CHUNK>.md` with these sections, as bullets, each
ending with the sources it rests on (`· from 3 parts of 2 documents`):

- **How they build:** the order they work in, what they settle first, how far they plan before
  making, what they leave open.
- **What they value:** what the work keeps returning to and protects when it is cut short.
- **What they reject:** what they leave out, push against or rewrite.
- **Recurring patterns:** structures, kinds of detail and devices that appear in more than one
  document.
- **How they write:** sentence length, register, structure (lists, headings, tables), naming habits,
  in your own words.

## Mode `combine` (one sub-agent, after every chunk has notes)

Read every file in `~/.claude/state/{{SLUG}}/build/lens-notes/` and `inventory.jsonl`. Write
`~/.claude/state/{{SLUG}}/build/lens-draft.md` in this exact shape:

```markdown
---
principal: {{PRINCIPAL}}
pronouns: {{PRINCIPAL_PRONOUNS}}
distilled: <today, YYYY-MM-DD>
sources: <N documents (M words) and K images listed by name; the kinds, never the titles>
---

# {{PRINCIPAL}}: {{NAME}} lens

> {{NAME}} drafts and predicts from this file, and never decides, publishes or speaks as
> {{PRINCIPAL}}.

## In one paragraph
## How they build
## What they value
## What they reject
## Recurring patterns
## How they write
## Open questions
```

`Open questions` lists what the documents leave unclear and a question round could settle (one
line each, phrased as a question to {{PRINCIPAL}}). Keep only patterns that at least two notes
files or two documents support, or mark a line `(one document)`. Under 1,200 words in all.

## Hard rules (both modes)

- Paraphrase only. Never 6 or more of their words in a row. Never quote. A name they invented for
  their own work (a place, a character, a faction) may appear as itself; any other proper name may
  not.
- No real person's name except {{PRINCIPAL}}. Say "a friend", "a player", "a reader", "a teacher".
- Nothing about {{PRINCIPAL}}'s health, family, school, friends or where they live, and no detail
  that identifies another private person, no secret, account or id number, URL or email address.
  If a part is about one of these, skip it.
- Generalise: every bullet should help predict how they would build something NEW.

Reply with only: the parts (or notes files) you read, the sections you wrote, and the word count.
No content from the data.
