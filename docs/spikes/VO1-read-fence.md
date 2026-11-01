# VO1: proving the read-wide fence

The question (VO plan, VO-D2 *read wide, exit narrow*): when a `claude -p` turn may use Read, Grep
and Glob over folders the person chose, does a secrets deny list hold **inside** those folders, is
no other tool offered, and is a settings file that fails validation caught rather than silently
ignored? VO6 (the deliberation relay's read-wide seats) copies the answer, and so does any later
Voice turn that is given file tools.

**Status: the instrument is built and its offline half is proven; the live runs are owed.** An unattended
run's nested `claude` can't sign in, so the two live runs (one Mac, then a second) are attended:
each runs `python3 scripts/read_fence_probe.py run` with the person's own signed-in Claude Code.

## The flags and rules a read-wide turn uses

```
claude -p --model <model> --no-session-persistence --output-format stream-json --verbose
  --strict-mcp-config --mcp-config '{"mcpServers":{}}' --tools Read,Grep,Glob
  --permission-mode manual --permission-prompts none
  --add-dir <each root> --settings <a settings file> --debug-file <a log for this turn>
```

(`--model haiku` and `--max-budget-usd 0.10` are the probe's own choices.) The settings file holds
only `{"permissions": {"deny": [...]}}`: for every name in the built-in deny list
(`builder/core/src/deny.rs`), `Read(//**/<name>)` and `Read(//**/<name>/**)`; for every run of
names, `Read(//**/<a>/<b>)` and `Read(//**/<a>/<b>/**)` (34 rules). `scripts/read_fence_probe.py`
holds them in one place (`FENCE_FLAGS`, `deny_rules()`), and a test keeps them equal to `deny.rs`.

- **`//`, not `**/`.** Claude Code anchors a rule written `**/.env` (or a bare `.env`) at the
  current directory, so it does not reach a folder added with `--add-dir` outside it; `//**/.env`
  is anchored at the filesystem root and does (`https://code.claude.com/docs/en/permissions`, read
  2026-11-01). The probe's `relative` turn shows the difference on a live run.
- **Catching a broken settings file.** `-p` ignores a settings file that fails validation without a
  word (`claude --help`). With `--debug-file`, a valid file writes
  `Applying permission update: Adding <n> deny rule(s) to destination 'flagSettings': [<the rules>]`
  to the log, and an invalid one writes nothing like it. A caller refuses a turn whose log lacks
  that line with exactly the rules it sent. This leans on a debug line Claude Code doesn't
  document, so it fails closed: if a later version changes the wording, turns are refused, never
  trusted.
- **The mode.** `manual` is the prompting default (it shows as `default` in the turn's `init`
  event); `--permission-prompts none` denies anything that would prompt. Never `auto` or
  `bypassPermissions`.

## What was measured (2026-11-01, Claude Code 2.1.292, macOS 27, signed out)

Each was seen without a sign-in, so it needed no usage:

- The `system`/`init` event of `stream-json --verbose` (printed before any reply, even signed out;
  on a Mac with hooks, the hooks' own events come first) lists
  `tools: ["Glob","Grep","Read"]`, `mcp_servers: []` and `permissionMode: "default"` for the flags
  above.
- `--settings` with a valid file or JSON string writes the line above, with the rules sent, before
  the `init` event; with `{"permissions":{"deny":"not a list"}}` it writes nothing like it and the
  turn runs anyway.
- `python3 scripts/read_fence_probe.py check` printed every check line `pass` (tools offered, MCP
  servers none, mode default, the 34 rules applied, a broken settings file caught) and exited 0;
  `run` stopped with *Sign in to Claude Code first*, as it should. On that Mac it also reported
  that the person's own settings bring hooks and plugins into every turn (by count only).

## What the live run proves (owed)

`run` adds a canary file for every deny-list entry inside the granted folder, each holding a
random invented word: the plain names, one nested two folders down, upper-case spellings
(`.ENV`, `SITE.PEM`), a symlink with an ordinary name pointing at `.env`, and a symlink pointing
out of the folder, plus a file outside every folder and an ordinary control file. It passes only
when the control was read, every other read was denied by Claude Code itself (a
`permission_denials` entry, not the model declining), no denied file's word appears anywhere in
the turn's output, and a Grep over the folder found the control's word and none of the others. For
the record, not pass or fail: how many denied names Glob listed, whether a `**/` rule held, and
whether `.env` was readable with no rules at all.

## Not covered

- Paths a seat may not list at all (the whole home folder, Q5) are VO6's to refuse.
- A Mac whose own settings widen reads (`defaultMode`, `additionalDirectories`, allow rules for
  Read, Grep or Glob) is VO6's `doctor` to refuse; the probe only reports it.
- Hard links under an ordinary name are outside a path list, as in `.ai/SECURITY.md`.
- Claude Code applies Read rules to Grep and Glob "best-effort", and its docs say it applies them
  to the folder Grep searches, not that it filters what Grep finds inside it. The live Grep line
  may well fail on non-hidden names such as `site.pem` (hidden files may drop out only because the
  search skips them). If it does, VO6 must treat Grep as unfenced inside a root. The probe
  measures Grep over one folder, not every way a search can be phrased.
- The person's own Claude Code settings still load into a read-wide turn: hooks, `env`,
  `apiKeyHelper` and turned-on plugins (the probe counts them, never printing a value). VO6 decides
  whether to drop them (`--setting-sources` or `--restricted`, which was measured on 2026-10-26 to
  also stop installed skills loading) or to refuse such a Mac in `doctor`.
- A folder added with `--add-dir` can bring its own `.claude/skills`, `commands` and `agents` into
  the turn (Claude Code's docs, *Working directories*). That is VO6's to handle for each root.
- Claude Code drops a malformed rule and keeps the rest of the list (a check measured
  `["Read(//**/.env)", "NotATool(x"]` logging one rule added), so a caller must compare the whole
  list it sent, not only the count; and a good `deny` list beside an `allow` that isn't a list
  was dropped whole (no line logged).
