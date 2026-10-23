# {{NAME}} decision codebook

Every record gets exactly one `category` and one `domain`. The lists are also in `member.json` →
`codebook`, which `crew_decisions.py check-labels` validates against.

| category | use for |
|---|---|
| scope | what to include or leave out, how much to write or build, which items of a list, what to cut |
| sequencing | order, now vs later, phasing, what goes out first, batching |
| methods | how the work is done: approach, design, data sources, analytic or technical choices |
| framing | the argument or angle, how a claim, result or proposal is positioned |
| writing-voice | wording, tone, length, structure of prose; titles; plain vs formal language |
| communication | messages and documents for other people: whether to send, to whom, what to say |
| collaboration | working with others: roles, credit, who does what, handling disagreement |
| compliance-ethics | rules, policies, ethics, data protection, accessibility, what is allowed |
| tooling | software, packages, file formats, version control, AI tools, where files live |
| verification | how to check something: facts, citations, data quality, what counts as done |
| placement-naming | file or section placement, names, labels, terminology |
| go-no-go | proceed, stop, submit or wait, accept a result, approve |
| other | none of the above |

The domains are {{PRINCIPAL}}'s own areas of work, set in the build's *Your domains* step (the same
list is in `member.json` → `codebook.domains`). Until that step, the defaults are:

| domain | use for |
|---|---|
| work | the main job or projects |
| writing | writing not tied to one project |
| admin | scheduling, service, paperwork, coordination |
| other | none of the above |

`escalates` (questions only, else []): every escalation class the QUESTION ITSELF touches, from:
send-external (a message to anyone else), submit (submitting or finalising a document, application
or form), money, irreversible, privacy (other people's data, credentials), commitment (agreeing to a
deadline, role, review or collaboration). Judge from the question and options, never the answer.

`signal` (messages only): ruling (a standing rule or preference: always, never, from now on),
correction (pushes back on what Claude did or said), direction (asks for a task or a choice),
ack (approval, thanks, a one-word go-ahead), other.

`reason` (training records only): WHY {{PRINCIPAL}} chose or said it, in your own words, at most 20
words. For a question that took the recommendation with no notes, say what made the recommended
option right. For other picks and own words, say what {{PRINCIPAL}} valued over the recommendation.
