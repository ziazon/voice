# Decisions: voice

## 2026-10-23: a public kit with no members

- **The repo ships no member.** Only `crew/_template/` and the build. Builders keep their member in a
  private copy of their own; a fork of a public repo is public, so the README says not to fork.
- **Miners are generic.** A script that mines a person's messages takes the people, their labels,
  the workspace and any filters from a config file outside the repo, and its tests use fictional
  fixtures.
- **No fixed paths.** Scripts locate the repo from their own path; docs never name a clone folder.
- **The folder keeps its name.** `crew/` and `crew_decisions.py` stay as they are; the docs say
  "AI assistant" and "member".

## 2026-10-23: a route for a principal with no Claude history

- **Two sources replace the history:** the principal's own documents, distilled into a lens
  (`documents` plus `lens-brief.md`), and question rounds answered on a form an adult relays
  (`round`). The kit names no principal; the case behind it is anyone without a Claude account of
  their own, such as someone under 18.
- **A round trains unless imported with `--heldout`;** a held-out round keeps that split through
  `merge --rounds` and never moves the history's cutoff, so the existing backtest scores the
  model blind on it. A whole round is held out or none of it.
- **`install.sh --no-history` never mines the Mac it runs on.** That Mac's history is the adult's.
- **PDF text comes from poppler's `pdftotext` when it is installed,** not from a Python
  dependency: the kit stays standard-library only, and a missing tool is a skip that says how to
  fix it.

## 2026-10-25: extra sources, each behind a consent date

- **Sources live in the gitignored `private.json`** as a `sources` list (kind, path, the kind's
  options); `member.json` may not hold one. A source is read only once its `consent` date is set,
  which the build writes after the principal's yes.
- **Mail comes from a local mbox** (Google Takeout), sent mail only, never a live mail connector:
  the kit reads only local files, and the agent never sees received mail. The filters run per
  paragraph, so one address in a mail drops that paragraph, not the mail.
- **Project folders feed the lens only,** read in place; files Claude wrote or edited start as not
  mine. Documents are not decisions, so they stay out of the decision corpus.
- **The live log trains, never tests** (`decisions-live.jsonl` is Train in install.sh and in the
  Rust `discover`), and skips a question already mined from the history.
- The Rust port of `sources` waits for the app's sources screen.

## 2026-10-26: shipped decision commands (PB8.1)

An app-made assistant's SKILL.md runs the `voice` shipped inside the app
(`Contents/MacOS/voice`), named as one quoted path when the assistant is made; the guided
build keeps `scripts/crew_decisions.py`. The template names both through `{{SIMILAR}}` and
`{{RECORD}}`. An assistant made before this keeps its old SKILL.md; make it again to get the new one.

## 2026-10-26: texts go to the decision commands on stdin (PB8.3)

The SKILL.md's look-up and record commands take their texts as one JSON object on stdin
(`--stdin`, a quoted heredoc), never as shell words, in the app's `voice` and the
guided build's Python alike; the flags stay for people typing the commands. JSON over one field
per line because a text can hold line breaks and a line that matches the heredoc's end. An
assistant made before this keeps its old SKILL.md; make it again to get the new one.

## 2026-10-31: files the app never opens (VO8.3)

The app checks every file of the person's it is about to read against a list of password and key
files (`voice_core::deny`), as given and with links resolved, without regard to letter case,
and never opens one that matches. The list is built in (the VO plan's list) plus the person's own
`filters.denied_paths` in member.json or private.json: names, or `~/`/`/` paths, in plain
letters only, because the app has no Unicode normalisation and macOS writes accented names
decomposed. It lives in core because every reader depends on core. Only the app applies it: the
Python kit's own readers don't, and its `private.json` reader refuses a `denied_paths` list, so a
member that sets one is read by the app only. A hard link to a key file under an ordinary name
gets past a path list; the text filters still apply to what is read.

## 2026-10-31: one provider interface for every model call (VO8.1)

Every place the app asks a model takes a `Provider` (`voice_agent::provider`), not
Claude Code itself: an id, a name, where its data goes (`Reach`: your own machines, Anthropic, a
third party) and one tool-less call. A `Brief` has no tools field, so no provider can be handed
one. The person's own Claude Code is the first provider, unchanged. Finding, checking and signing
in to Claude Code stay Claude Code's own, because a later provider will have none of them.

## 2026-10-31: where each source's data may go (VO8.2)

Every brief names the sources of what it carries, and every model call goes through the privacy
guard (`voice_agent::guard`): it hands the brief only to a provider whose reach every
source's ceiling allows, seals it for that provider, and refuses in plain words when none is
allowed, before anything is spent. A provider takes only a sealed brief and checks the seal again
first. The ceilings (`local-only`, `anthropic`, `any-consented`) default to the VO plan's gate
answer Q6: the person's Claude history, the assistant's own folder and the copy of their skills
may go to Anthropic; mail, calendar, contacts, family and legal skills stay on their own
machines. Two choices made at pickup:

- **What the person types into Voice is a source of its own** (`typed`: interview
  answers, notes on answers, a panel's question and steers), defaulting to `anthropic` because
  it is typed into steps that run on Claude Code. The person can keep it home like any other.
- **Ceilings live in member.json** (`"ceilings": {"<source>": "<ceiling>"}`), changed with
  `voice ceilings show|set`; private.json may not hold them, so one file is the only
  setting. The app gets its screen in a later slice.

`any-consented` reaches no third party yet: there is no third-party provider and no record of
the person agreeing to one. The build names mail when mail mined by the guided build can be in
its corpus (a `decisions-src-*` file in the state folder, or an `mbox` source in the config),
because merged records do not say where they came from. Claude Code's child no longer inherits
the variables that switch it to another provider or endpoint. The interview's draft is labelled
with the assistant's own folder only: it sends the kinds of decision and how many of each the
last build found, numbers but no record.

## 2026-10-31: a model server on the person's own machines (VO9)

The second provider is any model server on this computer or the person's own network that speaks
the OpenAI-style `POST /v1/chat/completions` with a JSON-schema reply (Ollama, llama.cpp's
`llama-server`, LM Studio, vLLM all do), set up per assistant with
`voice self-hosted set --url … --model …` and kept in the assistant's state folder
(`self-hosted.json`, 0600), never in member.json, because the address belongs to this Mac.
Choices made at pickup:

- **Claude Code first, the server second.** A step goes where it goes today unless its data is
  kept home; a person who wants everything local sets every ceiling to `local-only`.
- **This computer only, unless the person says their network.** The check is on the address
  connected to, not the name typed, for every address a name resolves to at each connection:
  loopback by default; with `--network`, also `10/8`, `172.16/12`, `192.168/16`, IPv6 unique-local
  (`fc00::/7`) and link-local.
  A private address isn't proof of the person's own network (on other Wi-Fi a stranger's device
  can answer at the same address or name), so `--network` is the person's choice and the words
  say so. `100.64/10` is left out (carrier-grade NAT is shared with strangers), so a Tailscale
  address doesn't count yet. No proxy, no redirects.
- **No token yet.** The common servers need none on a home network; a token kept in the
  Keychain is a later card.
- **A call costs $0 but still waits for the daily limit,** so one limit governs every step and
  every refusal reads the same.
- **The build cache keeps Claude Code's keys** (a rebuild reuses what the person already paid
  for) and keys another provider's replies apart; the backtest entry names who answered.
- **Panel turns take the narrowest ceilings across every seat,** because a seat's post carries
  its folder's data into the other seats' briefs.

The app's screen for it is a later card, as for ceilings.

## 2026-10-31: the operating-system sandbox is Seatbelt, one profile per child (VO11)

The VO11 spike (`docs/spikes/VO11-os-sandbox.md`) decides how the broker is packaged on macOS:

- **Seatbelt, not App Sandbox.** Each sandboxed process is started through `/usr/bin/sandbox-exec`
  with a profile kept in the repo and its folders passed as parameters. App Sandbox has a single
  on-or-off network switch, gives a child the whole app's grants, can't run a Claude Code
  installed outside the app, and needs a signed app (PB-D3).
- **Siblings, never nested.** macOS refuses a sandbox inside a sandbox, so an unsandboxed
  launcher (the app) starts each sandboxed piece itself: the `claude` child first (VO11.1), the
  broker's reading in a helper of its own later (VO11.2).
- **One port out.** A sandboxed child may connect only to a proxy on this computer, which allows
  `CONNECT` to port 443 of a listed name only. Claude Code follows `HTTPS_PROXY` and has no SOCKS
  support.
- **The profile's own tests are part of the checks.** `builder/agent/tests/sandbox_profile.rs`
  runs on every Mac in `scripts/ci-local.sh` (`VOICE_REQUIRE_SANDBOX=1`), since Apple
  doesn't document the profile language and an update could change it.
- **Not wired in yet.** Nothing runs under the profile until VO11.1, which waits on the owner's
  answer about where Claude Code's login lives inside it (the profile reaches no Keychain).

## 2026-10-31: the anonymizer copies a skill whole or not at all (VO3)

`voice anonymize` copies the skills a person agreed to, with private details taken out
(`voice_ingest::anonymize`, `::skills`; brief `plans/briefs/VO3-anonymizer.md`):

- **A yes per skill, kept in private state** (`~/.claude/state/<slug>/anonymizer/skills.json`),
  with the folder it was given for: a skill that moves waits for a new yes. claude.ai's synced
  skills are never offered; naming one is the yes. The person's `never_copy` list beats a yes.
- **The rules are data.** Built-in detectors (emails, phone and long numbers, secrets, other
  people's home folders, private hosts and addresses, ticket ids) are always on; the person's
  terms, patterns, allow list, never-copy list, dropped files and project folders live in the
  `anonymizer` object of their gitignored `private.json`, never in `member.json`.
- **Transform, then re-scan, then refuse.** Invisible characters and `<!-- private -->` sections
  go, home folders become `~`, a listed term becomes a placeholder (`<company-1>`) that keeps its
  number for good (`anonymizer/placeholders.json`, private). Then everything is checked again,
  terms included (wrapped over a line, or inside a longer word); any hit refuses the whole skill.
  `LOCAL.md`, `LEARNINGS.md`, hidden, password and key files, links and files over 1 MiB are left
  out unread.
- **The report names file, line and kind**, the line being the source file's, never the text.
- **`skills list` shows ids and folders as they are**, numbered (`1. <id>…`): the person needs
  them to agree. `anonymize`'s report never shows a found term; a skill whose name holds one is
  named by its number in that list, and a file path's private parts print as `*`.
- **A term is found inside longer words and through punctuation** (`mypebble-freight.com`), when it
  has 4 or more letters or digits, and `ss` and `ß` match each other. An allow entry exempts a hit
  only when one entry covers all of it; a file or folder that can't be read is left out as
  `unreadable`, and the run goes on.
- **A term needs a letter or a digit** (`\p{L}` or `\p{N}`, the classes the separator leaves out).
  Words with neither inside a term (a sign-off dash)
  are left out of its search, and a term with no letter or digit at all is refused when the rules
  load. Separators between a term's words have no length cap.
- **No hashes yet.** Refreshing on change, and the review screen for new skills, are VO4 (the
  mirror and its hashes: VO4.1, below).

## 2026-10-31: the app is Voice, and nothing is migrated (VO2)

The app in `builder/` was Persona Builder; it is Voice now, everywhere (brief
`plans/briefs/VO2-rename-to-voice.md` holds the whole name map).

- **The names:** the command is `voice`; the crates are `voice-core` (library `voice_core`) and
  `voice-<x>`; the app is `Voice.app` (`voice-app` inside, with the `voice` command beside it);
  the screens are `@voice/screens` with `VoiceApp` at the root; variables are `VOICE_*`; the
  bundle id is `dev.voice.desktop`, still a placeholder until signing.
- **Nothing is migrated.** The app is not released and no assistant exists outside invented test
  ones, so every name on disk moved and nothing reads the old ones: Claude Code's own sign-in
  (`Application Support/Voice/claude-code`), the GitHub sign-in in the password store
  (`voice.github`), the CLAUDE.md markers (`<!-- voice:`), the report schema (`voice/report/1`).
  An older build's files under the old names are left where they are.
- **Turning on uses the shared `~/.claude/voice/`**, which Panels use too: the rules go in
  `assistants/<slug>/RULES.md` and the record is `installed.json`. Turning on creates the shared
  folder (0700) when it is missing and never counts it as its own, so turning off never removes
  it or the panels in it, and an assistant named "Panels" can't land in the panels' folder.
- **The consent wording names Voice, so it is version 2**: a yes given to version 1 is asked again.
- **`.ai/handoffs/persona-builder.md` keeps its name and its old wording:** it is the dated record
  of what was built, under the names of the day.

## 2026-11-01: the skill mirror is fresh, absent, or marked out of date (VO4.1)

- **The mirror lives in private state**, `~/.claude/state/<slug>/mirror/` (0700 folders, 0600
  files), so `voice forget` deletes it with the rest.
- **Hashes of the copies only.** `.manifest.json` lists the SHA-256 of each file as written in the
  copy. A hash of a private source file would let a reader test a guess about what it says.
- **Every refresh starts from nothing**, so no copy made under older rules survives a refresh that
  succeeds. Withdrawing a skill removes the mirror at once.
- **A failed refresh never leaves the mirror looking fresh.** Once the overlap check has passed,
  a failure removes Voice's own copy (only a folder bearing its exact stamp, never a link). Before
  that check (the rules or the saved placeholders can't be read) or when the check refuses, Voice
  removes nothing and only drops the manifest. Each message says which happened, and none tells
  the person to delete a folder that may hold their own files.
- **Voice never empties a folder that overlaps the person's own**: a skill root, anything listed
  in one (even an entry discovery skips), a skill's folder, or a project listed in `private.json`.
- **Readers trust the mirror only with Voice's stamp and a regular, valid manifest**, and refresh
  first anyway (VO4.2).
- **A panel refreshes each seat's mirror before its first round** (VO4.2), in convene and in a
  resume that will run a round, under the state locks the panel already holds. A seat is refreshed
  when it has agreed to a skill, its agreements can't be read, or a mirror is there (or can't be
  checked), so after the start every seat's mirror is fresh, absent, or marked out of date; an
  assistant that never agreed to a skill gets no mirror. A failed refresh never stops the panel:
  one fixed line names the assistant, never a skill, a path or the error.
- **The mirror's path is shown starred** (`~/.claude/state/*/mirror`) whenever the slug holds a
  term, or the rules didn't load and the terms are unknown.

## 2026-11-01: a read-wide turn's fence, and how it is proven (VO1)

- **The fence is Claude Code's own permission rules**, given per turn: `--tools Read,Grep,Glob`,
  no MCP server, `--permission-mode manual` with `--permission-prompts none`, `--add-dir` per
  root, and the built-in deny list as `Read(//**/<name>)` and `Read(//**/<name>/**)` rules in a
  `--settings` file. `docs/spikes/VO1-read-fence.md` has the full argv; `scripts/read_fence_probe.py`
  holds it in one place and a test keeps its rules equal to `voice_core::deny`.
- **Anchored at `//`**, because Claude Code reads `**/.env` as "under the current directory", which
  a folder added with `--add-dir` is not.
- **A turn whose settings weren't applied is refused.** `-p` ignores a broken settings file
  silently, so the caller passes `--debug-file` and requires Claude Code's line saying it added
  exactly the rules sent. The line isn't documented, so this fails closed.
- **Proven on a person's own Mac, attended.** An unattended run's nested `claude` can't sign in, so the
  live runs are the person's: `python3 scripts/read_fence_probe.py run`, which passes only when
  every deny-list canary's read is denied by Claude Code (`permission_denials`) and no canary's
  contents appear in the turn's output. Until both Macs have passed, nothing relies on the fence.

## 2026-11-02: every `claude` runs in the sandbox (VO11.1c)

Supersedes the VO11 entry's *Not wired in yet*. The owner chose a login file in Voice's own
Claude Code folder (V-Q10, option 1), so VO11.1 went ahead in three parts: the proxy (VO11.1a), the
launch as a library (VO11.1b) and the wiring (VO11.1c).

- **One launch per `claude`, each with a proxy of its own.** The check's two runs, each call and
  the sign-in each start `sandbox::Launch` with an `egress::Egress` that lives exactly as long as
  the child. The environment is built from nothing, so no credential, provider switch or endpoint
  of the person's reaches Claude Code. `ANTHROPIC_API_KEY` can't either, so the note saying it might
  bill that key is gone.
- **Each launch gets a fresh run folder, never the session's work folder.** Measured on Mimi:
  a sandboxed `claude -p` leaves a `claude-<uid>/` folder in `CLAUDE_CODE_TMPDIR`, so a reused work
  folder would refuse the next call. A fresh 0700 folder in `runs/` beside the profile is just as
  empty and Voice-owned, and is removed when the launch ends. The work folder is gone.
- **The app opens the sign-in page itself.** Claude Code hands its link to a helper in the run
  folder. Voice checks it (`sandbox::valid_link`), opens it once with `/usr/bin/open`, and never
  shows, logs or returns it. A link that fails the check, or a page that won't open, stops the
  sign-in. `voice claude sign-in` works the same way, and Ctrl-C stops the sandboxed child too.
- **Tests run stub `claude`s directly, never production.** A `#!` script can't run in the sandbox.
  So the `voice-agent` feature `test-stubs`, enabled only from `[dev-dependencies]`, runs a script
  directly with the same arguments and environment. Native programs go through `sandbox-exec` in
  every build. Test builds of the CLI (`cargo test`, or any `--all-targets` build) carry the feature;
  its shipping build and the app's don't. `scripts/ci-local.sh` checks, for every target, that the
  CLI and app builds lack it, and builds them clean without it.
- **Still owed: the walk.** Nobody has yet signed in and asked inside the sandbox on a person's own
  Mac (lane V's walk list). Until that passes, the result rests on the stub tests and the signed-out
  runs measured here.

## 2026-11-02: the app reads through the reader (VO11.2c)

- **The app reads history only through the sandboxed reader**, with exactly the step's source and its permission checked both before launch and inside the reader.
- **Protocol violations are one fixed error.** Output is bounded and progress relayed line by line; a final line must agree with the exit status.
- **Reader deadlines bound silence.** A reader silent for 10 minutes, or keeping output open 5 seconds after its answer, is stopped.
- **Only Voice's fixed sentences reach the screen.** Any other reader refusal becomes the step's own sentence.
- **The export kind is stored and enforced on the opened handle.** A choice without its kind must be made again.
- **Missing projects need no reader.** With no `~/.claude/projects`, the app returns the empty answers itself (after the lock for the read) and reads nothing.
- **macOS only.** Other platforms keep reading in-process; skills wait for VO11.2d.

## 2026-11-02: the reader starts with only stdin, stdout and stderr (VO11.2c.2)

- **The reader starts through `posix_spawn` with close-by-default**, with only fds 0 to 2, instead of `Command`: Rust's macOS spawn can hand it another child's descriptors.
- **The reverse case is a retry.** Another child started while the reader's pipes are being made (marked close-on-exec a step later; macOS has no `pipe2`) can still inherit either end of its stdin or stdout; the deadlines (5 seconds after an answer, 10 silent minutes) bound the wait and the person tries again.

## Recordings

- Recordings of a real person's voice never enter git. `.gitignore` blocks audio; never weaken it.
