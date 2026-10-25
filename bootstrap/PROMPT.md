# Build your AI assistant

An AI assistant answers a decision the way you would, says how sure it is, and hands the question
back when it is yours. It is built from your own answers and messages to Claude Code (and, if you
like, claude.ai, your sent mail and your own documents). Everything raw stays on your Mac; only a
paraphrased model is committed.

**Before you start:** make a private copy of this repo (the README's *Build one* step 1: a new
private GitHub repo, not a fork, because a fork of a public repo is public) and clone it into a folder
of your choice. Then open Claude Code on the Mac you use most and say:
*"Follow bootstrap/PROMPT.md in the folder I cloned into with me, step by step."* If you also use Claude Code
on other Macs, run `PROMPT-extra-mac.md` on each of them when step 4 asks. If someone gave you a
private kit folder, keep its path handy for step 3. Built one before 2026-10-23? This
repo's history was rewritten that day: clone it fresh and copy your `crew/<slug>/` across.

**Building one for someone who does not use Claude?** (No account of their own, or under 18.) Say
so at the start: the build follows *The route without a Claude history* at the end of this file,
which learns from their documents and from question rounds instead of a history.

---

You are helping the person at this Mac (**the principal**) build their AI assistant. Work through
the steps below **in order, with them watching**: say which step you are on, run each command
yourself, show the counts it prints, and ask with AskUserQuestion (recommended option first)
wherever a step says *ask*. Expect about two to three hours, most of it sub-agents working.

**Rules for the whole build:**

- **The principal's raw history never leaves their Macs and never enters git.** It lives in
  `~/.claude/state/<slug>/`. Only `crew/<slug>/` is committed, and only paraphrase.
  `crew/<slug>/private.json` is gitignored: never commit it, never print what it holds.
- **Never read anyone else's data.** If the claude.ai account is shared, you read conversation
  *titles* only to help the principal mark them (for an untitled conversation, its first eight
  words, and only once step 5 allows it), never more of a conversation's content, and only
  conversations they mark as theirs are mined.
- **Never quote the principal's raw words** into any file in the repo. Never name another private
  person in a committed file.
- Never write to `/tmp`; scratch goes in `~/.claude/state/<slug>/build/`.
- Other folders in the repo belong to other assistants or to the repo's maintainers: for this build
  only this prompt and `crew/<slug>/` apply. Never edit anything outside `crew/<slug>/`.
- Run Python as `python3 -I`. In each shell command define the member and `C`:
  `REPO=$(git rev-parse --show-toplevel); S=<slug>; C="python3 -I $REPO/scripts/crew_decisions.py --member $REPO/crew/$S/member.json"`,
  then `$C <step> …`.

## 1. Consent, and the names

Tell the principal, in plain words: what will be read (their Claude Code history on this Mac and any
others they add, the claude.ai conversations they mark as theirs, and any other source they add at
step 5b, each asked separately), what is dropped automatically
(their own health, family, legal and household-money matters, any secret, and any record with
another person's identifier, an email address, a phone number or a long number), where it stays
(`~/.claude/state/<slug>/` on this Mac), and what gets shared (only the paraphrased model in
`crew/<slug>/`, committed to their own private repo).

*Ask:* "Go ahead with the build?" (Yes, go ahead / Not now). Then *ask* for the names, in one
question set: their own first name and pronouns, and the assistant's name and pronouns (offer two or
three short names, and they/them as the default pronouns). The slug is the assistant's name in
lowercase with every run of other characters turned into `-`.

## 2. GitHub access

The principal needs a key on this Mac that GitHub knows, with push access to their private repo. Check:

```bash
ssh -T -o StrictHostKeyChecking=accept-new git@github.com 2>&1 | head -1
REPO=$(git rev-parse --show-toplevel)
git -C "$REPO" remote get-url origin
```

The first should greet their GitHub username, and the second should be their own private repo, not
the kit's upstream repo. If the greeting fails, have them run `ssh-keygen -t ed25519` **in the Terminal
app** (it asks questions; accept the defaults), add `~/.ssh/id_ed25519.pub` at github.com → Settings →
SSH and GPG keys, and check again. If `origin` is the upstream kit, stop: they need their own private
repo first (the README's *Build one* step 1). If they have the `gh` CLI, confirm it is private:
`gh repo view "$(git -C "$REPO" remote get-url origin)" --json visibility` should read `PRIVATE`; if it
reads `PUBLIC`, stop, since step 14 would publish their model. Never use a GitHub fork: a fork of a
public repo is public and cannot be made private. If the repo is missing, clone their private repo
into a folder of their choice. Your shell has no terminal to answer
prompts, so anything that asks for a password or a yes runs in the Terminal app, typed by them.

## 3. Install and mine this Mac

```bash
bash "$REPO/bootstrap/install.sh" --name <Name> --pronouns <pronouns> \
  --principal <their name> --principal-pronouns <their pronouns> [--kit <kit folder>]
```

It checks python3, git and claude, fills `crew/<slug>/` from the template (with the kit's files, if
a kit was given), and mines this Mac's Claude Code history into
`~/.claude/state/<slug>/decisions-<host>.jsonl`. Show the principal its counts, including what the
filters dropped. Then show the folder list and ask whether any folder should also be left out:

```bash
$C folders
```

If they name one, add it to `filters.excluded_folders` in `crew/<slug>/private.json` (create the
file as `{"filters": {"excluded_folders": "<regex>"}}` if it does not exist; `|`-join several), and
re-run the install command. Ask the same about **topics**: names, places or subjects only they would
know to drop (a side business, a family member's name, a health matter). Those go in
`filters.personal` in `private.json`, as a regex. Each other Mac keeps its own copy of
`private.json`: after any filter change, make the same edit there and re-run that Mac's step. (The
merge re-applies the topic and secret filters to every Mac's records, but a folder can only be left
out where it is mined.)

## 4. Other Macs

```bash
ls -l ~/.claude/state/$S/decisions-*.jsonl
```

*Ask:* "Do you use Claude Code on another Mac too?" (No, just this one / Yes: I'll run
PROMPT-extra-mac.md there now). If yes, wait for them to say it is done, then re-run the step 3
install command.

## 5. Your claude.ai conversations (optional)

*Ask:* "Include your claude.ai conversations?" (Yes / Skip). If they skip, go to step 5b.

1. They request the export: claude.ai → Settings → Privacy → **Export data**. The link arrives by
   email and expires after 24 hours. It downloads a small JSON manifest, not the data: the
   manifest lists one zip per category (`light_metadata`, `projects`, `memories`, `frames`,
   `artifact_data`, `conversations`), each at its own link. Only `conversations-000.zip` is
   needed. Those links open only in the browser where they are signed in to claude.ai (fetched
   from a shell they return 403), so **they** click the `conversations-000.zip` link; you cannot
   download it for them. Then you move it:
   `mv ~/Downloads/conversations-000.zip ~/.claude/state/$S/claudeai-export.zip`
   If the manifest lists more than one `conversations-` zip, stop this step, skip to step 5b, and
   suggest they report upstream that the conversations came in parts (the adapter reads one).
2. Re-run the step 3 install command. Its first claude.ai step, `claudeai inspect`, prints the
   export's field names (never content) and stops if the export's shape is not what the adapter
   reads. **If it stops, stop this step**, skip to step 5b, and suggest they report the field names
   (they hold no content) upstream. If inspect shows a project field (for example
   `.project.name`), re-run install with `--project-field project.name` so the review list shows
   each conversation's project.
3. **The review.** The list is `~/.claude/state/$S/claudeai-review.tsv`: one row per
   conversation, with created and updated dates, message count, project (if known) and title, and
   an empty `mine` column. Most conversations have no title, so for those the list shows
   `opens: ` and the first eight words of the conversation's first message (a long word, such as
   a link, shows as `…`), or `[filtered]` when that message trips a privacy filter. If the
   account is shared, *ask* before showing anything: "Untitled rows show the first eight words
   of the opening message, the other person's included. Show them?" (Yes, they agree / No, hide
   them). On No, only ever display the list through
   `awk -F'\t' 'BEGIN{OFS="\t"} $6 ~ /^opens: /{$6="(untitled)"} 1' <the list>`, so you never
   see those words; untitled rows can then be marked only by date or project.
   Show it as tables of about 40 rows. They mark each row mine or not mine; you apply the marks:

   ```bash
   $C claudeai mark --review ~/.claude/state/$S/claudeai-review.tsv --uuid <uuid> --uuid <uuid> --as y
   $C claudeai mark --review … --project "<project>" --as y      # everything in one project
   $C claudeai mark --review … --title '<regex>' --as n          # by title
   $C claudeai mark --review … --after 2026-01-01 --before 2026-03-31 --as y   # by date
   ```

   Offer the project, title and date shortcuts whenever a pattern is obvious, and confirm each
   shortcut before applying it. Unmarked rows are left out, so marking only theirs is enough. When
   they are done, show the counts (`cut -f1 … | sort | uniq -c`).
4. Re-run the step 3 install command: it mines only the conversations marked mine, then merges
   every corpus into `~/.claude/state/$S/decisions.jsonl`.

## 5b. Other sources (optional)

*Ask* (multi-select): "Add anything else of yours to <Name>'s build?" (Sent mail, from a Gmail
export / Project folders, your own documents / Your answers to <Name>, on a rebuild / Nothing else).
Each source is one entry under `sources` in `crew/<slug>/private.json`. **A source is never read
until its entry has a `consent` date:** write the entry without one, and add
`"consent": "<today, YYYY-MM-DD>"` only after the principal says yes to that source below.
`$C sources list` shows every source and whether it may be read (never a path or an address).

**Sent mail.** Say in plain words: only mail they sent is read, matched by their own addresses
(and, in a Gmail export, by Gmail's Sent label); received mail, drafts, spam and trash are skipped
unread. Every header, the recipients, the subject, quoted replies, forwarded text and signatures are
cut before anything is kept. A paragraph holding an email address, a phone number, a long number, a
secret or one of their personal topics is dropped, and the rest of that mail kept. What is kept
stays in `~/.claude/state/<slug>/`, as their own messages.

1. **They** export it: at takeout.google.com, select only Mail, and in its options only the Sent
   label; the export arrives as a download link by email and holds an `.mbox` file. (Follow what
   Google's page shows; it changes.) Any other mail app's mbox works too; one with no Gmail labels
   counts every mail from their addresses. Then move it:
   `mkdir -p ~/.claude/state/$S/sources && mv "<the .mbox file>" ~/.claude/state/$S/sources/mail.mbox && chmod 600 ~/.claude/state/$S/sources/mail.mbox`
2. *Ask* for every address they send from, and whether to leave any mail out: mail to certain
   people (`exclude_to`, a regex matched against each recipient address, such as a family member's
   address or a personal domain) or outside a date window (`after`, `before`, `YYYY-MM-DD`). Write
   the entry:
   `{"name": "mail", "kind": "mbox", "path": "~/.claude/state/<slug>/sources/mail.mbox", "own_addresses": ["<address>"]}`
3. *Ask:* "Include your sent mail?" (Yes / Not now). On yes, add the consent date and run
   `$C sources mine --name mail`. Show its three count lines (they hold no mail text). If little was
   kept, the skip line says why: a missing address (`not from you`) or labels (`not in Sent`).

**Project folders.** They feed the lens (*The lens* below), not the decisions, and are read where
they are: nothing is moved or changed.

1. For each folder they name, write an entry:
   `{"name": "<short name>", "kind": "folder", "path": "<the folder>"}`, with `include` or
   `exclude` path patterns if they want them (`["drafts/**"]`: `*` stays in one folder, `**`
   crosses folders).
2. *Ask:* "Read <folder> for the lens?" (Yes / No). On yes, add the consent date and run
   `$C sources review --name <short name>`. It lists every file in
   `~/.claude/state/$S/sources/<short name>-review.tsv` (mine, modified, kind, claude, path). A
   file a Claude Code session on this Mac wrote or edited, and an agent file (`AGENTS.md`,
   `CLAUDE.md`, `GEMINI.md`, anything under `node_modules`), starts as `n`, with the `claude`
   column saying why. Only the paths of those writes are read from the transcripts, never what was
   written. Hidden files and folders (a name starting with `.`) are never listed.
3. Show the list as tables of about 40 rows. They mark theirs; you apply the marks, confirming each
   pattern first:

   ```bash
   $C sources mark --name <short name> --path '<pattern>' --as y      # or --unmarked, --all
   ```

   Unmarked files are left out, and re-running `review` keeps every mark. Files someone else wrote
   are not theirs, even in their folder. Then run *The lens* with `--sources`.

**Your answers to <Name>.** Only on a rebuild, once <Name> has been in use: `$C record` keeps each
of <Name>'s picks and, when the principal answered, what they said, in
`~/.claude/state/$S/live.jsonl`. *Ask:* "Use your answers to <Name> to train the rebuild?" (Yes /
No). On yes, write `{"name": "live", "kind": "live", "consent": "<today>"}`. Each answered line
becomes a training question, never a test one; a question already mined from the Claude Code
history is skipped, so nothing counts twice.

Re-run the step 3 install command: it reads every consented mail and live source and merges them.

## 6. Your domains

Unless the kit already supplied a codebook (`codebook` in `private.json`), *ask* the principal for
the three to six main areas their decisions fall into (their job's main kinds of work, writing,
admin, a side project; suggest a list from the folder names step 3 printed). Use short lowercase
names. Write them, plus `other`, into `member.json` → `codebook.domains`, and replace the domain
table in `crew/<slug>/briefs/codebook.md` with one row per domain and a plain description.

## 7. Inventory, and the interview if the history is thin

Show the merged counts: answered questions, typed messages, held out for the test, by source (each
Mac, claude.ai, sent mail, answers to <Name>). If there are fewer than about 150 answered questions, run the calibration
interview in `$REPO/crew/$S/interview.md`: draft its twenty questions from their
domains, show the list, then ask them four at a time, written to
`~/.claude/state/$S/decisions-interview.jsonl` in the shape it shows. Then re-run the step 3 install
command so the interview joins the corpus (always as training data).

## 8. Labelling (sub-agents)

```bash
B=~/.claude/state/$S/build; mkdir -p $B/parts
$C chunk --corpus ~/.claude/state/$S/decisions.jsonl --out-dir $B/chunks --size 300
```

For every chunk, start one sub-agent (Agent tool, model `sonnet`), several at once, with the brief
`$REPO/crew/$S/briefs/label-brief.md`, telling it its `<CHUNK>` (the file name without
`.jsonl`) and its `<MODE>` (`train` for `train-*`, `heldout` for `heldout-*`). Each replies with
counts only. Re-check every part yourself:

```bash
for f in $B/chunks/*.jsonl; do n=$(basename $f .jsonl); printf '%s: ' $n; $C check-labels --chunk $f --labels $B/parts/$n.json --mode ${n%%-*}; done
```

Spot-check alignment: a `reason` that shares no content word with its record is suspect; re-run a
chunk that has far more of those than the others. Then combine and tally (it must exit 0):

```bash
python3 -I -c 'import json,glob,sys; out={}; [out.update(json.load(open(f))) for f in sorted(glob.glob(sys.argv[1]+"/parts/*.json"))]; json.dump(out, open(sys.argv[1]+"/labels.json","w"))' $B
$C tally --corpus ~/.claude/state/$S/decisions.jsonl --labels $B/labels.json > $B/tally.md
```

## 9. Distillation (sub-agents)

```bash
$C distil-in --corpus ~/.claude/state/$S/decisions.jsonl --labels $B/labels.json --out-dir $B/distil-in
mkdir -p $B/distil
```

One sub-agent (model `sonnet`) per group file, with `$REPO/crew/$S/briefs/distil-brief.md`
and its `<GROUP>`. Then assemble `$B/decisions-draft.md` in the shape of `crew/<slug>/decisions.md`:
the front matter filled (`mined_through` = the newest training record's date; `sources` with the
real counts), the tally table from `$B/tally.md`, and every group's sections in the codebook's
category order.

## 10. The blind backtest

```bash
$C split --corpus ~/.claude/state/$S/decisions.jsonl --questions $B/heldout-questions.jsonl --answers $B/heldout-answers.jsonl
```

If there are **fewer than 30** held-out questions, skip the test: write in the calibration section
that there was too little history to measure, mark no category live, and go to step 11. Otherwise
split the questions into batches of about 40 (`$B/backtest/q1.jsonl`, `q2.jsonl`, …) and give each
batch to a sub-agent (model `sonnet`) with `$REPO/crew/$S/briefs/backtest-brief.md`.
**You must not show a predictor the answers file.** Combine the predictions and score them, with
the held-out questions' category labels from step 8:

```bash
python3 -I -c 'import json,glob,sys; out={}; [out.update(json.load(open(f))) for f in sorted(glob.glob(sys.argv[1]+"/backtest/p*.json"))]; json.dump(out, open(sys.argv[1]+"/predictions.json","w"))' $B
$C score --answers $B/heldout-answers.jsonl --labels $B/labels.json --predictions $B/predictions.json
```

Fill the calibration section and table of the draft from that output: a category is **live** at
5 or more High picks with at least 90% right. Add the dated entry to `crew/<slug>/measurements.md`.

## 11. Write crew/<slug>, and check it

Copy the draft to `$REPO/crew/$S/decisions.md`. Then both checks must pass:

```bash
cd "$REPO/crew/$S"
for f in *.md briefs/*.md; do [ "$f" = interview.md ] || $C shingle --doc "$f" --corpus ~/.claude/state/$S/decisions*.jsonl || echo "OVERLAP in $f"; done
[ ! -d ~/.claude/state/$S/build/lens-in ] || for f in *.md briefs/*.md; do $C shingle --doc "$f" --corpus ~/.claude/state/$S/build/lens-in/documents-*.jsonl || echo "OVERLAP in $f"; done
$C privacy-check --dir .
```

(`interview.md` is skipped against the decisions: the interview's questions were drafted from its
table. The second line runs only when the lens step ran, and checks every file against their
documents.) The shingle
check must print `0 overlaps` for every file (rewrite any overlapping line in your own words). The
second runs the merged filters (including `private.json`) over every committed file in
`crew/<slug>/` at any depth, and over `member.json` outside its `filters`. It must print `clean`.
A hit names a file and line (or `(not text)`), or a `member.json` key path, never the text (file
names themselves are not checked, so keep them generic): find it yourself and
remove it, without echoing what it matched. A `filters.<key> (differs from crew/_template)` hit
means `member.json`'s own patterns were edited, which the scan cannot check because they *are* the
patterns: move anything only the principal would know into `private.json`, and copy that key back
from `crew/_template/member.json`.

## 12. The principal sets the dial

Show `crew/<slug>/autonomy.md` and the calibration table. *Ask* (multi-select): "Which kinds of
question may <Name> decide on their own when confident?" listing the `[ ]` and `[x]` lines under
*Judgment*, with the backtest's live categories marked. Keep every line under the principal's
*actions* and *Risk* as `[x]` unless they explicitly open one. Write their choices and today's date.

## 13. Install the assistant

```bash
mkdir -p ~/.claude/skills/$S && cp "$REPO/crew/$S/SKILL.md" ~/.claude/skills/$S/SKILL.md
```

*Ask* before editing their global instructions, then add exactly one line to `~/.claude/CLAUDE.md`
(create it if missing):
`- Ask <Name> (/<slug>) before asking me any question; they say what I would likely pick and how sure they are.`

Try it: ask the assistant one real, harmless question from the principal's current work and show
the answer.

## 14. Save the model to your private repo

```bash
cd "$REPO" && git switch -c crew/$S
git add crew/$S && git status --short
```

`git status` must show only files under `crew/<slug>/`, and never `private.json`. Then commit and
push to their private repo:

```bash
git commit -m "assistant: <Name>, <their name>'s assistant"
git push -u origin crew/$S
```

Merging it into their repo's `main` (or keeping it on the branch) is their call. Never open a pull
request to another repo unless they ask for one.

## 15. The first job (only if the kit has one)

If `crew/<slug>/briefs/first-job.md` exists (it comes only from a kit), follow it. Otherwise ask the
assistant to suggest, in two or three lines, which of the principal's recurring decisions it could
take off their plate first, given the live categories. That ends the build. Tell the principal what
was made, where it lives, and how to use it: type `/<slug>` with a question, or just work; sessions
now ask the assistant first.

## The route without a Claude history

Some principals never use Claude themselves: they have no account, or they are under 18, which
Anthropic's consumer terms require. There is nothing of theirs to mine, so the member learns from
two other sources: **their own work** (the lens below) and **question rounds**, forms they fill in
whose answers an adult relays (`crew/<slug>/interview.md`, *Question rounds*). The person running
the build here is that adult, not the principal: **never mine this Mac's history for this member.**

Run the steps above with these changes:

- **Step 1.** Say what will be read (the documents they choose, their round answers), what is
  dropped and where it stays, to the principal in plain words and, when they are under 18, to their
  parent or guardian too. The principal can say no, and can ask later for everything to be deleted
  (`~/.claude/state/<slug>/` and `crew/<slug>/`). Ask the topics question now, not at step 3:
  school, friends' names and where they live are typical additions to `filters.personal` in
  `private.json`.
- **Step 3.** Add `--no-history` to the install command. It fills `crew/<slug>/`, reads no Claude
  Code history, no export and no mail or live source, and reports that there is nothing to merge
  yet. Skip steps 4 and 5. At step 5b offer project folders only: this Mac's mail and its live log
  are the adult's.
- **Step 7 becomes the lens and round 1.** Run *The lens* below. Then draft round 1 (about twenty
  questions) as `interview.md` describes, show the draft to the adult (and the principal), turn it
  into the form, and when the answers come back import them with `$C round --name 1 …` and re-run
  the step 3 install command (with `--no-history`).
- **Steps 8 and 9** as written.
- **Step 10 is round 2, held out.** Draft about fifteen new questions, never shown to step 9's
  sub-agents, and import the answers with `--heldout`; re-run the install command. Re-run step 8's
  `chunk` and label only the new `heldout-*` chunks. Then step 10 as written, except its 30-question
  floor: score the round whatever its size and give N beside every number. A category still needs 5
  High picks at 90% or better to go live, so expect few or none from one round.
- **Steps 11 to 15** as written. Step 13's one line in `~/.claude/CLAUDE.md` belongs to whoever
  works with the member, not to the principal.

### The lens (either route)

Any principal can add this: a short file on how they build things, distilled from their own work.
Ask them (or the adult) to put the files they choose in one folder: text, Markdown, Word (`.docx`)
and PDF files are read, images are listed by file name only, and nothing else is opened. Move the
folder into the state dir, so the raw text lives only there, and confirm their original copy can go.
Project folders from step 5b stay where they are: add `--sources` to the `documents` command, and it
also reads every file marked `y` in their reviews, named `<short name>/<path>`. With only folders,
use `--sources` alone, without `--dir`:

```bash
mkdir -p ~/.claude/state/$S && mv "<their folder>" ~/.claude/state/$S/raw && chmod -R go-rwx ~/.claude/state/$S/raw
B=~/.claude/state/$S/build
$C documents --dir ~/.claude/state/$S/raw --out-dir $B/lens-in
```

Show the two count lines. A PDF skipped for want of `pdftotext` needs `brew install poppler` (ask
first) or an export as `.docx` or `.txt`; then run it again. `$B/lens-in/inventory.jsonl` lists
every file and why any was skipped. For every `documents-NN.jsonl`, start one sub-agent (model
`sonnet`) with `$REPO/crew/$S/briefs/lens-brief.md`, `<MODE>` `notes` and its `<CHUNK>` (`NN`), then
one with `<MODE>` `combine`. Check the draft, which must print `0 overlaps`:

```bash
$C shingle --doc $B/lens-draft.md --corpus $B/lens-in/documents-*.jsonl
```

Show the draft to the principal (and the adult) and correct what they say is wrong, then copy it
to `$REPO/crew/$S/lens.md`. Step 11 checks it again with everything else.
