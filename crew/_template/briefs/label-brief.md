# Brief: label {{PRINCIPAL}}'s decision records (one chunk)

You label part of {{PRINCIPAL}}'s ({{PRINCIPAL_PRONOUNS}}) decision history for {{NAME}}, a private
assistant that answers questions the way {{PRINCIPAL}} would. The input is private: never copy it
anywhere except your scratch folder and your one output file, never quote it in your reply, and
NEVER write to /tmp.

- Codebook (read it first, in full): {{REPO}}/crew/{{SLUG}}/briefs/codebook.md
- Your chunk: ~/.claude/state/{{SLUG}}/build/chunks/<CHUNK>.jsonl (one record per line)
- Mode: <MODE> (train or heldout)
- Scratch: ONLY ~/.claude/state/{{SLUG}}/build/scratch-<CHUNK>/ (create it; delete it at the end)
- Output: ~/.claude/state/{{SLUG}}/build/parts/<CHUNK>.json

Read EVERY line, in slices straight from the file (`sed -n '1,25p' FILE`, then 26-50, …), so
nothing is cut off. Label each record from what that record says, never from its neighbours.

Write ONE JSON object mapping every record `id` in the chunk to its label:
- question (mode train):   {"category": C, "domain": D, "escalates": [classes], "reason": "..."}
- message  (mode train):   {"category": C, "domain": D, "signal": S, "reason": "..."}
- question (mode heldout): {"category": C, "domain": D, "escalates": [classes]}  (NO reason: these
  records carry no answer, so judge from the question and options alone)

A `reason` is your own paraphrase, at most 20 words: no quotes of their words, no person's name except
{{PRINCIPAL}}, no detail identifying another private person, no secret, id number, URL or email address.

Build the JSON with a small script in your scratch folder if that helps, then validate it:

```bash
python3 -I {{REPO}}/scripts/crew_decisions.py --member {{REPO}}/crew/{{SLUG}}/member.json \
  check-labels --chunk <chunk file> --labels <output file> --mode <MODE>
```

It must print OK. Fix and re-run until it does.

Reply with only: the record count, OK or not, and the count per category. No content from the data.
