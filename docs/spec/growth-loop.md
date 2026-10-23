# The growth loop: how an AI persona learns from feedback

**Version 1.** A persona is an AI assistant built to decide the way someone decides. This spec says how
a persona improves from the feedback people give it, safely: every change is proposed, approved by a
person, and measured, and a change that makes things worse is undone.

The spec is product-neutral. Any app can implement it, in any language. It defines five stages and the
file each stage writes; the files are what two implementations share, so each is pinned down by a JSON
Schema in [`growth-loop/`](growth-loop/).

## The five stages

1. **Keep the signals.** Each time someone reacts to the persona (picks an option, corrects it, marks
   one of its decisions, or states a rule), the app keeps one **signal**.
2. **Turn signals into cases.** A **case** is one test the persona must pass: a situation, and the
   answer the right person would want. Each case names the signals it came from.
3. **Propose a change.** The app drafts a **proposal**: the exact edits to the persona's files, as a
   diff, with a plain reason for each, and the new cases the change should make pass.
4. **Ask for approval.** The approver reads the proposal and approves or rejects it. Nothing is applied
   without an **approval** that says *approved*.
5. **Re-measure, then keep or revert.** The app applies the approved change, runs the proposal's new
   cases and every active case on the persona before and after, and writes a **measurement**. The
   change is kept only when every new case passes and no existing case got worse. Otherwise it is
   reverted.

```
signals → cases → proposal → approval → measurement → keep | revert
```

## Who approves

| The persona models… | `persona_kind` | Who approves (`role`) |
| --- | --- | --- |
| one person's decisions | `person` | `subject`: that person, and no one else |
| a product's or a team's behaviour | `product` | `staff`: someone the product's owners name |

An app never approves its own proposal, and never applies one on a timer or by default.

## Rules every implementation keeps

1. **Every file declares its shape.** Each record's `schema` field names its schema and version
   (for example `growth-loop/signal/v1`) and must validate against it.
2. **Nothing applies without approval.** A change reaches the persona only after an approval of that
   proposal with `decision: approved`, from the role the table above names for its `persona_kind`.
3. **Every case that counts is run.** A measurement's `results` hold exactly one entry for each case
   the proposal names (`new: true`) and one for each case that is `active` when the measurement starts
   (`new: false`), and no others: `retired` cases, and cases still `new` for some other proposal, are
   not run.
4. **Kept means no regressions.** A measurement's `outcome` is `keep` only when its results are
   complete by rule 3, every result with `new: true` has `after: pass`, and no result with `new: false`
   went from `before: pass` to `after: fail`. In every other case it is `revert`, and the persona
   returns to its `base` version.
5. **References resolve.** Every id a record names (a case's `from_signals`, a proposal's `cases`, an
   approval's `proposal`, a measurement's `proposal` and `approval`) is the id of an existing record of
   the right kind for the same persona. A measurement's `approval` approved its proposal.
6. **Signals and cases are private.** They can hold what a person actually said. Keep them where the
   app keeps private data (for a person's persona, on their own computer, readable only by them), never
   in a shared or public repository. The persona's own files, and so a proposal's diff, hold paraphrase
   only.
7. **Cases grow; they are not quietly dropped.** A case starts `new`. When a proposal that names it is
   kept, it becomes `active` and is measured from then on. When that proposal is rejected or reverted,
   the case stays `new` and a later proposal may name it again. Only the approver retires a case
   (`retired`); retired cases are no longer measured.
8. **Records are never rewritten.** A case's status changes by appending the whole case again, with the
   same `id` and the new `status` (see *Storage*). A proposal gets at most one approval. A rejected
   proposal stays on file with its approval and gets no measurement; a reverted one stays on file with
   its approval and measurement. So the history explains itself.
9. **The base must be current.** A proposal is applied only to the version it names as `base`. If the
   persona has changed since, the proposal is not applied; the app drafts a new one against the
   current version.

## Storage

Signals and cases are [JSON Lines](https://jsonlines.org/) files, one record per line, only ever
appended. Each proposal, approval and measurement is its own JSON file. Where they live is the app's
choice, within rule 6.

- **Ids** are lowercase letters, digits and dashes behind a fixed prefix (`sig-`, `case-`, `prop-`,
  `appr-`, `meas-`), unique per persona. The one repeat allowed is a case appended again to change its
  `status`: every other field must equal the first line's, and a reader takes the last line for each
  case id.
- **Times** are RFC 3339 with a time zone (`2026-03-04T15:20:00Z`).
- **Versions** (`base`, `candidate`) are any string that names one exact state of the persona's files,
  such as a git commit.
- **File paths** in a proposal are relative to the persona's folder: names of letters, digits, `.`,
  `_` and `-`, joined by `/`, none starting with a dot (so no `..`, no hidden files, no leading or
  trailing `/`).

## Checks the schemas cannot make

The schemas check each record on its own. An implementation also checks these, before it acts on a
record:

- a signal has `verdict` exactly when its `kind` is `verdict`;
- a case with `match: choice` has `options`, and its `expected` is one of them;
- a measurement names each case once in `results`, and its results are complete (rule 3);
- every rule above that spans two records (rules 2, 4, 5, 7, 8 and 9).

## The files

Every example below is invented: a persona for **Robin**, a fictional high-school teacher. The
examples are also stored as files and checked against their schemas by the repo's tests.

### 1. Signal

One reaction to the persona. Schema: [`growth-loop/signal.schema.json`](growth-loop/signal.schema.json).

| Field | Required | Meaning |
| --- | --- | --- |
| `schema` | yes | `growth-loop/signal/v1` |
| `id` | yes | `sig-…` |
| `persona` | yes | the persona's id |
| `at` | yes | when the reaction was given |
| `given_by` | yes | `subject` or `staff` |
| `kind` | yes | `pick` (chose between options), `correction` (said the persona was wrong), `verdict` (marked one of its decisions), `instruction` (stated a rule) |
| `area` | yes | the area of decision it belongs to, in the app's own list of areas |
| `situation` | yes | what was being decided |
| `wanted` | yes | the answer or behaviour the giver wanted |
| `persona_said` | no | what the persona answered, when it had answered |
| `verdict` | no | `agree` or `disagree`; set when `kind` is `verdict` |
| `source` | no | where the reaction came from in the app, such as `review-report` |

Example (`growth-loop/examples/signal.json`):

```json
{
  "schema": "growth-loop/signal/v1",
  "id": "sig-2026-03-04-001",
  "persona": "robin",
  "at": "2026-03-04T15:20:00Z",
  "given_by": "subject",
  "kind": "correction",
  "area": "communication",
  "situation": "A parent asks to move a student's quiz because of a family trip.",
  "persona_said": "Decline politely and point to the late-work policy.",
  "wanted": "Offer one make-up slot in the same week, then point to the policy for anything later.",
  "source": "review-report"
}
```

### 2. Case

One test the persona must pass. Schema: [`growth-loop/case.schema.json`](growth-loop/case.schema.json).

| Field | Required | Meaning |
| --- | --- | --- |
| `schema` | yes | `growth-loop/case/v1` |
| `id` | yes | `case-…` |
| `persona` | yes | the persona's id |
| `created` | yes | when the case was made |
| `from_signals` | yes | the ids of the signals it came from, at least one |
| `area` | yes | the area of decision |
| `situation` | yes | what the persona is asked to decide |
| `options` | no | the choices offered, when it is a choice; at least two |
| `expected` | yes | the right answer: one of `options`, or a description a grader checks |
| `match` | yes | `choice` (the answer must equal `expected`, which is one of `options`) or `rubric` (a grader, person or model, judges the answer against `expected`) |
| `status` | yes | `new`, `active` or `retired` (rule 7) |

Example (`growth-loop/examples/case.json`):

```json
{
  "schema": "growth-loop/case/v1",
  "id": "case-2026-03-04-001",
  "persona": "robin",
  "created": "2026-03-04T16:00:00Z",
  "from_signals": ["sig-2026-03-04-001"],
  "area": "communication",
  "situation": "A parent asks to move a student's test because of a medical appointment.",
  "options": [
    "Decline and point to the late-work policy",
    "Offer one make-up slot in the same week",
    "Let the student skip the test"
  ],
  "expected": "Offer one make-up slot in the same week",
  "match": "choice",
  "status": "new"
}
```

### 3. Proposal

A change to the persona's files, waiting for approval. Schema:
[`growth-loop/proposal.schema.json`](growth-loop/proposal.schema.json).

| Field | Required | Meaning |
| --- | --- | --- |
| `schema` | yes | `growth-loop/proposal/v1` |
| `id` | yes | `prop-…` |
| `persona` | yes | the persona's id |
| `persona_kind` | yes | `person` or `product`; it decides who may approve |
| `created` | yes | when it was drafted |
| `base` | yes | the persona version the diff applies to |
| `summary` | yes | one or two plain sentences for the approver |
| `changes` | yes | at least one change: `file` (a path inside the persona's folder, as *Storage* sets out), `diff` (a unified diff of that file), `why` (a plain reason) |
| `cases` | yes | the ids of the new cases this change should make pass, at least one |

Example (`growth-loop/examples/proposal.json`):

```json
{
  "schema": "growth-loop/proposal/v1",
  "id": "prop-2026-03-05-001",
  "persona": "robin",
  "persona_kind": "person",
  "created": "2026-03-05T09:00:00Z",
  "base": "3f2c1a7",
  "summary": "When a family asks to move a test or quiz, offer one make-up slot that week instead of declining.",
  "changes": [
    {
      "file": "decisions.md",
      "diff": "--- a/decisions.md\n+++ b/decisions.md\n@@ -12,3 +12,4 @@\n ## Communication\n \n - Replies to families are short and warm.\n+- A request to move a test or quiz gets one make-up slot in the same week; later than that, the late-work policy applies.\n",
      "why": "You corrected this on March 4: you offer a make-up slot first."
    }
  ],
  "cases": ["case-2026-03-04-001"]
}
```

### 4. Approval

The approver's answer to one proposal. Schema:
[`growth-loop/approval.schema.json`](growth-loop/approval.schema.json).

| Field | Required | Meaning |
| --- | --- | --- |
| `schema` | yes | `growth-loop/approval/v1` |
| `id` | yes | `appr-…` |
| `proposal` | yes | the proposal's id |
| `at` | yes | when they answered |
| `role` | yes | `subject` or `staff`; must match the proposal's `persona_kind` (see *Who approves*) |
| `decision` | yes | `approved` or `rejected` |
| `note` | no | anything the approver added |

Example (`growth-loop/examples/approval.json`):

```json
{
  "schema": "growth-loop/approval/v1",
  "id": "appr-2026-03-05-001",
  "proposal": "prop-2026-03-05-001",
  "at": "2026-03-05T18:30:00Z",
  "role": "subject",
  "decision": "approved"
}
```

### 5. Measurement

Every case run on the persona before and after an approved change, and the outcome. Schema:
[`growth-loop/measurement.schema.json`](growth-loop/measurement.schema.json).

| Field | Required | Meaning |
| --- | --- | --- |
| `schema` | yes | `growth-loop/measurement/v1` |
| `id` | yes | `meas-…` |
| `proposal` | yes | the proposal's id |
| `approval` | yes | the approval's id |
| `at` | yes | when the run finished |
| `base` | yes | the version before the change (the proposal's `base`) |
| `candidate` | yes | the version with the change applied |
| `results` | yes | one entry per case run, by rule 3: `case` (its id), `new` (`true` for the proposal's own cases, `false` for active ones), `before` and `after` (`pass` or `fail`) |
| `outcome` | yes | `keep` or `revert`, by rule 4 |
| `reason` | yes | one plain sentence the approver will read |

Example (`growth-loop/examples/measurement.json`):

```json
{
  "schema": "growth-loop/measurement/v1",
  "id": "meas-2026-03-05-001",
  "proposal": "prop-2026-03-05-001",
  "approval": "appr-2026-03-05-001",
  "at": "2026-03-05T18:42:00Z",
  "base": "3f2c1a7",
  "candidate": "8d41e0b",
  "results": [
    {"case": "case-2026-03-04-001", "new": true, "before": "fail", "after": "pass"},
    {"case": "case-2026-02-11-004", "new": false, "before": "pass", "after": "pass"},
    {"case": "case-2026-02-20-002", "new": false, "before": "fail", "after": "fail"}
  ],
  "outcome": "keep",
  "reason": "The new case now passes, and none of the other cases got worse."
}
```

## Changing this spec

The schemas allow no field they do not list, so every change to a record's shape, even one new
optional field, is a new version: `v2` in the `schema` field and a new schema file beside the old one.
Implementations read every version they have written, and write only the newest.
