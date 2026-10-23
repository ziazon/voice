# Assistant template

An assistant answers a decision the way one person (its *principal*) would,
says how sure it is, and hands the question back when it is the principal's to make. Each member
lives in `crew/<slug>/`. This folder is the starting point: `bootstrap/install.sh --name <Name>`
copies it to `crew/<slug>/` and fills the `{{…}}` placeholders. The build is guided step by step
by `bootstrap/PROMPT.md`; anyone can run it on a fork of this repo.

| File | What it holds | Filled by |
|---|---|---|
| `member.json` | Name, pronouns, state dir, gate mark word, privacy filters, codebook | install.sh (placeholders) |
| `IDENTITY.md` | Who the member is and what they never do | install.sh, then the principal edits |
| `decisions.md` | How the principal decides, per kind of question, with measured accuracy | the build (distillation + backtest) |
| `autonomy.md` | The dial: which kinds of question stay with the principal | the principal, at a gate |
| `measurements.md` | Every test of the member: backtests, interview, report verdicts | the build, then each report |
| `report.md` | The review report a body of work ends with | as is |
| `SKILL.md` | The Claude Code skill, installed to `~/.claude/skills/<slug>/SKILL.md` | install.sh |
| `interview.md` | The calibration interview, used when answered questions are few, and the question rounds that replace it for a principal with no Claude history | the build drafts its questions |
| `lens.md` | Optional: how the principal builds things, distilled from their own documents | the build (lens step) |
| `briefs/` | Briefs for the sub-agents that label, distil, backtest and write the lens | install.sh (placeholders) |

## Privacy rules (every member)

- **The raw corpus never enters git.** Transcripts, the claude.ai export, the principal's
  documents, the mined `decisions*.jsonl`, round answers, labels and sub-agent inputs stay in the
  state dir (`~/.claude/state/<slug>/`).
- **Everything committed here is paraphrase.** Before a commit, `crew_decisions.py shingle` finds 0
  runs of 6 of the principal's words in a row, and a grep for personal topics finds nothing.
- **The default filters keep the work and drop the person.** Dropped whole: records on the
  principal's own health, family, legal matters and household money, and any record carrying a
  secret, an identifier of another person, an email address, a phone number or a run of 7+ digits
  (privacy wins over recall). Health or legal *work* stays, because those filters match first-person
  phrases ("my doctor").
- **Your own specifics stay out of git.** Folders, topics or names only you would know to drop go in
  `crew/<slug>/private.json`, which is gitignored and owner-only: its `filters` extend the
  member's, and any other key (a `codebook` with your own domains, say) replaces it. A *kit* folder
  passed as `install.sh --kit DIR` supplies that file, and any template files you keep privately
  (`DIR/crew/…`, copied over the template when `crew/<slug>/` is first filled).
- **A shared claude.ai account is reviewed first.** Only conversations the principal marks as theirs
  are mined (`crew_decisions.py claudeai titles / mark / mine`).
