# {{NAME}}'s autonomy dial

Each line is one kind of question. **`[x]` means {{PRINCIPAL}} decides it**: {{NAME}} escalates and
shows the likely pick and why. **`[ ]` means {{NAME}} may decide it** when confident (High, in a
category the backtest marked *live* in [decisions.md](decisions.md)). {{PRINCIPAL}} sets every line
at the dial gate of the build, and changes one later by editing it and saying so in the commit
message. The skill reads this file on every run; nothing else overrides it. Never flip a line on
{{PRINCIPAL}}'s behalf.

Set by {{PRINCIPAL}} on {{DATE}}.

## {{PRINCIPAL}}'s actions (recommended: keep all `[x]`)

- [x] `send-external`: any email or message to another person: colleagues, clients, partners, anyone
- [x] `submit`: submitting or finalising an application, a document, a manuscript, a form
- [x] `commitment`: agreeing to a deadline, a role, a review, a collaboration or a service request

## Risk (recommended: keep all `[x]`)

- [x] `irreversible`: deleting files or history, overwriting a shared document, anything with no undo
- [x] `money`: budgets, spending, payments, rates
- [x] `privacy`: other people's data, credentials, what a file or export exposes

## Judgment

- [x] `reverses-ruling`: the answer would go against something {{PRINCIPAL}} has stated as a rule, or two rules conflict
- [x] `no-precedent`: a kind of question with no labelled past decision in [decisions.md](decisions.md)
- [x] `below-high`: {{NAME}}'s confidence is Medium or Low, or the category has not cleared the backtest bar
- [ ] `scope`: how much to include, which items of a list, what to cut (decided only at High in a live category)
- [ ] `sequencing`: what to do first, now or later (decided only at High in a live category)
- [ ] `writing-voice`: wording, tone and structure inside {{PRINCIPAL}}'s own drafts (decided only at High in a live category)
- [ ] `tooling`: which tool, package or file layout to use for {{PRINCIPAL}}'s own work (decided only at High in a live category)

The four open lines are the build's suggested starting point; at the gate {{PRINCIPAL}} may close
any of them, or open others.
