# voice

Build your own AI assistant that answers decisions the way you would. It learns from your own
answers and messages to Claude Code, says how sure it is, and hands a question back to you when it
is yours to make.

This repo is the kit: a member template, a guided build that Claude Code walks you through, and
the scripts and tests behind it. It ships no assistant of its own. An assistant you build is called
a *member*, and it lives in `crew/<slug>/` of your own private copy.

## What it reads, and where things go

- **It reads** your Claude Code history on your Mac (`~/.claude/projects/`) and, if you choose, a
  claude.ai data export. From these it keeps the questions Claude asked you with the option you
  picked, and the messages you typed.
- **Filters run first.** Records about your own health, family, legal matters or household money
  are dropped, and so is anything holding a secret, someone else's identifier, an email address, a
  phone number or a long run of digits. Your member's gitignored `private.json` can add patterns
  only you would know.
- **Raw data stays on your Mac**, in `~/.claude/state/<slug>/` (folders 0700, files 0600). The
  build's sub-agents (Claude sessions the build starts) read the filtered records there.
- **Only a paraphrase is committed**: `crew/<slug>/` holds how you decide, in the build's own
  words, checked so that no run of 6 of your words appears in it.
- **Nothing is sent anywhere else** except an optional copy between your own Macs and your own
  `git push`.

The assistant models your decisions from your history. It is not advice, and by design it never
sends, signs, approves or spends as you.

## Build one

You need a Mac with Claude Code, git, Python 3.9 or newer, and a GitHub account with an SSH key. A
build takes two to three hours, most of it sub-agents working, and uses a lot of Claude tokens.
It is safe to stop and re-run: the installer keeps what you have already marked.

1. **Make your own private repo.** Your member describes how you decide, so keep it private. Never
   use GitHub's Fork button: a fork of a public repo is public and can't be made private. Create an
   empty private repo on GitHub, then clone this repo wherever you like and point it at yours:

   ```bash
   git clone https://github.com/ziazon/voice.git
   cd voice
   git remote set-url origin git@github.com:<you>/<your-private-repo>.git
   git push -u origin main
   ```

   To pull later kit updates: `git remote add upstream https://github.com/ziazon/voice.git`, then
   `git pull upstream main`.

2. **Start the build.** Open Claude Code in that folder and say:
   *"Follow bootstrap/PROMPT.md with me, step by step."*
   If you use Claude Code on other Macs too, step 4 asks you to run `bootstrap/PROMPT-extra-mac.md`
   on each of them.

## Layout

| Path | What |
| --- | --- |
| `crew/_template/` | The member template: identity, decision model, autonomy dial, measurements, report, skill, sub-agent briefs. "Crew" is just the folder's name for the set of members you build |
| `bootstrap/` | `PROMPT.md` (the guided build), `PROMPT-extra-mac.md`, `install.sh` |
| `scripts/crew_decisions.py` | Mines, splits, tallies, labels and scores your decision history; every command takes `--member <member.json>` |
| `scripts/slack_mine.py`, `scripts/slack_select.py` | Optional: turn a Slack JSON export into samples and candidate decision threads for persona work (see below) |
| `scripts/ci-local.sh` | Runs every test and shellcheck; needs `shellcheck` (`brew install shellcheck`) |
| `tests/` | The suites `ci-local.sh` runs, all on fictional data |

A *kit*, in the installer's `--kit DIR` option, is a private folder of your own filter and
template overrides; most people don't need one.

## Optional: Slack miners

`slack_mine.py` reads a Slack export laid out as `users.json`, `conversations.json` and
`messages/<conversation id>.jsonl` (its docstring has the fields), and writes stats, samples and
candidate decision threads to a work folder outside any git repo. Who to mine comes from a config
file you keep outside the repo:

```json
{
  "workspace_url": "https://example.slack.com",
  "principal": "U0SAM",
  "people": {"U0SAM": "sam", "U0ALEX": "alex"},
  "filters": {"personal": "my therapist|divorce", "secret": "\\bpassw(or)?d\\b"}
}
```

```bash
python3 -I scripts/slack_mine.py --config ~/private/slack.json <export dir> <work dir>
python3 -I scripts/slack_select.py --config ~/private/slack.json <work dir>
```

## Rules

- No real people in this repo: no member, no persona, no name, handle, workspace or quote.
- Recordings of a real person's voice are never committed; `.gitignore` blocks audio files.
- See [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request, and
  [SECURITY.md](SECURITY.md) to report a privacy or security problem.
