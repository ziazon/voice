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

An app-made assistant's SKILL.md runs the `persona-builder` shipped inside the app
(`Contents/MacOS/persona-builder`), named as one quoted path when the assistant is made; the guided
build keeps `scripts/crew_decisions.py`. The template names both through `{{SIMILAR}}` and
`{{RECORD}}`. An assistant made before this keeps its old SKILL.md; make it again to get the new one.

## 2026-10-26: texts go to the decision commands on stdin (PB8.3)

The SKILL.md's look-up and record commands take their texts as one JSON object on stdin
(`--stdin`, a quoted heredoc), never as shell words, in the app's `persona-builder` and the
guided build's Python alike; the flags stay for people typing the commands. JSON over one field
per line because a text can hold line breaks and a line that matches the heredoc's end. An
assistant made before this keeps its old SKILL.md; make it again to get the new one.

## 2026-10-31: files the app never opens (VO8.3)

The app checks every file of the person's it is about to read against a list of password and key
files (`persona_builder::deny`), as given and with links resolved, without regard to letter case,
and never opens one that matches. The list is built in (the VO plan's list) plus the person's own
`filters.denied_paths` in member.json or private.json: names, or `~/`/`/` paths, in plain
letters only, because the app has no Unicode normalisation and macOS writes accented names
decomposed. It lives in core because every reader depends on core. Only the app applies it: the
Python kit's own readers don't, and its `private.json` reader refuses a `denied_paths` list, so a
member that sets one is read by the app only. A hard link to a key file under an ordinary name
gets past a path list; the text filters still apply to what is read.

## 2026-10-31: one provider interface for every model call (VO8.1)

Every place the app asks a model takes a `Provider` (`persona_builder_agent::provider`), not
Claude Code itself: an id, a name, where its data goes (`Reach`: your own machines, Anthropic, a
third party) and one tool-less call. A `Brief` has no tools field, so no provider can be handed
one. The person's own Claude Code is the first provider, unchanged. Finding, checking and signing
in to Claude Code stay Claude Code's own, because a later provider will have none of them.

## 2026-10-31: where each source's data may go (VO8.2)

Every brief names the sources of what it carries, and every model call goes through the privacy
guard (`persona_builder_agent::guard`): it hands the brief only to a provider whose reach every
source's ceiling allows, seals it for that provider, and refuses in plain words when none is
allowed, before anything is spent. A provider takes only a sealed brief and checks the seal again
first. The ceilings (`local-only`, `anthropic`, `any-consented`) default to the VO plan's gate
answer Q6: the person's Claude history, the assistant's own folder and the copy of their skills
may go to Anthropic; mail, calendar, contacts, family and legal skills stay on their own
machines. Two choices made at pickup:

- **What the person types into Persona Builder is a source of its own** (`typed`: interview
  answers, notes on answers, a panel's question and steers), defaulting to `anthropic` because
  it is typed into steps that run on Claude Code. The person can keep it home like any other.
- **Ceilings live in member.json** (`"ceilings": {"<source>": "<ceiling>"}`), changed with
  `persona-builder ceilings show|set`; private.json may not hold them, so one file is the only
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
`persona-builder self-hosted set --url … --model …` and kept in the assistant's state folder
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
  runs on every Mac in `scripts/ci-local.sh` (`PERSONA_BUILDER_REQUIRE_SANDBOX=1`), since Apple
  doesn't document the profile language and an update could change it.
- **Not wired in yet.** Nothing runs under the profile until VO11.1, which waits on the owner's
  answer about where Claude Code's login lives inside it (the profile reaches no Keychain).

## Recordings

- Recordings of a real person's voice never enter git. `.gitignore` blocks audio; never weaken it.
