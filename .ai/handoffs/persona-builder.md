# Persona Builder: slice records

One record per slice, newest first. Paraphrase only; no real person.

## PB8.9b1: open an assistant you saved on GitHub (2026-11-13, lane: V1)

- **Why:** one assistant is used on several computers (PB8.9, step 1.2). One computer builds her and saves her
  (paraphrase only) to a private GitHub repo; until now nothing could open that copy anywhere else. PB8.9b was
  split three ways at pickup: this slice (the engine, two app commands, `voice open`), PB8.9b2 (take the newest
  version) and PB8.9b3 (the screens).
- **What:** `voice open` lists the signed-in account's private repos (at most 100, newest pushed first) whose
  description is Voice's or whose name ends `-assistant`, and the assistants in each one's `crew/` on `main`
  (each `member.json` read through the contents API, at most 20 per repo). `voice open <owner>/<repo> <slug>`
  checks the repo is private, fetches `main` (git's object checks on) into a fresh bare `backup.git` inside a 0700
  staging folder under `~/.claude/crew/`, and writes out only `crew/<slug>/`: plain files only, every path part a
  plain ASCII name (no dot-files, so no `.git`), the deny list, 10 MiB a file, 50 MiB and 2,000 files in all,
  never `private.json` by any spelling. The member must name `~/.claude/state/<slug>` as its private folder and
  pass the privacy check and bring-in's own check. Only then is an older assistant backed up (PB8.7b's code, now
  shared by bring-in and open) and the copy moved in with no-replace renames. `save.json` records
  `role: member` and the building computer's name. Permission is asked again. On a member computer Save and all
  three save choices refuse with a sentence naming that computer. Save now adds a `Saved-on: <computer>` trailer,
  which is where the name comes from. App commands `saved_on_github` and `open_saved` (paired with `voice open`);
  `save_options` carries `memberComputer`. No screens.
- **Briefs:** `plans/briefs/PB8.9b1-open-saved.md`, fix-up 1 (Claude's review: a finished open reported as failed,
  a damaged `save.json` blocking a new choice, edge spaces in a computer name, bring-in's member check, unused
  parameters, CLI help) and fix-up 2 (the Verifier's and the security review's findings, below).
- **Who built it:** Codex, from every brief. Her sandbox blocks sockets, so the five Ask tests fail there; V1 ran
  the full gate outside it.
- **Reviews:** security review (sub-agent) on `aeb3074`: no HIGH; MEDIUM, a saved copy could plant a working
  `.git` folder; LOWs: a `Private.json` spelling, an unbounded fetch, invisible characters in a computer name,
  `member.json` keys only the Python kit reads. Verifier (Fable) on `aeb3074`: FAIL, must-fix: on a
  case-insensitive disk a `Private.json` (or `private.jſon`) from the saved copy was read as `private.json`;
  should-fixes: raw control characters in a refusal sentence, a false sentence for a mistyped slug, `.git` folders.
  All fixed in fix-up 2 except the two below.
- **Known limits:** the fetch takes all of `main`'s history (PB8.9b2's fast-forward check needs it; the repo is the
  person's own); `member.json` keys only the Python kit reads (such as `queues`) aren't checked on open. The
  SKILL.md's `voice` command is the building computer's (`"$HOME/Applications/…"` when both install there).
- **Verified so far:** full `ci-local.sh` passed on `aeb3074` (all passed). Codex's targeted suites green on
  `0bdd67f`; the full gate on `0bdd67f` was stopped at the run's time limit (no failure seen before it stopped).
  Repeat Verifier (Opus) on `0bdd67f`: PASS, one should-fix (an empty `~/.claude/crew` left when the open fails
  after staging), which is fix-up 3 (`plans/briefs/PB8.9b1-fixup-3.md`, not yet built).
- **Not landed yet.** Owed: fix-up 3, the full gate on its head, a delta Verifier (Opus), then the land.

## PB8.7e: replace the old one when turning an assistant on (2026-11-13, lane: V1)

- **Why:** an assistant made before Voice left `~/.claude/skills/<slug>/` and a free-text line in
  `~/.claude/CLAUDE.md`. Turning on refused because of the old skill folder (`install::turn_on`), but the
  app hid the reason behind its generic "nothing in your Claude folder was changed". Seen on a tester's
  Mac with 0.1.2. Now PB8.9a, step 1.1 of the PB8.9 plan (one assistant, taught from every computer).
- **What:** `voice_core::replace_old`. `find` is read-only: it finds an unclaimed skill folder and the
  CLAUDE.md lines outside Voice's markers that mention `/<slug>`. `replace` backs both up first
  (`backup::back_up`, byte-verified), then renames the skill aside in its own folder and checks it
  against the backup. It checks that CLAUDE.md's bytes still equal the backup and that each found line
  is still at its number, drops exactly those lines through the install journal (line endings and the
  final newline kept), then deletes the aside folder. Any failure before the rewrite puts everything
  back and removes the new backup (and its parent, when Voice made it). A failure after the rewrite is
  a success that names what's left over. The API's status reports `oldOne` (skill and lines, each cut
  to 200 characters). `turn_on` takes `TurnOnRequest { slug, replaceOld }`, refuses an old skill with
  the real sentence, and on a replace names the backup folder. `voice on --replace-old` does the same.
  Turn on shows the old one, offers *Replace the old one*, and offers *Turn <name> on and keep those
  lines* when only lines were found. Turning off never puts the old one back.
- **Briefs:** `plans/briefs/PB8.7e-replace-old.md`, then fix-ups 1 to 4. Fix-up 1: stale line numbers,
  and the sentence after a failed turn-on. Fix-up 2: the first Verifier's findings, plus a `find` error
  that no longer blocks a plain turn-on. Fix-up 3: the rollback-failed sentence printed a literal
  `<stamp>`. Fix-up 4: after a stamp collision that sentence named the wrong backup folder.
- **Who built it:** Codex, from every brief. Her sandbox blocks sockets, so V1 ran the API and UI
  suites outside it.
- **Verified (this run, on the rebased branch):** voice-core's tests (274 with fix-up 4) and
  `TurnOn.test.ts` (16, its first run anywhere). Mutations, each by `mutate-check.sh`, all killed:
  fix-up 3's sentence back to `<stamp>`, `says` without the stamp check and without its whole-sentence
  comparison; fix-up 4's sentence naming the stamp again, a suffix check that takes any digit, and `says`
  without the name check. Earlier runs' red and mutations are in the briefs' history; the full gate ran
  at the land.
- **Reviews:** Verifier (Fable) on fix-up 1's head: FAIL, three should-fixes, fixed in fix-up 2.
  Repeat Verifier (Opus) on `30aa6eb`: PASS, no must-fix. One should-fix (the backup name after a
  collision), which the security review (sub-task) also found as a LOW, fixed in fix-up 4. Delta
  Verifier (Opus) on `4c88d93`: PASS, no must- or should-fix (nit: the rejection list pins no empty
  name, `..` or `-0`; the code rejects them). Security review: no HIGH or MEDIUM.
- **Known limits, kept:** a save to CLAUDE.md in the microseconds between the byte check and the
  rewrite is lost (closing it needs a lock); after a failed rollback, a retry in the same second says
  "changed", and a later one goes ahead on what now sits at the skill path and leaves the first aside
  folder where it is (it and its backup are the ones the rollback-failed sentence named); `voice on --replace-old` doesn't list the lines
  before it removes them, and the screen lists them as found at status time (lines added since are
  removed too, after being backed up); the `~/.claude/` wording assumes the default Claude folder, as
  `backup.rs`'s sentences do; a chmod on the backup's parent could be sent through a link swapped in by
  a process running as the same person. The backup drops execute bits: filed as VF13 (`pxFzgciN`).
- **Not verified:** a live replace on a tester's Mac (walk owed: an older skill and line, *Replace the
  old one*, the backup folder holds both, turning off leaves them out).

## VF7c: the rest of the daily-limit words follow a subscription sign-in (2026-11-13, lane: V1)

- **Why:** VF7b's out-of-scope list. Three screen sentences still described the daily limit as always
  applying, which is wrong under a Claude subscription in a testers' build.
- **What:** under a subscription, Connect Claude's first paragraph says Voice is signed in with the
  person's Claude subscription and uses their plan; the Build screen says it stops for the day when
  Claude Code says the plan is nearly used up, unless *Keep going* is on; the new-panel note says each
  assistant's turns stop for the day the same way. No dollars. A Console sign-in, a sign-in that isn't
  ready yet, a failed check and every public build keep today's words exactly. Connect Claude uses the
  limit card's own condition (`status.ready && status.subscription`); `BuildRun` emits `subscription`
  from a watch on its spend (`usage` is set exactly under a subscription), and `Build` follows it;
  `NewPanel` calls `claudeCheck()` on its own, so a slow or failed check never holds up the list or the
  form. UI only: no DTO, Rust or generated type changes. Brief: `plans/briefs/VF7c-subscription-words.md`.
- **Left out:** the permission (consent) text. Changing it bumps the consent version, which asks every
  person again, and the card said public builds keep today's words: V-Q20, card VF7d (`XwB1BQNw`).
- **Who built it:** Codex, in two dispatches: the specs first (7 new tests red, each on a missing sentence
  or emit), then the code from a fresh run with the specs frozen (blob hashes unchanged).
- **Verified:** the UI suite (45 files, 619 tests) and its typecheck, run outside Codex's sandbox.
  Mutations, each by `mutate-check.sh` (backup, `cmp` restore): Connect Claude without `ready` and
  without `subscription`; BuildRun always `true` and without its repeat check; Build not listening;
  NewPanel without `ready` and without `subscription`, a failed check that shows an error, and the list
  waiting on the check: all killed. One survived, by design: an emit when spend is empty, a branch the
  screen never reaches (spend starts empty and is never emptied again); the guard is there for the type.
- **Reviews:** security review (sub-task): no findings. `claude_check` starts no sign-in, takes no lock
  and shows no message from the check on the panel. Verifier not required by the spec.
- **Not verified:** a real subscription sign-in in a testers' build (walk owed with VF7b's: Connect
  Claude, Build and a new panel show the subscription words).

## VF7b: no daily limit under a Claude subscription (2026-11-13, lane: V1)

- **Why:** V-Q16's answer. Signed in with a Claude subscription (testers' builds only), Voice stops for
  the day only when Claude Code says the plan is nearly used up (V-Q8's plan guard). The step amount
  stays, because `--max-budget-usd` is the only stop inside a call. A Console sign-in and every public
  build keep the $50 day and $5 step word for word.
- **What:** `Cap::for_sign_in(subscription)` drops the daily part and keeps the step; a saved step of 0
  becomes the $5 default, so a call under a subscription always has a stop (*Keep going* is unchanged and
  still passes none). It is applied with the Claude check's own `Ready::subscription` wherever a session
  is opened for calls: CLI `build`, `interview draft`, `refresh propose` and `apply` (through `cap_for`,
  which now runs after the check, so `--daily-cap-usd` is ignored under a subscription); the app's build,
  interview, both refresh flows and `ask` (through `building::open`); panels (`ClaudeTurns::new`). A place
  that forgot it would keep the daily limit, the safer side. `voice limit show` and `set` print
  `text::LIMIT_SUBSCRIPTION` / `LIMIT_SAVED_SUBSCRIPTION` under a subscription (`set` still saves the
  Console amounts, and drops the state lock before it checks the sign-in). `dto::ClaudeStatus` gains
  `subscription`; Connect Claude's limit card then shows only that sentence, and its dollar fields wait for
  the Claude check so a subscription never sees them for a moment. Briefs:
  `plans/briefs/VF7b-subscription-limit.md` and `VF7b-fixup1-review-findings.md`.
- **Who built it:** Codex, from both briefs. Her sandbox blocked sockets and pnpm, so V1 ran the
  integration, CLI and UI suites outside it.
- **Verified:** red first for the cap and `cap_for` tests (Codex). Outside the sandbox: the 29
  subscription-named tests in voice-agent, voice-api and voice-cli under `--features claude-subscription`,
  `cap_tests` and `explicit_call_cap_above_the_day_is_refused` in the public build, the UI suite (605
  passed after the fix-up; 604 before) and its typecheck. Each new call-site test pairs a `claude.ai` stub
  that runs past a $1.00 day with an `api_key` stub refused with the limit sentence. Mutations, each
  killed on an assertion, files restored and `cmp`-checked: `for_sign_in` returning `self`, zeroing the
  step, keeping a 0 step; `cap_for` ignoring the sign-in; `building::open` ignoring it; `ready_status`
  always false; each of the six call sites (CLI build, refresh propose, refresh apply, interview's
  new-session branch, the app's interview draft and refresh apply) passing `false`; the CLI interview's
  open; Connect Claude's subscription branch, its `ready` condition, its wait for the check and its
  `!signingIn`.
- **Reviews:** security review (sub-task): no HIGH or MEDIUM; three LOWs, two fixed in fix-up 1 (the 0
  step, the lock held during the sign-in check). Verifier (Fable) on `41c3a27`: no must-fix; should-fixes
  fixed in fix-up 1 (`--daily-cap-usd` still clamped the step; six untested call sites), nits fixed
  (literal expected sentences, two unpinned UI conditions). Kept as a known limit (security LOW 1,
  Verifier should-fix 2): the sign-in is read once when a run starts, so a person who signs out and back
  in with a Console account during a running build or panel finishes that run with no daily limit, each
  call still under its step stop. It needs the payer's own action.
- **Repeat Verifier (Opus) on `81edaec`:** PASS, no must- or should-fix. Nits kept as they are: under a
  subscription `--daily-cap-usd` isn't checked at all, so a bad value is ignored silently; build, interview
  draft and refresh apply now find and check Claude Code before they refuse a bad limit pair or a damaged
  `limit.json`, while refresh propose still refuses first (a test pins it), so with Claude Code also
  missing a person sees that error first and the limit error on the next try; `cap_tests` leaves its
  temp folder behind when it fails.
- **Not verified:** panels under a subscription (`voice-panel-runner` has no `claude-subscription`
  feature; checked by reading). A real subscription sign-in in a testers' build (walk owed, attended):
  Connect Claude's card, `voice limit show`, and a build past the saved daily amount.
- **Left:** the other words that still describe the daily limit as always applying (Connect Claude's first
  paragraph, the Build screen, the panels note, the consent text) are VF7c (`wHxQqwcA`, Backlog on VF7b).

## VO16.2b: Voice pins radio-panels v0.1.0 and deletes builder/panels (2026-11-12, lane: V1)

- **Why:** the deliberation protocol moved out of voice into the shared public repo `awna-ai/radio` (VO16.2, the
  Radio's half). Voice now uses that copy instead of keeping its own.
- **What:** `builder/api` and `builder/panel-runner` depend on
  `radio-panels = { git = "https://github.com/awna-ai/radio", tag = "v0.1.0" }`; `Cargo.lock` pins commit
  `03ee956`. `builder/panels` leaves the workspace and is deleted (its 66 reference fixtures and tests live in the
  Radio repo). The moved crate's one code change is `Dissent.crewmate` → `member`: a closed panel's `result.json`
  now saves `member`, one saved before still reads through a private `SavedDissent` with a `crewmate` alias (never
  rewritten), and the api reads `member`. `Cargo.lock`: serde_json 1.0.154 → 1.0.151 (radio-panels pins it exactly,
  same features); the re-resolve also flipped `cssparser-macros`' syn pick (it accepts `>=2,<4`), restored by hand
  to `syn 3.0.9` so nothing else moves (`cargo metadata --locked` accepts it). Briefs:
  `plans/briefs/VO16.2b-pin-radio-panels.md`, `-fixup-1.md`.
- **Who wrote it:** Codex, from the brief (she stopped correctly on the syn flip; resumed after the fix) and one
  fix-up; Claude reviewed it.
- **Dependency intake:** `awna-ai/radio` is public, MIT, our own org; the tag resolves to the locked commit locally
  and on GitHub; no `build.rs`, proc-macro or `.cargo` config in it; no package new to voice's lock; its source
  diff against voice's old `builder/panels` is the one rename plus two generator comments (security review and
  Verifier each checked with `diff`). serde_json 1.0.152 to 1.0.154 carry no security fix (release notes and a
  source diff: a `-0` fix under `arbitrary_precision`, which voice doesn't enable, `as_f32` finiteness, the
  position reported for invalid UTF-8, a no_std hasher).
- **Security review (Opus):** no HIGH or MEDIUM. LOW, recorded, not fixed here: radio-panels' exact `=` pins mean
  voice can't move serde, serde_json or regex without a new Radio tag (a Radio-side choice); the tag is unsigned,
  so a later `cargo update` should check the commit it moves to (`--locked` builds can't follow a moved tag); a
  saved dissent entry still accepts unknown keys, as before (dropped on re-serialize).
- **Verifier (Fable, `a215985`, now `6265f12`): PASS**, one should-fix: no test checked the name a real close saves (blanking
  `member` in the `From` impl survived). Fix-up 1 (`2887aa8`, now `1498193`) asserts it in the 15-minute time-box test; the
  mutation now fails. Nits left: a `result.json` written now can't be read by an older Voice; the reported column
  of an invalid-UTF-8 error in a growth record can differ by serde_json version; no `.ai/DECISIONS.md` entry for the
  first git dependency. Repeat Verifier (Opus) PASS on `3677588` (fix-up 1 and the record).
- **Rebased (run `193704-r1`):** the first land passed `ci-local.sh` but lost the push race to PB8.8, and the
  rebase conflicted only in `Cargo.lock` (PB8.8's 0.1.1 → 0.1.2 bump next to the deleted `voice-panels` entry);
  resolved by keeping the deletion, `cargo metadata --locked` accepts it. `git range-diff`: every commit `=` except
  the build commit, whose change is those version lines. Nothing on `main` names `voice_panels` or `builder/panels`.
  Repeat Verifier (Opus) PASS on `019e06c`: range-diff and lock checked, `cargo test -p voice-panel-runner
  -p voice-api` 538 passed.
- **Verified:** red first (the new-key api test failed on unchanged code; the old-key api test and the reopen test
  failed after the switch, before the alias); mutations killed: the alias removed, the api reading `crewmate`, the
  `From` impl blanking `member`; Verifier's scratch probe: a real time-box close with dissent, its key renamed to
  `crewmate`, read by status, list and detail and resumed, file unchanged; `cargo test -p voice-panel-runner
  -p voice-api` 533 passed; workspace clippy clean; `bash scripts/ci-local.sh` at the land.
- **Not verified:** a clean-machine build fetching the git dependency with an empty cargo cache (the cache here was
  warm); radio-panels' own tests (they run in the Radio repo).

## PB8.7c: choose which of the older assistant's own files come in (2026-11-12, lane: V1)

- **Why:** PB8.7b brought an older assistant's history in, but its own files (identity, autonomy, interview,
  lens, decisions, measurements, briefs, private settings) stayed behind and the new folder was all template.
- **What:** `voice_core::old_files` offers the older folder's `IDENTITY.md`, `autonomy.md`, `interview.md`,
  `lens.md`, `decisions.md`, `measurements.md`, `briefs` and `private.json`, each with a plain reason when it
  can't come in (a link, not a plain file, shared with another file by a hard link, unreadable, too big, not
  plain text, an autonomy list Voice can't read, private settings that change more than privacy or wouldn't
  work). `bring_in` takes the ticked names, refuses an unoffered one before the backup, then copies the
  checked bytes from the backup's copy over the template through folder handles (`openat`, no link
  followed), 0600, and reads the folder back (`read_member`, the filters compiled, the anonymizer rules
  parsed); any failure undoes the whole bring-in. `SKILL.md` and `member.json` stay Voice's. `voice new
  --bring-in --leave <file>`, `voice existing` and the tick boxes on *Make a new assistant* share it. Briefs:
  `plans/briefs/PB8.7c-choose-old-files.md`, `-fixup-1.md`, `-fixup-2.md`, `-fixup-3.md`.
- **Decisions at pickup** (the spec leaves them open; each takes the safe side): `decisions.md` capped at
  256 KiB and `briefs/codebook.md` at 64 KiB, the sizes Voice reads; `private.json` may hold only `filters` and
  `anonymizer`, since `read_member` lets any other key replace a `member.json` field (name, private folder);
  `briefs` is one level of plain files, at most 64, `.DS_Store` skipped.
- **Who wrote it:** Codex, from the brief and two fix-ups; Claude reviewed it. Fix-up 1 (security review LOW
  1–4, Verifier 1–6): a broken filter, an empty filter (it would mark all text secret) or a bad anonymizer
  setting came in and broke every later run; hard-linked and unreadable files; `.DS_Store`; codebook case;
  writes by path; the words said "untick" in the CLI; unpinned link guards and cap boundaries. Fix-up 2 (my
  review): fix-up 1 had made the core test helpers part of the release library.
- **Verified so far:** red first per Codex's runs (24 core, 6 API, 6 CLI in the build; fix-up 1's privacy, file
  and words tests). Screen tests: 3 fail against `main`'s screen, pass with it (83 green, typecheck clean).
  My mutations: dropping the 0600 `set_permissions` and dropping the consent restore each fail a test.
  Security review (Opus, `20018ca`): no HIGH or MEDIUM, four LOW, fixed in fix-up 1 (LOW 4b, a failed clean-up
  leaving a half-made folder, is PB8.7b's existing behaviour). Verifier (Fable, `20018ca`): claims 1, 2, 4, 5
  verified, 3 and 6 partly; 8 mutations killed, 9 survived (pinned in fix-up 1).
- **The delta passes and fix-up 3** (run `125732-r1`): delta security review (Opus, `20018ca..8b2fb60`): no HIGH
  or MEDIUM, three LOW: `private.json` filters Voice never reads were compiled (a large file of them stalled the
  offer), a filter matching empty text (`(?:)`, `|`, `a*`) still came in and marked all text secret, and a hard
  link planted in the new folder was written through. Delta Verifier (Opus, same range): PASS on reading (the
  machine's load gate held, so no mutation ran), one should-fix: a `briefs` folder Voice can list but not
  search made the offer fail instead of leaving `briefs` out. Fix-up 3 (`plans/briefs/PB8.7c-fixup-3.md`,
  Codex, `1a2335e`) fixes all four: only the three filters Voice compiles are compiled; `check_pattern`
  refuses a pattern matching empty text; each target is unlinked and created fresh (`O_EXCL`), so a planted
  link of either kind is replaced, never written through (two fix-up 1 tests now say "replaces"); entry and
  metadata errors in `briefs` are the `READ` problem. Red first per Codex. My mutations on fix-up 3: no
  `unlinkat` fails 4 tests, no empty-match check fails 5. The delta Verifier's six unrun mutations: I ran
  five on `1a2335e` (no link-count check, no `.DS_Store` skip, a case-sensitive codebook cap, unreadable as
  `FILE`, no `O_NOFOLLOW` on folders), each failed its test; the sixth (no consent restore) is the one
  killed above. Repeat Verifier (Opus, `0178ed3..1a2335e`): PASS on reading (the gate held again), two nits
  left: `.` as a filter still matches nearly everything (fails closed, toward hiding more), and no test puts
  a folder at a target name (it errors into the undo, by reading).
- **Gate:** `bash scripts/ci-local.sh` all passed on `1a2335e` (2 workers, the load gate at throttle).
- **Known limits:** the after-copy check is a second line of defence the offer rules already cover, tested
  through a private seam. The offer's link-count check and the read that follows it are two path lookups, and
  the after-copy read-back reads by path: a same-user race only. Light and dark not looked at (owed to an
  attended pass).

## PB8.7b: back an older assistant up, then bring its history in (2026-11-12, lane: V1)

- **Why:** PB8.7a made Voice refuse a name an older assistant already used. This slice offers the way
  through: back the older one up, then make the Voice assistant over its kept history, so *Put it together*
  merges the old `decisions.jsonl` instead of replacing it.
- **What:** `voice_core::backup::back_up` copies the older assistant's private folder, its skill, `CLAUDE.md`
  and its old member folder into `~/.claude/voice-backups/<slug>/<stamp>/` (0700 folders, 0600 files). It
  never follows a link except a linked `CLAUDE.md` itself, refuses a FIFO or device without blocking, checks
  every byte and link target of the copy, and removes the copy on any failure.
  `voice_api::assistants::bring_in` refuses first (a linked or file private folder, a
  `decisions-before-voice.jsonl` already there, a `decisions.jsonl` that is a link or folder), holds the state
  lock, backs up, renames `decisions.jsonl` to `decisions-before-voice.jsonl` (hard link then remove, never
  overwriting), makes the assistant and clears consent, and undoes all of it on failure. `voice new
  --bring-in`, the `bring_in_assistant` command and *Back it up and bring it in* on *Make a new assistant*
  share it; `voice existing` and the screen say whether an older one can be brought in. The older skill,
  `CLAUDE.md` and old folder are never changed here (PB8.7e takes them out). Briefs:
  `plans/briefs/PB8.7b-back-up-and-bring-in.md`, `-fixup-1.md`, `-fixup-2.md`.
- **Who wrote it:** Codex, from the brief and two fix-ups; Claude reviewed it. Fix-up 1 (review): `voice
  existing` said "can't" twice; `local_stamp` copied `local_day` (it now sits beside it in
  `voice_agent::ledger`, sharing one `localtime_r`), the bring-in code sat below a test module, and the two
  make paths repeated their request checks. Fix-up 2 (security review LOW 1, LOW 2 and a nit; Verifier A, C,
  D, E, F): the screen showed the "check there's space" sentence for every backup refusal (now the backup's
  own sentence); a linked or folder `decisions.jsonl` was called "a history from before Voice"; an old skill
  naming `~/.claude` as its folder made the backup copy itself; the new backup folder was briefly 0755 and
  existing parents were never tightened (now 0700 from the start, owner checked); two 64 KiB buffers per
  folder level overflowed the app's 2 MiB thread at about 15 levels (now one heap pair; the 60-level red test
  aborted before); the backup is kept when the old history can't be put back; the Make button read "Making
  it…" while backing up. Fix-up 3 (repeat Verifier, claim 3): a source inside `voice-backups` (an old
  `SKILL.md` naming `~/.claude/voice-backups/<slug>/member.json`) still copied itself; the check now refuses
  either direction, and `<stamp>` is set 0700 again whatever the umask.
- **Verified:** red first for every new test (Codex's runs: six core tests against a stub, ten API tests, three
  CLI, six screen, the lockdown list; fix-up 2's reds included the deep-folder abort). Green at 2 workers:
  core backup 12, API bring-in 11, CLI assistants 12, screen 12, app lockdown 8, fmt and clippy clean.
  Security review (Opus, `079b8c0`): no HIGH or MEDIUM; LOW 1, LOW 2 and the nit fixed in fix-up 2.
  Verifier (Fable, `079b8c0`): no data-loss path; the hazard verified (skipping the rename leaves 2 of 4
  questions after a merge); 6 of 9 mutations caught, the 3 survivors (open flags, verify pass, linked
  `<slug>`) now have tests or are noted below; races judged accepted limits for a single-user app.
  Repeat Verifier (Opus, `35c7b02`): 7 of 8 verified, claim 3 refuted in part (fixed in fix-up 3, red then
  13 core backup tests green); her mutation putting the buffers back on the stack aborted the deep-folder
  test (restored, `cmp` clean). Probes on APFS: other letter case and a linked `~/.claude` are refused.
- **Known limits:** `decisions-before-voice.jsonl` is a History input (the plan's PB8.7 decision), and merge
  lets History win a duplicate id (tested in `corpus_rules.rs`, mirrored in the Python merge). So an
  interview record an older merged `decisions.jsonl` also holds is re-split rather than always trained. No
  record is lost; changing that rule is out of this slice. The `O_NOFOLLOW` and `O_NONBLOCK` open flags only
  matter in a race and have no test. The state lock, the refusal of a backup folder another user owns, and
  keeping the backup when `decisions.jsonl` can't be put back are read, not tested. Light and dark screens were not
  looked at (owed to an attended pass).

## PB8.7a: look before making, never reuse an older assistant's folders silently (2026-11-12, lane: V1)

- **Why:** making an assistant whose name matched one built before Voice silently reused its private
  folder (`~/.claude/state/<slug>/`); a later *Put it together* would replace a `decisions.jsonl` Voice
  never wrote, which may be that assistant's only full history. PB8.7 was split at pickup into PB8.7a to
  PB8.7e (the family's decisions are in lane V's spec, § PB8.7); this is the first.
- **What:** `voice_core::existing::find` reports an older assistant of a slug: its private folder holding a
  file (names and a count only), a skill Voice's record doesn't claim, `CLAUDE.md` lines naming `/<slug>`
  outside Voice's markers, and the member folder its old `SKILL.md` names. A name whose Voice crew folder
  exists has nothing older. `new_assistant` and `voice new` refuse while anything is found, making and
  changing nothing; `find_existing` and `voice existing` report it, and *Make a new assistant* checks first
  and lists it in plain words. Briefs: `plans/briefs/PB8.7a-look-before-making.md`, `-fixup-1.md` to
  `-fixup-4.md`.
- **Who wrote it:** Codex, from the brief and four fix-ups; Claude reviewed it. Fix-up 1 (review): Codex had
  added refusals of a linked Claude, `state` or `skills` folder, which turning on supports, so any such
  person could make no assistant; a Voice assistant's own state read as "older"; `CLAUDE.md` was read
  whole before its cap. Fix-up 2 (security review MEDIUM and two LOW, Verifier should-fixes): one capped,
  non-blocking, fstat-checked open per file read (a FIFO or huge `member.json` named by a planted
  `SKILL.md` could hang the check), the old folder never under `~/.claude/state` (a planted `--member`
  there made the finder open a file in the private folder), its `private.json` never opened, an unclosed
  marker hides nothing, a finder error shows the look-before sentence.
  Fix-up 3 (repeat Verifier, Opus, on `36a631e`: 5 of 7, two refuted): the private-folder guard compared
  spelled-out paths, so other letter case (APFS ignores it) or a linked `~/.claude` still reached a
  `member.json` inside it; it now compares device and inode up the candidate's ancestors. A Voice block is
  a begin and the end with its own id.
  Fix-up 4 (repeat Verifier, Opus, on `0ba9a77`: 5 of 5, two should-fixes): pairing each begin with its
  end searched every later line, so a `CLAUDE.md` of unclosed begins near the 4 MiB cap took over ten
  minutes; one pass now maps each end id to its lines. A test for a linked `~/.claude/state`.
- **Verified:** red first for each test (Codex's runs); touched crates' tests 884 passed and screens 579
  passed at 2 workers after fix-up 2; fmt and clippy clean. Mutations caught and restored (`cmp`):
  the screen's found branch and its name-change reset. Security review (Opus): no HIGH; MEDIUM and two
  LOW fixed in fix-up 2. Verifier (Fable, `6c7e738`): 7 of 8 verified, claim 2 refuted in one corner (fixed
  in fix-up 2), probes on scratch homes showed a refused `voice new` leaves the tree byte-identical.
  After fix-up 3: touched crates 886 passed, screens 579 (Codex's run, 2 workers).
  Repeat Verifier (Opus, `0ba9a77`, run `073815-r1`): all 5 claims verified, including probes on APFS that
  letter case, a linked `~/.claude` or `~/.claude/state`, the `/System/Volumes/Data` spelling and `..` all
  reach the private folder's device and inode. Fix-up 4's new test was red first (617 s against its
  10-second bound), then 0.05 s; `voice-core` 205 passed, fmt and clippy clean (Codex, 2 workers).
  Rust mutations caught and restored (`cmp`), each by a named test: the inode guard off, the block skip
  off, an end of any id closing a block, the next end replaced by the id's first, `O_NOFOLLOW` off, the
  plain-file check off, the install record never claiming, a linked state folder not counted, the
  refusal in `voice new` off, the refusal in `new_assistant` off. One survived and is equivalent: skipping
  an unreadable ancestor in the inode walk, since the component walk after it refuses the same folder.
- **Not verified:** a light and dark look at the warning (owed under `## Walks owed`); the old guided
  build's `SKILL.md` on a real machine (the `--member` rule follows `bootstrap/PROMPT.md`'s template); a
  FUSE or bind mirror of the private folder (another device number); a folder swapped between the check
  and the open (a local attacker with write access, as before this slice). A line starting with a Voice
  marker is skipped even outside a block, as before.
- **Known limit (security review, Opus, `6c7e738..d98bc77`: no HIGH or MEDIUM, this LOW kept):** `CLAUDE.md`
  is read through a link, as Claude Code reads it (it is often a dotfiles link), so a `CLAUDE.md` planted
  as a link into the private folder is read, capped, and only the numbers of lines naming `/<slug>` come
  out; nothing of it reaches a screen. Delta Verifier (Opus, `d98bc77`): 4 of 5 verified, the run figures
  left to the land's gate; no findings.
- **Left for PB8.7b:** bringing an older assistant in after a backup. Until then a found name is refused,
  including a Voice assistant whose crew folder was deleted by hand while its state stayed.

## PB8.6: the build shows how far it got and how much is left (2026-11-12, lane: V1)

- **What:** someone watching a real build kept clicking *Run the build again* with no idea how
  many runs it would take. (1) The build counts its paid steps and sends them with each line
  (`build::Progress { line, done, total }`): label parts, distil groups, test batches. The distil share
  is the settings' group count until sorting ends, then the groups that have decisions, so the total
  never grows. `BuildProgress` carries them as `steps` (`None` for a refresh's test). The Build screen
  draws a real bar with "3 of 25 steps done, 22 left."; after a stop or a failure the bar stays with
  "Stopped after 3 of 25 steps, 22 left."; a finished build clears it. (2) A misfit counts how many
  builds that part has stopped (its second tries' rejected replies, `rejected-<stem>-<digits>-2.json`
  in `build/`, at least 1). From the second time the sentence says running the build again is unlikely
  to help, instead of inviting another run. A rejected reply takes the next free second in its name, so
  two failures in one second are both kept and counted. The first failure keeps PB8.5's sentence; grow's
  sentences are unchanged. Briefs: `plans/briefs/PB8.6-build-progress.md`,
  `plans/briefs/PB8.6-fixup1-empty-group-and-style.md`.
- **Who built it:** Codex, from both briefs. Her sandbox can't bind loopback sockets or reach the
  npm registry, so V1 ran every stub test, the screen checks and the mutations on Mimi. Fix-up 1 was
  V1's review: no test covered the total dropping (the stub fills both groups in every existing
  corpus), plus style.
- **Verified (Mimi, load1 200 to 350%):** `cargo test --locked -p voice-build -p voice-api -p voice-grow
  -p voice-cli` with `VOICE_REQUIRE_PARITY=1`: 69 suites ok, the 7 new or changed tests among them.
  `pnpm -C builder/ui typecheck` clean; vitest 45 files, 573 tests passed (before fix-up 2, whose test
  Codex ran: 17 in the file, failing with the guard removed; the land's gate runs them all).
  Mutations (`cp`, restored, `cmp`), each one putting back the old behaviour for one claim, all caught: the count ignored (old
  sentence every time); no free name; any middle counted; distil steps not counted; the total not
  recomputed; `is_misfit` refusing the new ending (the app shows `BUILD_FAILED` again); the checking line
  without steps; the bar only while running; steps not reset between runs; the stopped wording.
- **Reviews:** security review (Opus sub-task): no HIGH or MEDIUM. Two LOW, not taken: the free
  name is check-then-rename, so two builds of one assistant in the same second could lose a rejected
  reply (the build lock already allows one build per assistant); `stopped_times` lists `build/` by path,
  so the person swapping their own folder for a link could change only the count. Verifier (Fable, on
  `e00ff23`, run because the diff adds over 400 lines; the spec said not required): all 7 claims
  verified, each by a mutation or probe; tree left clean. Should-fixes: the `if (progress.steps)` guard
  was untested, fixed in fix-up 2 (a test-only brief, `plans/briefs/PB8.6-fixup2-steps-guard-test.md`;
  the test fails with the guard removed). Not taken: `is_misfit` checks only the opening and ending, as
  on `main`, so its safety still rests on `text::misfit` being the only producer; after a failure while
  sorting, "N left" can overstate by the number of empty groups, since the total is still the upper
  bound (it is exact from the first distil line on, and a whole group with no decisions is rare). Her
  probe also showed a changed corpus starts again at the first-time sentence.
- **Not verified:** the screen in light and dark (walk owed); a real build on a real history (the next
  tester build). Rejected replies are never cleared, so the count is per identical part for the
  assistant's life; a changed part gets a new cache name and starts again at one.

## PB8.5: a label answer that doesn't fit no longer stops the build silently (2026-11-12, lane: V1)

- **What:** a real build (11 label parts) kept part 1 only on its 8th call, and part 2 failed both tries
  of the next run, behind the generic "The build stopped before it finished" line. Three faults, all
  fixed. (1) A label or backtest retry was checked alone. The model answers "didn't fit" with only its
  corrections, so every record it left out counted as missing. Now `Asker::ask_by_id` merges the retry
  over the reply that didn't fit, by id: from the earlier reply it keeps only ids that were asked for
  and not sent again, then checks the merged reply whole. The retry is told every missing record id and
  "you may give only the entries for the ids named above". An id the model made up is only ever counted,
  never written into the retry. Distil and `builder/grow` keep their full-reply retry. (2) Each rejected
  reply is kept with its problem, 0600, in the state folder's `build/` as
  `rejected-<cache name>-<secs>-<try>.json`, and is never reused. (3) The app shows the build's own
  misfit sentence: the part, and counts such as "1 record had no label, 2 labels didn't fit the
  codebook". `text::is_misfit` recognises the sentence by its fixed text; the daily limit still wins
  first. Briefs: `plans/briefs/PB8.5-label-misfit-reason.md`,
  `plans/briefs/PB8.5-fixup1-review-should-fixes.md`.
- **Who built it:** Codex, from both briefs. Her sandbox can't bind loopback sockets, so every
  stub-driven test failed there before any call. V1 ran the reds, greens and mutations on Mimi. V1 also
  wrote the `.ai/SECURITY.md` build-row update. One V1 error, caught by the Verifier and fixed in
  fix-up 1: a `sed` meant for the new tests also turned three existing mined-mail tests from
  `python().expect` into a silent skip.
- **Verified:** red as found, with the original source under the new tests. The partial-retry tests
  stop with "didn't fit twice". In the app, the misfit build showed `BUILD_FAILED` instead of the
  reason. Green: `voice-build`, `voice-api`, `voice-cli` and `voice-grow` all pass. Mutations (`cp`,
  restored, `cmp`): merge returning the reply unchanged fails the 3 partial-retry tests; `is_misfit`
  always false fails `a_misfit_build_shows_its_reason`; removing the rejected write fails
  `two_misfits_keep_both_rejected_replies`; the plain retry text swapped for the partial one fails
  `a_distil_retry_keeps_the_full_reply_instruction`. Full gate: `bash scripts/ci-local.sh` passed on
  `3b290d6` and on `9314bce`, both run with `RUSTC_WRAPPER=` because `sccache` couldn't start
  inside the job's background shell (`Operation not permitted`). The first run on `3b290d6` died on
  that, after clippy had passed.
- **Reviews:** security review (Opus sub-task) found no HIGH or MEDIUM. Of its LOWs, the
  `ids differ:` prefix filter and the stale SECURITY.md line were fixed in fix-up 1. Not taken:
  rejected replies are never swept while the assistant exists. Each one is a paid call's reply, they
  are kept on purpose for walks, and forgetting the assistant removes them. Verifier (Fable, on
  `3b290d6`): all six claims verified; should-fixes (two unpinned behaviours, distil's plural and
  "their" wording, the repeated-id refusal, the stray `sed`) fixed in fix-up 1. Repeat Verifier
  (Opus, on `9314bce`): all seven claims verified, one should-fix: a record id that repeats inside a part is listed missing twice and `label_problems`' count line then reads as a wrong label (no model words leak; only a corpus with repeated ids). Fix-up 2 brief written (`plans/briefs/PB8.5-fixup2-repeated-record-ids.md`).
- **Fix-up 2 (`2a2ceac`, Codex from the brief):** `check_label_reply` and `check_backtest_reply` list
  each missing id once, in first-appearance order, as `ById` does, so the count line matches
  `label_problems`' and is filtered out. Two new tests pin the exact told and shown text. V1 on Mimi:
  green; mutations (`cp`, restored, `cmp`), each dedupe made a no-op in turn, fail only their own
  test, the label one with the Verifier's exact probe text. Security review (Opus sub-task): no
  findings (`missing` is built from the app's own records only). Delta Verifier (Opus, `2a2ceac`):
  PASS, no must-fix or should-fix. Two nits on older behaviour, not taken here: the backtest prompt
  shows a repeated question twice, so answering both copies reads as "predicted twice"; a record
  with an id but no `kind` counts its label as extra, then `label_problems` stops the build with
  "Voice couldn't check its labels". Whether a real corpus can
  produce either was not checked.
- **Not verified:** the cause on the real history. The replies from that build were not kept (that is
  this slice's fix), so the merge answers the lead (retries that hold only corrections) but not
  whatever made the first tries fail. A walk after the next tester build should read the
  `rejected-label-*` files. Also untested: a run that fails twice still starts from scratch on the
  next run. Carrying a rejected reply over into the next run's first try is an option for PB8.6 or
  later.

## VF12: the over-long mail test measures processor time (2026-11-11, lane: V1)

- **What:** `tests/crew_extra_sources_test.py`'s `test_an_over_long_mail_is_skipped_before_the_filters_run`
  gave a whole `crew sources mine` run 5 s on the clock. At load 21 to 32 on an 8-core Mac it took
  7.0 s inside `ci-local.sh` on `lane-v1/VF9` and 8.5 s alone, so VF9's gate could not go green. Now it
  bounds the processor time the mining run used (`resource.getrusage(RUSAGE_CHILDREN)`, user plus
  system) at 3 s. A slow pattern costs processor time; a busy computer costs only clock time. Test
  only. Brief: `plans/briefs/VF12-over-long-mail-processor-time.md`.
- **Who built it:** Codex, from the brief, then a one-line fix-up from V1's review (the bound from 5 s
  to 3 s).
- **Verified:** red as found: the old test failed alone at 8.5 s (load about 280 to 390% of cores).
  Green: the new one uses about 0.5 s of processor time. Mutation (`cp`, restored, `cmp`): with the
  `MAX_MESSAGE_CHARS` skip removed, the run used 4.4 to 4.5 s of processor time (49 s on the clock),
  so at a 5 s bound only the `1 too long` check caught it; at 3 s the processor bound fails first
  (`4.4 not less than 3`). The whole file passed (34 tests).
- **Reviews:** security review (sub-task, on VF9 and VF12 together): no findings; `self.mine` waits for
  the CLI, so `RUSAGE_CHILDREN` counts it, and a full hang still hits its 60 s timeout. Verifier: not
  required (spec says no; test only).

## VF9: the sign-in stop test waits for its files (2026-11-11, lane: V1)

- **What:** `voice-agent`'s `stopped_sign_in_kills_the_process_group_within_a_second` set the stop
  after a fixed 200 ms, then read the `parent` and `pid` files its stand-in writes when it starts. On a
  busy computer the stop could come first, so a file never existed and `Stub::file` panicked with
  `NotFound`. Now the stand-in writes `pid` through a temporary file and `mv`, the stop waits for `pid`
  (polled every 10 ms, at most 10 s), and the one-second bound is timed from the stop, not the start.
  Test only. Brief: `plans/briefs/VF9-sign-in-stop-test.md`.
- **Who built it:** Codex, from the brief. Her sandbox can't open the loopback connection the sign-in
  uses, so none of her test runs reached the test; V1 ran every proof below outside it.
- **Verified:** (run `045310-r1`, load about 160 to 280% of cores) the cause is confirmed: with the
  stand-in slowed by `sleep 0.5`, the old test fails with the same `NotFound` at
  `tests/common/mod.rs:96`, and the new one passes. Mutation (`cp`, restored, `cmp`): `kill_group`
  killing only the parent fails the new test with `pid <n> survived`. 10 of 10 runs in a row passed.
  fmt and clippy clean.
- **Reviews:** security review (sub-task): no findings; it traced that `SIGN_IN_STOPPED` comes only
  after the stop is set, so the bound can't pass by saturating to zero. Verifier: not required (spec
  says no; test only).

## G3.2a: draft practice questions on kinds you name (2026-11-10, lane: V1)

- **What:** `voice interview draft` chose its own kinds of decision: every kind for a full interview,
  or the thin kinds an earlier build's report or labels show for a short one. A helper that already
  knows where an assistant is weak (for example, the kinds its calibration table shows one miss short
  of live) couldn't ask about those kinds, and couldn't keep that round apart from the person's own
  interview. `voice interview draft <MEMBER_DIR> --kind <KIND> [--kind …] [--per-kind <N>]` now drafts
  a practice round: 1 to 5 distinct kinds from the codebook (never `other`), N questions on each (1 to
  4, default 4). It is a new `Form::Practice`, with its own file `interview-practice-questions.json` and
  ledger step `interview-practice`, whatever the corpus holds. The kinds are checked before any lock,
  ledger line or call. `--replace` touches only the practice file. It prints a `voice keep` next step,
  because a practice round's answers are kept one at a time as growth signals, not through
  `interview answer`, which never reaches the practice form. Without `--kind` nothing changes. This is
  what G3.2b's weekly runner calls for a member that has no Voice build report.
  Brief: `plans/briefs/G3.2a-practice-kinds.md`.
- **Who built it:** Codex, in two runs: the tests first (dispatch A), then the code by a fresh run
  that couldn't edit them (dispatch B; the spec's blob hash was unchanged). Her sandbox blocks the CLI
  fixture's sockets, so V1 ran the suites outside it.
- **Verified:** red first: the 5 new CLI tests failed on clap's unknown `--kind`, and the 12 existing
  ones passed. Green: 17 passed. Eight mutations, each one restored byte for byte by `mutate-check.sh`
  after a green baseline, and every one failed the tests (exit 101): `other` accepted, a repeated
  kind accepted, a sixth kind accepted, the practice file swapped for the short one, the ledger step
  swapped, kind coverage not checked for practice, the per-kind count ignored, and the prompt's
  practice wording swapped. (The first pass of those checks exited 127 because zsh doesn't split an
  unquoted command variable, so nothing had run; it was redone with the command spelled out.)
- **Reviews:** security review (sub-task): no findings. A kind can't reach a path, prompts get only
  codebook kinds, errors print kinds escaped, refusals spend and write nothing, and the caps, lock and
  guard apply as for the other forms. Verifier (Fable; the spec says no, run because the diff with
  its brief and record passes 400 lines): PASS on `64ddf46`, no must-fix. Four of five should-fixes
  done in fix-up 1 (`plans/briefs/G3.2a-fixup-1.md`, Codex): one kinds-list helper, `--per-kind`'s
  default shown once, a test that a refused kind prints escaped (its mutation failed the tests), and a
  test that the validity check runs before the repeat check. Kept as it is: nothing shows that a
  waiting full interview doesn't block a practice round (only the short one is tested), since an
  implementation that checks the wrong file is unlikely.
- **Gate:** `bash scripts/ci-local.sh` on `64ddf46` failed only `subscription_refresh_propose_closes_with_usage`
  (`--features claude-subscription`), because the Verifier ran a feature-less `cargo test -p voice-cli` in the
  same worktree mid-gate and rebuilt the shared binary; alone it passed 3 of 3. The re-run on `3fe09a5`
  was still running when run `051137-r1` hit its time limit; the next run reruns it before the PR.
  (Run `194044-r1`) gated with VF10 and VF11 as one stacked head, `144a4f6`: `ci-local.sh` all
  passed (VF11's record has the details).
- **Not verified:** a live practice draft through a real Claude Code. G3.2b's runner, and G3.1c's
  attended walk after it, are the first live callers.

## VF11: the build stop test gives its stand-in 30 seconds to start (2026-11-10, lane: V1)

- **What:** `voice-api`'s `stop_second_call_is_busy_then_reuses_first_kept_reply` stopped one
  `ci-local.sh` run on `lane-v1/VF10` (load 20 to 25) and failed 1 of 3 runs alone at load 20:
  `wait_for` gave the stub 5 s to write `stdin1` (`stub never reached …`), and a `SendError` panic
  followed from the dropped receiver. `wait_for` only proves the stub started, so its bound in
  `builder/api/tests/building.rs` is now 30 s (the stub call that writes `stdin1` then sleeps 30 s, so
  the stop still lands while it runs). Every behaviour bound is unchanged (the stopped build answers
  within 10 s). A new test, `wait_for_allows_a_slow_stub_to_start`, writes the file after 6 s. Test
  only. Brief: `plans/briefs/VF11-build-stop-stub-start.md`. Likely cause, not confirmed: a loaded Mac
  takes over 5 s to start the build and the Python stub.
- **Who built it:** Codex, from the brief; her run hit its 30-minute limit after green, before her
  report. Her sandbox blocks the fixtures' private connection, so 21 `building` tests failed there
  with that message.
- **Verified:** red first on its assertion: the new test failed with `stub never reached …/started`
  (6.1 s), then passed with the 30 s bound (Codex's log). (Run `194044-r1`) one mutation caught:
  the bound put back to 5 s fails the new test (`stub never reached …/started`), restored byte for
  byte (`cmp`).
- **Reviews:** security review (sub-task): no findings. `wait_for` is called only to prove the stub
  started; the stop is sent after it returns and timed on its own (10 s), so the longer bound can
  hide only a slow start. The new test writes only inside its own temporary home. Verifier: not
  required (test only).
- **Gate:** VF10, VF11 and G3.2a gated as one stacked head, `144a4f6` on `lane-v1/stack-1110`:
  `bash scripts/ci-local.sh` all passed (4 test threads, load about 100% of effective cores), the
  whole `building` suite outside the sandbox included. They land as one PR so `main`'s tree is the
  gated one.

## VF10: the stdin refusal test accepts a command that stops reading early (2026-11-10, lane: V1)

- **What:** `voice-cli`'s `invalid_stdin_is_a_usage_error_before_searching` failed now and then on a
  busy computer with a `BrokenPipe`: the test's `run` helper writes all of its input before waiting,
  and a command that refuses its arguments can exit before reading stdin, which closes the pipe. The
  likely trigger is the test's last loop (clap refuses the arguments first), not the 64 KiB case the
  spec guessed. `run` in `builder/cli/tests/similar_stdin.rs` now ignores a `BrokenPipe` on that write
  and still asserts on the exit status and stderr; any other write error still fails. A new unix test,
  `run_accepts_a_command_that_exits_without_reading_stdin`, sends 1 MiB to `sh -c 'exit 3'` and
  checks exit 3. `record.rs`'s tests already handle the same case this way. Test only. Brief:
  `plans/briefs/VF10-stdin-broken-pipe.md`.
- **Who built it:** Codex, from the brief.
- **Verified:** (run `095419-r1`) red first: the new test failed with the broken pipe; green 4 of 4,
  fmt and clippy clean; one mutation caught (letting a 64 KiB + 1 input through fails the refusal
  test).
- **Reviews:** security review (sub-task): no findings; every caller of `run` still asserts the exit
  status (and stderr where it did before), and only `BrokenPipe` is ignored. Verifier: not required
  (spec says no; test only).
- **Not verified:** the cause is likely, not confirmed: the flake never reproduced alone (5 of 5 passed).

## G3.1a: an approval or a rejection keeps the approver's words (2026-11-10, lane: V1)

- **What:** the growth loop spec lets an approval carry a `note`, and the record had the field, but
  nothing could set it: every answer was saved with no note. `voice refresh approve|reject` now take
  `--note <NAME or FOLDER/NAME>`, a `{"note": "..."}` JSON file inside the assistant's private folder,
  read the way `voice keep` reads its signal (`place`, `validate_json_name`, `read_private_at_max`,
  16 KiB, under the state lock, then `refresh::answer_locked`). The note is trimmed and refused when
  empty, when it holds a control character other than newline and tab, or when the file has any other
  shape; a refused note leaves no answer on file and the change still open. Without `--note` nothing
  changes. Nothing new is printed and the note is never echoed. `refresh::answer` and `answer_locked`
  take `note: Option<&str>`; the app passes `None`. This is how G3.1b's weekly runner will keep the
  subject's own words with an answer given elsewhere. Brief: `plans/briefs/G3.1a-approval-note.md`.
- **Who built it:** Codex, from the brief. Her sandbox blocked the CLI test fixture's sockets, so V1
  ran the CLI and full suites outside it.
- **Verified:** red first on their assertions (Codex): the help test (no `--note`) and the library
  note tests (`None` saved); the CLI behaviour tests couldn't reach their assertions in her sandbox,
  so V1 proved them by mutation, each restored byte for byte (`mutate-check.sh`, after a green
  baseline of the 14 note tests): the note not passed to the record (2 failed), the control-character
  check removed (1 failed), the trim removed (3 failed). Outside the sandbox: voice-grow 23 passed,
  the CLI refresh suite 35 passed. `bash scripts/ci-local.sh` on `c6eacb6` (load about 19): every step passed until `voice-agent`'s `sign_in_opener_failure_kills_the_child` (a 5 s timing check this slice doesn't touch; it passed 3 of 3 runs alone); the rerun with 2 workers was cut short by run `002721-r1`'s time limit. Run `022728-r1` reran it on `60b8762` (4 test threads, load about 10): `ci-local: all passed`.
- **Reviews:** security review (sub-task): no HIGH or MEDIUM. Three LOWs kept as known limits: the
  note file's own mode isn't checked (only its folders are; `voice keep` reads the same way, and the
  runner writes its notes 0600 in a 0700 folder); format characters such as bidi marks pass (the note
  is never shown or put in a prompt today: refuse or escape them the day it is); a missing state folder
  gets `voice mine`'s wording (unreachable, a proposal must exist there). Verifier (Fable): PASS on
  `8403e88`, run at the PR because the brief and records took the diff past 400 lines; three nits
  kept as they are: format characters pass (the LOW above), a note with Windows line endings is
  refused as holding a control character, and a byte-order mark before `{` is refused as the wrong
  shape. All three refuse, or never reach a reader.
- **Not verified:** nothing reads the note yet; G3.1b's runner is the first writer.

## G0.1: one kept and one undone refresh on a scratch member, and what it cost (2026-11-09, lane: V1)

- **What:** the growth loop's first live run, on a scratch copy of an existing assistant (its
  filters, its 15 areas and its 97 KB `decisions.md`), in a testers' build from `cdb0ec8` signed in
  with a Claude subscription. Ten corrections were kept (7 that differ from the assistant's pick, 3
  that match; 2 more refused as personal and skipped). The first `refresh propose` was refused by the
  copied-words guard (`check_words`) and passed on the retry; `apply` **kept** it, 9 of 9 tests. A
  reversed correction then gave a change that `apply` **undid**: 2 earlier passing tests failed with
  it, 10 of 11. Walked attended; the scratch member, its state and its logs are deleted.
- **Cost:** 11 calls, all Sonnet, 105 s inside calls, about 342 thousand tokens (about 11.6 thousand
  of them output). Each refresh or answer call carried about 35 thousand tokens, nearly all of it
  read from cache, so a 97 KB `decisions.md` fits one call. Claude Code's own estimate for the day was
  about $1.28. Wall time per step: `propose` 28 s (34 s for the refused first try, 10 s for the
  second change), `apply` 18 s and 17 s. The plan's weekly share couldn't be isolated: the account's
  usage page moved from 8% to 9% after the first `apply`, but other work was using the same account,
  so that move is account-wide, and the later readings were dropped.
- **Found on the way:** VF6 (`voice claude check` names a sign-in a Mac can't start) and VF7 (a
  subscription sign-in still reported dollars; VF7a below, VF7b parked).
- **Not verified:** the weekly share per `propose` and `apply` (needs an account nothing else uses
  during the run).

## VF7a: a subscription sign-in reports calls and tokens, not dollars (2026-11-09, lane: V1)

- **What:** under a Claude subscription there is no per-call bill, yet every report of today's use
  ended in dollars. When Voice is signed in with a subscription in a testers' build, `voice spend`,
  `voice build`, `voice interview draft`, `voice refresh propose` and `apply`, and the app's Build and
  Refresh screens now close with today's calls and tokens from Voice's own ledger, for example
  *Today, 11 calls to Claude and about 342 thousand tokens, on your Claude subscription.* Tokens
  are all four counts Claude Code reports (input, output, cache writes, cache reads); a missing or
  invalid count adds 0 and never stops spending. `claude::Status` and `Ready` gain `subscription`
  (true only for `claude.ai` or `oauth_token` in a build that offers it); `dto::Spend` gains
  `usage`. A Console sign-in and every public build keep the dollar words exactly. The daily limit
  itself is unchanged, in dollars (VF7b, parked on V-Q16). Brief:
  `plans/briefs/VF7a-subscription-usage.md`, plus one fix-up (`tokens_in` returns 0 for a line that
  isn't JSON instead of panicking; unreachable today, since the ledger read validates every line).
- **Who built it:** Codex, from the brief and the fix-up. Her sandbox blocked sockets and pnpm, so
  V1 ran the integration and UI suites outside it.
- **Verified:** red first for the ledger, words and sign-in tests (Codex) and the
  fix-up's test (a panic before the change); V1 saw the integration and UI tests pass outside her
  sandbox: the voice-agent, voice-api and voice-cli suites in the default and `claude-subscription`
  builds (one flake in `tests/stop.rs` at load about 12, passed alone and on a full rerun), the UI
  suite (569 passed) and its typecheck. Mutations: removing the usage paragraph from `BuildRun.vue`
  failed one Build test, from `Refresh.vue` two Refresh tests; dropping cache reads from the sum and
  forcing `subscription: false` were each caught (Codex's report). `bash scripts/ci-local.sh` on
  `ce26213` (2026-11-10, run `211454-r1`, load about 8): the first run stopped on
  `egress::tests::header_deadline_is_absolute` (a connection reset at its end-of-stream read; VF7a
  doesn't touch `egress.rs`, and the test failed 1 of 5 runs alone, so it was filed as VF8); the
  rerun passed in full (*ci-local: all passed*).
- **Reviews:** security review (sub-task): no HIGH or MEDIUM; one LOW kept as a
  known limit: in testers' builds `voice spend` and the app's spend read find Claude Code (which
  makes Voice's profile folders if missing) and start two sandboxed lookups (version and sign-in
  status), so they can take up to their time limit if Claude Code hangs; public builds start nothing.
  Verifier (spec: not required; run because the diff adds over 400 lines): Fable PASS, no
  must- or should-fix; three nits kept as they are (the known limit above; a run whose every ledger
  read fails ends on the dollar line; one report reads the ledger twice).
- **Not verified (walk owed, attended):** a real subscription sign-in in a testers' build showing
  calls and tokens at the end of `voice refresh propose`, and on the app's Build and Refresh screens.

## VF3: Claude Code keeps its own login file inside Voice's sandbox (2026-11-09, attended)

- **What:** the subscription sign-in failed inside the sandbox because Claude Code 2.1.292 saves
  its login by running `security`, which the profile refused, and then skipped its own file
  fallback. By the owner's choice (V-Q15 option 1) the profile now lets the child start
  `/usr/bin/security` and nothing more: the Keychain stays closed (no Keychain services,
  `Library/Keychains` denied), so Claude Code keeps its own `.credentials.json` (0600) in Voice's
  profile, which Voice never reads. Voice excludes the profile folder from Time Machine (checks the
  exclusion attribute, else runs `/usr/bin/tmutil addexclusion`; a failure is silent and retried).
  New `voice claude sign-out` and an app *Sign out* button run Claude Code's own `auth logout` and
  report an error if the file is left, never deleting it; a sign-in and a sign-out never run
  together, and neither waits on the other's lock. On macOS the Console sign-in is refused before
  anything starts: Claude Code's `auth login --console` creates an API key on the person's Console
  and then cannot save it with the Keychain closed (measured), so VF5 gives the public build a pasted
  key instead. Brief: `plans/briefs/VF3-claude-login-file.md` (with four fix-ups).
- **Who built it:** Codex, from the brief and its fix-ups.
- **Measured first (attended, real sandbox, Claude Code 2.1.292):** the subscription sign-in with
  only the `security` line added succeeds, writes the file in the profile, answers a call, and
  `auth logout` removes it; the Console sign-in asks for `org:create_api_key` and fails to save; a
  made-up key passed in the child's environment is accepted with no sandbox change (`auth status`
  signed in, the call reaches the API and is refused as an invalid key; a real key not yet run;
  VF5's basis). Recorded in the lane handoff, V-Q15.
- **Verified:** `bash scripts/ci-local.sh` all passed outside Codex's sandbox (it requires the
  sandbox tests to run) on the build plus fix-ups 1-2, and again with fix-up 3 at load about 15
  (2 build jobs, 2 test threads); `the_keychain_stays_closed` ran in both builds. After fix-up 4
  (a doc sentence and one test's wait) Codex ran the api suite in both builds (368 and 375
  passed); the land reruns the whole gate. Codex saw the core tests red first; not every test did
  (listed in her report).
- **Reviews:** Verifier: Fable PASS with three should-fixes (a vacuous find assertion, the sign-out
  holding the sign-in lock over child runs, a SECURITY sentence on `security`'s entitlements), all
  applied in fix-up 3; a repeat pass (Opus) PASS with one doc should-fix (fix-up 3's SECURITY
  sentence said the base profile allows the AppSSO service; `system.sb` defines it only inside
  `system-network`, which this profile doesn't call, checked by the attended session) and one test
  nit (the shutdown test's wait too short to fail), both applied in fix-up 4; a final pass (Opus) on
  the committed head PASS, after which this record's one overclaim was corrected. Security review:
  no HIGH or MEDIUM; three
  LOW, left as known limits: the CLI and the app hold separate locks, so a CLI sign-out can overlap
  an app sign-in (as two sign-ins already can); the backup check follows a link swapped in at the
  wrong moment inside the 0700 folder; quitting the app mid-sign-out does not wait for it.
- **Not verified (walk owed, attended):** a real subscription sign-in, call and sign-out in a
  testers' build on Mimi, then G0.1.

## VO17: a Console sign-in for everyone, the subscription only in testers' builds (2026-11-09, lane: V1)

- **What:** Voice signs its own Claude Code in with `claude auth login --console` (an Anthropic
  Console account, API billing), by the owner's ruling VO-D10. The subscription sign-in
  (`--claudeai`) is compiled only with a new cargo feature, `claude-subscription`, off by default
  and passed through by the api, CLI and app crates; `build-mac-app.sh --testers` builds a tester's
  copy. Status reads `loggedIn` and `authMethod`: `api_key` is signed in, while `claude.ai` and
  `oauth_token` count only in a testers' build; a public build tells the person to sign in again
  with a Console account, also when a sign-in itself ends on a subscription account. The link
  check takes the sign-in kind (Console: `platform.claude.com/oauth/authorize?` only), and so does
  the sandbox probe, which now signs in with Console too. Connect Claude shows the subscription
  button, and the CLI the `--subscription` flag, only in a testers' build. The egress list is
  unchanged: the Console sign-in's hosts were already on it (read from Claude Code 2.1.292's
  bundle). `ci-local.sh` runs the agent, api and CLI suites with the feature first, then the
  default build, checks that default app and CLI builds lack the feature, and runs clippy with it.
  Brief: `plans/briefs/VO17-console-sign-in.md` (with two fix-up sections).
- **Who built it:** Codex, from the brief and its two fix-ups.
- **Verified:** api and CLI suites green with and without the feature (2 test threads, load about
  15); mutation checks by `cp` and `cmp`: removing the post-sign-in refusal turns the new api and
  CLI tests red, a subscription login that runs `--console` turns both new feature tests red, and
  the probe's old link check fails 5 probe tests; the probe test passes on `e2ebd2e` (40 OK).
  `bash scripts/ci-local.sh` all passed on `e6821fe` and again on `0522663` (the code head
  `e2ebd2e` plus this record), at load about 6.
- **Reviews:** security review: no findings, on the first build and on the fix-ups. Verifier: Fable
  PASS on `e65e06e` with three should-fixes (all applied); a repeat pass (Opus) found the probe
  still accepted only subscription pages (fixed in `e2ebd2e`); a further repeat pass (Opus) PASS on
  `e2ebd2e`.
- **Incident:** the first Verifier ran a freshly built testers' CLI and started a real
  `claude auth login --claudeai` in Voice's own profile; it was left waiting for the browser and
  nothing was saved. Every later brief and Verifier prompt forbids running a built binary, the app
  or the real `claude`.
- **Not verified (walk owed, attended):** a real Console sign-in on Mimi, a tester build's
  subscription sign-in, and a default build refusing a profile signed in with a subscription.

## VF1: the anonymizer's load test holds on a busy computer (2026-11-08, lane: V1)

- **What:** `two_hundred_three_run_terms_load_within_five_seconds` asserted a 5-second wall-clock
  bound and failed when the computer was busy (5.43 s inside `ci-local.sh`, 12.8 to 30.6 s alone, at
  load about 14). It becomes `two_hundred_three_run_terms_load`, which only asserts the 200 terms
  load. The guard moves to a new unit test in `builder/ingest/src/anonymize/terms.rs`,
  `term_patterns_stay_small`: every pattern a three-run term and a 12-word term build must compile
  under a fixed `size_limit` (`1 << 18` and `1 << 20`). Compiled size doesn't depend on load, and it
  is what `1425860`'s fix shrank. Test-only: no behaviour changes. Brief:
  `plans/briefs/VF1-load-test-under-load.md`.
- **Who built it:** Codex, from the brief (her first pass stopped at a threshold the first brief got
  wrong; the amended brief set one limit per term).
- **Verified:** the new test passes, and fails with `CompiledTooBig(262144)` when `1425860`'s
  counted separators are put back (mutation by `cp` and `cmp`, by Codex and again by V1); a second
  mutation (the three-run limit at `1 << 16`) fails it too. `anonymize_rules` passes; clippy and fmt
  pass. `bash scripts/ci-local.sh` all passed on `dff08da` (VF4 and VF2 below
  VF1), the same tree as this landed head outside `.ai/`, at load about 14. Its first gate run
  stopped on timing tests VF1 doesn't touch; those are VF4 and VF2.
- **Reviews:** security review: no HIGH or MEDIUM; one LOW (nothing bounds how many patterns one
  term builds), left as a coverage note for VF2. Verifier: not required (test-only).
- **Not verified:** the limits under a `regex` version other than 1.13.1 (locked); a new version
  could move the sizes, and the test would then say so.

## VF2: three anonymizer tests hold on a busy computer (2026-11-08, lane: V1)

- **What:** three tests in `builder/ingest/tests/anonymize_file.rs` asserted wall time and failed
  `ci-local.sh` at load 11 to 19 with no change to the code. The two term-word tests (from
  `9f77698` and `ffaca9e`) lose their timing and become
  `punctuation_only_term_word_is_found_after_a_long_dash_run` and
  `combining_mark_only_term_word_is_found_after_a_long_mark_run`; their guard moves to a new unit
  test in `builder/ingest/src/anonymize/terms.rs`, `letterless_words_are_left_out_of_bounded_patterns`,
  which checks that `"- Rowan"` and `"\u{0903} Rowan"` build the same bounded patterns as
  `"Rowan"`. That is what the two fixes changed, and it doesn't depend on time.
  `verifier_10_many_matches_have_bounded_line_lookup` (from `0dff027`, the linear line lookup) keeps
  a timing check, because nothing else can see that fault: it compares a 512 KiB scan with the
  median of five 8 KiB scans in the same run, with a relative deadline of 256 times (at least 1 s).
  Test-only. Brief: `plans/briefs/VF2-anonymizer-tests-under-load.md`.
- **Who built it:** Codex, from the brief, in one pass.
- **Verified:** mutations, each by `cp` and `cmp` (by Codex): both term-word filters removed
  (`9f77698` reverted) fails the unit test on `"- Rowan"`; the filters back on
  `char::is_alphanumeric` (`ffaca9e` reverted) fails it on `"\u{0903} Rowan"` only; the old
  newline-counting lookup (`0dff027` reverted) fails `verifier_10` on its deadline, after 216 s.
  Ten runs of `anonymize_file` in a row at load 12 to 14, all green, ratios 13 to 60 against the
  256 bound. Clippy and fmt pass. `bash scripts/ci-local.sh` all passed on the stack's head
  `dff08da` (VF4, VF2, then VF1's test-only commits on top) at load about 14; VF2 alone had clippy,
  fmt and its ten runs.
- **Swept:** every other wall-clock assert in the crates the last gate never reached
  (`voice-ingest`'s other test files, keystore, panel-runner, panels, save, steps) and the Python
  tests: the ones left have 5 s or more of slack and have passed in every gate so far.
- **Reviews:** security review (Opus): no HIGH or MEDIUM. Two LOW, left as they are: the
  test's comment says a regression fails fast, but with `0dff027` reverted it takes about 230 s (the
  small scan slows too); and no end-to-end timing guard on term inputs is left. Verifier: not required (test-only).
- **Not verified:** the ratio at load above about 14 (the gate ran at about 14); VF1's
  review LOW (bound how many patterns one term builds) was considered and left: `Term::new` builds at
  most five (one exact, and a word and a run pattern for each of at most two spellings), by its
  structure.

## VF4: the gate's timing tests hold on a busy computer (2026-11-08, lane: V1)

- **What:** four egress tests in `builder/agent/src/egress.rs`, one probe test in
  `tests/claude_sandbox_probe_test.py` and one reader test in `builder/api/src/reader_client.rs`
  checked a deadline with 150 to 500 ms of slack, and failed
  `ci-local.sh` when the computer was busy (load 11 to 23) with no change to the code. Each now
  leaves a wide gap between the deadline working and the fault it guards: the dial bounds get 1 s
  of slack, under the client's 2 s read timeout; the test resolver `slow` waits 10 s, so the capped
  test's first two resolvers are still in flight on the third request; the header test waits up to
  four header windows for the close and allows three; the probe's idle test uses a 1 s window with
  six 0.25 s gaps; the reader's `progress_resets_the_quiet_deadline` uses a 1 s quiet window with
  eight 0.25 s gaps (added after the first gate run failed on it, as the brief's amendment). Every
  lower bound and count stays. Test-only. Brief:
  `plans/briefs/VF4-egress-tests-under-load.md`.
- **Who built it:** Codex, from the brief. Her sandbox refuses TCP binds, so V1 ran the Rust
  mutations and repeat runs herself.
- **Verified:** red on `main`: `resolvers_in_flight_are_capped` failed 2 of 3 runs at load about
  20, and the probe's idle test 3 of 4 at load about 11. Mutations, each by `cp` and `cmp`: the
  header deadline restarted on each read fails `header_deadline_is_absolute`; the dial waiting for
  its resolver fails the capped test and both resolution tests; no cap on resolvers in flight fails
  the capped test; the idle deadline not reset by traffic fails the probe test (by Codex and again
  by V1); the reader's quiet deadline set once for the whole read fails the reader test (by Codex
  and again by V1). Red on `main` for the reader test too: 3 of 6 runs failed at load about 14.
  Repeat runs: the reader tests 10 of 10 (by Codex, load 12 to 14); the probe tests 10 of 10 (by Codex, then by V1 at load 14 to 17); the egress
  tests 44 of 45 at load 7 to 19. The egress suite takes as long as before (7.2 s, against 7.3 to
  7.7 s). Clippy and fmt pass. `bash scripts/ci-local.sh` all passed on the stack's head `dff08da` (VF4,
  then VF2 and VF1 on top) at load about 14.
- **Reviews:** security review (Opus): no HIGH or MEDIUM. Two LOW, left as they are: the
  upper bounds now let a deadline overshoot of 2 to 3 times pass (the faults themselves, a
  trickling client and a slow resolver, are still caught); the probe test's final read has about
  1 s of headroom before the child's 2 s timeout. Verifier: not required (test-only).
- **Not verified:** `reader_client.rs`'s two kill tests (`< 400 ms` against a 50 ms deadline),
  which never failed here and whose message and kill-count asserts catch their fault without the
  time bound; which test failed the one red egress run of 45 (it wasn't captured, and 35
  later runs didn't repeat it); the other timing tests in the crate, which this slice didn't
  touch.

## VO1.1: the read-fence probe on a second Mac (2026-11-05, lane: V1)

- **What:** the record of the attended run of `scripts/read_fence_probe.py run` on a second Mac
  (2026-11-05, Claude Code 2.1.292, another account's own sign-in, a fresh clone of the public
  repo at `10ee408`): *the fence held*. `docs/spikes/VO1-read-fence.md` gains *The second Mac's
  run*, its status now says two Macs, and *Not covered* says "both live runs" for Grep and Glob.
  DECISIONS has a 2026-11-05 entry superseding the 2026-11-04 one's *a second Mac's run is owed*.
- **For VO6:** nothing changes. Every check line matched the first Mac's; only the Mac's own hooks
  and plugins (7 and 1, against 18 and 7) and the cost (US$0.23, against US$0.15) differed.
- **Who wrote it:** V1 directly (records only, no code; nothing for Codex).
- **Verified:** the two runs' outputs compared by `diff`, 40 lines each: exactly the two lines above
  differ. The second output is as pasted into the lane's question by the owner; this runner did not
  see the run.
- **Reviews:** security review (Opus): no HIGH or MEDIUM; three LOW wording fixes made (another
  account, not a person; the remote-shell sign-out is the owner's report; no reason given for the
  symlink and outside-file refusals). Verifier (Fable) on `28d7d8e`: PASS, no must-fixes; her two
  should-fixes (rewrap, whole sentences) and her optional *Not verified* line made.
- **Not verified:** which 2 names Glob listed on either Mac (the probe prints counts only); why the
  second run cost more (each turn is capped at US$0.10 on both); why the remote shell read signed
  out (not seen by this runner, so the spike gives it as the owner's report); why `out-link.md` and
  `outside/far.md` were refused (the probe records the denial, not its reason; no deny rule names
  them).

## VO1: the live run, and VO1 closes (2026-11-04, lane: V1)

- **What:** the record of the attended live run of `scripts/read_fence_probe.py run` on Mimi
  (2026-11-04, Claude Code 2.1.292, signed in): exit 0, *the fence held*. Its output is now in
  `docs/spikes/VO1-read-fence.md` (*What the live run showed*), and DECISIONS has a 2026-11-04
  entry superseding the 2026-11-01 one's *until both Macs have passed*.
  The owner chose to close VO1 on this one Mac (V-Q11, option B); the second Mac's run is its own
  follow-up, VO1.1 (attended), and blocks nothing.
- **Findings recorded for VO6:** every deny-list read refused by Claude Code, nested, upper-case and
  symlinked canaries included, and the file outside the folder refused for being outside it; Grep
  held; Glob listed 2 of the 22 denied names (names only); a `**/.env*` rule did not hold for an
  `--add-dir` folder, so the `//` form stays required; with no rules `.env` was read. The run cost
  US$0.15 against V-Q11's "a few cents" estimate. Whether to keep Grep and whether a denied name may
  reach a turn are left to VO6.
- **Who wrote it:** V1 directly (records only, no code; nothing for Codex).
- **Verified:** the output block matches V-Q11's pasted output line for line (compared by `diff`);
  `bash scripts/ci-local.sh` all passed on `0f3b07c` (records only since).
- **Reviews:** security review (Opus): no HIGH or MEDIUM; two LOW wording fixes made (a fenced
  turn's output, not any output; no private-tool word), and one noted: `.ai/SECURITY.md` gets a
  read-wide row from the first shipped code that reads wide (now in the spike's *Not covered*).
  Verifier (Opus, a records pass on VO1, whose code passes are on record) on `0f3b07c`: FAIL, one
  must-fix, the same "any output" claim (the no-rules turn did read `.env`), fixed; should-fixes
  made: the outside file is refused for being outside the folder, not by a rule; "only to the child"
  unverified and dropped; Glob's names stated for this run only. Delta Verifier (Opus) on `137fc34`:
  PASS; her two optional wording fixes made (which turns the no-word check covers; the hooks line).
- **Not verified:** the second Mac (VO1.1); which two names Glob listed (the probe prints counts only);
  whether any of the Mac's 18 hooks or 7 plugins played a part in a refusal (the Verifier found none
  of the person's own settings hooks act on Read, Grep or Glob; plugins' hooks unchecked; the no-rules
  turn shows only the rules refuse `.env`: with the same settings loaded and no rules, it was read).

## VO14.4b: the Ask screen (2026-11-03, lane: V1)

- **What:** the app's screen for VO14.4a's Ask, as V-Q12 answered (a). A home card, *Ask one
  assistant*, between your assistants and Panels, opens `Ask.vue`: pick an assistant (one without
  your permission is disabled), tick which permissions that are on feed this one question (ticks
  start empty each time; the calendar's box is disabled when no file is chosen, or it moved or is damaged), see
  where it may go, type the question (1 to 2,000 characters), then the answer (line breaks kept)
  and the library's line saying who answered and what it was given. One app command,
  `ask_assistant`, calls `voice_api::ask` unchanged and is paired with `voice ask` in `PARITY`.
  Leaving is blocked while a question is out. Brief: `plans/briefs/VO14.4b-ask-screen.md` (§ 9 the
  review's and the Verifier's fixes).
- **Decided at pickup (V1):** where it may go is a sentence built from what's ticked, not a route
  asked of the backend (that would be a second command); the exact answer comes back after the ask,
  and a refusal comes back as the guard's own sentence. A home card, since Ask starts by picking an
  assistant, as Panels does.
- **Who built it:** Codex, two dispatches (§§ 1–8; § 9). V1 committed.
- **Tests:** `builder/ui/src/screens/Ask.test.ts` (23), `ask/words.test.ts` (2), one in
  `VoiceApp.test.ts`, one row in `app/tauri-backend.test.ts`, `builder/app/tests/lockdown.rs` and
  `builder/cli/tests/app_parity.rs` (the new pair).
- **Mutations (killed, V1, each restored with `cmp`):** `open()` no longer blocked while asking;
  ticks not cleared on a new choice; a stale permissions answer applied; the radios left enabled
  while asking.
- **Gates:** security review (Opus): no HIGH or MEDIUM; LOW, the calendar line said only a model
  server on your own machines could answer, though one set up with `--network` can be on the local
  network (fixed, § 9.1); LOW, the plain line didn't say Claude Code sends to Anthropic (fixed,
  § 9.1); LOW, Ask has no Stop, so a hung call holds the screen until the 15-minute call limit (Claude Code's and the model server's)
  (recorded, not fixed: a stop belongs to the library). Verifier (Fable) on `91632fd`: PASS, no
  must-fix, from reading (the resource gate held her test runs; V1's runs below cover them);
  should-fixes: the plain line didn't say a lowered ceiling with no server means a refusal (fixed,
  § 9.1), a second `running` after unmount (fixed, § 9.2). Her note: the filter that sends only
  permissions that are on is defence in depth no test reaches, since the screen never renders an off
  one. `VOICE_REQUIRE_SANDBOX=1 bash scripts/ci-local.sh` all passed on `91632fd`; the UI suite (556)
  and typecheck passed again on `d490116`, and `ci-local.sh` all passed on `6c0d22c`. Delta Verifier (Opus) on `6c0d22c`: PASS; her should-fixes (the security row said Ask routes before anything is read, when only the calendar comes after the route; the calendar box is also disabled for a damaged choice; the 15-minute limit is every call's) fixed in the records.
- **Not verified:** the screen in the running app, light and dark, and a real answer from Claude Code
  or a model server (walk owed); "nothing else" in the where-lines and the library's line leaves out
  the names and today's date the prompt carries (VO14.4a's wording, kept the same).

## VO14.4a: Ask, in the library and the terminal (2026-11-03, lane: V1)

- **What:** `voice_api::ask` and `voice ask <member_dir> --question-file <path> [--with calendar.read]`:
  one question to one built assistant, with the permissions the person ticks this time, as V-Q12
  answered (a). The brief carries the assistant's `decisions.md`, the question, today's date and,
  when ticked, the calendar events (at most 500), each fenced as data; the system prompt is fixed
  text. The guard routes before the calendar is read: with the calendar ticked and no model server it
  refuses with its own sentence, reading, logging, spending and running nothing. The calendar is
  read only after a route is found (step `ask` in the audit log), before the session is opened.
  Claude Code's sign-in is checked only when the route picks it (it must still be installed). The reply is cleaned, capped at 4,000
  characters, returned and never kept. Brief: `plans/briefs/VO14.4a-ask.md` (§ 8 the review's fixes, § 9 the delta Verifier's).
- **Decided at pickup (V1):** VO14.4 split into VO14.4a (this) and VO14.4b (the app's screen), since
  together they were bigger than VO14.3, which filled a run. The reply isn't withheld for looking
  private, since it goes only to the person who asked. A question the filters call secret is refused.
- **Who built it:** Codex, three dispatches (§§ 1–7; § 8; § 9). V1 committed.
- **Tests:** `builder/api/tests/ask.rs` (10), `builder/api/src/ask/tests.rs` (12),
  `builder/cli/tests/ask.rs` (5).
- **Mutations (killed):** the Verifier's reasoning over the suite (the calendar read before routing,
  the consent re-check dropped, Claude checked always, the session before the read), and V1's two:
  the question file checked against the built-in deny list only; the invisible-character mapping
  dropped.
- **Gates:** security review (Opus): no HIGH; MEDIUM, the question file skipped the person's own
  `denied_paths` (fixed, § 8.1); LOW, invisible characters in the reply (fixed, § 8.2); a residual
  note recorded as a limit in `.ai/SECURITY.md`. Verifier (Fable) on `9b0b81c`: PASS, no must-fix;
  should-fixes, all in § 8 (60 added lines in files she saw, inside her pass): the flaky model-server
  test (an accepted socket keeps the listener's non-blocking flag on macOS, which she proved with a
  probe), a false comment, an `unreachable!` and a dead fence fallback. Two notes she left, not
  fixed: `decisions.md` is read and Claude's profile folder ensured before the route (neither is a
  source read, a log line or a spend); a revoke between the calendar read and the call still sends
  the events once (the read is logged). Delta Verifier (Opus) on `ccb5ad7`: code PASS, records FAIL
  (the names were said to be fenced; they aren't, so the record now says so; the route was said to
  come before anything is read, now narrowed to the calendar). Her should-fix: § 8.2 as V1 wrote it
  flattened every answer's line breaks (`hidden` counts `\n` and `\t`) and let an all-invisible reply
  through as a space; fixed in § 9.
- **Not verified:** the real sandboxed calendar reader end to end with an Ask (the tests pass the read
  as a closure; the reader itself is VO14.2's); a real model server and a real Claude Code answer.
  The walk comes with VO14.4b's screen.

## VO14.3: the permissions and audit-log screens (2026-11-03, lane: V1)

- **What:** a card on the Save page, `What your assistant may read` (`Permissions.vue`), with
  `What Voice has logged` (`AuditLog.vue`) inside it. Per capability (only `calendar.read` today):
  on or off, the day given and the window in the CLI's words, the window's two inputs, Give
  permission or Change the window, and Turn off; under the calendar, the chosen file or folder
  (its name cleaned as the CLI shows it) and Choose a file / Choose a folder through the native file
  window. The log shows the last 50 lines newest first in the CLI's words, then up to 1,000.
  `voice_api::permissions`: `list_permissions`, `give_permission`, `turn_off_permission`,
  `check_calendar_choice` and `use_calendar_export` (for the native window), `audit_log`; five new
  app commands through the lockdown lists, each paired with its `voice grants|calendar|audit`
  command. Brief: `plans/briefs/VO14.3-permissions-and-audit-screens.md` (§ 10 the review's fix,
  § 11 the Verifier's should-fix).
- **Decided at pickup (V1):** giving permission is in the app too, not only Turn off (the person
  is there, and the grant is their own dated consent; giving again is how the window changes).
  Only fixed sentences cross as failures; an unknown capability id isn't echoed. Two fixes in
  VO14.2's code: `BAD_GRANTS` told the person to give permission again, which fails on a damaged
  file, so it now points to `voice grants show`; and `read_calendar_through` passed a damaged
  file's text, which holds its path, into a `Failure`. A moved or damaged calendar choice is shown
  on the card (`calendar_problem`) instead of failing it, so the person can choose again.
- **Who built it:** Codex, four dispatches (§§ 1–9; § 10; § 11; § 12). V1 committed: Codex's sandbox
  can't write a worktree's git index.
- **Tests:** `builder/api/tests/permissions.rs` (10), the broker test in
  `builder/api/src/calendar/tests.rs`, `lockdown.rs`, `app_parity.rs` (the five pairs),
  `Permissions.test.ts`, `AuditLog.test.ts`, `permissions/words.test.ts`, `VoiceApp.test.ts`,
  `tauri-backend.test.ts`.
- **Mutations (10, all killed, the Verifier's nine plus V1's one):** the damaged check under the
  lock dropped; every `Invalid` passed through the broker; `UNKNOWN` echoing the id; the GONE/BAD
  mapping dropped; give without the lock; the 1–1,000 bound dropped; no reload after a failed
  change; an input not disabled while busy; the calendar problem not cleared on a choice; the
  days-back bound moved to 32.
- **Gates:** security review (Opus): no HIGH or MEDIUM; two LOW, recorded below. Verifier (Fable)
  on `78b88a7`: PASS, two should-fixes: a blank window input sent as 0 (fixed in § 11, 23 added lines in
  files she saw, inside her pass) and the broker's wide `BAD_GRANTS` mapping (recorded below).
  Delta Verifier (Opus) on `869427e`: PASS; her should-fixes: a test that Turn off works while
  the window inputs are invalid (§ 12, the one mutation that had survived) and this record's line
  count, both fixed.
  `VOICE_REQUIRE_SANDBOX=1 bash scripts/ci-local.sh`: all passed on `78b88a7`, and again at the land.
- **Not verified:** the native file window and the screens in a running app (no Tauri launch in a
  run); a walk is owed.
- **Left (known limits):** any `grants::load` error, a passing read error included, is shown as
  "damaged, delete it", so a person could delete a good file after a passing fault. In
  `read_calendar_through` every refusal other than no-permission reads as `BAD_GRANTS`, including
  a bad step name, which no person can send today (the steps are constants), and an error reading
  the file is shown as `LOG`. VO14.4 (first use, V-Q12) is next.

## VO14.2: the calendar, from a file the person exported (2026-11-02, lane: V1)

- **What:** the first capability that reads. The person picks an exported calendar (`.ics`, one
  file or a folder of up to 64), kept as `calendar-export.json` in the state folder
  (`voice calendar choose`, no history consent needed: the grant is this source's permission).
  It is read only by the sandboxed reader (`voice reader read-calendar`) with that export as its
  only source; the reader loads the grant itself and uses its window. The broker
  (`voice_api::calendar::read_calendar_through`) requires the grant first (a refusal is logged),
  finds the Mac's zone outside the sandbox, checks the reader's answer (window, dates, overlap,
  count, hidden characters) and logs the read, or drops it when the log line can't be written.
  `voice_ingest::calendar`: the system zone database (TZif v2+), RFC 5545 content lines, a
  bounded recurrence subset (daily, weekly BYDAY/WKST, monthly BYMONTHDAY ±/ordinal BYDAY,
  yearly), EXDATE and overrides matched by instant, the window as overlapping local days, the
  text filters, caps and a 5,000,000-step budget per read with skip-ahead. Only title, place,
  start, end, all-day and status leave the parser; descriptions, attendees and the rest are never
  read. `voice calendar show` prints the read. Brief: `plans/briefs/VO14.2-calendar-from-an-export.md`
  (§ 13 part A's review, § 14 the security review's and Verifier's fixes, § 15 the live run's,
  §§ 16–17 the repeat Verifier's must-fix, § 18 her should-fixes).
- **Decided at pickup (V1):** times are converted with the system database, never guessed (an
  unknown zone, a Windows zone name or a future DST rule is "not understood" and counted); the
  zone is found outside the sandbox (the reader can't follow `/etc/localtime` into `/var/db`);
  off macOS the read refuses; not understood counts only entries that could touch the window.
- **Who built it:** Codex, six dispatches (part A §§ 2–5; part B §§ 6–13; § 14; § 15; § 17;
  § 18). V1 committed §§ 17–18 herself: Codex's sandbox can't write a worktree's git index.
- **Tests:** `builder/ingest/tests/calendar_rules.rs` and `calendar_export.rs`, the `recur.rs`
  and `read.rs` unit tests, `builder/api/src/calendar/tests.rs`, the reader protocol and Judge
  tests, `builder/agent/tests/grants.rs` (no state folder), `builder/cli/tests/calendar.rs`
  (through the real sandbox) and the CLI wording unit tests.
- **Mutations (12, all killed):** the broker's `require` swapped for a bare load; the log error
  ignored; the window check; the hidden-character check; the `from` check; the text filters;
  EXDATE; the window's last day; the override build left uncharged; the override maps built per
  master; the soft hyphen dropped from the shared hidden list; `text_check` no longer sharing it.
- **Live:** the built `voice` in a throwaway home with an invented assistant and calendar:
  no choice, no grant (refused and logged), a grant, then a Berlin 16:00 entry shown at 10:00 New
  York and 00:00 Tokyo, a weekly COUNT=4 series showing its two in-window occurrences, an
  out-of-window entry left out, an HOURLY rule counted, the DESCRIPTION/ATTENDEE canary absent,
  and the audit log's refused, given and read lines. The first Verifier's hostile shape (199,000
  daily entries from year 1, 23 MB) went from about 20 minutes to 9 seconds (debug build).
- **Gates:** security review (Opus): no HIGH, two MEDIUM (CPU and EXDATE memory in the reader),
  four LOW, all fixed or recorded as limits (§ 14). Verifier (Fable): FAIL on the CPU must-fix,
  plus the UTC override should-fix; fixed in § 14. Repeat Verifier (Opus) on `83119be`: FAIL, the
  override maps rebuilt per master from every override sharing its UID (quadratic, about 18
  minutes at 100k × 100k); fixed in § 17 (built once per UID and all-day, charged once). Repeat
  Verifier (Opus) on `fea0e58`: PASS (4,000 random calendars identical to `83119be`'s output;
  99,990 × 99,990 under one UID at most 1.41 s in release), with should-fixes: tag characters
  and supplementary variation selectors reaching the model, and a soft hyphen shown as a space,
  both fixed in § 18 (45 lines, inside the pass); the § 17 test pins one `prepare` call, not
  other per-master work, accepted. Security review (Opus) of § 17's delta: no findings; the one
  LOW it noted outside the delta (tag characters) is § 18's first fix.
  `VOICE_REQUIRE_SANDBOX=1 bash scripts/ci-local.sh`: all passed on the final code.
- **Not verified:** a non-macOS build's refusal path at run time; real Google, Apple and Outlook
  exports (fixtures are shaped like them, invented).
- **Left:** VO14.3 (the screens) and VO14.4 (first use, V-Q12) are next. Not done, from the
  delta Verifier on `3fed7f8` (existing before this slice): a zero-width character inside a word
  becomes a space before the text filters see it (`Ther<U+200B>apy` reads as `Ther apy`), so a
  split word can slip past a filter; dropping in-word format characters, as § 18 does for the
  soft hyphen, would close it.

## VO14.1: grants and the audit log (2026-11-02, lane: V1)

- **What:** `voice_agent::grants`: typed read capabilities (`calendar.read`, source `calendar`; no
  write kind exists), a dated (the local day), scoped (a window of 0–31 days back and 1–92 ahead)
  and revocable grant per assistant in its state folder (`grants.json`, 0600, strict, versioned,
  read only when owner-only; a damaged file is never "no grants"), and an append-only audit log
  (`audit.jsonl`: four line shapes, UTC time, capability, window, step, count and a fixed reason,
  never a path, content or error text). `require` refuses and logs; `record_read` logs a read
  (both for VO14.2). A grant or revoke whose log line can't be written puts the old file back; a
  grant whose rollback also fails is removed (fail closed). New
  `voice_ingest::private::{read_private_tail_optional, read_private_owned_optional_at_max}`, sharing
  one opener with `read_private_at_limit`. CLI `voice grants show|give|revoke`, `voice audit show
  [--last N]` (newest first, the last 1 MiB). Brief: `plans/briefs/VO14.1-grants-and-audit-log.md`
  (§ 9 the review's test gaps, § 10 the security review's and Verifier's fixes, § 11 the repeat
  Verifier's should-fixes).
- **Decided at pickup (V1):** VO14 split into VO14.1 to VO14.4; calendar first from an exported
  file (the mail precedent), contacts and a live calendar later and not carded; no write
  capability; where a read is first used is V-Q12 (VO14.4 waits on it). Grants live in the
  state folder, never member.json (permission belongs to this person and this computer). Grants
  sit in `voice_agent` beside the privacy guard and the ledger (`local_day`, `utc_stamp`).
- **Who built it:** Codex, four dispatches (the slice; § 9; § 10; § 11).
- **Tests:** `builder/agent/tests/grants.rs` (round trip and exact bytes, window bounds, re-give,
  revoke twice, every damaged form incl. version 0, the object spellings and 0640, `require`'s
  refusal lines, the rollback both ways, a refusal whose log append fails, step names, the audit
  reader's shapes, bad lines, `last`, the 1 MiB tail, symlink and 0644 logs, forget); the
  `grants.rs` unit tests (each branch of the fail-closed rollback); `builder/cli/tests/grants.rs`
  (exact output, no state folder, unknown or multi-line capability, ranges, lock);
  `builder/ingest/tests/private_at.rs` (tail read and owner-only read).
- **Mutations (20, all killed, `mutate-check.sh`):** give and revoke rollback removed; revoke's
  no-op guard; version check loosened (survived first, test added in § 9); duplicate check;
  window bound; `require`'s refusal append error swallowed (survived first, test added in § 9);
  a damaged refusal not logged; the step check in `require` and in the reader; the stamp check;
  `older` on the last cut; the cut first line kept; the tail's owner check; the CLI give lock;
  the owner-only grants read swapped and its check removed; control characters kept; the remove
  result ignored in the rollback.
- **Live:** the built `voice` in a throwaway home with an invented assistant: give, show, a bad
  capability and window, revoke twice, an audit line with an extra field (counted, not shown),
  `--last 1001`; files 0600.
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 bash scripts/ci-local.sh` all passed on `d89a1a9`
  `8ed801f` and d737eb9.
- **Security review (Opus, on `d89a1a9`):** no HIGH or MEDIUM. LOW, fixed in § 10: a failed
  rollback could leave a grant unlogged (now fail closed); an object spelling of an id was
  accepted. LOW, recorded as a limit in SECURITY: a torn audit line after a failed append can hide
  the next entry.
- **Verifier (Fable, on `d89a1a9`): PASS**, should-fixes done in § 10: the object spelling; a
  group- or world-readable `grants.json` was read; the unknown capability shown raw. Carried to
  VO14.2's brief: `require` on an assistant with no state folder returns the append's raw error,
  so VO14.2 needs a fixed sentence there. Not done (trivial): a cut landing exactly on a line
  start drops that line; a second `fstat` in `read_private_at_limit`.
- **Repeat Verifier (Opus, on `8ed801f`):** PASS, should-fixes done in § 11 (d737eb9, 23 lines in files she saw): line and paragraph separators and bidi controls in a shown id become spaces; `Capability` and `Reason` no longer derive serde. Not done: a 0640 `grants.json` is called damaged rather than naming the real cause, as the brief asked.
- **Not verified:** `local_day` under non-UTC time zones (tests fix the clock at midday UTC); a
  real full disk (the fail-closed branch is unit-tested through its decision function).

## VO11.2d: the skills reading goes through the reader (2026-11-02, lane: V1)

- **What:** on macOS the app's four skill actions (`voice_api::{list,agree,withdraw,refresh}_skills`)
  and a panel's refresh of each seat's skills copy start the shipped `voice` as the reader
  (`voice reader skills`) instead of reading in the app's process. Its sources are
  `voice_ingest::skills::sources`, named beside `discover`: the skill roots that exist
  (`~/.claude/skills`, the plugin list and each plugin's `skills`, each listed project's
  `.claude/skills`, `~/.codex/skills`) plus the real folder of each linked skill, synced account or
  linked `synced` folder that holds a skill; then `voice_agent::reader::usable`, the reader's own
  per-source rule (`bounds` + `accepted`, shared with `Reader::new`), drops what the reader would
  refuse. `MAX_SOURCES` 64 to 256. A panel already holds every seat's state lock, so it sends
  `refresh-held`, which takes the lock when free and runs unlocked only on `BUSY`. A skill root the
  reader may not read (`EPERM`) is skipped with the "couldn't be read" note, in the terminal too.
  Brief: `plans/briefs/VO11.2d-skills-through-the-reader.md` (§ 11 the reviews' fixes).
- **Decided at pickup (V1, measured on this Mac, Darwin 27):** inside a Seatbelt profile a missing
  path answers `ENOENT` even where reading isn't allowed and an existing non-allowed folder
  `EPERM`, so absent roots needn't be sources; `sandbox-exec` took 300 `-D` parameters and rules,
  each still biting. Link targets are sources one by one, never their parent folders (one real
  setup links 48 skills from one folder; a parent would expose unlinked neighbours). A refused
  source drops its skills rather than failing the screen. `refresh-held` rather than passing the
  lock across processes.
- **Who built it:** Codex, five dispatches (the slice; § 11 the reviews' fixes; § 11.9 a linked
  `synced` folder holds accounts, found in V1's review of § 11.1; § 11.10 and § 11.11 the repeat
  Verifiers' should-fixes).
- **Tests:** `skills_discover.rs` (`sources` covers every discovered skill, in order, plugin links
  inside and outside the install, a `.ssh` target out; empty home; an unreadable project root noted
  once; link targets need a regular `SKILL.md`; synced containers at both depths);
  `reader_profile.rs` (`usable` keeps what `Reader::new` accepts, drops state, crew, deny-listed and
  overlapping sources, caps at 256); reader protocol (`refresh-held` under a held lock, with a free
  lock, with ids, on a lock error); `reader_client` (unknown notes dropped, a bad deny entry shows
  the step's sentence); panel runner (the injected refresh's result reported per seat, skipped when
  the mirror isn't in use); `reader_app.rs` through the built `voice` under the real sandbox (list,
  agree, refresh, withdraw and `refresh-held` equal the in-process answers and mirror files; `GONE`
  and `NONE` pass through; a skill linked into a member-denied folder or another assistant's folders
  isn't listed; a canary outside every source never reaches the state folder; no run folder left).
- **Mutations (all killed, `mutate-check.sh`):** link targets dropped from `sources`; the member
  deny check removed from `accepted` (path and canonical both: one alone survives, since a
  canonical target makes them the same check); root `EPERM` back to an error; `refresh_held_in`
  taking the lock unconditionally; the panel ignoring the refresh result; the holds-a-skill check
  removed; the state and crew refusal removed (through the real sandbox); `refresh-held` dropping
  the lock it took; the ancestor-of-a-root refusal removed; the crew-link refusal removed; the
  ancestor check run over link targets too.
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 ci-local.sh` all passed on `08b4cd3`, `bcda62a` and
  `8a53154`; after § 11.11, `skills_discover` and `reader_app` green with no skip on `ed5c587`.
- **Security review (Opus, on `08b4cd3`): one MEDIUM, fixed:** a linked folder became a whole
  source with no skill in it (`~/.codex/skills/x -> ~/.codex`), and nothing refused another
  assistant's folders; now a target must hold a skill and no source may sit in any assistant's
  state or crew folder. LOW, fixed: `refresh-held` unlocked when the lock was free; notes not
  re-checked by the client; a deny entry's text could reach the screen; quadratic canonicalizing.
  LOW, recorded as a limit in SECURITY: withdrawing in the app needs the reader to start.
- **Verifier (Fable, on `08b4cd3`): PASS**, four should-fixes, done: plugin skills linked inside
  their install; the doc's list of refused sources; SECURITY's limits for an unreadable root; a
  `SKILL.md` that is a link outside the sources shows the note through the reader.
- **Repeat Verifiers (Opus):** on `bcda62a` PASS, three should-fixes, done in `8a53154`: a linked
  `synced` folder pointing at a folder that holds a skill root (`~/.codex`) made it all a source,
  now refused; the crew refusal now follows links (a crew folder or entry linked into a private
  copy); the depth wording. On `8a53154` PASS, one should-fix, done in `ed5c587`: the
  ancestor check compared with link targets too, so entry order could hide synced skills; now it
  compares with the roots only. Noted, accepted: a crew entry linked to home refuses every source
  (fails closed).  On `ed5c587` PASS, no findings.
- **Not verified:** the app's own screens (a walk is owed); a `-D` argument list near 256 sources
  with long paths (300 short ones measured).

## VO11.2c.1: open history files through the listed folder's descriptor (2026-11-02, lane: V1)

- **What:** `voice_ingest::folder::Folder` (new, `builder/ingest/src/folder.rs`) holds one
  descriptor of a folder: `names()` lists it through a fresh `openat(".")` stream, `stat()` is
  `fstatat` on it, `folder()` and `file()` are `openat` with `O_NOFOLLOW | O_CLOEXEC`. Mining
  Claude Code history (`mine_with_progress`, `folders`) opens `~/.claude/projects` once, lists it,
  opens each project folder through that descriptor, lists the folder and opens each session file
  through the folder's descriptor; a claude.ai export folder (`claudeai::read_folder`) counts its
  files and opens `conversations.json` through the descriptor it was already opened with. The
  checks are as before, and in the same order (deny list, names, link, regular file, hard link on
  the opened file); the export's file count still counts a link to a file. `skills/read.rs`'s
  `DirectoryStream`, `errno` and `open_at` moved into `folder.rs` unchanged. A folder swapped for
  a link after it was listed is refused; one renamed after it was opened is still read where it
  went. Docs: the two SECURITY rows (the open residual closed, the deny row's limits exact) and a
  DECISIONS bullet. Brief: `plans/briefs/VO11.2c.1-open-by-folder-descriptor.md`.
- **Measured (both reviewers' probes):** on macOS `openat(O_DIRECTORY | O_NOFOLLOW)` on a link
  answers `ENOTDIR`, not `ELOOP`, so `Folder::folder` calls an `ENOTDIR` a link when `fstatat`
  says the entry is one; on a FIFO it answers `ENOTDIR` at once (no hang).
- **Who built it:** Codex, one dispatch; V1 wrote the ENOTDIR comment and the doc sentences, and
  the review fixes.
- **Tests:** `folder.rs` (names twice, stat with and without following, links refused, a renamed
  folder can't redirect a read); `mine.rs` swap tests at the walk's seam (a project folder
  swapped for a link after listing is refused; a session folder swapped after listing reads the
  listed folder; a session file swapped for a link is refused; a hard link made after listing is
  left out), the hard-link and FIFO tests ported; `claudeai.rs` (an export folder swapped after
  opening reads the opened one; `conversations.json` swapped for a link is refused; the count
  still counts a link to a file). Red seen on the by-path versions for the three swap tests.
- **Mutations (all killed, `cp` backup and `cmp` restore):** session, project and
  `conversations.json` opened by path; `O_NOFOLLOW` dropped (session file, export); two hard
  links allowed; the count without following (Codex's seven); session by path again (V1);
  session, project, export by path and `open_at` without `O_NOFOLLOW` (Verifier's four).
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 ci-local.sh` all passed on `1ddc0b2` (the reader's
  sandbox tests read history and exports through the new code); after the fixes,
  `cargo test -p voice-ingest` (244), `voice-cli` `mine`, `claudeai`, `reader_app`,
  `reader_sandbox`, clippy and fmt green on `0560495`.
- **Security review (Opus, on `1ddc0b2`): no HIGH or MEDIUM.** LOW, all fixed in `0560495`: a
  projects entry gone between the listing and its `fstatat` stopped mining (now skipped, since
  nothing is read; on `main` only a vanished non-folder was ignored, a vanished folder or link
  stopped mining); the deny check resolves links by path while the read goes through the
  folder already opened, so with `~/.claude/projects` a link repointed mid-read a rule written
  only in its resolved form can miss (recorded as a known limit); "renamed" said "after it was
  opened".
- **Verifier (Fable, on `1ddc0b2`): PASS.** Should-fix done: the deny row over-promised a folder
  refusal for an export (an export folder swapped after opening is read where it went). Notes:
  `Folder::open(path, false)` on a link gives `ENOTDIR` (only tests use it); the ENOTDIR to link
  mapping can show the other sentence under a second swap (a refusal either way); Linux
  untested.
- **Repeat Verifier (Opus, on `ca802dc`): PASS.** Should-fix done in `57e5abb` and here: the
  skip's comment and this record said "as before", true only for a vanished non-folder. No test
  covers the skip.
- **Not verified:** Linux; a live walk is not owed (no screen changes).
- **Skills Review:** ✅ tdd-skill (red on the by-path versions, 12 mutations killed) ·
  ✅ security-and-hardening (review, Verifier, every finding fixed) · ✅ source-driven-development
  (macOS `openat` errnos probed, std's `DirEntry::file_type` read by the review) ·
  ✅ plain-language (doc sentences) · ✅ code-quality (helpers moved, not copied)

## VO11.2c.2: the reader starts with only stdin, stdout and stderr (2026-11-02, lane: V1)

- **What:** `voice_agent::spawn` starts a child with `posix_spawn` and
  `POSIX_SPAWN_CLOEXEC_DEFAULT`: its pipes on 0 and 1, `/dev/null` on 2, the folder by
  `addchdir_np`, `SIGPIPE` reset as std does, and an empty signal mask (stricter than std, which inherits it); the parent's ends are
  close-on-exec. `Spawned` waits without a reaper race (EINTR retried, `ECHILD` marks it reaped)
  and its `Drop` kills and reaps a child not yet waited for, so a panic while reading (in the
  progress callback) no longer leaves the reader running. `Reader::spawn` and `Reader::command`
  share one `launch` (argv, environment, folder); `reader_client::run` uses `spawn` and the new
  `supervise` (the old post-spawn steps, deadlines passed in). A password or key rule added after
  a download was chosen now shows the password-or-key sentence on the three export steps, before
  any reader starts (`history::export_allowed`, shared with the choice). Docs: the spike, the
  SECURITY row and a DECISIONS entry; the remaining window (the reader's pipes marked
  close-on-exec a step after `pipe()`, and the app's other children start through std, which
  never closes by default) is a retry bounded by the deadlines. Brief:
  `plans/briefs/VO11.2c.2-reader-only-std-fds.md` (§§ 7 to 9 are the measurement and review
  fixes).
- **Measured (V1):** Rust 1.98's std makes a macOS pipe with `pipe()` then sets close-on-exec in
  a second call, and its `posix_spawn` path never sets `CLOEXEC_DEFAULT` (toolchain source). Under
  the reader profile a program also holds a Unix socket it opens itself at start, the system log
  socket `system.sb` allows (present with `Command` too and with the environment emptied, while
  the app held only fds 0 to 2; `/bin/ls` opens two more of its own), so the test compares
  descriptors by device and inode, both sides through `/dev/fd/<n>` (for a pipe `fstat` and
  `stat` of `/dev/fd/<n>` give different identities on macOS).
- **Who built it:** Codex, four dispatches (the build, §§ 7, 8, 9); V1 ran the sandbox tests
  and wrote the doc sentences on the reverse case.
- **Tests:** `builder/agent/tests/reader_spawn.rs` (sandbox): the reader's descriptors share no
  identity with the app's pipes and sockets and fd 2 is `/dev/null`, with a `Command` control
  that must see an inherited fd 200; spawn and command run the same zsh and print the same folder,
  args and environment; the parent's ends are close-on-exec. `reader_client.rs`: the quiet
  deadline kills, a closed output with no exit is bounded, a panic in progress kills and reaps.
  `reader_app.rs`: a deny rule added after choosing.
- **Mutations (all killed, `cp` backup and `cmp` restore):** no `CLOEXEC_DEFAULT`; an extra
  inherited fd through a file action (Verifier); the kill closure `|| {}`; `wait_for_exit` as a
  plain `wait()`; `Drop` without the kill; the parent ends left without close-on-exec; `/dev/null`
  opened on fd 9 instead of 2; `spawn` started in the home folder.
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 ci-local.sh` all passed on `7160e46` and on
  `4cd7776` (see the land); `VOICE_REQUIRE_SANDBOX=1 cargo test -p voice-agent -p voice-api -p
  voice-cli` green.
- **Security review (Opus, on `7160e46`): no HIGH or MEDIUM.** LOW: the docs said only the app's
  end could leak (fixed); the drift test compared `command` with what `command` is built from
  (replaced by running both). INFO: the cleared signal mask (stricter than std, kept), a child
  reaped elsewhere (hardened), the non-empty check that leaned on the log socket (replaced).
- **Verifier (Fable, on `7160e46`): PASS**, no must-fix. Should-fixes, all done within the
  60-line allowance in files she read: the parent ends' close-on-exec, stderr on `/dev/null` and
  the spawn's folder untested (each now killed by a test); the docs' cause (the window is the
  reader's own pipe step). Not verified by her: the Linux `Unsupported` stub's build, the 1.5 s
  bounds under heavy load (they passed at 2x load).
- **Repeat Verifier (Opus, on `5ad095f`, the delta from `7160e46`): PASS**, no must-fix; her five
  mutations (the spawn's folder, a dropped environment entry, the parent ends' flag, fd 2 on
  `/dev/null`) all killed. Two wording should-fixes, applied: the cleared signal mask is stricter
  than std, and the app's own children are not known (not proven) to write to an inherited pipe.
- **Not verified:** the app built from `main` on a Mac (walk owed by VO11.2c already covers the
  reads); Linux compile of the stub.

## VO11.2c: the app reads history and the claude.ai export through the reader (2026-11-02, lane: V1)

- **What:** on macOS the app's `claude_code_folders`, `read_claude_code`,
  `claudeai_export_status`, `list_claudeai_chats`, `read_claudeai` and the status inside
  `choose_claudeai_export` start the shipped `voice` as `voice reader <step>` under the VO11.2a
  profile, with only that step's source (`~/.claude/projects`, or the chosen download), after
  checking the assistant and permission in the app. `voice_api::reader_client` judges the
  reader's output as untrusted: lines capped at `MAX_LINE` + 64 bytes and the total at
  `MAX_LINE` + 1 MiB, one key per line, progress only on the two reads (relayed as it arrives),
  a last `result` with exit 0 or `error` with exit 1; anything else is the fixed `STOPPED`
  sentence, after a kill and a wait. An `error` line is shown only when it is one of Voice's own
  fixed sentences, else the step's own sentence. A helper thread reads the output; the app waits
  at most 10 minutes between lines, and 5 seconds after the final line and for the exit. A
  download in a password or key folder is refused when chosen, with its own sentence, and nothing
  is saved. The request goes on
  stdin, which is then closed; stderr is `/dev/null`. Run folders are in `reader-runs` beside the
  Claude Code profile. The export choice stores whether it is a folder or a file (VO11.2b note
  a): a wrong kind is refused when chosen, and the reader refuses a changed kind on the opened
  handle (`claudeai::read_export_as`); a choice saved before has no kind and is asked for again.
  A missing `~/.claude/projects` gets its empty answers in the app (one shared zero
  `ReadCounts`, after the lock for the read) without the reader. Other platforms read in the app
  as before. Notes c (pipes, closed stdin, judged by the last line) and the canonical home are
  applied; note b (open by the folder's descriptor) is carded as **VO11.2c.1** (`UbpJwLJl`,
  Backlog), and starting the reader with only fds 0 to 2 (Rust's spawn on macOS sets
  close-on-exec on a new pipe in a second step) as **VO11.2c.2** (`ZNBr0ZhN`, Backlog), both
  documented as known limits. Brief: `plans/briefs/VO11.2c-app-reads-through-the-reader.md`
  (§§ 8 and 9 are the review fixes).
- **Who built it:** Codex, three dispatches (the build, § 8, § 9); her sandbox can't run
  `sandbox-exec`, so V1 ran the sandbox tests and the bypass mutations. V1 moved the DECISIONS
  entry above the file's closing `## Recordings`.
- **Tests:** judge, deadline and exit-wait unit tests in `reader_client.rs`; 8 sandbox tests in
  `builder/cli/tests/reader_app.rs` through the API with the built `voice` (answers equal the
  in-process ones, a hard-linked canary in no answer or state file, progress relayed, no run
  folder left, permission checked before anything starts, changed and old-shape export choices
  refused, a download under `.ssh` refused unsaved, each of the six steps `STOPPED` when the
  program exits 0 with no answer); a kind test in `claudeai_rules.rs` and an in-process
  stored-kind test in `history.rs`.
- **Mutations (all killed, `cp` backup and `cmp` restore):** progress allowed on any step; a
  result accepted with any exit status; `read_export_as` without the kind check; after § 9, the
  Verifier's bypass (`read_claude_code` calling `read_claude_code_in` instead of the reader) and
  the export status bypassed the same way, each failing the six-step test.
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 cargo test -p voice-api -p voice-cli -p voice-ingest
  -p voice-app` all passed on `afe0247`, and `-p voice-api -p voice-cli -p voice-ingest` on
  `da6cd7c` (the 8 `reader_app` tests ran, none skipped); `VOICE_REQUIRE_SANDBOX=1
  ci-local.sh` all passed on `afe0247` and on `da6cd7c`.
- **Security review (Opus, on `afe0247`): two MEDIUM, one LOW.** No deadline (fixed, § 8);
  inherited descriptors through Rust's non-atomic close-on-exec (carded VO11.2c.2, documented);
  the reader's error text shown unfiltered (fixed, § 8). Sound: the bounds, protocol, reaping,
  run folder, home, kind checks, error mapping, public-repo rules.
- **Verifier (Fable, first pass on `afe0247`): FAIL**, one must-fix: the tests didn't prove each
  step starts the reader (her bypass mutation passed all 7). Should-fixes: the deny sentence lost
  for a chosen download, the shortcut calling the in-process readers, no timeout, a double read;
  all fixed in § 9 (`da6cd7c`).
- **Repeat Verifier (Opus, on `da6cd7c`): PASS**, no must-fix; her own bypass (the chat list
  read in the app) killed by the six-step test. Should-fixes: the DECISIONS line on the
  missing-projects shortcut was stale (fixed in this record commit); carried to VO11.2c.2's
  card: the docs leave out the reverse inherited-pipe case (another child holding the reader's
  stdout turns a right answer into `STOPPED` after 5 s); no test pins `run`'s kill closure and
  bounded wait (a `|| {}` or a plain `wait()` would pass); a panic in the progress callback skips
  the kill and wait; a deny-list change after choosing shows `EXPORT_BAD` until chosen again.
- **Walk owed:** read Claude Code history and a claude.ai export through the app.

## VO11.2b.1: the repeat Verifier's two should-fixes (2026-11-02, lane: V1)

- **What:** the two should-fixes VO11.2b's repeat Verifier left. (1)
  `find_matches_list_without_listing_crew` gains `aliased`, a crew folder that is a link to a
  folder named `elsewhere`, so `find` is checked against `list` for a linked folder whose target
  has another name, and then checked to find it at the link's own path. The case-only check at
  its end now asserts `list` holds no `ari` (it asserted an empty list, which `aliased` broke).
  No production line changed. (2) The VO11 spike's reader section said every history step takes
  the state lock; only `read-claude-code`, `list-claudeai-chats` and `read-claudeai` do (they call
  `locked`; `claude-code-folders` and `claudeai-export-status` call `readable`: slug and
  permission only). Brief: `plans/briefs/VO11.2b.1-repeat-verifier-should-fixes.md`.
- **Who built it:** Codex, two dispatches: the brief missed that the case-only check asserted an
  empty list, so § 4 was added after her first run failed it (which also shows the case-only
  branch runs on this Mac's disk).
- **Mutation (killed):** `find`'s condition without `!is_symlink()`, applying the canonical-name
  check to links too, fails the test at the loop's `aliased` comparison (Codex, and V1 by `cp`
  backup and `cmp` restore).
- **Verification:** `cargo test -p voice-core assistants` 10 passed; clippy and fmt clean (Codex);
  `bash scripts/ci-local.sh` all passed on `5563261` (UI 464 tests, build green).
- **Security review (Opus, on `5563261`): PASS**, no findings. Production code unchanged; the
  doc sentence checked against `history.rs` and `reader.rs` (narrower than before, not broader);
  the case-only check still catches a wrongly listed `Ari`. As context, not a finding: `find`
  following a linked crew folder gives nothing extra to whoever can make the link (outside the
  sandbox, as the user, who could edit `member.json` directly), and the linked `member.json`
  still may not be a link, must carry the link's slug, and has its state folder and consent checked.
- **Verifier:** not required (the spec: a test and a doc line).

## VO11.2b: the reader in the CLI (2026-11-02, lane: V1)

- **What:** a hidden `voice reader <step>` (not in `--help`) for the app's six reading steps
  (`claude-code-folders`, `read-claude-code`, `claudeai-export-status`, `list-claudeai-chats`,
  `read-claudeai`, `skills` with list, agree, withdraw or refresh). The protocol is
  `voice_api::reader`: one strict request object on stdin (64 KiB), JSON lines out (`progress`,
  then one `result` or `error`, each at most 16 MiB), fixed sentences for a bad request, a panic or
  a result too big to send; the existing `voice_api` functions do the work, so their checks run
  inside the reader. The strict one-object parser moved from the CLI's `stdin_json` into
  `voice_api::json_object`, shared. Brief: `plans/briefs/VO11.2b-reader-in-the-cli.md`.
- **Made to work under the VO11.2a profile:** the assistant is found by its own folder
  (`voice_core::assistants::find`), since the reader may not list `~/.claude/crew`; the private
  state walk passes through the folders above the state folder with `O_SEARCH` (macOS only).
- **Hard links:** walks leave out files with a second name (sessions and skill files checked on the
  entry and again on the opened file; the two files opened by name, on the opened file): a Claude Code session counts in the never-read number, a `conversations.json` in an
  export folder is refused, a skill file is left out as `a hard link`, and a hard-linked
  `installed_plugins.json` hides plugin skills with the existing note. A chosen export file is read
  whatever its link count.
- **Who built it:** Codex, three dispatches (§ 1–4, § 5–8, the § 10 review fixes). Her sandbox
  refuses sockets and a sandbox inside it, so V1 ran the sandboxed tests outside it, with
  `VOICE_REQUIRE_SANDBOX=1`: all six passed first time, including the no-sources control that
  shows the profile, not the code, refuses.
- **Mutations (all killed, each restored and `cmp`-checked):** no `O_SEARCH`; no hard-link check
  in the session listing, in `open_session`, in the export folder, in the skills walk before the
  open (first survived, since the check after the open caught it; killed by a test with an
  unopenable hard link, § 10.5); `hide = true` dropped; `readable` back on `list`; the line limit.
- **Security review (Opus): no HIGH or MEDIUM**; six LOWs. Fixed: a panic's text on stderr, an
  over-long error with no last line, the case-only folder name. Sent to VO11.2c's spec: store the
  chosen export's kind, open by folder descriptor, close the reader's stdin and judge by the last
  line; informational: state files aren't link-checked (the reader can't make a link there).
- **Verifier (Fable): PASS** with three should-fixes (the plugin list, the docs on which steps
  check permission, the case-only name), all fixed in § 10. **Repeat Verifier (Opus) on
  `2d02c66`: PASS** with two should-fixes, carried to their own card VO11.2b.1 (the run's time ran
  short): a test that a symlinked crew folder whose target has another name is still found, and the
  spike's sentence that every history step takes the lock (only the three reading steps do).
  `ci-local.sh` all passed on `275ca9b` and on `2d02c66`.
- **Not verified:** nothing reads through the reader in the app yet (VO11.2c), and skill
  discovery under the profile with every real root (VO11.2d: a root that exists but isn't a source
  reads as *not permitted*).

## VO11.2a: the reader's profile and launch, as a library (2026-11-02, lane: V1)

- **What:** `builder/agent/sandbox/reader.sb`, a Seatbelt template for Voice's own reader (the
  shipped `voice` command): the system reads of `claude-code.sb`, the program file, the member
  folder read-only, the state and a fresh run folder read-write, no network, no fork, no other
  program, `trustd` closed, the same secrets denies (no login exception). `reader::Reader` fills
  in one read rule per consented source and a metadata-only rule for every folder above each
  allowed path, every path a `-D` parameter, never profile text. Unwired until VO11.2c. Brief:
  `plans/briefs/VO11.2a-reader-profile.md`.
- **Split from VO11.2 at pickup** into VO11.2a to VO11.2d (lane V's spec): the reader is the
  shipped `voice` in a hidden mode (VO11.2b), hard links refused by the reader when found by a
  walk (VO11.2b), the app wired for history and the export (VO11.2c), then skills (VO11.2d).
- **Measured at pickup (Mimi, macOS 27.0.1):** resolving a path inside an allowed folder needs
  metadata reads on every folder above it; metadata-only `literal` rules are enough and still
  refuse the parent's listing and a sibling's content. A descriptor walk from `/` opens those
  folders only with `O_SEARCH` (`O_RDONLY|O_DIRECTORY` is refused), which VO11.2b must apply to
  `builder/ingest/src/private.rs`.
- **Who built it:** Codex, three dispatches (§ 1–5, the § 6 review fixes, the positive control).
  Her sandbox refuses a sandbox inside it, so V1 ran every sandbox test outside it, with
  `VOICE_REQUIRE_SANDBOX=1`. One test bug found that way: the path-in-profile check matched
  `/private` inside the template's `/private/etc`; it now checks the generated rules hold no `/`.
- **Mutations (all killed, each restored and `cmp`-checked):** no ancestor rules (`paths_resolve`;
  a first try that left an empty `(allow file-read-metadata)` passed, because a filterless rule
  allows every path, hence § 6.3); `(allow network*)` (`no_network`); no `env_clear`
  (`environment_is_cleared`); no `trustd` deny (`trustd_is_refused`).
- **Security review (Opus): no HIGH.** MEDIUM: the person's own deny list wasn't applied to
  sources (now a `deny` argument; the OS profile holds only the built-in list, since a name
  pattern can't enter the profile without writing text into it). LOWs: very broad sources, nested
  folders, missing trustd and hard-link tests, an empty metadata rule. All fixed in § 6.
- **Verifier (Fable): PASS with should-fixes** (very broad sources; notes on `/dev/fd`, `/`'s
  listing, the empty rule). **Repeat Verifier (Opus) on `33e0d21`: PASS with one should-fix**, a
  positive control for the hard-link test, added. Her notes for VO11.2b: the caller's deny names
  cover only a source's own path, not files inside a folder source (the reader's own walk must);
  broad folders such as `~/Documents` are accepted, so the caller decides what to offer; give the
  reader only pipes or `/dev/null`.
- **Not verified:** nothing reads through this yet; the reader command and the app come next.

## VO11.1c: every `claude` through the sandbox (2026-11-02, lane: V1)

- **What:** the check's two runs, each call and the sign-in each start their own
  `sandbox::Launch` with their own `egress::Egress` and an environment built from nothing.
  Each launch gets a fresh run folder. The work folder, `provider_override`, the session markers,
  `sign_in_command`, `api_key_set` and `API_KEY_SET` are gone. The sign-in watcher opens the
  checked link once with `/usr/bin/open`, and a refused link or a failed opener kills the child.
  `stop_on_interrupt` lets Ctrl-C stop `voice claude sign-in`. The `test-stubs` feature, enabled
  only from dev-dependencies, runs `#!` stubs directly; `ci-local.sh` checks the CLI and app
  lack it and runs clippy without it. Brief: `plans/briefs/VO11.1c-claude-through-the-sandbox.md`.
- **Measured at pickup (Mimi, Claude Code 2.1.292):** a sandboxed signed-out `-p` leaves
  `claude-<uid>/` in its run folder, so the run folder is fresh per launch, not the work folder
  (DECISIONS 2026-11-02).
- **Who built it:** Codex, two dispatches (§ 1–7, then § 8). Her sandbox can't bind loopback, so
  V1 ran every launching test outside it. V1 removed one stray blank line.
- **§ 8, found by V1's `ci-local.sh`:** two first launches at once raced to create `runs/` (and
  `ensure_profile`'s folders); the loser got `RUN_FOLDER_FAILED`. `AlreadyExists` now falls through to
  the existing private-folder checks. Red before the fix: 10/20 and 20/20 runs; green after: 20/20.
- **Mutations (all killed, each restored and `cmp`-checked):** Fresh → Given(profile)
  (`a_call_runs_tool_less_in_a_fresh_run_folder…`); the link opened twice (`sign_in_opens_the_valid_link_exactly_once…`);
  no `env_clear` on the direct branch (`every_launch_gets_only_its_own_environment`); a fixed proxy
  port (`claude::tests::every_native_launch_uses_its_own_proxy_and_sandbox`); no SIGINT handler
  (`claude_sign_in_stops_the_child_on_ctrl_c`).
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 bash scripts/ci-local.sh` all passed on `a78b66a`'s
  tree. Live: the production-built CLI, with a scratch home and the person's variables poisoned,
  ran real Claude Code 2.1.292's `--version` and `auth status` through the sandbox, reported
  "isn't signed in", and left `runs/` empty.
- **Security review (Opus, on `f3cd706`): PASS**, no HIGH or MEDIUM, four LOWs:
  1. L1: open the link with `/usr/bin/open -u` from `/`, so a URL that matches a planted relative
     path can't open a file.
  2. L2: `--target all` on the three `cargo tree` checks.
  3. L3: test and `--all-targets` builds of the CLI carry the bypass; shipping builds don't. The
     DECISIONS wording should say so.
  4. L4: install the Ctrl-C handler at the top of `claude_sign_in`, so the first status check is
     covered too.

  To take: L1, L2, L4 and the L3 wording. Not taken: sweeping run folders left by a crash.
- **Verifier (Fable, first pass on `f3cd706`):** started, and had not reported when the run's time
  ran out. Nothing landed.
- **Not verified:** a signed-in sign-in and ask inside the sandbox (the walk).
- **Run `100952-r1`, the LOWs (brief § 9, `cce7f00`):** Codex built L1–L4 (`5fdc9cd`): `open -u`
  from `/` behind a new `open_command`; `--target all` on the three `cargo tree` checks; the
  Ctrl-C handler first in `claude_sign_in`, with a stop check after the first status; the DECISIONS
  wording. V1 moved one `use` line above the new `let`. Codex ran fmt, both clippy runs, the
  `--all-targets` build and the three `cargo tree` checks (the dev one still finds `test-stubs`, the
  two shipping ones don't). Her sandbox can't bind loopback, so the new Ctrl-C test is **not yet
  run**, and neither is its red check against the old `main.rs`.
- **Verifier (Fable, first pass on `cce7f00`): PASS**, no must-fix. Claims 1–5 verified by
  mutation; L2 observed to catch a Linux-only dependency that the old check missed. One should-fix:
  deleting `opened = true` fails no test. Briefed as § 9 item 5 (`0b2f09f`), not yet built.
- **Held:** the resource gate held on swap-thrash (fseventsd at about 30 GB) before the tests could
  run.
- **Run `103238-r1`, the tests and item 5:** V1 ran `cargo test -p voice-cli --test agent` (16/16)
  and `the_browser_opener_uses_a_url_from_the_root_folder` (pass). Red check: with `cce7f00`'s
  `main.rs`, `claude_sign_in_stops_on_ctrl_c_during_the_first_check` fails on
  `out.status.code().is_some()` (the CLI dies by SIGINT); restored, `cmp` identical. Codex wrote
  item 5's test, `sign_in_never_opens_a_second_valid_link` (`a210848`, tests only); her sandbox
  can't bind loopback, so V1 ran it: `--test profile` 16/16, and with `opened = true` deleted it
  fails `left: 2, right: 1` (restored, `cmp` identical). fmt and clippy clean.
- **Repeat Verifier (Opus, on `a210848`): PASS**, no must-fix, no should-fix. She checked that
  `-u` from `/` and `valid_link` leave no file or option route to `open`, that no Ctrl-C window
  leaks a run folder or proxy, the three `cargo tree --target all` checks by hand, and that
  item 5's test can't pass vacuously.
- **Security review (Opus, on `a210848`'s delta): no HIGH or MEDIUM.** One LOW, already accepted
  in brief § 9 item 3: Ctrl-C during a hung first check waits out up to two `CHECK_TIMEOUT`s
  (about 60 s) and starts the `auth status` run after the stop; cleanup holds. A follow-up could
  pass `stop` into `status`.
- **Verification:** `bash scripts/ci-local.sh` all passed on `a210848`'s tree (throttled, 2 jobs).
- **Not verified:** a signed-in sign-in and ask inside the sandbox, through the app (the walk).
- **Skills Review:** ✅ tdd-skill (both new tests red on their assertions, then green) ·
  ✅ security-and-hardening (LOWs taken, review and repeat Verifier clean) ·
  ✅ source-driven-development (`man open`, `cargo tree --help`) · ✅ code-quality (tests only in
  the last commit; production unchanged)

## VO11.1d: the egress proxy's should-fixes (2026-11-02, lane: V1)

- **What:** the four should-fixes VO11.1a's repeat Verifier left, in `builder/agent/src/egress.rs`
  and `tests/egress_descriptors.rs`. (1) The listener is non-blocking and the accept loop polls
  the stopped flag every 10 ms, so `stop()` makes no wake connection and returns even when the
  process is out of descriptors with nothing of the proxy's to free; each accepted socket is set
  back to blocking (macOS's `accept(2)` gives it the listener's properties; Linux's does not).
  (2) A Drop guard gives a resolver's slot back when it returns, panics or never starts.
  (3) A no-connection case: `stop()` with no client and the descriptor table full; the file's two
  tests take one `Mutex`, since the limit is process-wide. (4) `one_way_traffic` runs at
  `idle` 1.2 s for 4 s. Brief: `plans/briefs/VO11.1d-egress-should-fixes.md`.
- **Who built it:** Codex, one dispatch; her sandbox refuses loopback binds, so V1 ran every
  socket test outside it. V1 reflowed one doc comment.
- **Red:** on `origin/main`'s `egress.rs`, the new descriptor test fails, *stop hung with no
  connections or descriptors*, at 3 s (V1, and the Verifier 1 of 1, the security review 3 of 3).
  The guard's test failed on the count (1, not 0) before the guard (Codex, in her sandbox).
- **Mutations (`mutate-check.sh`, all killed):** the listener left blocking (both descriptor
  tests); an accepted socket left non-blocking (`header_deadline_is_absolute`,
  `connection_slots_are_released`); the guard without its decrement
  (`panicking_resolver_gives_back_its_slot`); one idle clock per direction (`one_way_traffic`).
  No test for a failed thread spawn (it can't be made on demand).
- **Verification:** `cargo test -p voice-agent --lib egress` 19 passed, 1 ignored;
  `egress_descriptors` 2 passed; clippy and fmt clean; `VOICE_REQUIRE_SANDBOX=1 ci-local.sh` all
  passed on `695c763`.
- **Security review (Opus, on `695c763`): PASS**, no HIGH or MEDIUM. Measured: an accepted
  socket inherits `O_NONBLOCK` on this Mac, and would fail closed in `header()` if one ever
  stayed so; 10 idle proxies used about 0.1% of a core each; 10 proxies stopped in 67 ms.
- **Verifier (Fable, first pass on `695c763`): PASS**, no must-fix. 32 runs of the egress tests
  at 4 and 16 threads and 13 of the descriptor tests at load 13 to 14, all green; the four
  mutations re-run and killed. Both left one nit, not taken: a socket whose
  `set_nonblocking(false)` fails is dropped without a stopped check (the next poll sees the flag).

## VO11.1a: the egress proxy (2026-11-02, lane: V1)

- **What:** `builder/agent/src/egress.rs`, an HTTP `CONNECT` proxy in `voice-agent`, one per
  `claude` launch on `127.0.0.1:0`, std threads only, no new dependency: `CONNECT <host>:443`
  only for `api.anthropic.com`, `platform.claude.com`, `claude.ai` (ASCII case ignored; no
  trailing dot, IP literal, user info or other port); the request line and headers within one
  10 s deadline and 8 KiB; 16 connections (`503` past that); the name resolved once on a helper
  thread inside the 10 s dial deadline, refused if any address is not public (IANA special
  ranges; IPv6 only inside `2000::/3`, minus `2001::/23`, `2001:db8::/32`, `2002::/16`,
  `3fff::/20`; mapped and NAT64 `64:ff9b::/96` judged by their IPv4), each address a share of
  the time left; resolvers in flight capped at 16; a tunnel closed after 5 minutes with no byte
  either way (one clock for both directions); `stop()` and `Drop` close everything and join.
  Not wired in yet (VO11.1c). Brief: `plans/briefs/VO11.1a-egress-proxy.md` (§ 9 and § 10: the
  fixes after review).
- **Who built it:** Codex, in three dispatches. Her sandbox refuses loopback binds, so every
  socket test failed at bind for her; V1 ran them all outside it.
- **Found by V1 on the first build:** one dead address ate the whole dial deadline (measured:
  `127.0.0.2` hangs to its timeout on macOS); a failed `accept` ended the proxy for good; a test
  unwrapped a shutdown on a reset socket. All fixed in § 9.
- **Mutations by V1 (cp, cmp, fresh mtime), all killed:** the dial share, the absolute header
  deadline, the pipelined bytes, the slot limit, the idle close, the shared idle clock, the stop
  sweep, `Drop`, the public check, the bounded resolution, the host list, the live host list
  (`production_network` fails without `api.anthropic.com`), stop() back to one wake, the sweep
  before the join, the resolver cap. No test for the `EINTR` retry or the accept-error wait
  (neither can be made on demand).
- **Live proof:** `production_network` (ignored by default) reached `https://api.anthropic.com/`
  through `Egress::start()` and was refused `https://example.com/`.
- **Verifier (Fable, first pass on `99637d3`): PASS**, no escape found (17 tests green 12 times
  at 16 and 32 threads under load; parser edges, a blocked write and a hanging dial under stop,
  40 connections against a limit of 2). Her should-fixes are § 10.
- **Security review (Opus, on `99637d3`): FAIL**, one MEDIUM, measured: out of descriptors,
  stop()'s single wake connect failed and the join hung forever. § 10 sweeps the sockets before
  and after the join and retries the wake (`tests/egress_descriptors.rs`). **Re-check on
  `4a2cb71`: PASS** (stop() returned in 212 ms with the table still full; the resolver cap
  measured not to leak). LOW left: a panicking resolver would not give back its cap slot (it
  can't panic today); VO11.1c or the app should raise the soft `RLIMIT_NOFILE`.
- **Verification:** `cargo test -p voice-agent --lib egress` 18 passed, `egress_descriptors`
  passed, clippy and fmt clean on `4a2cb71`; `VOICE_REQUIRE_SANDBOX=1 ci-local.sh` all passed on
  `99637d3`. On `4a2cb71`, one `ci-local.sh` run failed in `--test profile`
  (`stopped_sign_in_kills_the_process_group_within_a_second`, a file not found in a stub), a test
  whose files the branch doesn't touch; on `eeb72f4` it passed 5 times alone, and
  `VOICE_REQUIRE_SANDBOX=1 ci-local.sh` all passed (V1, run `062159-r1`), so a flake under load
  ~20, not traced.
- **Repeat Verifier (Opus, on `4a2cb71`): PASS**, no must-fix. Her four should-fixes (a
  non-blocking accept, a Drop guard for the resolver count, a deterministic no-connection test, a
  wider `idle` in `one_way_traffic`) are VO11.1d.

## VO11.1b: the sandboxed launch, as a library (2026-11-01, lane: V1)

- **What:** VO11.1 split in three (V1, run `190933-r1`): VO11.1a the egress proxy, VO11.1b this,
  VO11.1c the wiring. `builder/agent/src/sandbox.rs` builds one `/usr/bin/sandbox-exec` launch of
  the person's own Claude Code: the profile compiled in and passed with `-p`; the program with
  links resolved and refused unless Mach-O (`text::NOT_NATIVE`; an older npm script install's
  `node` is outside the sandbox; current npm installs are the native binary, per
  `https://code.claude.com/docs/en/setup`); a fresh 0700 run folder under `<app folder>/runs`, or
  the caller's work folder; an environment built from nothing (`HOME`, `USER`, `LOGNAME`, a fixed
  `PATH`, a checked `LANG`, the profile and run folders, the proxy, telemetry and the updater off,
  `BROWSER` only while signing in); `--setting-sources project,local` before any subcommand; off a
  Mac, `text::MAC_ONLY`. The sign-in link: Claude Code runs `$BROWSER <link>` with the
  localhost-callback link, so `BROWSER` is a fixed `#!/bin/bash` helper in the run folder writing
  it to `sign-in-link`, which the library reads once and checks. Not wired in yet (VO11.1c).
  Brief: `plans/briefs/VO11.1b-sandboxed-launch.md` (§ 8 and § 9: the fixes after review).
- **The profile:** `process-exec` narrowed to the program (`PROGRAM`), plus the helper and
  `/bin/bash` while `SIGN_IN=yes`; reads of the program file, not its folder; `trustd` denied.
- **Measured by V1 on Mimi** (macOS 27.0.1, Claude Code 2.1.292, signed out, throwaway folders):
  `--setting-sources` after `auth status` is refused (`unknown option`), before it works; with the
  narrowed profile `--version`, `auth status`, a signed-out `-p` and an `auth status` reading an
  invented `.credentials.json` (`loggedIn: true`, so no `/usr/bin/security` needed) all work; a
  `-p` with an invented token reached `api.anthropic.com` through the probe's proxy and got
  `401 Invalid bearer token` with `trustd` denied, while `nscurl` fails certificate checks under
  the same deny; the link Claude Code prints is the paste-a-code one, the one it hands `$BROWSER`
  is the localhost-callback one; `/bin/sh` is a shim needing `/private/var/select/sh`, hence
  `#!/bin/bash`; the client id in that link is a constant in the binary. Token and roles hosts from
  the binary: `platform.claude.com/v1/oauth/token`, `api.anthropic.com/api/oauth/claude_cli/roles`.
- **Who built it:** Codex, in three dispatches (the brief; § 8; § 9). Her sandbox refuses a nested
  one, so the profile and live launch tests skipped for her and V1 ran them
  (`VOICE_REQUIRE_SANDBOX=1`).
- **Security review (Opus sub-agent, on `9f25552`):** one HIGH, fixed in `b41663d`: a child could
  rename its profile (or run) folder away and leave a symlink to another 0700 folder, which the
  next launch accepted (now the profile denies moving the two folders and `Launch::new` refuses a
  link given in their place). MEDIUMs fixed: a FIFO at `sign-in-link` blocked the reader
  (`O_NONBLOCK`); the link now must carry Claude Code's own client id, `response_type=code` and a
  `http://localhost:<port>/callback` redirect. LOW fixed: the child can't overwrite the helper.
  Recorded for VO11.1c: descriptors the app holds without close-on-exec pass to the child.
- **Verifier (Fable, first pass on `9f25552`):** FAIL on the FIFO and the folder swap (both fixed
  in `b41663d`), nothing else at must-fix. She ran 11 mutations (10 red on their assertions; the
  `trustd` deny's removal survived), a live signed-out launch (link `Ready` in 386 ms, not opened)
  and paths with spaces and quotes. Should-fixes in brief § 10, not yet built: the helper left in
  a `Given` folder, the child's `chmod` of its folders, a `trustd` test, the probe's `LANG`, a
  flaky 500 ms wait in `only_sign_in_can_listen` (failed once under load at 241%, passes alone).
- **Fix-ups by V1:** the § 9 dispatch was stopped mid-mutation at the run's end, with
  `decode_query_value` in a mutated state; V1 restored the two lines to the § 9.3 rule (`?` on the
  byte and the digit) and the link tests passed (17).
- **Verification:** `VOICE_REQUIRE_SANDBOX=1 cargo test -p voice-agent` all green on `9f25552`;
  on `b41663d` `launch` 17 and `sandbox_profile` 14 of 15 (the flaky one above); probe tests 37
  then the § 9 set OK; `cargo fmt --check` clean; clippy green in Codex's run (18 min, workspace).
  **Not yet:** § 10, `ci-local.sh`, the repeat Verifier (Opus) on the final head, the land.
- **§ 10 and § 11 (V1, run `233604-r1`, code `752a97f`, brief `fd61093`):** Codex built § 10; her
  run died on a dropped connection mid-mutation and left Drop's `sign-in-link` removal deleted.
  V1 found it by comparing the tree with her last printed diff and restored it. V1's mutations
  outside the sandbox (cp, cmp): the folder-mode deny, the whole `trustd` deny and each of its two
  names, Drop's helper and link removals, and cleaning before the helper is made were all killed
  by their tests; one survived: a plain launch also cleaned a `Given` folder, so it could delete a
  sign-in's helper running beside it. § 11 (Codex) fixed that, red first, and both its mutants
  were then killed. `VOICE_REQUIRE_SANDBOX=1 cargo test -p voice-agent` passed all 17 binaries.
- **Repeat Verifier (Opus, on `752a97f`): FAIL**, two blockers, both measured: a child can add an
  access list to its folders (`chmod +a`), opening them to other users while the mode reads 0700
  (the security review on the same delta found it too, plus `chflags uchg`); and the link check
  compares query keys raw, so `client%5Fid` sits beside the pinned `client_id`. Should-fixes: a
  folder that already carries an access list is accepted; an old `sign-in-link` in a `Given`
  folder is handed out; two exit codes asserted loosely. All six are brief § 12, not yet built.
- **§ 12 (V1, run `011923-r1`, code `1e6b3f7`):** Codex built all six. V1's 11 mutations outside
  the sandbox (each of the three new profile names removed, the ACL refusal off, the ACL type
  zeroed, the key check removed or loosened in Rust and Python, the old link kept) were all
  killed; the exit codes 4 and 126 held under the real sandbox. **Repeat Verifier (Opus) and the
  security review: FAIL, the same HIGH found twice and measured:** `setattrlist` (mode 0777,
  group `everyone`, an access list) still changed both folders past the four operation names, so
  a child could open the login file to every account. The Verifier also caught a stale build:
  V1's mutation restore (`shutil.copy2`) kept the files' old mtimes, so cargo kept the last
  mutant's library; the `ci-local.sh` started then was void and is not counted.
- **§ 13 (code `fd2cf10`):** one `deny file-write*` on the two folder literals replaces both
  folder rules; a `setattrlist` regression test with an `(allow default)` control; a fresh run
  folder goes through `private_folder`; a folder another account owns is not private. V1: real
  sandbox `launch` 25 and `sandbox_profile` 18 green; 5 mutations killed (the owner check's
  removal survived, untestable then); the real Claude Code 2.1.292 still runs signed out under
  the profile (probe `status`). **Repeat Verifier (Opus): PASS** (37 ways to change a folder's
  own metadata all refused, move and replace still refused, contents writable). Security review:
  the HIGH and both LOWs closed; one LOW left, the `runs` parent not checked, which was also a
  Verifier should-fix.
- **§ 14 (code `9074221`):** the `runs` parent goes through `private_folder`; the owner check is
  tested against root-owned `/private/var/audit` (it skips if that folder isn't root-owned 0700
  with no access list; it would panic, not skip, if the folder were missing, which a stock macOS
  never is). V1's 2 mutations (owner check removed, parent check removed) killed; real sandbox
  `launch` 27 and `sandbox_profile` 18 green.
- **Delta Verifier (Opus) on `9074221`: PASS.** Should-fix left: the owner test fails instead of
  skipping when run as root. Security review of the § 13 and § 14 code: no finding open.
- **Verification at the land:** `VOICE_REQUIRE_SANDBOX=1 bash scripts/ci-local.sh`: all passed on
  `9074221`. **Not verified:** the `SIGN_IN=yes` profile against a real sign-in (signing in needs
  the person); Linux and Windows (the library refuses off a Mac).
- **For VO11.1c:** a folder with any access list is refused, even a deny-only one such as macOS
  puts on `~/Documents`, so don't offer one as `Given`; a sign-in should use `Fresh`.

## VO1: prove the read-wide fence, the probe (2026-11-01, lane: V1)

- **What:** `scripts/read_fence_probe.py`, the instrument that proves a read-wide `claude -p`
  turn's fence on a person's own Mac, and its offline tests. `check` (no files; needs no sign-in)
  shows the turn is offered only Glob, Grep and Read, no MCP server, the default mode, that the 34
  deny rules were applied, and that a broken settings file is caught. `run` (signed in, a few
  Haiku turns) adds invented canaries for every deny-list entry inside the granted folder and
  passes only when each read is denied in `permission_denials`, no canary's word reaches any
  fenced turn's output, and a whole-folder Grep's own result holds only the control word. The
  flags and rules VO6 copies, what was measured, and what is left out: `docs/spikes/VO1-read-fence.md`
  and DECISIONS. Brief `plans/briefs/VO1-read-fence-probe.md`.
- **The live runs are attended.** This run's nested `claude` reads `loggedIn: false` and no token
  reaches it (as X-Q124 and X-Q135 found), so `run` on Mimi and on Teo are the person's (V-Q11).
  The card is Parked on them; nothing relies on the fence until both pass.
- **Who built it:** Codex, test first (`06e6a5a`), then three review rounds (`e204b1f`,
  `c0b09e5`, `d67301e`).
- **Verified:** 26 offline tests on Python 3.9.6 and 3.12. Live, signed out: `check` printed every
  line `pass` and exited 0 (on Mimi, this Mac's own settings bring 18 hooks and 7 plugins into
  every turn, reported by count); `run` stopped at *Sign in to Claude Code first*. Measured by V1
  before the brief, signed out: the `init` event's `tools`, `mcp_servers` and `permissionMode`
  (`manual` shows as `default`); the debug log's `flagSettings` line for a valid settings file and
  nothing for an invalid one. Mutations by V1 (`cp` backup, `cmp` restore), all 20 caught: the rule
  list not compared, a leaked word ignored, a later try's leak ignored, the broken check always
  passing, a fenced turn skipping the guard, an API key passed to the child, extra tools allowed,
  words not scrubbed, full-path symlink matching, the broken check's tautology, a glob leak not
  counted, the relative rules beside the folder, the Grep path unchecked, the Grep filters unchecked, each turn's
  offer unchecked (read, grep, glob), the control word read from the whole stream, an error
  result accepted, a Grep refusal ignored. `ci-local.sh`: see the land.
- **Reviews:** security review (Opus): two MEDIUM and four LOW, all fixed (brief § 4): symlink
  canaries passing untried; the person's own settings (hooks, `env`, `apiKeyHelper`, plugins) still
  reaching the turn, now reported by count and left to VO6; a bounded second read; SIGTERM and
  SIGHUP clean-up; the glob turn scanned; bidi and zero-width controls removed. Verifier (Fable,
  first pass, `06e6a5a`): FAIL, three should-fix (the symlink false pass; the broken-settings
  check true by construction; the relative rules' file anchored above the folder), fixed in
  § 4. Repeat (Opus, `e204b1f`): FAIL, the Grep counted on any call (§ 5). Repeat (Opus,
  `c0b09e5`): FAIL, the control word from another call (§ 6). Repeat (Opus, `d67301e`): **PASS**.
  Two notes accepted: a glob turn that times out isn't counted as failed (a leak there could go
  unseen only if a process outside the killed group held the pipe), and the Grep refusal check
  doesn't test that `permission_denials` is a list (a `None` crashes, fail-closed).
  Records Verifier (Opus, `9fe61eb`): two one-line fixes in the spike doc, made: the `init` event
  isn't always the first line (hooks' events come first), and a private-tool word in a public doc.
- **Not verified:** everything that needs a signed-in Claude Code: whether the `//**/` rules
  actually deny each canary, whether Grep is fenced inside the folder (the docs promise only
  "best-effort"; it may well not be), letter case, symlinks, and the `permission_denials` shape
  (a different shape fails closed as *refused another way*). Teo. Linux.

## VO4.2: refresh the skill mirror at convene (2026-11-01, lane: V1)

- **What:** before a panel's first round, in `voice panel convene`, in a `resume` that will run a
  round, and in the app (all through `voice_panel_runner`), each seat whose mirror is in use
  (`voice_ingest::skills::mirror_in_use`: agreements that name a skill or can't be read, or a
  mirror that is there or can't be checked) is refreshed with VO4.1's `refresh`, under the seat
  state locks the panel already holds. A failed refresh drops its error unread and the panel goes
  on; its progress gets one fixed line before "Round N is starting.": "Voice couldn't bring
  <name>'s copy of your skills up to date, so the panel goes on without it. To see why, run voice
  skills refresh in a terminal." No panel turn reads the mirror yet. Brief
  `plans/briefs/VO4.2-refresh-at-convene.md`.
- **Who built it:** Codex, test first (`533db1f`), and the review fix (`c76d735`). Its own `ci-local.sh` stopped at the sandbox
  profile tests (a sandbox can't start inside Codex's sandbox); V1 ran it outside: all passed.
- **Verified:** `bash scripts/ci-local.sh` all passed at `533db1f`. Mutations by V1 (`cp` backup, `cmp`
  restore), all caught: no refresh at convene (3 red), none at resume (1 red), resume's refresh
  before its checks (the refused resume, 1 red), `mirror_in_use` always true (2 red), a mirror
  folder that can't be checked counted as absent (1 red).
- **Reviews:** security review (Opus): one LOW, fixed (brief § 5, `c76d735`): the line showed the
  assistant's name raw, and nothing checks it when it is read (`private.json` can override it), so
  escapes or a newline could fake progress lines. `skills_not_refreshed` now cleans it to one line,
  cut to 64 characters plus an ellipsis (V1's mutation, the newline kept, 1 red). Its note: a panel start can
  now trigger the mirror's known deep-link limit, so the SECURITY row says so.
  Verifier (Fable, first pass, on `b806027`): PASS. One LOW accepted: only the `NEED_STEER`
  refusal pins "a refused resume refreshes nothing"; the resume that only closes an expired panel
  has no test (it needs a time box that has run out, which the app-level tests can't fake). Nits:
  the 64-character wording (fixed here); the name shows a private term as-is, as
  `assistant_problem` already does.
- **Not verified:** Linux; running as root (the failed-refresh tests skip there); the line in the
  running app (the panel screen shows every progress line as it comes; no screen change).

## VO4.3: the skills screen (2026-11-01, lane: V1)

- **What:** a "Your skills" card on the Save page (`screens/Skills.vue`, words in
  `screens/skills/words.ts`), per assistant: every skill on this computer with its state, those
  waiting for a yes first; a yes or a stop per skill; and "Bring the copy up to date", whose
  report shows as plain text. Four app commands in `builder/api/src/skills.rs` (`list_skills`,
  `agree_skills`, `withdraw_skills`, `refresh_skills`) call the same library as `voice skills`,
  under the same lock, with no history permission (they never read the history). Withdraw's mirror
  removal is now one shared step, `voice_ingest::skills::remove_after_withdraw`; the CLI's output
  is unchanged. The discovery notes are ingest consts, and the screen keeps only those three.
  Brief `plans/briefs/VO4.3-skills-screen.md`.
- **Who built it:** Codex, test first: the api, app and parity (`2e368ed`), then the screen
  (`85e838d`, brief § 10: red at assertions first, with `node_modules` installed). Codex changed
  one UI error fixture (`private detail` → `hidden backend diagnostic`), because the intro itself
  says "private details".
- **Reviews:** security review (Opus): no findings; it noted that the attribute-binding allowlist in
  `plain-text.test.ts` covers only the panel screens. That is on purpose (panels show model
  output); the skills screen is covered by the no-`v-html` scan and the literal-`<img>` test.
  Verifier (Fable, first pass): PASS, one LOW: when a choice is saved but reading the list again
  fails, the screen says it couldn't look through the skills; accepted and recorded in SECURITY.
  Her note that skill places now cross to the webview is in SECURITY's webview row.
- **Verified:** `bash scripts/ci-local.sh` all passed at `85e838d` and again with these records.
  Mutations by V1 (`cp` backup, `cmp` restore), all caught: the stale refresh failure mapped to the
  generic sentence (2 red), the waiting-first sort dropped (1 red), agree sending the skill's
  name instead of its id (4 red).
- **Not verified:** the card in the running app (a walk is owed, both themes); Linux.

## VO4.1: the skill mirror and its hashes (2026-11-01, lane: V1)

- **What:** `voice skills refresh <member_dir>` copies every skill the person agreed to, through
  VO3's engine, into the assistant's private mirror `~/.claude/state/<slug>/mirror/`, with
  `.manifest.json` holding the SHA-256 of each copied file (the copies only). Every refresh starts
  from nothing; the report adds counts since the last refresh (new, updated, unchanged, removed),
  never a removed skill's name. Withdrawing a skill removes the mirror. `voice anonymize --out`
  shares the copying loop and writes the manifest too. `voice forget` already deletes it. Brief
  `plans/briefs/VO4.1-skill-mirror.md`; § 8 holds every review round. DECISIONS has the rules.
- **Who built it:** Codex, test first: the mirror (`a187de5`), then the fixes after each review
  round (`2984159`, `0b30b89`, `50c7f40`, `2b7ddbe`, `4397690`).
- **The rounds** (each finding reproduced before it was briefed):
  - First security review (Opus): 2 medium (a refresh that couldn't remove the mirror still said
    it had; withdraw left the copy until the next refresh), 2 low. First Verifier (Fable): PASS,
    one should-fix (a term in the assistant's own name printed in the mirror's path). → § 8.1–8.3,
    and V1's § 8.5: no message tells the person to delete a folder holding their own files.
  - Repeat Verifier (Opus) FAIL and a second review: error paths printed the real path, a stale
    stamp, withdraw's `?`. → § 8.6 (`0b30b89`).
  - Verifier PASS, third review MEDIUM: an overlap refusal emptied a project the person had listed
    inside the mirror. → § 8.7 (`50c7f40`): `check_output` / `finish_output`, `drop_manifest`,
    the whole state folder masked.
  - Delta Verifier (Opus) FAIL and review HIGH, the same bug one step earlier: a refresh failing
    before the overlap check (bad `private.json`, unreadable `placeholders.json`) still emptied it.
    → § 8.8 (`2b7ddbe`): such a failure leaves every file, drops only the manifest, and says the
    copy is out of date; § 8.8.1 kept the removed-copy sentence tested.
  - Delta review on `2b7ddbe`: HIGH fixed, 3 LOW. → § 8.9: never claim "marked out of date" when
    the manifest stayed; nothing a skill root lists (a root itself, a deny-listed or non-skill
    folder, `synced`, a plugin entry) may sit inside the mirror when it is emptied (`4397690`).
    V1's review added § 8.9.5: hidden entries too, and a deny-listed project to prove the roots are
    checked both ways (a mutation of that check had survived). LOW 3 accepted (SECURITY).
  - Delta Verifier (Opus) on `4397690` FAIL, narrowly, reproduced: a deny-listed listed project
    (`~/vault.key`) was skipped before `protect`, so a link in its `.claude/skills` into the mirror
    was emptied. → § 8.10 (`57c7042`): `protect` runs first in the project loop; tests for
    `anonymize --out` into the mirror and a synced account's entries; the overlap hint reads right
    both ways. Plugin skills folders recorded with the depth limit (SECURITY). Repeat Verifier
    (Opus) on `9b3ec61`: PASS. Delta review: no HIGH or MEDIUM; both found one LOW (an unreadable
    listed project's skills folder now stops `list`/`agree`/`withdraw`/`anonymize` too, with its
    real path; fails safe, same as an ordinary project's already did), recorded in SECURITY.
    Left as nits: withdraw's overlap line still says to delete the folder after moving things out;
    refresh reports any `check_output` error as an overlap.
- **Verified:** `bash scripts/ci-local.sh` all passed at `a187de5`, `2984159`, `0b30b89`,
  `50c7f40`, `2b7ddbe`, `4397690` and `9b3ec61`. Mutations by V1 (`cp` backup, `cmp` restore): 11 before § 8.8, all
  caught; § 8.8's four (each pre-check error back to `Other`, the raw slug shown, the manifest
  kept), all caught; Codex's for § 8.8.1 (the removed sentence) caught; § 8.9's: protected
  entries ignored (5 red), `drop_manifest` always `true` (2 red), roots one way (survived, so
  § 8.9.5's test; then red), `protect` dropped from the roots (5 red); § 8.10's three
  (`protect` after the deny-list skip, `&[]` in `anonymize`, no `protect` on synced accounts),
  all caught.
- **Not verified:** Linux; running as root (the mode tests skip there); one `claudeai_parity` failure under heavy load in the Verifier's first run (passed alone and on rerun; cause not checked).
- **For VO4.2:** a mirror without Voice's stamp and a valid manifest is out of date: never read
  it. Refresh first, as the spec says.

## VO2: Persona Builder is Voice (2026-10-31, lane: V1)

- **What:** the app is Voice everywhere: the `voice` command, crates `voice-core` (`voice_core`)
  and `voice-<x>`, `Voice.app`, `@voice/screens` with `VoiceApp`, `VOICE_*` variables, the bundle id
  `dev.voice.desktop`, and every name on disk (brief `plans/briefs/VO2-rename-to-voice.md` § 1).
  Nothing is migrated (DECISIONS, 2026-10-31). This file keeps its name and its older entries
  keep the old names.
- **Beyond a rename:** turning on now keeps rules in `~/.claude/voice/assistants/<slug>/` and
  the record in `~/.claude/voice/installed.json`, beside Panels' `~/.claude/voice/panels/`. A
  plain rename to `~/.claude/voice/<slug>/` would have put an assistant named "Panels" inside the
  panels' folder, and once turning on had created `~/.claude/voice` itself, every later refresh
  would fail "left in place" as soon as a panel existed. The shared root is created 0700 and never
  counted as the install's. The consent wording names Voice, so its version is 2.
- **Commits:** `558d770`, `cd5891e` brief; `b5438ac` code rename (Codex, 282 files); `b7d6ed1`
  docs (Codex, 64 files; three record-path lines restored by hand); `8974b92` the shared root and
  consent version (Codex, test first); `4e8401f` brief § 5 and DECISIONS; `5c4467b` the security
  review's fixes (Codex, test first); `ef0e0a1` brief § 6 and `b022cfe` the Verifier's fixes (Codex, test
  first: the record's refusals ignore letter case, as a Mac's disk does; a leftover
  `voice/assistants` must be private too).
- **Verifier:** first pass (Fable) PASS, high, at `2d75914`, after reproducing every claim live; her
  should-fixes became `b022cfe`. Repeat pass (Opus) PASS, high, at `b022cfe`: every recased,
  dotted, doubled-slash and Unicode spelling of a record entry refused, live. **Open should-fixes,
  messages only:** the new `assistants` check runs before the record is read, so a `~/.claude/voice`
  that is a file now reads `…/voice/assistants: Not a directory` instead of `…/voice is not a
  folder`, and a symlinked root with a public `assistants` gets the chmod hint before the symlink
  refusal (both still refuse and write nothing; fix: read the record first). The privacy checks
  here and in Panels look at mode bits, not macOS ACLs.
- **Review of the mechanical part:** every removed line was put through the name map and
  compared, token for token, with the added lines of its file. The only differences were Cargo
  dependency names (`voice-core`), rustfmt moving imports, the consent digest, and the two moved
  Vue files (identical after the map). The Cargo.lock diff changes names only.
- **Security review** (sub-agent, Opus): 0 high, 1 medium, 6 low. Fixed: the installer deleted any
  `Voice.app` already in `~/Applications` (now only one with this app's bundle id); turning on
  accepted a `~/.claude/voice` others could open (now refused, as Panels refuse it); a hand-edited
  record could claim the shared root or a panel's file (now refused). Refuted: the VO11 sandbox
  test still reading the old variable (it reads `VOICE_REQUIRE_SANDBOX`). Accepted, not fixed:
  `Application Support/Voice/` and the password-store service `voice.github` are generic names
  another app could share (same user, no privilege boundary crossed; `ensure_profile` makes an
  existing `Voice/` folder 0700). Left on testers' Macs: see below.
- **Verified:** red first on every new test (eight in `8974b92`, six in `5c4467b`); a mutation that
  records the shared root as the install's failed five tests and was restored (`cmp`);
  `bash scripts/ci-local.sh` all passed at `8974b92`, `5c4467b` and `b022cfe`. Live, with the built `voice` in a throwaway
  home: `new`, `consent agree`, `on` wrote `voice/assistants/wren/RULES.md` and
  `voice/installed.json` with the root at 0700 and the CLAUDE.md line
  `@~/.claude/voice/assistants/wren/RULES.md`; with a panel folder added, `off` removed both and
  kept `voice/` and `voice/panels/`.
- **Not verified:** the app built as `Voice.app` and opened (owed walk); Linux and Windows builds.
- **Left for an attended pass:** the GitHub OAuth app behind the shipped client id may still be
  called Persona Builder on GitHub; the app now says Voice. On a Mac that ran an older build:
  `security delete-generic-password -s persona-builder.github -a persona-builder` (and revoke that
  token on GitHub), delete `~/Library/Application Support/Persona Builder/` and
  `~/.claude/persona-builder/`, and remove any `<!-- persona-builder:` block from
  `~/.claude/CLAUDE.md`. Mimi has none of these (checked 2026-10-31).

## VO3: the anonymizer engine (2026-10-31, lane: V1)

- **What:** `persona-builder skills list|agree|withdraw` and `persona-builder anonymize`: find
  every skill folder on the computer, keep a yes per skill (and the folder it was given for) in
  private state, and copy each agreed skill with private details replaced or refused. The rules
  are data: built-in detectors (emails, phones and long numbers, secrets, other people's home
  folders, private hosts and addresses, ticket ids) plus the person's terms, patterns, allow and
  never-copy lists in their gitignored `private.json`. Transform, re-scan, refuse: a term becomes
  a numbered placeholder kept for good; anything still found refuses the whole skill. The report
  names file, line and kind, never the text. Code in `builder/ingest/src/anonymize*` and
  `skills*`, the CLI in `builder/cli/src/skills.rs`. Brief `plans/briefs/VO3-anonymizer.md`
  (section 9 is the repeat Verifier's list, section 10 how each was fixed). DECISIONS has the
  rules. Out of scope: hashes and refresh, the review screen (VO4), the export (VO5).
- **Who built it:** Codex, in nine dispatches: the engine (`209b846`), the review fixes
  (`0dff027`, `5a003ee`), section 10 in two runs (`07dabb9`; `d6043cd` with a follow-up that
  put the private file-name check back on unreadable files), and the fixes after the security
  review and the second repeat Verifier (`1425860`), the delta Verifier's (`9f77698`), and a VO11
  test that read a non-blocking socket (`fa6533d`: on macOS an accepted socket inherits the
  listener's non-blocking flag; it failed once in ci-local under load, then passed 10 of 10).
- **Security review:** on the engine (Opus): with the first Verifier, 15 issues between them, all
  fixed in `0dff027`. On the section-10 delta (Opus): one MEDIUM, fixed in `1425860`: counted
  separators (`{1,8}`, `{0,8}`) over a large Unicode class pushed a term of 4 or more pieces over
  the regex size limit, so the run refused (now `+` and `*`; a 5-piece host, a 12-word term and
  200 terms load, tested). LOW (memory on long term lists): the class now appears once per join,
  not measured again by V1. Nit (the test guard's `unwrap` in `Drop`) fixed. No disclosure or bypass found.
- **Verifier:** first pass (Fable, on the engine): see above, fixed in `0dff027`. Repeat (Opus, on
  `0dff027`): FAIL, 2 must-fixes (a term with punctuation inside or inside a longer word was kept;
  a mode-000 file printed its name and stopped the run) and 4 smaller items. Repeat (Opus, on
  `8d35218`): PASS, all six reproduced as fixed through the binary; should-fixes (`ss` and `ß`
  mixed in one term; a skill folder that can't be read vanished silently) fixed in `1425860`.
  Delta pass (Opus, on `1425860`): PASS; should-fix (a term starting with a punctuation-only word,
  such as a sign-off dash, made the search quadratic: 77 s on a 40000-dash line) fixed in `9f77698`
  (such words are dropped; a term with no letter or digit is refused; now 75 ms). Final delta
  pass (Opus, on `bc02aab`): PASS; should-fix (a word of only combining marks, which Rust counts
  as letters and the separator class doesn't, kept it quadratic) fixed in the next commit: one
  `[\p{L}\p{N}]` test for the words and the load check (8.6 s on 8000 marks before, 0.5 s on
  20000 after). Recorded, not
  fixed: 2000 terms of 12 words take about 4 GB to load (better than before); a term of more than
  about 20 dot or dash pieces still fails as too complex (fails safe, names the term); the exact
  match swallows separators such as an empty table cell between a term's words.
- **Accepted trade-off:** a term of 4 or more letters is found inside longer words (`Vale` in
  "available"), so ordinary text can be refused; the report gives file and line, and one allow
  entry for the whole word fixes it. Outside the design: look-alike letters, accents added to an
  ASCII term, punctuation inside a term's own word.
- **Verified:** `bash scripts/ci-local.sh` all passed at `5a003ee`, `8d35218`, `fa6533d` and the final head
  (red once at `1425860`, on the VO11 socket test above; V1, outside Codex's sandbox, which can't open the `self_hosted` tests' listeners). Mutations
  by V1 (`cp` backup, `cmp` restore): allow exemption by overlap instead of single cover (four
  tests red); an unreadable file's open failure returning its path (red). **Not verified:** Linux,
  running as root (the mode-000 tests skip there).

## VO11: the operating-system sandbox spike (2026-10-31, lane: V1)

- **What:** the answer to gate Q12, in `docs/spikes/VO11-os-sandbox.md`: Seatbelt through
  `/usr/bin/sandbox-exec`, one profile per child, started side by side by an unsandboxed launcher
  (macOS refuses a sandbox inside one), egress through a `CONNECT` proxy on one local port. App
  Sandbox is out for now (one network switch, children get the whole app's grants, the person's
  own Claude Code needs a pick and an extra entitlement, it needs signing). Landed: the profile
  for the `claude` child (`builder/agent/sandbox/claude-code.sb`: deny by default; reads of the
  system's code, the program, profile and run folders; writes to the profile and run folders; out
  to the proxy port only; listening only while signing in; the secrets deny list last, with
  Claude Code's own `.credentials.json` in its profile folder the one exception; no Keychain),
  its tests (`builder/agent/tests/sandbox_profile.rs`, 11, run in `ci-local.sh` on macOS with
  `PERSONA_BUILDER_REQUIRE_SANDBOX=1`), and the probe for the walk
  (`scripts/claude_sandbox_probe.py`, 31 tests). **Not wired into `claude.rs`:** that is VO11.1,
  which waits on V-Q10 (where the login lives inside the sandbox). Brief:
  `plans/briefs/VO11-os-sandbox-spike.md` (sections 6 and 7: the fixes after review).
- **Measured by V1 on Mimi** (macOS 27.0.1, Claude Code 2.1.292, throwaway folders, `canary`
  files): the last matching rule wins; symlinks out are refused, hard links in are readable, the
  child can't make one; Seatbelt's regexes ignore letter case, on a case-sensitive disk image too
  (so the profile's letter classes are a backup); `claude -p` needs `CLAUDE_CODE_TMPDIR`; a
  signed-out `-p` runs to `Not logged in` inside the profile; signed out with telemetry off, it
  makes no network call; only the proxy port is reachable, DNS fails; Claude Code's sign-in needs
  a listener, and Seatbelt's `localhost` filter also allows one on every address (reachable from
  the Mac's network address; no rule tried closed it without refusing the sign-in's own);
  `claude auth status --json` exits 1 when signed out. The research sub-agent (Sonnet) read
  Apple's, Anthropic's, OpenAI's and Tauri's sources; the report cites them.
- **Who built it:** Codex, in three dispatches: the profile, tests and probe (`fb25206`); the
  probe's status and version fixes (in `fb25206`, before commit); the review fixes (`9620330`); the repeat Verifier's fixes (fourth dispatch, the commit after `f3a6df9`).
  Her sandbox refuses a nested one, so the profile tests skipped for her and V1 ran them.
- **Security review (Opus sub-agent, on `ee99585`):** three mediums, all defence in depth (the
  child has no tools): the child could read the person's own terminals through `/dev`, and
  `/Library`, `/private/var/db` and `/usr/local` held logs, crash reports and installed tools
  (fixed in `9620330`, with a test over every such path the test can open, 23 on Mimi); `system.sb`'s
  `trustd` grant is a theoretical way out (recorded, VO11.1 checks); the tunnel to Anthropic is
  opaque (accepted, recorded). Lows fixed in the probe: the child's process group always killed,
  `--profile` refusing the home folder and unmarked folders, control characters stripped, proxy
  caps, cleanup failures reported. Recorded for VO11.1: `process-exec` not narrowed; the profile
  folder lasts, so `--setting-sources` on every call.
- **Verifier (Fable, first pass on `ee99585`):** PASS. She re-ran the measurements, the tests and
  the probe, and fetched six sources. Should-fixes: the same `/dev` and `/Library` reads (fixed);
  the App Sandbox quote was cut short (completed: an Open-panel pick plus
  `com.apple.security.files.user-selected.executable` can run it); private-tooling words in the
  public report (removed); `[::1]` on the proxy port is reachable too (noted).
- **Repeat Verifier (Opus, on `f3a6df9`):** PASS. Should-fixes, fixed in the fourth dispatch: a
  UDP datagram reached the proxy's port number (`remote ip` allows UDP; now `remote tcp`, with a
  test); `/usr/local/etc` doesn't exist on Mimi, so the check now uses `/usr/local`. Nits fixed in
  the report and here; recorded for VO11.1: a descendant that calls `setsid` escapes the probe's
  group kill (the sandbox still holds it).
- **Verified:** `bash scripts/ci-local.sh` all passed at `ee99585`, `f3a6df9` and the final head
  (V1, outside the sandbox, two jobs under the resource gate's throttle). Profile tests 11/11. Live probe on Mimi:
  `status` reads *Claude Code 2.1.292 runs inside the sandbox. Signed in inside the sandbox: no.*;
  `ask` gets Claude Code's own *Not logged in*; `--profile ~` is refused. Mutations by V1 (`cp`
  backup, `cmp` restore), six killed: the deny list moved before the allows, the login exception
  removed, the network rule widened to every local port, the listener made unconditional, `/dev`
  and `/Library` put back, the network rule back to `remote ip`. One equivalent: the letter classes dropped (Seatbelt ignores case
  here).
- **Not verified (walk owed):** a signed-in Claude Code inside the profile: the sign-in finishing
  there and writing the profile's own file, pasting the code when no browser opens, TLS from
  Claude Code itself through the proxy, a real `ask`. This Mac had no Claude Code login, and the
  run could not write a Keychain test item.
- **Next:** VO11.1 (Parked on V-Q10), VO11.2 (Backlog, after VO11.1).

## VO9: a model server on the person's own machines (2026-10-31, lane: V1)

- **What:** the second provider behind VO8's contract (`builder/agent/src/self_hosted.rs`,
  VO-D5): any server speaking the OpenAI-style `POST /v1/chat/completions` with a JSON-schema
  reply (Ollama, llama.cpp's `llama-server`, LM Studio, vLLM; the reply shape was read in
  Ollama's `openai/openai.go` and llama.cpp's `server-common.cpp`). `Reach::OwnMachines`; set
  per assistant with `persona-builder self-hosted show|set|off` (`--url`, `--model`,
  `--network`), kept in the state folder (`self-hosted.json`, 0600); `set` checks the server
  lists the model (`GET /v1/models`) before it takes the state lock. Every address is checked
  in ureq's resolver at every request (this computer only; with `--network`, the private
  ranges and link-local too); no proxy, no redirects, a 10-second connect limit, a 16 MiB reply
  cap. The request has no tools; a reply with tool calls, an unfinished answer or non-JSON
  content fails. A call is on the ledger at $0 and waits for the daily limit. The guard lists
  Claude Code first, then the server (`guard::providers`), at every CLI and app guard and in
  panel turns; with no own-machines provider a refusal adds how to set one up. Panel turns now
  take the narrowest ceilings across every seat (`Ceilings::narrowest`, carried from VO8.2) and
  refuse a post over 12,000 characters. The build routes before its cache: Claude Code's keys
  are unchanged (pinned in a test), another provider's are `<id>/<model>`; the backtest's
  measurements entry names who answered (`Built.by`), a Claude-only build writing today's words.
  Shared brief checks moved to `provider::check_brief`; `Provider::model` added. Brief:
  `plans/briefs/VO9-self-hosted-model.md` (section 10: the fixes after review).
- **Decided at pickup** (recorded in DECISIONS): the OpenAI-style endpoint (the one all four
  servers share); Claude Code first in the order; settings in the state folder, not
  member.json; no token yet; the daily limit still governs a $0 call; Claude's cache keys kept;
  `100.64/10` (Tailscale) left out.
- **Who built it:** Codex, in three dispatches: the provider (`ce854de`), the wiring
  (`c11cbe7`), the review fixes (`3a2d593`). Her sandbox can't open listeners, so V1 ran the
  listener tests; the first run found the test server's accepted socket inheriting
  non-blocking mode on macOS (a 16 MiB write stopped early), fixed in dispatch two.
- **Security review (Opus sub-agent, on `c11cbe7`):** one medium, fixed in `3a2d593`: a
  private address isn't proof of the person's own network (on other Wi-Fi a stranger's device
  can answer as `studio.lan` or hold `192.168.1.20`), so the server is this computer only unless
  set with `--network`, whose words say the risk. Lows: *Stop* doesn't cut off a call already
  waiting on the server (recorded in SECURITY); a panel post's length was only asked of the
  server (fixed). Info: the cloud metadata address is link-local (recorded).
- **Verifier (Fable, first pass on `c11cbe7`):** PASS, all ten claims verified with the ureq
  3.4.2 source (`run.rs` resolves on every request before the pool; `tcp.rs` dials only the
  resolver's addresses). Should-fixes: the records uncommitted at her pass (committed here); a
  connect timeout read as a slow step (fixed: unreachable); routing before the cache means a
  step narrowed to `local-only` with no server errors rather than reusing a Claude reply (by
  design; noted here).
- **Verified:** `bash scripts/ci-local.sh` all passed at `c11cbe7` and at `3a2d593` (V1's runs,
  outside the sandbox; the second with 2 jobs under the resource gate's throttle). Listener
  tests by V1: agent `self_hosted` 16, CLI `self_hosted` 8. Mutations by V1 (`cp` backup, `cmp`
  restore), seven killed: the provider's seal check removed, the cache key's provider part
  dropped, Claude dropped from the order, the tool-call refusal off, the resolver's check off,
  the draft's server words, and the loopback-only default widened to the private ranges; Codex
  killed more (no proxy, the refusal's outcome, the panel fold).
- **Not verified / residual:** no call to a real model: no model file on this Mac, and pulling
  one onto a 24 GB Mac mid-run was not done (walk owed, `## Walks owed` in the lane file). The
  app has no screen for the server yet (a card to file). DNS rebinding was proven from ureq's
  source and the resolver's unit test, not against a live name.

## VO8.2: ceilings per source, and routing in the privacy guard (2026-10-31, lane: V1)

- **What:** every brief names the sources of what it carries (`Brief.sources`), each source has a
  ceiling, and every model call goes through the privacy guard (`builder/agent/src/guard.rs`,
  VO-D2, VO-D5, gate Q6). `persona_builder::ceilings` holds the nine sources (Claude history, the
  assistant's own folder, what the person types, the copy of their skills, mail, calendar,
  contacts, family and legal skills) and the three ceilings (`local-only`, `anthropic`,
  `any-consented`), defaulting by Q6. The person sets one in member.json's `ceilings` with
  `persona-builder ceilings show|set` (private.json may not hold them). `Guard::call` routes to the
  first provider every ceiling allows, seals the brief for it (`Sealed`, no public constructor),
  and refuses before any spend, child or ledger line with a sentence naming the source that kept
  it home and the `ceilings set` that would let it go. `Provider::call` takes only a `Sealed`;
  Claude Code checks it first. `any-consented` reaches no third party yet. The steps name: build
  `[claude-history, typed, crew-folder]`, plus `mail` when mail the guided build mined can be in
  the corpus (a `decisions-src-*` file, or an `mbox` source in the config); interview
  `[crew-folder]`; refresh and apply as the build without mail; a panel turn
  `[crew-folder, typed]`, under that seat's own ceilings. Claude Code's child drops
  `CLAUDE_CODE_USE_*`, `ANTHROPIC_BASE_URL`, every `ANTHROPIC_*_BASE_URL` and
  `ANTHROPIC_FOUNDRY_RESOURCE` (carried from VO8.1's reviews). Brief:
  `plans/briefs/VO8.2-ceilings-and-routing.md`.
- **Decided at pickup** (the spec left both open): the interview's answers, notes on answers and
  a panel's question and steers are one source, `typed`, defaulting to `anthropic`; ceilings are
  changed on the command line now, the app's screen is a later card.
- **Who built it:** Codex, in six dispatches: the red tests (frozen in `0cee2ee`; V1 then changed
  one expected sentence, the no-consent refusal, and recorded it in the brief), the
  implementation, the mechanical move of the older tests onto the guard (the red dispatch had
  left them), tests for the eight CLI and API sites and Claude Code's own seal check (a mutation
  check by V1 showed nothing caught a site using the default ceilings), the review fixes
  (section 8), and the staged-file sweep.
- **Security review (Opus sub-agent, on `4445c72`):** one medium, fixed in `5a9b045`: mail the
  guided build mined reached the build's briefs unnamed, so mail at its `local-only` default
  went to Anthropic, and `ceilings show` said the app didn't read mail. V1 had found the same
  path while the reviews ran. Lows: `ceilings set` followed a link and wrote in place (fixed:
  `O_NOFOLLOW` read, the state lock, a synced staged sibling); managed Claude Code settings can
  still route the child elsewhere and a panel turn is checked under the receiving seat's ceilings
  only (both recorded in `.ai/SECURITY.md`).
- **Verifier (Fable, first pass on `4445c72`):** PASS with should-fixes: the refusal's dangling
  comma after the provider's name (fixed), a test for a linked member.json (added), the
  interview's counts (recorded in DECISIONS), and `CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST` as a
  hardening (recorded as a candidate: not verifiable live here, since this Mac's Persona Builder
  profile isn't signed in). She marked the sources claim proven without seeing the mail path.
- **Repeat Verifier (Opus, on `5a9b045`):** PASS with should-fixes: a staged member.json left by
  an interrupted `ceilings set` was never swept (fixed in `ad0700c`, 33 lines, within the
  pass); the mail marker residual below (recorded); the core `write_member` now used only by
  tests (left).
- **Verified:** `bash scripts/ci-local.sh` all passed at `4445c72`, at `5a9b045` and at the records head after the sweep fix (V1's runs,
  outside Codex's sandbox, where seven `save` listener tests fail). Mutations by V1 (`cp` backup,
  `cmp` restore), eleven killed: `allows` ignoring `local-only`, the env scrub off, private.json
  ceilings allowed, a panel seat on default ceilings, the build naming fewer sources, `is_set`
  never set, Claude Code skipping the seal check, the CLI build and the API apply on default
  ceilings (the last three after the added tests; all three survived before them), and the mail
  file and the `mbox` config each ignored. Live: the debug `persona-builder ceilings show` and
  `set` on an invented assistant in a scratch folder (set, an unknown ceiling, an unknown source,
  private.json holding ceilings), each as the brief says.
- **Not verified / residual:** no live model call (the profile here is signed out); the refusal
  is proven against the stub Claude Code. Mail is recognised by markers only: a merged corpus
  still holding mail after both the mined file and the `mbox` entry are gone, or copied from
  another Mac, is not named. Managed settings, and a MITM proxy (proxy variables pass on
  purpose), can still see or route briefs. Follow-ups: the app's ceilings screen; the narrowest
  ceiling across seats before a second provider (VO9).

## VO8.1: the provider interface, Claude Code its first adapter (2026-10-31, lane: V1)

- **What:** one `Provider` trait in `builder/agent/src/provider.rs` (VO-D5): `id`, `name`,
  `reach` and `call(session, brief) -> reply`. Today's `claude::Call` is renamed `Brief` (same
  fields: step, instructions, records, model, reply shape; no tools field) and `Usage` and `Reply`
  moved beside it. `Reach` is ordered `OwnMachines < Anthropic < ThirdParty` for VO8.2 to compare
  with a source's ceiling. The person's own Claude Code implements it with its call body moved
  unchanged (same argv, checks, cap and ledger line) and reports `Anthropic`; finding, checking and
  signing in stay on `Claude`. `AGENTS` gains each provider's reach; the app's `AgentInfo` and
  `persona-builder agents` are unchanged. The build (`Asker`, `build`), the interview (`draft`),
  `refresh`, `apply` and panel turns take `&dyn Provider`. No behaviour change. Brief:
  `plans/briefs/VO8.1-provider-interface.md`.
- **Who built it:** Codex, in two dispatches: the tests (red, they did not compile; frozen in
  their own commit) and the implementation with them unchanged. Existing tests changed only in
  imports and `Call` to `Brief`.
- **Tests:** `builder/agent/tests/provider.rs` (Claude's id, name and reach through the trait;
  `AGENTS` agrees; the order of `Reach`; one call through `&dyn Provider` leaves one ledger line)
  and `builder/build/tests/provider.rs` (an in-process fake provider drives the interview draft,
  its one retry, and a build step with its cache).
- **Verification:** `bash scripts/ci-local.sh` all passed. Mutations 6 of 6 killed (Claude's
  reach, `AGENTS`' reach, the order of `Reach`, the model a build step sends, the interview's step
  name, the build cache's reuse). Live: `persona-builder agents` prints the same line as before.
  Not verified: a real call to Claude Code (the stub-based tests cover the argv byte for byte).
- **Security review (Opus sub-agent):** no high or medium; the moved call body is identical to
  the old one (diffed, signature line only). **Verifier (Fable):** PASS, nothing to fix in this
  slice.
- **Carried to later slices** (from both reviews; on the VO8.2 and VO9 cards):
  - Before VO8.2 routes by reach: Claude Code inherits the person's environment, so a
    `CLAUDE_CODE_USE_BEDROCK` or `CLAUDE_CODE_USE_VERTEX` (or a base-URL override) would send
    briefs to a third party while `reach()` says Anthropic. Remove those from the child's
    environment, or make `reach()` follow them.
  - Before VO9: the cap and the ledger line live inside `Claude`'s call, and `Session::start`,
    `finish` and `discard` are crate-private, so a provider in another crate can't charge the cap.
    Have the agent crate own one wrapper (checks, start, the provider's send, finish) and give
    providers only the send.
  - Claude's model names cross the seam (`claude::DEFAULT_MODEL` in panel turns, `valid_model`),
    and the build cache's key leaves out the provider's id, so a cached Claude reply would be
    reused for another provider under the same model name.

## VO8.3: the secrets deny list for paths (2026-10-31, lane: V1)

- **What:** a path-level list of files Persona Builder never opens, whatever they hold (gate
  amendment A2, VO-D2), in `builder/core/src/deny.rs` (`persona_builder::deny::DenyList`). Built
  in: any path part named `.ssh`, `.aws`, `.gnupg`, `.env*`, `.envrc`, `.npmrc`, `.netrc`,
  `.pgpass`, `*.pem`, `*.key`, `*.p12`, `*.keychain`, `*.keychain-db` or `.credentials.json`, and
  the runs `.config/gh`, `.docker/config.json`, `Library/Keychains`. The person adds names or
  `~/`/`/` paths in `filters.denied_paths` (member.json and private.json, appended like
  `pasted_prefixes`; absent, every config reads and writes as before). A path is checked as given
  and with its links resolved, without regard to letter case; a person's entry must be plain
  letters (macOS writes accented names decomposed and the app has no Unicode normalisation), and
  `/`, `~/`, `~` or one holding `//` is refused. Checked before mining Claude Code
  history (the history folder refused; a denied project folder left out unlisted and counted as
  excluded; a denied session file left out unopened and counted, the summary saying so only when
  one was), before reading a claude.ai export
  (and its `conversations.json`), the `shingle` document and a panel's question or steer file; the app shows
  the refusal in place of "can't read the export" and counts left-out files with the private
  ones. Saving refuses an assistant folder that holds such a file. Lives in core, not ingest,
  because every reader (ingest, save, cli, api) depends on core. Brief:
  `plans/briefs/VO8.3-path-deny-list.md`.
- **Split:** VO8 was split at this pickup into VO8.1 (the provider interface), VO8.2 (ceilings and
  routing) and VO8.3 (this); VO9, VO13 and VO14 now wait on the sub-cards.
- **Who built it:** Codex, in four dispatches: the tests (red, they did not compile; frozen in
  `00fb02a`), the implementation with them unchanged, a fix-up from V1's review (a denied project
  folder that is a link had been listed through the link, now refused as every link there was
  before; `DenyList::builtin()` read `HOME`, now it takes no environment), and a fix-up from the
  two reviews below (`55bdf4a`), each red first. The one edit to a frozen test: the bad-entry
  sentence and its new shapes.
- **Security review (Opus sub-agent):** no high. Medium, fixed: an accented entry typed composed
  missed a folder named decomposed on disk. Lows fixed: `~//Private` meant `/Private`; the folder
  table counted denied sessions. Lows recorded, not fixed: hard links (below); the check-then-open
  gap (below); the Python kit refuses a `denied_paths` list (`.ai/DECISIONS.md` 2026-10-31); an
  empty `denied_paths: []` is dropped when the app rewrites member.json; common secret files the
  plan's list lacks (`.git-credentials`, `*.kdbx`, `.kube/config`, `.config/gcloud`,
  `.vault-token`, `.cargo/credentials.toml`, `.pypirc`, `id_rsa`/`id_ed25519` outside `.ssh`),
  left for a later decision on widening the list.
- **Verifier (Fable, first pass on `20cc8c1`):** PASS with should-fixes, none letting a denied
  file's bytes through, all fixed in `55bdf4a`: the folder table's count; a denied project folder
  was listed; `/` or `~/` as an entry denied everything; the panel showed full paths. Its other
  point stands as scope: the panel's question and steer files and `claudeai inspect` load no
  member, so they use the built-in list only.
- **Repeat Verifier (Opus, on `55bdf4a`):** PASS. Two optional points not taken: the test that a
  denied project folder is never listed would still pass if a `read_dir` came back while it was
  counted (a `chmod 000` folder would pin it); entries such as `*`, `/*` or `~/*` are accepted and
  deny everything, which fails safe.
- **Verified:** `bash scripts/ci-local.sh` all passed at `20cc8c1` and again at `55bdf4a` (V1's
  runs, outside Codex's sandbox, whose listener refusal fails seven `save` tests). Mutations by V1
  (`cp` backup, `cmp` restore), nine of nine killed: `.ssh` dropped from the list, links not
  resolved, case kept, the export's first check, save's check, the private.json merge, the
  plain-letters rule, a denied folder not counted as excluded, a denied session counted in the
  folder table. Live, the debug `persona-builder claudeai inspect` with an
  invented home: an export under `.aws`, a link to it from an ordinary folder and
  `DL/../.AWS/export.zip` refused with the sentence; an ordinary export folder read as before.
- **Not verified / residual:** a hard link to a denied file under an ordinary name is not caught
  (a path list can't see it; the text filters still apply to what is read); a local process that
  swaps a folder for a link between the check and the open (it could read the file itself
  anyway); Unicode normalisation differences in the person's own entries (built-in entries are
  ASCII). The Python kit's own readers (`scripts/crew_*.py`) and the privacy check
  (`check.rs`, byte-compared with Python) are unchanged.

## VO10.3: Panels on the app's screen (2026-10-30, lane: V1)

- **What:** the app gains Panels. From the home screen, *Ask a panel* opens a list of the
  person's saved panels and a form to start one: the question, the kind of answer (one written
  answer or a list of 1 to 12 items), which of their assistants sit (2 to 24, only those with
  permission given), and how long they may take (ask after five minutes, 15 minutes, an hour,
  or no limit). A running panel shows its rounds as they finish and can be stopped (a stop pressed
  before Rust has named the panel is sent as soon as the first progress line names it). A saved
  panel shows its question, status, seats and rounds (how far apart they were, posts kept from the
  others and why, who couldn't answer), and, by status: the time question's three answers when
  paused, *Carry on* when stopped, failed, saved as running or paused without a question, a Stop
  for one saved as running, and for a finished one the result (the answer, how they grouped
  with each group's share, every dissent quoted in full, the whole result as raw text) and *Ask
  them to reconsider*. Every model-written string is shown as text through Vue's interpolation,
  never as HTML or rendered markdown. Five app commands (`convene_panel`, `resume_panel`,
  `stop_panel`, `list_panels`, `panel`) are thin calls into VO10.2's `builder/api` functions,
  added to the command lists, the capability, the generated permissions and the CLI parity
  table. Brief: `plans/briefs/VO10.3-panels-on-screen.md`.
- **Who built it:** Codex, in four dispatches: the tests against placeholder screens (A, 89
  red on assertions, checksums frozen), the implementation with the tests unchanged (B, the
  checksums checked), then three fix-ups (C, D, E), each red first.
- **Verified:** `ci-local.sh` all passed after C and again after D (Codex's own run after B failed only on seven `save` tests whose local listeners its sandbox refuses). Mutations by V1 in
  place (`cp` backup, `cmp` restore), each a test failure: `v-html` on a dissent (two tests), the
  remembered stop, bytes counted as characters, the 24-seat cap, the time question without
  `asked`, `Your assistants` while a panel runs, a bound `href` on a panel screen. One survived:
  the seat list's permission filter, redundant because such a box is disabled and Rust refuses
  the seat; removed in D.
- **Security review (Opus sub-agent):** no high or medium. Lows, fixed in D: once Stop was
  pressed a real failure showed as "Stopped" (in Rust a stopped run resolves, it never rejects);
  a panel saved as running (after a window reload) had *Carry on*, which can only fail as busy,
  and no Stop; step navigation did not itself refuse while a panel ran.
- **Verifier (Fable, over `b835343..6749441`):** all six claims PASS, no must-fix, each checked
  with her own mutations (a `v-html`, the remembered stop, a swapped capability). Should-fixes,
  done in D: the two stop outcomes above; a paused panel without a time question was a dead
  end; the binding guard missed `v-bind="{…}"` and `:[x]`; the background counter gave no
  reason its count jumps for some letters. Noted, unchanged: a stop cannot save round 1's turns,
  which are paid for (the runner checks the flag after a round's turns); a panel run from the
  command line looks "running" here and this screen's Stop cannot reach it.
- **Repeat Verifier (Opus, over `6749441..ef2dfd7`):** all seven claims PASS, no must-fix, 17
  mutations in a scratch copy (13 caught; the misses were her should-fixes). Should-fixes, done
  in E (`daa15cd`): the `goToStep` guard was untested and unreachable (Panels opens only from
  home, which clears the assistant), so it is removed and a test proves no step shows there; the
  binding guard is now an allowlist (`class`, `disabled`, `id`, `key`, `result`, `start`,
  `value`), catching `.srcdoc`, `v-bind.prop`, `:srcset`, `:HREF` and the rest; a stop on a panel
  saved as running now says it was asked. `ci-local.sh` all passed after D. **E (83 added lines,
  mostly tests) has no Verifier pass yet and no ci-local run of its own:** a delta pass and the
  gate come before the land.
- **Landed by lane keeping (2026-10-31):** lane V was braked idle with this branch stranded,
  so the lane keeper landed it. Delta Verifier (Opus) on `ef2dfd7..daa15cd`: PASS, no must-fix;
  a repeat pass at the head: PASS. Should-fixes left for V1: `plain-text.test.ts` checks only
  top-level bindings (deleting `visit(node.children)` passes; add a nested `<template><a :href>`
  case); its literals `1` and `7` should be `NodeTypes.ELEMENT` and `NodeTypes.DIRECTIVE`;
  `PersonaBuilder.vue` `open()` keeps a `panelsRunning` guard as unreachable as the removed
  `goToStep` one (keep both or drop both). The gate ran on the land.
- **Not verified:** the screens in the real app window, light and dark, and a real Claude Code
  panel (Mimi's `claude` is not signed in, and a panel spends real usage): owed as a walk.
- **Skills Review:** ✅ tdd-skill (frozen red tests, then mutations) · ✅ security-and-hardening
  (two independent reviews, every fix red first) · ✅ frontend (loading, empty and error states on
  each read; neighbours' patterns) · ⚠️ design-guide (Tabler classes only; no screenshot pass,
  owed with the walk) · ✅ plain-language (every sentence fixed in the brief, checked by the
  Verifier) · ✅ source-driven-development (the runner's state machine read, not recalled)

## VO10.2: the one-Mac panel runner (2026-10-30, lane: V1)

- **What:** `builder/panel-runner` (`voice-panel-runner`) runs a panel of the person's own
  assistants on this computer, ported from the reference relay (commit `6f754c91`) without hosts,
  skills, the mirror, notify or polling. Each turn is the agent crate's tool-less Claude Code call
  (`step: panel-turn`, a `{post}` schema) on that assistant's own ledger and daily limit, only with
  its permission given and a built `decisions.md`, which the brief carries as data. Every other
  post reaches a brief fenced, scrubbed and cleaned; the leak guard (the protocol's plus the
  runner's own path and email checks and every seated assistant's personal and secret filters)
  withholds a post on any finding. Files in `~/.claude/voice/panels/<id>/`, 0600 in 0700, opened by
  descriptor without following links. The time box asks and pauses at once (no polling); stop
  discards the round; resume trims after a crash, takes the time-box answer, or reopens a closed
  panel with a steer. `persona-builder panel convene|resume|status|list` and `builder/api`'s
  `panels.rs` (DTOs and generated types) share one path; the app's commands and parity rows wait
  for VO10.3. Carried from VO10.1's review and closed: member names must be assistant slugs, saved
  weights finite and within 0 to 1, posts scrubbed of tag characters, the soft hyphen and variation
  selectors, and the `file:`, `C:\`, spaced and `localhost` gaps. Where panels live is this slice's
  choice: not under `~/.claude/state/`, whose names are assistant slugs. Brief:
  `plans/briefs/VO10.2-panel-runner.md`.
- **Who built it:** Codex: the tests against a pinned API with placeholder bodies (A, 76 red on
  assertions), the implementation with the tests frozen (B, hashes checked unchanged), then the
  review fixes (C, D), each red first.
- **Verified:** `ci-local.sh` all passed (before and after the fixes). Mutations by V1 in a scratch
  copy, each a test failure: the tag-character range, the UNC check, the decoded-block check, the
  scrubbed filter check, the brief's scrub, the weight range, the consent check; the slug check
  on read-back names survived (shadowed by the member-list check) and gained two tests in C.
- **Security review (Opus sub-agent):** one medium, fixed in C: the guard checked the raw and
  scrubbed post, but the brief forwards `clean(scrub(post), 8000)`, and `clean` turns other control
  characters into spaces, so `/Users\u0001/…`, `me\u0001@…` or a filter phrase split by U+0001
  passed the guard and appeared whole in the next brief. `guard::forwarded` is now the one
  definition of the shared text, checked beside the raw and scrubbed forms.
- **Verifier (Fable):** PASS but for the same must-fix (M1, fixed in C). Should-fixes, done in D:
  scrub the reopened result before its fence; catch `me＠localhost`, `~/notes.txt` and `~user/`
  after a comma or colon; clean an assistant name in every error. Left as a stated limit: a
  detail spelled out in plain words.
- **Repeat Verifier (Opus, over `d4a6275..ad29f1b`):** PASS; M1 proved closed (control
  characters, the 8000-character cut completing a filter word, decoded blocks, errored and
  malformed posts), and each new test shown to fail with its fix reverted. Should-fixes, done in E
  (`db7776f`): the question, background and steer are stored scrubbed, as the guard saw them
  (a steer `see` + U+200B + `/Users/…` passed and showed as a path); U+FE6B counts as an `@`; a
  doubled tilde or a tilde after a word starts a home path; a tilde before a digit (`~7/10`) is
  prose. Left as notes: a post opening with `@domain` sits after its fence label, and a block item
  re-escaped by JSON (`\`) can show UNC-shaped in the author's own last block and in
  `result.md`; neither reveals more than the guard already passed. Prose that looks like an
  address (`Meet @ noon. See you`) is withheld on purpose: it has the shape of `a @ b . com`.
- **Delta Verifier (Opus, over `ad29f1b..db7776f`):** PASS; 62 inputs compared before and after,
  every earlier case still caught, the 200-character and 4096-byte limits hold, the 13 new tests
  fail on the old source. One should-fix, done in F: E's user-name class (`[A-Za-z_]`) dropped
  non-ASCII names (`~Ølaf/x`); it is now any word character but a digit. Notes, unchanged: a path
  glued to a word (`see/Users/…`) passes, as the protocol's own check requires a space or
  punctuation before the `/`; a name starting with a digit cannot be told from `~3/4`.
- **Not verified:** a real Claude Code turn (Mimi's `claude` is not signed in, and a panel spends
  real usage): only a stub on `PATH`. Owed as a walk with VO10.3's screens.
- **Skills Review:** ✅ tdd-skill (frozen red tests, then mutations) · ✅ security-and-hardening
  (two independent reviews, both fixes red first) · ✅ source-driven-development (the relay and the
  agent crate read, never recalled) · ✅ code-quality · ⚠️ plain-language (CLI and API sentences
  only; the screens are VO10.3's) · ⚠️ my own dispatch D brief first held a bare-words email
  pattern that flags ordinary prose; caught before dispatch.

## VO10.1: the panel protocol as a Rust crate (2026-10-29, lane: V1)

- **What:** VO10 split into VO10.1 (this crate), VO10.2 (the one-Mac panel runner) and VO10.3
  (Panels on the app's screen). `builder/panels` (`voice-panels`) ports the reference deliberation
  protocol (Python, reference commit `a7f31d72`) with no I/O: the last-block parser, distances,
  rounds with holdouts and stopping, groups, dissent, the result and its markdown, the time box,
  the leak guard and fences. Its state, round report and result read and write the reference's
  JSON. Named for the coming rename, so it needs no rename later. No new dependency (`regex`,
  `serde`, `serde_json` were already in the workspace). Brief:
  `plans/briefs/VO10.1-protocol-crate.md`.
- **Who built it:** Codex, in separate dispatches: the tests against a pinned API with placeholder
  bodies (A), then the implementation with the tests frozen (B), then review fix-ups (C, D, E, F).
  The tests' hashes were checked unchanged after every implementation dispatch.
- **Tests:** the 66 portable vectors, copied byte for byte (SHA-256 checked against the source),
  under the reference test's rule; `reference_probes.json`, 49 parse edge cases plus cleaning,
  casefolding, leak-guard, fence and markdown vectors and 12 panels, all produced by running the
  Python reference; the reference's unit tests, ported; hostile-input tests (100,000 backticks,
  50,000 fence lines, 10,000 unclosed openings, 500-deep JSON, every control character).
- **Found in review and fixed:** an unquoted overflowing number (`draft: 1e400`) was kept as text
  where Python refuses it; a process-wide term-regex cache that grew without bound (now built per
  call); an unquoted `-0` rewrite that a mutation check showed could never change a verdict
  (removed). A differential run then showed the markdown's `D:` line differing in the last digit:
  Python 3.12 and later add floats with compensated (Neumaier) summation, so the crate now does
  too (`src/sum.rs`); with a plain sum swapped into the reference, the two matched byte for byte.
  serde_json's default float parser can be one ulp off, so the crate turns on `float_roundtrip`;
  Cargo features unify, so every workspace crate's serde_json now parses floats exactly.
- **Verified:** red first for every dispatch (A: 15 protocol tests plus the fixture and probe
  suites failing on assertions; C: the three overflow probes; E: all 12 panels). Mutations by V1,
  each a real test failure (exit 101) after a green baseline: the deadlock count, the holdout
  weight floor, the one-third group link, the overflow rule, the casefold table, `\r` cleaning,
  the fence length, Python's float format, raw control escaping, the compensated sum's final step
  and `float_roundtrip`; one survivor became the dead-code removal above. Differential, by V1,
  outside the repo: 20,000 random cases (panels, time boxes, hostile posts) through both the
  Python 3.12 reference and this crate, 0 mismatches, the markdown compared byte for byte.
- **Not verified:** Python 3.10 and 3.11 sum floats plainly, so their last digits can differ from
  this crate's (within the reference's 1e-9 rule); `match_key`'s casefold table is Python 3.12's
  (Unicode 15.0), so letters added in later Unicode versions fold by Rust's own lowercase there
  (the leak guard's word class and lowercase are Python's, below; its `\s` is the `regex`
  crate's plus U+001C–U+001F, equal to Python's on every code point today); `statistics.mean`
  is exact in Python and a compensated sum divided by the count here (used only by the zero-weight
  fallback and the time-box projection), which can differ in the last digit. No tag yet: one
  waits on VO16's versioning, and until then the land commit is the pin.
- **Security review:** no high. Medium, fixed by narrowing the claim: `.ai/SECURITY.md` said
  `clean` strips invisible characters, but tag characters (U+E0000–E007F), the soft hyphen and
  variation selectors pass, exactly as in the reference; the row now says so. Lows, left for
  VO10.2 to close because they need a runner-side rule rather than a parity change: the leak guard
  misses `file://` and `C:\` paths, spaced or `localhost` emails and terms split by a zero-width
  space; a member name is written outside any fence in the markdown's `Members:` line, so names
  should be limited to a fence label's characters; a hand-edited state with huge weights makes `D`
  NaN, which saves as `null` and cannot be read back, so states should reject weights that are not
  finite or are outside 0 to 1.
- **Verifier (Fable):** FAIL, one must-fix, fixed (dispatch G): the leak guard used the `regex`
  crate's `\w`, `\b` and `\s`, which differ from Python's, so a term followed by a variation
  selector, a joiner or a combining mark was missed (fail-open). Dispatch G moved to Python's
  class shapes (`\p{L}\p{N}_`) and a hand-written term match following Python's case-insensitive
  rule (simple lowercase plus Python's extra-case table, so `İstanbul` and `ıstanbul` match
  `istanbul`). A term over 1,000 characters counts as found without a search (a deliberate
  difference that fails closed). Should-fixes done: field lines are stripped as Python strips them
  (U+001C–U+001F too), and a raw-control probe now exercises the JSON escaping path. After the fix:
  48,000 random leak-guard cases and 6,000 random panels, time boxes and posts against the
  reference, 0 mismatches (random cases that, as the repeat pass showed, never reached the two
  gaps below). Recorded as limits, not
  changed: a JSON lone surrogate (`"\ud800"`) is refused, since a Rust string cannot hold one,
  where Python keeps it; a hand-built state is read more strictly than the reference (a round
  number at `u32::MAX`, draft ids such as `b.r1_0` or with non-ASCII digits, a block missing
  `objections` or with a float rating are refused); a non-finite projection or `D` saves as `null`.
- **Repeat Verifier (Opus, 2026-10-30, over `c883f5a..5f19194`):** FAIL, two must-fixes, both
  fail-open, fixed (dispatch H, `1fd6b5d`). (1) `\p{L}\p{N}` and `to_lowercase()` follow the Rust
  toolchain's Unicode, newer than Python 3.12's 15.0.0: 5,004 letters added since counted as word
  characters, so `secret` beside one of them was missed. (2) Dispatch G's brief took `\b`'s
  word-ness from the term's characters; Python reads the text's, and they differ for case-equal
  pairs such as U+0345 and `ι`. Now the word class and the simple lowercase the term match, the
  email and the path patterns use are generated from Python 3.12 by `gen_sre_tables.py`
  (`src/sre_table.rs`, 748 word ranges, 1,433 lowercase pairs; regenerated byte for byte), and the
  boundary reads the text. Red first: 168 reference probes (164 failing), then 110 more after a
  mutation (lowercase through Rust) survived, since a new letter at a term's edge is stopped by the
  boundary before case is compared; the 110 put it mid-term. Mutations, each a probe failure
  (exit 101): boundary from the term (4), lowercase through Rust (110), the class as
  `\p{L}\p{N}_` (52), one new letter as a word character. Differential by V1 against the reference
  under Python 3.12.14, every code point: 2,224,126 term-boundary cases, 5,560,315 email and path
  cases, 6,096 case-equal pairs at a term's edge and middle (30,480 cases), 0 mismatches; 27 hand
  cases, the one difference being the 1,000-character rule above. The repeat pass on `aa3ce62`
  (Opus) proved the code: her own differential, 15,604,140 cases over every code point (term
  edges, one-character terms, email and path positions, 5,874 case-equal pairs), all equal to the
  reference; she failed it only on the pairs-versus-cases wording, fixed here. Her should-fixes,
  done: six email and path punctuation probes (a `$` added to either class is now caught), and
  the generator refuses any Python but 3.12 (the security review's low). Left: nothing in CI
  regenerates `sre_table.rs` (CI's Python may not be 3.12); the probes and the sortedness test
  guard it. Security review of the delta: no high, no medium.

## PB8.4: the lasting place is the two Applications folders only (2026-10-27, lane: V1)

- **What:** filed from PB8.2's Verifier; V-Q9 answered option 1 (narrow the roots). A release
  build now writes the shipped command's path into a `SKILL.md` only from under `~/Applications`
  (where `install.sh` puts it) or `/Applications`; before, anywhere under the home folder passed,
  so an unzipped copy in `~/Downloads` made an assistant that broke once the download was deleted.
  One line in `word_from` (`builder/core/src/cli_command.rs`); `.ai/SECURITY.md` says the same.
  The refusal sentence (`CLI_PLACE`) already names the Applications folder in the home folder, so
  it is unchanged. Brief: `plans/briefs/PB8.4-applications-folders-only.md`.
- **Who built it:** Codex, from the brief, with no corrections.
- **Verified:** red first (Codex's report: the new test failed on its Downloads assertion before
  the change). Mutation by V1: with the home root put back, the new test fails at the Downloads
  assertion. The test also covers a sibling folder named `ApplicationsOld`, which must not pass.
  Codex's sandbox could not run the whole `ci-local.sh` (no loopback sockets, no npm); V1 ran it
  on `e6e5d88`'s tree: all passed. Live, by V1, on a release build of the CLI with a throwaway
  home: a copy in `<home>/Downloads/PB/` refused with the install sentence and made no folder; the
  control, a copy in `<home>/Applications/PB/`, made the assistant and wrote
  `"$HOME/Applications/PB/persona-builder"` into its `SKILL.md`.
- **Not verified:** the app screen showing the sentence (a release app build, owed with the PB8
  walk); the guard test is macOS-only (Linux's `/tmp` is refused for another reason).
- **Security review:** no high or medium. One low: a `~/Applications` that is a symlink to a folder
  outside home now makes that folder the root (planting it needs write access to home already).
- **Verifier (Fable):** PASS, no must-fix. Should-fixes done: the brief no longer says the owner
  chose it (V-Q9 was answered for them); this record names the commit CI ran on. Noted limits:
  the symlinked `~/Applications` case above, and PB8.2's limits, unchanged.

## PB7.7e: alert banners reach readable contrast in both themes (2026-10-27, lane: V2)

- **What:** Tabler 1.6.1 colours an alert's text with its variant colour, on a tint of that same
  colour. Computed from Tabler's own rules (`.alert`'s two `color-mix(in oklab, …)` formulas), the
  light theme's warning is 2.02:1 (background `#FFF8F0`; the walk measured `#FFF8EF`), success 2.56,
  info 2.83 and danger 4.21; the dark theme's danger is 3.15. `theme.css` now sets the text colour of
  each failing alert (light warning `#7A4D00`, success `#1D6B2C`, info `#1F5C99`, danger `#A82525`;
  dark danger `#F28B82`), leaving the tinted backgrounds and `.alert-important` alone. Brief:
  `plans/briefs/PB7.7e-alert-contrast.md`.
- **Decided at pickup:** the card said dark was fine, but computing it showed dark danger at 3.15:1,
  so it is fixed too. The other three dark alerts pass (5.17, 4.69, 6.52) and are unchanged. The
  card also asked only to check success and danger; info failed as well, so it is fixed too.
- **The check:** `theme.test.ts` reads Tabler's CSS, checks the alert formulas are still Tabler's (so an
  upgrade that changes them fails), computes each alert's background in OKLab from the theme's
  surface, and expects 4.5:1 for all four variants in both themes, naming the theme, variant and
  ratio on failure.
- **Who built it:** Codex, from the brief, plus one review follow-up (no log line in the test).
- **Verified:** red first (`light warning 2.02`); UI tests (284), typecheck and build green; mutation
  checks, each caught: warning back to Tabler's amber (`light warning 2.02`), dark danger rule removed
  (`dark danger 3.15`). Security review: no findings. Verifier: not required by the spec. `bash scripts/ci-local.sh` runs at the land.
- **Not verified:** the look in the running app (owed under `## Walks owed`).

## PB8.2: the shipped command's path is only written from a lasting place (2026-10-27, lane: V1)

- **What:** filed from PB8.1's security review and Verifier. Making an assistant writes the path of
  the `persona-builder` command the app ships into its `SKILL.md`; a release build now refuses that
  (`CLI_PLACE`: "Persona Builder needs to be installed before it can make an assistant. Install
  it with install.sh, as "How to install.txt" shows …") unless the canonical path is under the home folder or `/Applications`, not under
  `/Volumes` or an `AppTranslocation` folder, and the command and every folder above it are owned
  by the person or root with no write for others (group write allowed: `/Applications` is
  `root:admin 0775`). One entry point, `shipped_cli_word(home)`, used by the app's and the CLI's
  *new assistant*. A development build skips the rule: every integration test runs with a scratch
  home while the test build lives in `target/`, and `tauri dev` runs from the clone. The template's
  `decisions.md` names the skill's look-up (§2) instead of `crew_decisions.py similar`; its other
  two mentions of the script (the `generated_by` line, the tally placeholder) are the guided build's
  counting steps and stay. Brief: `plans/briefs/PB8.2-lasting-place.md`.
- **Who built it:** Codex, from the brief, plus a one-line correction where the brief's own test was
  too broad (it banned every mention of the script in `decisions.md`; now only the look-up).
- **Verified:** red first for each of the eight new unit tests (Codex's report); four mutations
  caught by V1 (the others-write bit, the passing-disk check, the roots check, the debug-only
  switch). Live, by V1, on a release build of the CLI with a throwaway home: a copy outside home
  and `/Applications` refused with the sentence and made nothing; a copy on a mounted disk image
  (`/Volumes/…`) refused the same way; the control, a copy under the home's `Applications`, made the
  assistant and wrote `"$HOME/Applications/PB/persona-builder"`. `ci-local.sh` green on `5c76a65`.
- **Not verified:** the owner check has no test (a folder owned by someone else needs root to
  make; a mutation of it survives); Gatekeeper translocation itself (the component check is
  unit-tested); the app screen showing the sentence (a release app build, owed with the PB8 walk).
- **Security review:** no high or medium.
- **Verifier (Fable):** PASS, no must-fix. Should-fix done: the sentence now matches
  `How to install.txt`. Should-fix parked as V-Q9 (card PB8.4): the unzipped app
  in `~/Downloads` still passes (the card names "the home folder"); narrowing the roots to
  `~/Applications` and `/Applications` changes the card's rule, so it is the maintainer's. Noted limits: a disk
  image or share mounted at a folder under home passes; a launch through
  `/System/Volumes/Data/…` is refused though it would last; ACL write grants are not checked.

## PB2.6: a multi-select answer stored as a list, and records it can't read (2026-10-26, lane: V2)

- **What:** Claude Code now writes a multi-select answer as a list of strings, each an option's exact
  label or the person's own words. Both readers (`builder/ingest/src/mine.rs`, `scripts/crew_decisions.py`)
  take each element that equals a label as a pick (in the list's order, once) and the rest, stripped,
  as own words joined with ", ". A string answer reads as before. Brief: `plans/briefs/PB2.6-list-answers.md`.
- **Decided at pickup (the card left it open):** the app's reader skips a record of an unexpected
  shape and counts it (`Mined.unreadable`) instead of stopping the whole read. Folder and file safety
  checks (links, FIFOs, non-files) still stop it. The CLI summary adds "; skipped N records of an
  unexpected shape" only when N > 0; *Your history* adds "N entries it couldn't read were left out."
  only then. Python, the reference script, doesn't skip: it stops on most such records (a list
  holding a non-string, a non-object question), but an answer that is a number, `true` or an object
  it writes as own words (`7` reads as own words `7`); Rust skips both kinds. Accepted differences,
  older than this slice (Rust used to stop on all of them). Why: one odd record in thousands of
  conversations shouldn't block a build, and skipping only ever leaves things out. The skip matches
  the shape error alone (`unreadable` in `mine.rs`); any other error from a record still stops.
- **Shape evidence:** counted on a real history by shape only (no text read or kept): 128 list
  answers, all on multi-select questions, every element a string; 361 elements equal a label, 27 don't
  (at most one per list), 20 contain a comma.
- **Who built it:** Codex, from the brief, plus two review follow-ups: no copy of the carried reply per
  record, and two test rows (two own-word elements; Python's own dedupe check).
- **Verified:** red first (each new test failed for its stated reason, the screen's two included);
  mutation checks, each caught: the ", " joiner (both readers), dedupe (both), stripping (both), the
  skip count, the list path, the summary suffix, the screen's plural and its only-when-above-zero.
  Security review: no findings.
- **Verifier (Fable):** PASS, no must-fix, two should-fixes, both taken: the skip now matches only the
  shape error (`unreadable`, a unit test pins `Io`, `Exists` and other `Invalid` as stopping; Codex,
  from the brief's follow-up section; red seen as a compile failure), and the Python sentence above
  is corrected (checked by calling `parse_answer` on `7`, `True` and an object).
- **Mutation of the narrowed skip:** `unreadable` forced to always true fails
  `only_a_shape_error_counts_as_an_unreadable_record`; restored and checked with `cmp`.
- **Repeat Verifier (Opus 5.5, after the rebase onto PB8.3):** PASS, no must-fix. `git range-diff`
  shows the brief and both code commits identical; only the record's neighbour changed. Targeted
  `cargo test` and `clippy -D warnings` on the ingest crate and `tests/crew_decisions_test.py` passed.
  Her one should-fix, kept as a known limit: when an assistant record is skipped, the next message
  is paired with the reply before it (that one record's context only; before this slice the whole
  read stopped instead).
- **Not verified:** a live read of a real history with this build. `bash scripts/ci-local.sh` runs at
  the land.

## PB7.9b: *Sign in to Claude*, in Persona Builder's own Claude Code profile folder (2026-10-27, lane: V1)

- **What:** split from PB7.9 (an attended app review, F3; the person chose a profile folder of the
  app's own). Every `claude` Persona Builder starts, the app's and the CLI's (the check, each call,
  the sign-in), runs with `CLAUDE_CONFIG_DIR` set on that child to
  `~/Library/Application Support/Persona Builder/claude-code` (0700, its parent too; a looser mode
  tightened, a link or file refused), so its runs never load the person's own `~/.claude` setup.
  The one finder (`find_in`, now given the home) attaches it, so no route can miss it. *Connect
  Claude* offers *Sign in to Claude* when Claude Code runs but isn't signed in: it runs
  `claude auth login --claudeai` with no terminal, waits (Cancel, leaving the screen, 10 minutes or
  quitting stops it), then checks again. The signed-out message names the `claude` it checked
  ("Claude Code 2.1.292 at /opt/homebrew/bin/claude isn't signed in for Persona Builder yet…", home
  shown as `~`). The CLI gains `persona-builder claude sign-in` (the terminal inherited). Amends
  PB-D9: Persona Builder starts Claude Code's login; the person still signs in on Claude's page, and
  it never reads, sets or passes a credential. Brief: `plans/briefs/PB7.9b-sign-in-to-claude.md`.
- **Checked at pickup** (the spec's open point): Claude Code 2.1.292's login, run with stdin from
  `/dev/null`, no terminal and a throwaway profile, opens its page with `open` from `PATH`, whose
  redirect is a localhost callback, and waits without exiting at end of input: it completes
  without a terminal. Its paste-the-code fallback needs one, so only the CLI offers it.
- **Who built it:** Codex, from the brief in one pass, plus two review fix-ups (the new DTO field
  moved last; no markdown in the plain-text install note) and the Verifier's should-fix.
- **Verified:** red first for each new test (Codex's report: agent profile, status and sign-in
  tests, the API sign-in slot, the CLI, the lockdown list, the screen); six mutations caught (the
  profile variable dropped, the already-signed-in short-circuit, the busy check, offering sign-in
  for a `claude` that doesn't run, the unmount cancel, the 0700 tightening). Live, by V1: the built
  CLI's `claude check` against the real Claude Code 2.1.292 with a throwaway home gave the new
  sentence and made both folders 0700. `ci-local.sh` green on `333beb0`.
- **Security review:** no high or medium. One low, kept as a known limit: the home is masked in
  the shown path only when the path is spelled under `$HOME`.
- **Verifier (Fable):** PASS, no must-fix. Should-fix done: a cancel that lands during the check
  before the login now never starts it (test `cancel_during_status_never_starts_login`, red then
  green). Not taken: an ownership check on the profile folder (every foreign-owned case she traced
  fails safe). Noted limits, also in `.ai/SECURITY.md`: a force-quit leaves the login waiting for
  its callback; a cancel from a screen being left could, within milliseconds, stop a sign-in the
  re-opened screen starts.
- **Not verified:** a real sign-in end to end (needs the person at the browser), that the profile's
  login is kept apart from `~/.claude`'s in the Keychain, and the screen in the running app: owed
  as a walk. The VO2 rename moves the app's name: the profile folder's name changes with it, so a
  renamed app asks for one more sign-in unless VO2 moves the folder.

## PB2.7: the state lock never reads busy just because another thread started a program (2026-10-26, lane: V1)

- **What:** filed by V1 after `pack_clears_leftover_and_busy_refuses` failed a records-only land on
  `main`'s code. A child another thread starts shares the state lock's open file description from
  fork until exec (`O_CLOEXEC` closes it only at exec), so a lock taken again in that window read
  busy ("already working on this assistant in another window") with nothing working.
  `lock_state_folder_after` now tries a refused (`EWOULDBLOCK`) lock again on the same descriptor
  for up to 300 ms, 2 ms apart, before it answers busy; the folder-identity check after it is
  unchanged. Every caller (CLI, API, grow, agent) goes through it. The comment in
  `builder/agent/tests/lock.rs` now gives the cause of the 2026-10-25 flake that moved its test
  there. Brief: `plans/briefs/PB2.7-lock-beside-a-spawn.md`.
- **Who built it:** Codex, from the brief in one pass.
- **Verified:** cause first, on `28fa5ea`: a scratch program's 20,000 lock-and-drops read busy 0
  times alone and 1,402 and 1,487 times beside a thread running `true` in a loop; `use_it` failed 4
  of 30 parallel runs, 0 of 30 single-threaded. Red: the new test read busy 873 of 2,000 times
  before the fix (Codex); green after. Mutation: grace set to 0, caught (93 of 2,000; V1). `use_it`
  30 of 30 (Codex) and 10 of 10 (V1) in parallel; ingest and agent suites, clippy, fmt green.
- **Security review:** no high or medium. One low, already a known limit (`.ai/SECURITY.md`, the
  spending row): a run reads the limit just before it takes the lock, and a run that meets a save
  now waits it out (up to 300 ms) instead of being refused, so it may run at the limit it read just
  before that save. The row now says so; re-reading the limit under the lock is left for later.
- **Verifier (Fable):** PASS, no must-fix. Her own probe confirmed the cause on both of Rust's macOS
  spawn paths (posix_spawn and fork); the longest window she produced was 77 ms against the 300 ms
  grace, at 2 to 3 times the cores' load. She weighed `fcntl` locks (per process, so a second lock
  in one process would pass) and an in-memory registry (no help across processes) and found the
  bounded retry the right call. Her should-fix, a comment on what a failure of the spawn test
  means, is in `e14b522`. Note: a lock that's really held now answers busy after 300 ms.
- **Landed:** `7ad91f6`, `f8c5628`, `e14b522`; `ci-local.sh` green at the land.
- **Not verified:** the app's own threads under a real build (the unit test drives the same function).

## PB7.9a: 0 means no limit, and *Save the limit* is greyed out only when a value is invalid (2026-10-26, lane: V1)

- **What:** split from PB7.9 (an attended app review, F1 and F2). 0 in either field of the
  daily limit means no limit, allowed past the $1,000 cap that bounds a typed amount; "step ≤
  daily" applies only when both are set. With no step limit but a daily one, each call is given
  what is left of the day (rounded down to whole cents) as `--max-budget-usd`; with no limit at
  all no budget is passed, and a call that reports no cost is counted at $0 (`cost_from:
  unknown`). One `Cap::budget_for` decides for the ledger and the API's "limit reached" message.
  Connect Claude checks both amounts as they are typed, by the backend's rules, shows the reason
  beside the field (`is-invalid`, `aria-invalid`, `aria-describedby`), and enables *Save the
  limit* whenever both are valid, the unchanged defaults included. The spend line on Build and
  Refresh says when there is no daily limit; the CLI's flags say "0 for no limit". Brief:
  `plans/briefs/PB7.9a-no-limit-and-save.md`.
- **Decision taken at pickup:** the card said a step with no limit "omits `--max-budget-usd`".
  With a daily limit set, that would let one call run past the day, against the screen's promise,
  so such a call is given the rest of the day instead; the flag is left out only when neither is
  set. The maintainer may overrule it.
- **Who built it:** Codex, from the brief in one pass.
- **Verified:** red first for every group (Codex's report lists each failure seen), then green.
  Mutations, each caught: no remaining-day branch, always passing the budget, `unchangedLimit` back
  on the button, dropping "both above 0" on step ≤ daily (Codex); dropping the screen's `daily > 0`
  guard (V1). V1 re-ran the agent crate's tests, the CLI's limit tests (they drive the built binary)
  and the whole UI suite (271). Full `ci-local.sh` passed at the land. Security review: no high;
  one medium, plausible (below). Verifier (Fable): PASS, no must-fix; a 200,000-pair float probe
  found the overshoot never above 1e-9 and every budget whole cents.
- **Known limit (security review, documented, not fixed):** with a daily limit and no step limit,
  the day's last call can be given as little as $0.01. Claude Code's help doesn't say whether
  `--max-budget-usd` stops a call before its first turn is billed, so that call may take the day
  over by what it costs; every later call is then refused. The screen's help line and
  `.ai/SECURITY.md` say so. A floor on the last budget would be a new product rule, left to the maintainer.
  In that mode a killed or cut-off call is counted at the whole rest of the day.
- **Not verified:** a live Claude call (it would spend real usage) and the screen's look in both
  themes, owed to the next attended walk.

## PB8.3: the record and look-up texts can't run as shell code (2026-10-26, lane: V1)

- **What:** an assistant's `SKILL.md` used to have Claude write each question, pick and answer
  as a double-quoted shell word, so `$(…)` or backticks in one ran when Claude ran the command.
  `similar` and `record` now take `--stdin`, in the app's `persona-builder` and the guided build's
  `scripts/crew_decisions.py` alike: one JSON object (at most 64 KiB, strict UTF-8, only the known
  keys, no key twice, text values; `rule` and `answer` may be null). Anything else is a usage
  error and nothing is written. The template and the guided build's backtest brief pass the
  texts through a quoted heredoc (`<<'END_OF_TEXT'`), which the shell does not expand and valid
  JSON cannot end early. The flags stay for people typing the commands. Brief:
  `plans/briefs/PB8.3-stdin-texts.md`.
- **Who built it:** Codex, from the brief in one pass plus the Verifier's should-fixes.
- **Verified:** red first, then green. Tests run the made `SKILL.md`'s own blocks through
  `/bin/sh` (Rust and the guided build's Python) with a text holding `$(touch …)`, backticks,
  `${HOME}`, quotes, a backslash and a line `END_OF_TEXT`: nothing runs and every text is kept
  exactly. Mutation checks, each caught: the heredoc delimiter unquoted (both end-to-end tests),
  Python taking a repeated key, Python taking a lone surrogate, Rust taking an unknown key. Live:
  the built command under zsh in a scratch home kept the texts and created nothing. Targeted
  cargo tests, `tests/crew_decisions_test.py` and `tests/crew_install_test.sh` passed. Security
  review: no high or medium findings. Verifier (Fable): PASS; her probe ran 84 inputs against both
  programs with no difference, under sh, bash and zsh; four should-fixes, all fixed (a Rust
  lone-surrogate test, `similar`'s usage line naming `persona-builder`, the security paragraph's
  first sentence, and the template saying a real line break is refused). Repeat Verifier (Opus
  5.5) on the fixes and this record: PASS, no must-fix; her two should-fixes, to state the limit
  below in full here and in `.ai/SECURITY.md`, both taken (the template's wording with them).
- **Not verified:** the full `ci-local.sh` (it runs at the land). An assistant made before this
  keeps its old `SKILL.md`; make it again.
- **Known limit:** the safety rests on Claude writing valid JSON. A text with a real line break
  and a line `END_OF_TEXT` would end the heredoc early, and every line after it would then run
  as shell code (the repeat Verifier ran `touch` that way under sh, bash and zsh). The commands
  refusing the cut-off JSON does not stop those lines; writing a line break as `\n`, as the
  template says, is the protection.
- **Left:** `scripts/crew_personas.py similar` (the persona panel briefs) still takes its query
  on the command line.

## PB8.1: the installed app ships the look-up and record commands (2026-10-26, lane: V1)

- **What:** an assistant the app makes now has a `SKILL.md` whose look-up (`similar`) and record
  (`record`) commands run the `persona-builder` shipped inside the app,
  `Persona Builder.app/Contents/MacOS/persona-builder`, written as one double-quoted word
  (`"$HOME/Applications/…"`) when the assistant is made. The template names both commands
  through two new placeholders, `{{SIMILAR}}` and `{{RECORD}}`, filled first; the guided build
  fills them with its Python commands as before. New `persona-builder record`, byte for byte
  Python's `record` (timestamp aside), appends to `live.jsonl` only in a state folder that
  already exists (so it never undoes Forget), under a file lock rather than the build's lock.
  The Mac build copies the command into the bundle and signs inside out (the command, then the
  app, no `--deep`, per Apple TN2206); the installer refuses a copy without it. Brief:
  `plans/briefs/PB8.1-shipped-commands.md`.
- **Why not Tauri's `externalBin`:** its build script copies the sidecar into the cargo target
  folder under its bare name, the CLI's own output name, and needs a target-triple copy a
  universal build doesn't make (`tauri-build` 2.7.0 `copy_binaries`).
- **Who built it:** Codex, from the brief in two parts, plus a follow-up (U+2028/U+2029 refused
  in the path) and the Verifier's fix-ups (a value starting with `-`, `record --help` naming the
  assistant's flags, a symlink test that proves `O_NOFOLLOW` on its own).
- **Verified:** red first, then green. Mutation checks, each caught: creating a missing state
  folder, accepting `$` in the path, a 65 KiB line limit; dropping `O_NOFOLLOW` first survived (the
  symlink test's target was readable by others, so the mode check refused it), and the test was
  fixed. Live: `bash scripts/build-mac-app.sh` built the real bundle; installed into a scratch home
  with `install.sh --no-open`, the bundled command made an invented assistant, and the
  `SKILL.md`'s own `similar` and `record` lines, run through `bash`, found the invented precedent
  and wrote one 0600 line; after `forget --yes` the record line refused and recreated nothing.
  `bash scripts/ci-local.sh` passed. Security review: no high or medium findings. Verifier
  (Fable): PASS, two should-fixes, both fixed. The repeat pass (Opus) then failed the first fix:
  taking every value that starts with a dash let a flag left empty swallow the next flag's name
  and write a wrong line, where Python refuses; a second repeat pass found the same through a
  following `--flag="text with a space"`. Fixed with a rule stricter than Python's on purpose: a
  value given on its own that starts with two dashes, or with one dash and no space (a lone `-`
  and a negative number aside), is refused; such a value can still be given as `--flag=value`.
  So a flag left empty never takes another flag as its value. One difference is kept on purpose:
  Python refuses a value that starts `-h` and has a space (`"-how should we name it?"`), because
  argparse reads it as its help flag; Rust keeps it as the text it is. Every other difference a
  916-case probe found is a Rust refusal. A test covers each case, and `record --help` works again
  without an assistant folder.
- **Not verified:** making an assistant from the app's window (the app path is covered by
  `builder/api/tests/assistants.rs` and the Verifier's reading of the bundle; owed with PB8's
  walk). An assistant made before this keeps its old `SKILL.md`; make it again.
- **Left, as cards:** the path is accepted from any location (an app opened from a disk image, or
  run translocated by Gatekeeper, would write a path that goes away); the template's
  double-quoted texts Claude fills (`"<q>"`, `"<pick>"`) would run `$(…)` (the same with Python);
  `crew/_template/decisions.md` still mentions `crew_decisions.py similar` in prose.

## PB7.7d: stop for the day when Claude Code says the plan is nearly used up (2026-10-26, lane: V1)

- **What:** V-Q8's answer (option 1): dollars only for the limit (PB7.7c), plus a guard from
  the plan. Each call now reads Claude Code's stream (`--output-format stream-json --verbose`) in
  place of the single JSON reply, because only the stream carries a `rate_limit_event`. The worst
  plan status a call reports (`allowed_warning` or `rejected`) goes on its ledger line as `plan`.
  Any such line today refuses every later call for that assistant until the next local day, with a
  fixed sentence the screens show (`text::PLAN_STOPPED`). A rejected plan explains a failed call
  (outcome `plan-limit`). A ledger `plan` value other than those two reads as a damaged ledger, so
  nothing spends. The percentage choice from the mock isn't built. Brief:
  `plans/briefs/PB7.7d-plan-guard.md`.
- **Sources:** the Agent SDK's `RateLimitEvent` (Python docs) and the headless docs' stream-json
  section, read 2026-10-26; the event's exact form (`type: "rate_limit_event"`, `rate_limit_info`
  with `status`) was read from Claude Code 2.1.292's own code.
- **Who built it:** Codex, from the brief, plus two follow-ups: a single ledger read in `start`,
  and a test the Verifier asked for. The brief's private `plan_stopped_today` was folded into
  `today()`.
- **Verified:** red first (all ten plan tests, the argv test and the api test failed for the stated
  reasons), then green; mutation checks, each caught: no `--verbose`, `start` ignoring the stop,
  `allowed` counted as stopping, first result instead of last, any ledger `plan` value accepted,
  the rejected rewrite limited to `error`. Security review: no findings. Verifier (Fable): PASS. One
  should-fix (a rejected plan with no result line, or no answer) is covered by
  `a_rejected_plan_explains_any_failed_reply`. `bash scripts/ci-local.sh` is the land's gate (`direct-to-main.sh land`).
- **Not verified:** a live signed-in call. From this job run Claude Code isn't signed in, so the
  stream's shape is proven by stubs and by Claude Code's code, not on the wire. If the live result
  line lacked `structured_output`, every call would end `no-output` and fail (fail closed: counted
  at Claude Code's cost, or at its budget when none is given). Owed as a walk.
- **Expected behaviour (not bugs):** a rejected plan with a signed-out reply shows *not signed in*
  now and the plan sentence on the next call. When both limits are reached, the screens show the
  dollar sentence. The stop is per assistant.
- **Known limits:** in `.ai/SECURITY.md`. Claude Code reports only on a change, at a level it
  doesn't document. A stopped or cut-off call leaves no output.

## PB7.7b: each screen's words checked against its mock (2026-10-26, lane: V2)

- **What:** split from PB7.7 (the reviewed mocks). The screens take the mocks' words wherever the app
  shows the same thing and the mock's sentence is true of what the app does. The changes:
  - "answered questions" throughout, "messages you typed" and "kept back to test".
  - The assistant's name in the Sources, Build, Report and Save headings (no pronouns: people type
    their own, so the grammar can't be known).
  - "Start the build", "Run the build again" and "Stop for now", with a help line about what stopping
    keeps. The Build lede no longer points at "the limit below": it says the limit is set on Connect
    Claude.
  - "Agree and continue", "How your assistant refers to you", "Keep my answer", "Your answers so far",
    "To strengthen it", "Open GitHub", "Save <name>".
  - Help lines for Connect Claude (no password; Claude only for now), for approving or rejecting a
    change, for the repo name (the rule `valid_repo_name` holds), and for turning it on.
  - The build's stopped, failed and private messages in `builder/api/src/building.rs` say "run the
    build again", like the button beside them.

  Left as they are, on purpose: the permission text (pinned by its SHA-1, as the record of what each
  person agreed to), the step titles down the left, the "Next: …" buttons, and everything a mock
  shows that the app doesn't have (the "Before you start" facts, the kinds of work, the sources'
  privacy counts, the report's word list). Brief: `plans/briefs/PB7.7b-screen-words.md`.
- **Who wrote it:** Codex, in three passes: the brief, a count line it had half-changed, and the
  review fixes. Claude wrote the brief, checked each mock sentence against the code before taking
  it, and reviewed every diff.
- **Commits:** `c39ffc5` (brief), `c083b4f` (screens and tests), `be207cf` (fixes from the reviews),
  `d731bea` (fixes from the delta pass). The brief's first version named a real person; that fix is
  folded into `c39ffc5`, so no commit carries the name (the checkout's push guard refused it).
- **Verified:**
  - Red first: the new tests run against the old screens gave 51 failures in 18 files. Then green:
    260 screen tests, the type check, and the API's 190 tests.
  - Mutations: three by the Verifier (stop help, privacy hint, repo-name hint), each red and put back
    with `cmp`. Codex ran the review fixes red first.
- **Reviews:**
  - `/security-review` (sub-agent): three Lows, all fixed. The review hint had said "It's checked
    before it's kept", but the filters catch private topics and secrets, not names. The turn-on line
    had said turning off "removes exactly what was added", but a changed file is left where it is.
    The brief named a real person.
  - Verifier (Fable; the lane runs her on every PB7 slice): the code PASS, with every new sentence
    traced to the code. Her must-fix was the same name. Her should-fix was the Rust build messages
    still saying "building again" under the renamed button; that is fixed here too. The delta pass (Opus): PASS, with three mutations of
    her own red (the stopped, failed and private messages). Two of her notes are fixed in `d731bea`:
    turning off removes the whole block in the instructions even if it was edited, so the line now
    says "except a file you've changed since"; and the daily-limit message says "Run the build again
    tomorrow". Left: the "but not names" hint is true of the template's filters, and an assistant
    whose own filters list names catches more than it says (harmless).
- **Not verified:** no live walk of the screens (owed to the next attended walk). Still in old words, noted by the
  Verifier and left as they are: "Drafted <date>" (the mock says "Change drafted"), and the Save
  cards' headings "Use it in Claude Code" and "Use it in claude.ai" (the mock groups them under
  "Turn <name> on").
- **Skills Review:**
  - ✅ plain-language: each sentence taken from the reviewed mocks, and only when true.
  - ✅ tdd-skill: red first (51 failures), mutations red.
  - ✅ security-and-hardening: two sentences that promised more than the code does, fixed.
  - ⚠️ project-config: the brief said `node_modules` was installed, but a fresh worktree has none,
    so Codex's first pass couldn't run the tests. Install with `pnpm -C builder/ui install
    --frozen-lockfile` before dispatching.

## PB7.7a: the look, readable form fields, and "this computer" (2026-10-26, lane: V2)

- **What:** split from PB7.7 (the reviewed mocks). `builder/ui/src/theme.css` sets the Navy & Amber
  palette from the mocks over Tabler, in both themes. The app loads it right after Tabler's CSS, and
  a host app can load it as `@persona-builder/screens/theme.css` (the README says so). Form fields
  get a literal colour per theme (`--tblr-bg-forms`, `--tblr-bg-forms-disabled`), and each theme sets
  its own `color-scheme`. Before, Tabler's `[data-theme=dark]` rule set `color-scheme: dark`, so
  `light-dark()` put a near-black field under the palette's navy text whenever a host page's theme was
  dark. Every user-facing "this Mac" or "your Mac" now says "this computer" (`NOT_INSTALLED`,
  `DOES_NOT_RUN`, `API_KEY_SET`, the `only_mine` refusal, the claude.ai unzip line). "Show in Finder"
  now says "Show the folder" (assistants and the claude.ai files), and `.ai/SECURITY.md` matches.
  Brief: `plans/briefs/PB7.7a-look-fields-this-computer.md`.
- **Who wrote it:** Codex, in one pass. Claude wrote the brief, reviewed the diff and ran the live
  check.
- **Commits:** `95530a5` (brief), `4233ff3` (code and tests).
- **Verified:**
  - Red first, then green. These tests are new: `builder/agent/tests/text.rs`, the extra assertion in
    `builder/ingest/tests/private_at.rs`, `builder/ui/src/wording.test.ts` (no `Mac` or `Finder` in any
    screen source), and `builder/ui/src/theme.test.ts` (literal colours, no `light-dark(`, Tabler
    loads first, the export exists).
  - Seven mutation checks each turned a test red, and each was put back and checked with `cmp`.
  - Rust: 332 tests pass. Screens: 230 tests pass, plus typecheck and build.
  - Live check: the built CSS in headless Chrome, in four setups: light, dark, light under a dark
    host `data-theme`, and dark under a light one. The field's contrast was 14.03:1 or 14.53:1 in
    every setup, with the palette's body, primary and field colours.
  - The control was the same palette without the field fix. Under a dark host it gave 1.28:1, the
    faint text the mock review found. Screenshots matched the numbers.
- **Reviews:** `/security-review` (sub-agent) found nothing. Verifier (Fable; the lane runs her on
  every PB7 slice): PASS. She found 344 Rust and 247 screen tests passing, clippy and fmt clean, and
  5 mutations of her own each red. Her WCAG figures are at least 4.5:1 for every text pair in both
  themes. Two notes are left for later. A host that sets `data-theme="dark"` on a wrapper, not on
  `<html>`, still flips Tabler's link, alert and hover colours, but fields and body text stay
  readable. Tabler's placeholder grey is 2.52:1 on the white light field (older than this slice).
  Her should-fix is done: the code commit no longer carries a "By Codex" line.
- **Not verified:**
  - No Windows or Linux webview was checked; only Chrome on this Mac.
  - The root `README.md` still says "your Mac" when it describes the kit. It is outside this item.
  - Each screen's wording against its mock is PB7.7b.
- **Skills Review:**
  - ✅ tdd-skill: red first, seven mutations.
  - ✅ frontend: one stylesheet, loaded after the framework and exported for a host.
  - ✅ design-guide: literal per-theme field colours, with the contrast measured.
  - ✅ plain-language: "this computer", "Show the folder".
  - ✅ security-and-hardening: nothing remote in the CSS, and no warning text loses meaning.
  - ⚠️ source-driven-development: Tabler's own built CSS (1.6.1) was the source, not its docs.

## PB7.5d: the CLI's build, interview draft and keep take the state lock before they read (2026-10-26, lane: V1)

- **What:** filed from PB7.5c's reviews. The three CLI commands read history, then took the
  creating state lock, then wrote files made from what they read, so a forget in between left a
  recreated state folder holding them. Now `build` opens the ledger only on a state folder that
  already exists (`Session::open_existing`, as the app does) and prepares again under the lock;
  `interview draft` does the same when the folder was there at its first look (else it may make
  it, as before) and plans again under the lock (`draft_plan`); `keep` takes the non-creating lock
  before it reads the signal and keeps it with `keep_locked`. A folder forgotten in the race reads
  as the PB7.5c "isn't there yet" sentence. The PB7.5d known limit is gone from `.ai/SECURITY.md`.
  `consent agree` is left as is (only a version and a date). Brief:
  `plans/briefs/PB7.5d-cli-spenders-lock-before-reading.md`.
- **Who wrote it:** Codex, one pass; Claude committed, ran the gates and wrote this record and one
  SECURITY.md wording fix.
- **Verified:** red first, in `builder/cli/tests/history_lock.rs`: a stub Claude Code whose sign-in
  check deletes the state folder (a forget between the first read and the lock), for `build` and
  `interview draft`: refused, no state folder afterwards, no Claude call, member files unchanged;
  `keep` is busy under a held lock on a signal that doesn't exist (a read first would say it
  doesn't exist), then keeps it; `keep` with no state folder creates none. Mutation checks (cp
  backup, `cmp`): by Codex, `build` on the creating open, `interview draft` always creating, always
  non-creating (the fresh-assistant `interview.rs` tests go red), `keep` reading before the lock; by
  V1, `build` on the creating open again (red). The CLI's tests, clippy and fmt green;
  `ci-local.sh` in the land (it never lands on red).
- **Reviews:** `/security-review` (sub-agent): no high or medium; two lows left: `interview draft`
  on a folder not there at first look can still, if a `mine` and a forget both land before its
  lock, leave a folder holding questions drafted from the book alone (no history); and the CLI's
  missing-file errors still print the full path (only to the person's own terminal; older than
  this slice). Verifier (Fable): PASS, no should-fix; one nit fixed (the SECURITY.md sentence now
  says no folder holding files made from the forgotten history), two left (the "isn't there yet"
  advice reads oddly for `keep`; `build` on a missing folder still shows the raw file error first).
  Out of scope, noted: `spend` and `refresh propose|apply` still use the creating open, so after a
  forget they make an empty state folder (they read history only under the lock; the limit is read
  before it, the spending row's known limit).
- **Not verified:** a forget from the app's window during a real `build`; the race is shown with
  the stub's sign-in check.

## PB7.5c: the command-line history commands take the state lock (2026-10-26, lane: V1)

- **What:** filed from PB7.5b's reviews. The CLI's ten history commands (`mine`, `claudeai
  titles|mark|mine`, `merge`, `split`, `chunk`, `distil-in`, `round`, `interview answer`) now hold
  the state lock from before their first read of `~/.claude/state/<slug>/` until they return, so a
  forget (app or CLI) is refused as busy while one runs, or the command finds nothing; before, one
  running during a forget could write part of the history back. `mine` and `claudeai titles` may
  make the state folder (creating lock); the other eight take the non-creating lock and answer a
  missing folder with a fixed sentence (`claudeai mark|mine` keep today's "no review file yet").
  Dry runs take no lock. A failed `mine` or `claudeai titles` (a missing projects folder or
  export) checks those before the lock, so it leaves no empty state folder. The PB7.5c known limit
  is gone from `.ai/SECURITY.md`; a new one names PB7.5d. Brief, with its fix-up pass:
  `plans/briefs/PB7.5c-cli-history-lock.md`.
- **Who wrote it:** Codex, in two passes (the slice; the review fix-ups). Her sandbox can't bind
  sockets, so Claude committed, ran the gates and wrote this record.
- **Verified:** red first (`builder/cli/tests/history_lock.rs`: busy for each of the ten, files
  unchanged, then success; nothing recreated for the eight; a dry run not refused; no state folder
  after a missing projects folder or export). Mutation checks (cp backup, `cmp`): by Codex,
  removing `merge`'s lock, making the lock always creating, locking `mine`'s dry run, and moving
  `claudeai titles`'s lock below its read (caught by a junk review under the lock); by V1, removing
  `round`'s lock (both its tests red). The CLI's tests, clippy and fmt green; `ci-local.sh` in the
  land (it never lands on red).
- **Reviews:** `/security-review` (sub-agent): no high; one low from this slice (the empty state
  folder after a failed `mine`/`titles`), fixed; one medium predating it, filed as PB7.5d
  (`EMin30aO`): the CLI's `build`, `interview draft` and `keep` read history before they take the
  creating lock. Verifier (Fable): PASS, two should-fixes (the claudeai lock-order test; "a forget
  waits" was wrong, it is refused as busy), both fixed, plus two nits fixed (two lock helpers, a
  test comment).
- **Not verified:** no live race with a real forget during a long `mine`; the lock is shown by
  holding it from the test. `consent agree` still writes `consent.json` with no lock (noted on
  PB7.5d).

## PB7.7c: a daily limit in dollars, set on Connect Claude (2026-10-26, lane: V1)

- **What:** split from PB7.7. Each assistant keeps its own limit in `limit.json` in its state folder
  (0600; default $50 a day and $5 a step when none is saved). Every app command that spends (build,
  the interview draft, the refresh draft and its test) opens the ledger at that limit, so the
  build's *Today's use* shows it. Connect Claude gains the mock's *How much it may use* card, dollars
  only: *Stop for the day at about* and *Most one step may use*, *Save the limit*. Two app commands
  (`spending_limit`, `set_spending_limit`); the CLI gains `limit show|set`, and its
  `--daily-cap-usd`/`--call-cap-usd` flags now default to the saved limit. `Cap::new` now also needs
  each step at least $0.01 and both amounts in whole cents, since Claude Code gets its budget to
  the cent. Brief (two passes): `plans/briefs/PB7.7c-daily-limit.md`. Built by Codex; one copy fix
  (the load-failure sentence) and the $0.01 floor by Claude.
- **Verified:** red first for the new tests (agent `cap.rs`, api `building.rs` and its race tests,
  CLI `agent.rs`, `ConnectClaude.test.ts`); mutation checks by Codex (four: the default cap in
  `open`, the consent re-check under the lock, `load` accepting a refused file, clamping an explicit
  step cap) and V1 (the $0.01 floor). `ci-local.sh` green on the head.
- **Reviews:** security review (sub-agent): two lows; the sub-cent step budget fixed, the
  load-before-lock window recorded as a known limit in `.ai/SECURITY.md`. Verifier (Fable): FAIL on
  a stale test sentence (fixed), three should-fixes fixed (the CLI's missing-folder sentence, a test
  for an explicit step cap above the day, whole cents); repeat (Opus): PASS, three nits left (a
  value within 1e-8 of a cent is kept unrounded; no sub-cent damaged-file case in the test; 10000 on
  the screen gets the amount sentence, not the $1,000 one).
- **Known limits:** a run reads the limit just before it takes the state lock, so a limit saved in
  that moment applies from the next run; a limit set below today's spend shows only when the next
  step is refused. The share-of-plan choice is PB7.7d, parked on V-Q8.
- **Not verified:** the screen in a window (owed as a walk), and a real build at a saved limit.
- **Rebased onto PB7.5a** (V1, 2026-10-26): `building::open` loads the saved limit first, then
  opens with PB7.5a's `Session::open_existing` (a missing state folder still reads as no saved limit,
  then the permission sentence); `.ai/SECURITY.md`'s spending row carries both slices' sentences.
  `/security-review` (sub-agent) on the rebased branch: no high or medium; two new lows left: the
  CLI's `limit set` checks no permission (the app re-checks it under the lock; no history is
  written), and `Cap::save` writes through a creating path, so only something outside the app
  deleting the folder between the lock and the write could recreate it holding `limit.json` alone.
  Delta Verifier (Opus): PASS, PB7.5a's race tests and the limit race tests green at the rebased
  head; nits left: a forget between the limit file's check and its read shows the damaged-limit
  sentence instead of the permission one, and that sentence covers any limit read failure.
  `ci-local.sh` runs again on the rebased head in the land (it never lands on red).

## PB7.5a: app commands that spend open the ledger only on an existing state folder (2026-10-26, lane: V2)

- **What:** a finding from PB7.5's Verifier pass and security review (inherited from PB7.4). The
  app's one ledger opener (`building::open`: build, the interview draft, the refresh draft and
  test) opened the spending ledger with a lock that creates the state folder, so a forget landing
  between the permission check and that open recreated an empty state folder (no data written, no
  call made). It now uses a new `Session::open_existing`, on the non-creating state lock; the CLI
  keeps today's open. The state lock itself now checks after `flock` that it holds the folder at
  the path (device and inode): a folder forgotten or replaced between the open and the lock
  answers the permission sentence (non-creating) or "busy" (creating). That closes the item
  PB7.3a left. PB7.5's two refresh forget tests now check no state folder is left. No new
  sentence. Brief: `plans/briefs/PB7.5a-ledger-on-an-existing-state-folder.md`.
- **Who wrote it:** Codex, in five passes (the slice; the lock identity fix-up; the rebase onto
  PB7.5; one more test; the test race below). Her sandbox refuses commits and socket tests, so Claude committed, ran
  the gates and wrote this record.
- **Verified:** red first in each pass, then green, with a mutation check per claim (cp backup,
  restored by `cmp`): five across the first three passes, as the brief lists them, and in the last
  the re-open after `flock` made creating, which turns the new
  `creating_lock_refuses_a_folder_forgotten_before_the_lock` red on the folder still existing.
  `ci-local.sh` green on `29c773a` before the rebases onto PB8 and PB7.5b, and on the rebased head
  in the land (it never lands on red). `/security-review` (sub-agent) on the branch twice and on
  the last test: nothing medium or higher. Verifier (Fable): PASS, two should-fixes, both fixed;
  repeat (Opus): PASS, one should-fix (the last test), fixed; a delta pass (Opus) after the
  rebase onto PB8: the code unchanged by it and the new test closing that should-fix, both proven.
- **The red gate, and its fix:** the first land's `ci-local.sh` failed on
  `lock_with_nothing_between_still_locks` (busy just after a lock was dropped): the ingest lib tests
  failed about two runs in three at the default thread count (earlier runs used two threads, which
  hid it). The cause is a test race, not a product bug: a child started by one lib test holds a copy
  of every open descriptor until it execs, another test's lock included. The state lock's
  folder is opened close-on-exec, so a started child (Claude Code, in the app) lets go of the lock
  once it runs: proven by a probe, kept as `a_started_child_keeps_no_lock_once_it_runs`, which goes
  red with `O_CLOEXEC` removed. The ingest lib tests that lock or start a child now take one shared
  test mutex (`serial_test`), as the `grow` and `api` tests already do: 0 of 40 runs failed at the
  default thread count (and 0 of 20 in Claude's own check), against 34 of 40 with a fresh mutex per
  call. `/security-review` (sub-agent) on this delta: no findings; a delta Verifier (Opus): PASS,
  two wording nits (this line, fixed; the new test's comment says its 200 ms wait lets the child
  exec, though `spawn` likely returns after the exec already: harmless, left).
- **Not verified, left:** no live proof in the app: the race needs a forget inside two system
  calls, which only the test seam reaches. Rebased onto PB7.5b, whose command-line forget holds
  `lock_existing_state` and so now gets the same check (by reading the code, a folder replaced between the open and the
  lock prints "Nothing to forget"); its tests ran green on the rebased head, no new test for that
  pairing. The CLI's own history commands still take no lock (PB7.5c).

## PB7.8: a short interview for the thin kinds only (2026-10-26, lane: V1)

- **What:** from V-Q7 (answered with option 1). With 150 or more answered questions, `interview draft`
  (CLI and app) no longer refuses when the report has thin kinds of decision: it drafts four questions on
  each of the five thinnest kinds (fewer than five examples, never `other`), in its own form
  (`interview::Form::Short`: round `interview-short`, files `interview-short-questions.json`,
  `interview-short-answers.json`, `decisions-interview-short.jsonl`, always trained), so a kept full
  interview is never overwritten. The report's item and the app say *Answer a few questions about it*,
  with an *Answer a few questions* button that leads to the interview; the interview card names the
  kinds. `interview answer` takes `--corpus` and follows the same form. Brief (three passes):
  `plans/briefs/PB7.8-short-interview.md`. Built by Codex.
- **Verified:** red first for every new test (build, report, ingest, CLI, api, both screens, the
  installer); mutation checks by Codex (three), the Verifier (five) and the repeat pass (four), each
  caught and restored with `cmp`. `ci-local.sh` green on the head (twice: before and after the second
  fix-ups). No live Claude call (fake Claude only; a real draft spends plan usage).
- **Reviews:** security review (sub-agent): two lows, both fixed (the installer's merge and host
  names; an empty interview refused before any call). Verifier (Fable): PASS, two should-fixes (a thin
  `other` row was a dead end: fixed; the form follows the count: a known-limit line in
  `.ai/SECURITY.md`). Repeat (Opus): PASS.
- **Known limits:** with a long history, a sixth thin kind, or a thin row no longer in the codebook,
  still gets the item though this short interview won't ask about it (the next one asks the thinnest
  five again); a second short interview replaces the first one's kept answers, as the full one does.
- **Not verified:** the screens in a window (owed as a walk), and a real draft through Claude Code.

## PB7.5b: the command-line forget takes the state lock (2026-10-26, lane: V1)

- **What:** `persona-builder forget <member> --yes` now previews first (keeping its refusals for a
  `state_dir` outside `~/.claude/state/<slug>`, a link or a non-folder), then holds
  `lock_existing_state` for the delete: it is refused as busy, with nothing deleted, while the app or
  another command holds the lock, and it never creates the state folder (a folder gone by the time
  of the lock prints the usual "Nothing to forget"). The preview takes no lock. Brief:
  `plans/briefs/PB7.5b-cli-forget-lock.md`.
- **Behaviour change, accepted:** when `~/.claude` itself is a link, the preview still counts the
  files but `--yes` now refuses with the lock's "is a link or not a folder" sentence (before, it
  deleted). That matches the app, whose history commands refuse a linked Claude folder too.
- **Verified:** two new tests in `builder/cli/tests/forget.rs` (busy, then deleted once the lock is
  dropped; a missing folder is never made), the busy one red first; mutation checks by Codex, V1
  and the Verifier (four: dropping the lock fails the busy test; a creating lock fails the
  missing-folder test unless it is behind the existence check, a race covered by reading the code,
  not by a test). `ci-local.sh` green.
- **Reviews:** security review (sub-agent): nothing medium or higher. Verifier (Fable): claims 1, 2
  and 6 proven, 3 to 5 partly: the SECURITY row put the CLI under the app's confirmed-count rule
  (fixed), the linked-`~/.claude` change above (recorded), and the CLI's own history commands take
  no lock at all, so one running during a forget can write history back (pre-existing; filed as
  PB7.5c, `hrXidQ59`, and noted as a known limit in the row).

## PB8: the unsigned Mac app and its installer (2026-10-25, lane: V1)

- **What:** `bash scripts/build-mac-app.sh` makes an ad-hoc signed `Persona Builder.app`
  (`bundle.macOS.signingIdentity: "-"`, hardened runtime; PB-D3: no Apple signing until the public
  release) and zips it with `builder/install/install.sh` and a plain `How to install.txt`, plus the
  zip's SHA-256. The installer needs no password and no Homebrew: it checks the signature and the
  chip, copies the app into `~/Applications`, clears the download quarantine on that copy only,
  checks the signature again and opens it. `--universal` makes a copy for both chips (needs both Mac
  Rust targets). Brief: `plans/briefs/PB8-mac-installer.md`.
- **Decided at pickup (the plan didn't say):** `~/Applications`, not `/Applications` (no password);
  the installer refuses a copy built for the other chip in plain words; the person compares the
  SHA-256 in Terminal before running the installer (step 3 of the how-to), since the installer
  travels in the zip and can't vouch for itself; the build runs only from a git checkout and refuses uncommitted
  changes under `builder/`, `crew/` and the Cargo files, because the app embeds the template and the
  screens; the SHA-256 goes in a separate message that says to check it first.
- **Verified:** `tests/mac_install_test.sh` (eight cases on fake signed apps, a space in the path;
  wired into `ci-local.sh`), red first for the config test; mutation checks by Codex (seven), V1
  (one) and the Verifier (ten: each case live; the two signature checks and the quarantine re-check
  overlap by design). Live: the real app built on an Apple-chip Mac (18.5 MiB, arm64,
  `Signature=adhoc`), the zip unpacked both as the Finder does and with Terminal `unzip`, marked
  as a download and installed into a scratch home: signature valid, no quarantine left, the
  unpacked original untouched. `spctl` rejects it, as expected for an app Apple has not notarized:
  with the quarantine cleared, Gatekeeper doesn't check it. `ci-local.sh` green.
- **Reviews:** security review (sub-agent): one medium, the SHA-256 step missing from the how-to
  and the SECURITY row overclaiming; fixed. Verifier (Fable): five of six claims proven, the sixth
  partly (the zip's AppleDouble entries broke a Terminal `unzip`; fixed with `--norsrc
  --noextattr`). Repeat pass (Opus): no must-fix; its three should-fixes are in (a case for a
  symlink out of the bundle, caught when the first signature check is removed; the build's check
  fails closed outside a git checkout; the separate message carries the SHA-256).
- **Not verified:** the app opening on a person's Mac (a window; owed to the attended install),
  Intel Macs (no Intel Rust target on the build Mac). Left as known: the binary carries the build
  account's home path in panic locations (cargo registry and rustup toolchain paths; no project path).

## PB7.5: how well it knows you, and review, from the app (2026-10-25, lane: V1)

- **What:** the app's last two empty steps. *How well it knows you* reads the last build's report
  in plain words: overall and per kind of decision (Strong / Getting there / Thin, as badges), what
  would help where it is thin, the honest line when fewer than 30 questions were kept back to test
  it, and how the assistant has grown since the build. *Review* keeps an answer the person writes
  (an `instruction`, or a `correction` when they say what the assistant answered; source
  `review`), shows how many tests the answers have made, and runs a refresh: Claude drafts a
  change, each point is shown as before and after with its reason, the person approves or
  rejects it, and an approved change is tested on all their answers with its lines as they come,
  then kept or undone. Six new app commands in `builder/api/src/review.rs` (44 in all), each in
  the parity table. Every step in the side list now has a screen. Brief:
  `plans/briefs/PB7.5-report-and-review.md`.
- **Shared with the CLI:** the grow crate gains `keep_locked` and `answer_locked` (the CLI's
  `keep` and `answer` minus the lock, so the app runs them under its own non-creating lock with
  the permission re-checked), `refresh::waiting` (the open change and whether it is approved),
  `diff::removed_lines`, `apply::growth_counts` and a public `apply::reason`. The test run sends
  progress lines (as it is now, with the change, checking the written answers), and the CLI's
  `refresh apply` prints them before its outcome; nothing else in the CLI's output changed.
- **Decided at pickup (the plan didn't say):** the webview names only a change it was shown (an
  answer only for the waiting, unapproved change; a test only for the waiting, approved one; else
  "that change isn't waiting for your answer any more", before any write or call); the app keeps
  `instruction` and `correction` answers only (no report marks or picks yet); no stop for a
  refresh's test (it writes nothing to the assistant until it finishes); finishing a test whose
  result couldn't be saved stays a CLI step. No new crate or package (`persona-builder-grow` is a
  workspace crate the api now uses).
- **Who wrote it:** Codex, in four passes (the Rust side; the screens; Review reloading its status
  when shown again, from Claude's review; the Verifier's fixes). Claude briefed, reviewed, wrote
  this record, ran the gates outside Codex's sandbox and committed.
- **Verified:** `ci-local.sh` green on `189e76f` (socket tests included) and on the final code
  head `8a75701`. The touched crates' tests (grow, api, cli, app) also green on `97692aa`. Codex's
  mutation checks, each red and restored by `cmp`: the waiting-change id check dropped from
  answering, an unapproved change accepted for testing, an empty `reason`, the privacy check
  dropped from keeping, `keep` in place of `keep_locked` (busy), the permission re-check after
  the ledger opens dropped (apply, then draft), the "with the change" line dropped, before and
  after swapped, a command in `COMMANDS` only, *Review* enabled before permission, *Review*
  mounted with `v-if` (its lines lost), the reload on showing *Review* again dropped.
  `/security-review` (sub-agent): nothing medium or higher; one low (below). Verifier (Fable) on
  `189e76f`: pass, all eleven claims proven, one should-fix inherited from PB7.4 (below) and four
  nits, three fixed in `8a75701` (47 lines, in files she saw: *Review* reloads by default when a
  host omits `active`, a new draft clears the last test's lines, a base moved under the test reads
  as "changed since you approved"), plus the missing draft forget-between test the security
  review asked for.
- **Not built, filed:** PB7.5a (`cjtJmnI6`): the app commands that spend (build, the interview
  draft, the refresh draft and test) open the ledger with a lock that creates the state folder,
  so a Forget landing between the first check and the open recreates the state folder holding only
  an empty `agent/` folder (no data, no call: the permission re-check refuses). Recorded as a known limit in `.ai/SECURITY.md`.
- **Not verified:** the screens in a running app, light and dark (owed walk). One run of the api
  tests inside Codex's sandbox hit an intermittent busy answer in the existing `use_it` tests;
  the rerun and both full `ci-local.sh` runs passed.
- **Skills Review:** ✅ tdd-skill (red first; 13 mutation checks, the permission one counted twice: apply, then draft) · ✅ security-and-hardening (the
  webview can only act on the change it was shown; permission re-checked under every lock) · ✅
  frontend (Review kept mounted across steps and reloaded when shown; loading, empty, error
  states) · ⚠️ design-guide (light and dark not seen: owed walk) · ✅ plain-language (every
  sentence from the brief, checked word for word by the Verifier) · ✅ source-driven-development
  (built on the grow crate's own keep, refresh and apply).

## PB7.4: build, stop, spend and the interview, from the app (2026-10-25, lane: V1)

- **What:** the app's *Build* step. It shows today's spend against the daily limit, builds the
  assistant on a worker thread with the build's plain lines arriving as they come (each with the
  spend just then), and has a *Stop* that ends the running Claude Code call and writes nothing, so
  the assistant stays as it was. When the history is thin (under 150 answered questions), an
  interview card drafts twenty questions through Claude Code, shows them four at a time with the
  answers kept as the person goes (`interview-answers.json`, the round-answers shape), keeps them as
  `decisions-interview.jsonl`, then puts the history together and builds again. Seven new app
  commands in `builder/api/src/building.rs` (38 in all, with PB7.6b's), each in the parity table.
  The build stays mounted while the person looks at other steps, and *Your assistants* is disabled
  while it runs. Brief: `plans/briefs/PB7.4-build-and-interview.md`.
- **How the stop works:** a per-assistant flag (`Builds`, process-wide, registered before the
  gate) reaches `claude.rs` through `Claude::with_stop`: a set flag refuses a call before it starts
  (no ledger line, no child), and `child.rs` kills the running child's whole process group and
  reaps it, the call counted on the ledger at its whole budget (outcome `stopped`). The flag is read
  once more after the privacy check and before anything is saved. Steps already paid for stay in
  `build/`, so building again carries on.
- **Shared with the CLI:** the CLI's `build` body moved into `builder/build/src/run.rs`
  (`prepare`, `check`, `save`), which the app calls too; the CLI's output is unchanged (its tests
  pass as they were). `interview draft` without `--labels` now weights the questions from the last
  report's areas, in the CLI and the app alike.
- **Decided at pickup (the plan didn't say):** the app uses the default limit ($50 a day, $5 a
  call) with no setting for it; changing it stays a CLI flag. No new crate or package
  (`persona-builder-build` and `persona-builder-steps` are workspace crates the api now uses).
- **Who wrote it:** Codex, in three passes (the slice; Claude's review fixes; the Verifier's
  should-fixes). Claude briefed, reviewed, resolved the rebase onto PB7.6b, added one line (Forget
  also unmounts a stale build card), wrote this record, ran the gates outside Codex's sandbox and
  committed.
- **Verified:** `ci-local.sh` green on `6f1c779` before the rebase (898 Rust tests, socket tests
  included, plus the screens, Python suites, fmt, clippy and the build) and again on the rebased
  head `6c1a0e6` (933 Rust tests, 173 screen tests) and on `9613f48` (935 and 174), and on the final head after PB7.3a (938 and 174). Codex's ten mutation checks, each red and restored by `cmp`: the flag check dropped from the
  child's wait loop (a cut-off instead of a stop), a stopped call counted at $0, the flag read
  before saving dropped (a late stop saved), the slug registered after the lock (an early stop
  lost), the registry entry never removed, a pick past the last option accepted, the consent check
  dropped, a command in `COMMANDS` only, *Build* enabled before permission, the build card mounted
  with `v-if` (its lines lost on leaving the step). `/security-review` (sub-agent): nothing medium
  or higher; of its lows, the checks on how many answers and picks now run before the lock. Verifier (Fable) on
  `6f1c779`: pass with three should-fixes, all fixed in `e54258c` (26 lines, in files she saw): answers removed
  before new questions are written, *Stop* can be pressed again (a stop sent before the build
  registers is a no-op), and the stop tests now also prove `report.json` isn't written.
- **Rebased on PB7.6b** (landed meanwhile; the brief, slice and fixes `d2906cc`, `6f1c779`, `e54258c`
  are now `e8f7fe6`, `df6d03f`, `6c1a0e6`): 16 files conflicted, each resolved by hand, both slices
  whole; `dto.rs`, `parity.rs` and `commands.rs` rebuilt as `main`'s file plus this slice's
  additions, since their hunks shared closing lines. A line-by-line comparison of the patch before
  and after showed only the expected list neighbours and the one added line.
- **Repeat passes (Opus):** on the rebase: pass, nothing from either slice lost or doubled (all 60
  changed files compared line by line), with two should-fixes about Forget racing a build, fixed in
  `9613f48`: the build and the interview draft check permission again once they hold the state lock
  (a Forget between the first check and the lock would otherwise let loaded history go to Claude),
  and Forget also clears the frame's *building* state (the unmounted card's last event is dropped).
  Each has a test seen red first. A last pass on `9613f48`: pass, nothing to fix (Forget and the build share one
  non-blocking state lock, so whichever comes second is told it's busy).
- **Rebased again, on PB7.3a** (V2's history writers checking permission under the state lock,
  landed meanwhile): its `locked()` replaced the `lock()` helper this slice reused, so the two
  interview writers (`save_interview_answers`, `keep_interview_answers`) now take `locked()`, which
  checks permission again once the lock is held and refuses a state folder Forget removed. That
  glue (four lines) was written by Claude in the conflict resolution. Final commits: the brief
  `354a20e`, the slice `e5e3d47`, the fixes `53b7aaa` and `3b79964` (`6c1a0e6` and `9613f48` before this rebase).
  A repeat pass (Opus) on this rebase: pass, the only code differences the four lines above.
- **Not verified:** the screens in the running app, light and dark (owed walk); a real Claude Code
  build and stop (the tests use a stub `claude`); closing the app mid-build (the process ends; the
  call already sent is counted at its budget, as before).
- **Left for later:** the report screen is PB7.5's. The Verifier noted that once today's spend is
  within one call of the limit, any build failure reads as the limit reached (the next build would
  be refused anyway).

## PB7.3a: history writers check permission under the state lock (2026-10-25, lane: V2)

- **What:** a Verifier finding from PB7.6b's repeat pass. The six app history writers checked
  permission, then took the state lock, which recreates the state folder; a forget in that gap let
  a writer recreate the folder and write history with no permission on record. Now each writer
  goes through one helper, `locked()`, that checks permission, takes a new non-creating lock
  (`private::lock_existing_state`: a missing folder answers the permission sentence), and checks
  permission again while holding it. `lock_state` and its other callers are unchanged. No new
  sentence. Brief: `plans/briefs/PB7.3a-permission-under-the-lock.md`.
- **Who wrote it:** Codex. Claude briefed, reviewed, ran the full gate outside her sandbox (the
  sandbox can't open local sockets, so seven existing wire tests can't run there), and committed.
- **Verified:** red first (both race tests failed on `unwrap_err` of `Ok`), then green; three new
  unit tests in `history.rs` drive a real forget, and a withdrawn permission, through a seam
  between the check and the lock. Mutation checks, each red on its own assertion and restored by
  `cmp`: the creating lock swapped back in (the folder is recreated), and the re-check under the
  lock removed. `bash scripts/ci-local.sh` green on `860d23a`. `/security-review` (sub-agent): no
  high or medium. Verifier (Fable): all seven claims proven, no must-fix or should-fix.
- **Not verified, left:** the lock guards the folder that was open, not the path: if the folder
  were deleted and permission given again inside the two system calls between opening and
  locking it, a writer could write unguarded into the new folder. The Verifier judged it
  unreachable from the app (a forget while the lock is held answers "busy"); a cheap hardening,
  comparing the locked folder with the path after locking, was not built. The command-line
  writers take no permission or lock (unchanged, a separate route).

## PB7.6b: turn it on, the claude.ai files, forget, from the app (2026-10-25, lane: V2)

- **What:** the *Save* step's other three cards. *Use it in Claude Code*: whether this assistant,
  another one, or none is on, with buttons to turn it on, update it, or turn off whichever is on
  (one at a time; "this one" means the same slug **and** the same folder, so a same-named copy
  turned on from elsewhere counts as another). *Use it in claude.ai*: the Project files made in
  `~/.claude/state/<slug>/claude-ai/`, replaced whole under the state lock (the old files stay when
  a make fails), listed by name only when they look like the pack's own files, shown in Finder,
  with the three steps to add them to a Project. *Forget*: a preview of how many files go, a
  confirmation that says what goes (the history, the working files, the permission, the saving
  choice) and what stays (the assistant's folder, any copy on GitHub, Claude Code), and a delete
  that carries the previewed count and is refused when the count under the lock differs. Eight
  new commands, each in the parity table. `claude_home` (the `CLAUDE_CONFIG_DIR` rule) moved from
  the CLI into the api; `pack::no_kinds_yet` and `private::BUSY` name two sentences the app
  recognises. A listed assistant whose name fails `check_name` is no longer found. Brief:
  `plans/briefs/PB7.6b-use-and-forget.md`.
- **Who wrote it:** Codex, in two passes (the slice; the gate fix-ups). Claude briefed,
  reviewed, ran the gates outside her sandbox, wrote SECURITY's row and this record, and
  committed.
- **Verified:** `ci-local.sh` green on `b612489` and `4bf8616`, and again on `0e0099f` after the
  rebase onto PB7.3 (the command lists, parity table, backend and frame merged by keeping both
  slices' entries; Rust, the socket tests included; the screen tests; the Python suites). Shas
  below are from before that rebase: `b612489` is `ac17446`, `4bf8616` is `0e0099f`. Codex's mutation checks, each red and restored
  by `cmp`: the folder comparison in this-or-other dropped, a stale off slug accepted, a changed
  forget count accepted, every file in `claude-ai` listed, the forget lock skipped, the old
  `claude-ai` removed before packing, a capability name dropped, Forget sending 0; then Forget
  keyed by `resets` again, the name check dropped, the zero-count check dropped.
  `/security-review` (sub-agent): no high or medium. Verifier (Fable) on `b612489`: 8 of 9 claims
  proven; one must-fix (the frame remounted Forget right after a delete, so its "Deleted N files"
  never showed) and two should-fixes (an unchecked assistant name in sentences; a folder gone
  between preview and delete answered "there's more to forget"), all fixed in `4bf8616`.
- **Not verified:** the app itself was not run (no screen control in a job run): the walk is
  owed. A `~/.claude` that is itself a symlink makes the claude.ai files and Forget answer their
  fixed failure (the private-folder walk refuses links): safe, but a usability gap, noted by the
  security review. `on` and `off` take no lock against each other or the CLI (as the CLI's own
  never did).
- **Carried, not built (to be sliced before PB8):** the app-made `SKILL.md` names `python3 … crew_decisions.py`,
  which the installed app doesn't ship (PB7.6's note from PB7.2).
## PB7.3: your history, Claude Code and claude.ai (2026-10-25, lane: V1)

- **What:** the app's *Your history* step. After permission, the person sees which Claude Code
  folders will be read and which are left out, counts first, then reads. For claude.ai they choose
  their download (a zip, or the folder their Mac unzipped) in a native window Rust opens (`rfd
  =0.17.2`; the webview never names a path), see that it can be read, mark which chats are theirs,
  and read those. *Put it together* merges both into `decisions.jsonl`. Eight new app commands, in
  `builder/api/src/history.rs` (the file window itself in `builder/app/src/commands.rs`), each in the
  parity table (23 in all, with PB7.6a's): every one first checks the slug
  against the listing, that the history lives in the private Claude folder, and the current
  agreement, in Rust; every writer holds the state lock for the whole command. The chosen download's
  path is kept as `claudeai-export.json` in the state folder (Forget removes it with the rest).
  Reads report progress through a Tauri channel (no capability needed in tauri 2.12.0), at most
  101 messages. Ingest gained `mine_with_progress` (both sources), `DEFAULT_HOLDOUT`, `is_mine`,
  `conversations` and `list_private` (the state folder listed through its checked descriptor; the
  CLI's `merge` now uses it). Brief: `plans/briefs/PB7.3-sources.md`.
- **Dependency intake (rfd =0.17.2):** the native file and folder picker, MIT, default features
  off (`gtk3` only, for Linux builds). It is the only new package in `Cargo.lock`: everything it
  uses is already there through Tauri. Pinned exactly; already in the local cargo cache, so built
  offline. Called only from `choose_claudeai_export`; on macOS it hands the panel to the main
  thread itself while the app runs (`src/backend/macos/utils.rs`, `run_on_main`, read in the cached
  source).
- **Who wrote it:** Codex, in two passes (the slice; the Verifier's test fix). Claude briefed, reviewed (one fix: a stray blank line in
  the bridge's imports), wrote this record, ran the gates outside Codex's sandbox and committed.
- **Verified:** `ci-local.sh` green on `6fd13ae` and again on `b44f1b9` (836 Rust tests with the
  lock test; on `6fd13ae` 835, 77 screen tests, plus the Python
  suites, fmt, clippy and the build). Codex's mutation checks, each red and restored by `cmp`: the
  consent check dropped (a read accepted), a dry run that writes (a file appeared), the progress
  throttle removed (251 messages, not 101), an ingest error's text passed to the screens (the temp
  home's path leaked), the lock dropped from marking (it succeeded under a held lock), a command in
  `COMMANDS` only (16 against 15), *Your history* enabled before permission, one *Yours* click
  sending every chat. `/security-review` (sub-agent): no findings.
  Verifier (Fable) on `6fd13ae`: pass, no must-fixes; she checked the consent gate and the lock on
  every command, `list_private`'s descriptor handling, the channel's ACL exemption in tauri 2.12.0
  and rfd's main-thread hand-off in the cargo registry. One should-fix, fixed in `33c1a96`: only two
  of the six writers had a test proving they refuse while the state lock is held; the other four
  now do, each mutation (that command's lock dropped) red. Two notes left as they are: a dry run
  takes the state lock, which creates the state folder if it were missing (permission already made
  it, so nothing changes in practice); two Claude Code folders with the same project name would
  share a list key on the screen (a render warning only).
- **Rebased on PB7.6a** (`a6ac8ca`, landed meanwhile; the slice `6fd13ae` is now `ce69a57`, the lock
  test `33c1a96` now `d7e85dc`, and the earlier shas above are the pre-rebase ones): the shared lists (commands, capability,
  handler, parity, DTOs, the screens' backend) carry both slices, PB7.6a's first; the side list
  enables *Your history* after permission and *Save* without it, as PB7.6a has it; its test of the
  unavailable steps now covers four to six, since step three says *Needs your permission first*.
  `ci-local.sh` green on the rebased branch (868 Rust tests, 116 screen tests).
- **Not verified:** the app was not run (no screen pass, no real file window): owed as a walk.
  Codex's sandbox couldn't bind sockets or create a non-UTF-8 file name, so `list_private`'s
  skip of such a name has no test on this file system; the screens' first red run was blocked by
  the offline pnpm store, so their reds were seen only through the mutation checks.

## PB7.6a: save a copy, from the app (2026-10-25, lane: V2)

- **What:** PB7.6 was split at pickup into PB7.6a (this) and PB7.6b (turn it on, the claude.ai
  files, forget). The app's *Save* step, reachable once an assistant is chosen (saving reads no
  history, so it doesn't wait for permission): where to keep the assistant, the recommended place
  picked; the free-account line with a button that has Rust open GitHub's signup page; GitHub
  sign-in in the app, by the device flow (the code shown, GitHub's device page opened by Rust, a
  wait the person can cancel); keep a choice (an existing repo picked by its place and kind in
  Rust's own list, never a path); save, with the privacy check's result; sign out. Eight new
  commands, each in the parity table. The device code stays in Rust (`SignIns`), the token goes
  to the password store only, and a cancelled, superseded or signed-out sign-in saves nothing.
  `text::is_said` is the only way a save-crate sentence reaches the screens; anything else is a
  fixed fallback. `FolderPrivacy` and `UnavailableRemotes` moved from the CLI into the api; one
  listed-assistant lookup now serves every slug command. Git's error line loses any
  `user:password@` before it is shown. Brief: `plans/briefs/PB7.6a-save.md`.
- **Who wrote it:** Codex, in four passes (the Rust half; the screens with Claude's review
  fix-ups; two screen fixes; the gate fix-ups). Claude split the card, briefed, reviewed, ran the
  gates outside her sandbox, wrote SECURITY's row and this record, and committed.
- **Verified:** `ci-local.sh` green on `36e8071` (835 Rust tests, the socket tests included; 81
  screen tests; the Python suites). Facts checked: GitHub's device-flow page is
  `https://github.com/login/device`, and the app's part is to show the code
  (docs.github.com, *Authorizing OAuth apps*, read 2026-10-25). Codex's mutation checks, each red
  and restored by `cmp`: `is_said` accepting anything, the device code added to the answer, the
  last cancel check dropped, an out-of-range choice accepted, the listed-slug check skipped (again
  after the fix-up), a URL taken from GitHub's reply, a capability name dropped, the save step
  enabled with no assistant, the post-token cancel check dropped, the kind check dropped, the
  userinfo strip dropped. `/security-review` (sub-agent): no high or medium, four lows. Verifier
  (Fable) on `d4acd74`: pass with three should-fixes (a cancelled sign-in could still save its
  token; a choice by index could follow a shifted list; git's error line could carry a URL's
  sign-in), all fixed in `36e8071`; the security review's matching lows with them. Verifier
  repeat pass (Opus) on `36e8071`: pass.
- **Left as they are (low):** two *existing* repos of the same kind can still swap places between
  looking and choosing, which needs an assistant folder outside `~/.claude` in a second repo (the
  app never makes one there); the userinfo strip cuts at the first `@`, so a password holding a
  raw `@` (git wants `%40`) leaves its tail; `is_said` matches a sentence's shape, so a future
  core error that happens to start or end like one would pass; `start` holds the sign-in lock
  across its request (a cancel waits up to ureq's 30-second limit).
- **Not verified:** the app was not run (no screen pass), and the live GitHub sign-in from the
  app waits with V-Q4's walk: both owed. The real Keychain and browser opener ran only as stubs.

## PB7.2: your permission, a new assistant, connect Claude (2026-10-25, lane: V1)

- **What:** the app's first working steps. A person sees their assistants, makes a new one in
  `~/.claude/crew/<slug>/` (V-Q5) from the template compiled into the binary
  (`template::FILES`, checked byte for byte against `crew/_template/`; the `install.sh` parity
  test now covers the embedded path too), gives permission before anything is read, and checks
  that their own Claude Code is installed and signed in (PB-D9). The permission is kept per
  assistant as `consent.json` (`version`, `agreed_on`) in its private state folder, and counts only
  at the wording's current version; the wording is pinned by a SHA-1 that a test checks. Making an
  assistant clears any old record for its name. CLI: `persona-builder new`, `assistants`,
  `consent show|agree`; app: six new commands (async, so the Claude check stays off the main
  thread), each in the parity table; screens: Assistants, NewAssistant, Consent, ConnectClaude,
  with Connect reachable only after permission. *Show in Finder* reveals the folder
  (`open -R`). Brief: `plans/briefs/PB7.2-consent-new-connect.md`.
- **Who wrote it:** Codex, in four passes (the Rust half; the screens; Claude's review fix-ups;
  the gate fix-ups). Claude briefed, reviewed, wrote SECURITY's two rows and this record, ran the
  gates outside her sandbox and committed.
- **Verified:** `ci-local.sh` green on `c7d55c3` (803 Rust tests, 42 screen tests, plus the
  Python suites). Codex's mutation checks, each red and restored by `cmp`: a template entry
  dropped, the wording changed without its hash, the slug check skipped, a core error passed to
  the screens, an old-version record accepted (api and CLI), an unmatched command, `String(error)`
  in `failureMessage`, Connect enabled before permission, `-R` dropped, the old record left on a
  new assistant. `/security-review` (sub-agent): one medium (the folder button could launch a
  linked `.app`) and two lows (an old permission inherited by a new assistant of the same name;
  an "already exists" sentence built from disk), all fixed in `c7d55c3`. Verifier (Fable) on
  `db8a5da`: pass, with three should-fixes (the wording overstated Forget and left out the piece
  of Claude's reply kept with each message; the inherited permission), all fixed.
  Verifier repeat pass (Opus) on `c7d55c3`: pass, three nits left as they are (the screens' test
  data still carries the first wording; the CLI repeats one api sentence; saved answers keep
  Claude's whole question, which "your answers to its questions" already implies).
- **Not verified:** the app itself was not run (no screen pass): owed as a walk. The Claude check
  against a real Claude Code ran only through stubs here.
- **For the owner to slice (noted under PB7.6):** an assistant the app makes has `{{REPO}}`
  = `~/.claude`, so its `SKILL.md` points at `~/.claude/scripts/crew_decisions.py` for `similar`
  and `record`, which the installed app doesn't have.

## PB4.3c: refresh apply, the last hardening from its review (2026-10-25, lane: V2)

- **What:** the PB4.3b record's follow-ups, all about hand-edited private records.
  `diff::added_lines` splits rows on `\n` as `diff::apply` does and drops one `\r` only as a
  `\r\n` line ending after a `\r\n` unchanged or removed line in the same part of the change (not
  before a `\ No newline at end of file` row). `check_candidate` checks each added line for length
  and hidden characters before trimming it, so a stray `\r` refuses the change. `measure` returns
  the proposal it checked (`Measured` is now a struct with `proposal`, `measurement`, `already`)
  and the CLI hands that one to `finish`, with no second read. A rerun checks a saved undone result
  by the keep-or-undo rule again (a kept one can't be: its tests are active by then). The
  duplicate-approval test checks the message. `.ai/SECURITY.md`'s refresh row says all of this,
  and drops the old `\r` limit. Brief: `plans/briefs/PB4.3c-apply-hardening.md`.
- **Who wrote it:** Codex, in three passes (the slice; a fix-up; the gate fix). Claude briefed,
  reviewed, wrote the SECURITY row and this record, ran the gates and committed. The brief was
  wrong on one point: a leading tab was already refused, earlier, as a damaged change (only `\n`,
  `\r` and `<` get past `check_stored`). Codex's first pass changed that refusal's message to fit
  the brief; the fix-up put it back, so `\r` is the only character the new check catches.
- **Verified:** `ci-local.sh` green on `f14a65e` (754 Rust tests, 0 failed, plus the Python
  suites). Codex's mutation checks, each red and restored by `cmp`: the old `str::lines` split,
  trimming before the check, the CRLF allowance removed, the old `Measured` enum (no compile), the
  duplicate-approval guard, the undone-result recheck, the recheck applied to kept results too (the
  idempotency test goes red), and the end-of-file `\r`. `/security-review` (sub-agent): one low,
  fixed in `f14a65e`. Verifier (Fable) on `7025e88`: pass with two should-fixes, the same
  end-of-file `\r` and the SECURITY wording, both fixed in `f14a65e`; she ran eight mutations of
  her own, all red.
- **Not verified:** that the proposal is handed on is held by the type, not a test (no seam to
  change the record between the two calls).
- **Known, accepted:** in a `\r\n` file, a change that adds a line above the first one, or after a
  last line with no line ending, is refused (the card's rule taken literally; `decisions.md`
  starts with its title and the build ends it with a newline). Earlier drafts shown to the model
  when proposing now keep a stray `\r` from a hand-edited record (the reply is checked in full).

## PB7.1: the app's frame and its locked-down bridge (2026-10-25, lane: V1)

- **What:** the Persona Builder app starts. `builder/api` holds the app's commands as plain Rust
  with no Tauri dependency (so another app's pane can call the same code), each answer a DTO
  exported to TypeScript by ts-rs into `builder/ui/src/types/generated/`, plus the CLI-parity
  table: every app command names the `persona-builder` commands that do the same, and a CLI test
  runs each one. `builder/app` is the Tauri 2 window with one command (`app_info`: the version and
  the agent list), a capability granting only that command and no `core:` permission, no plugin,
  no global Tauri object, no file drops, a strict CSP, and a navigation guard that allows only the
  app's own origin (and the dev server only when `tauri::is_dev()`). `builder/ui` is the screens
  as a package (`@persona-builder/screens`) behind a `Backend` interface, with the seven steps
  listed (*Not here yet*) and a Welcome screen; only `src/app/tauri-backend.ts` imports Tauri.
  The rule for every later screen: the webview never names a file or folder. `ci-local.sh` checks
  the generated types are current and committed, then runs the screens' install, typecheck, tests
  and build, skipped with a printed line when pnpm is missing, older than 10, or unreadable. Brief:
  `plans/briefs/PB7.1-app-frame.md`.
- **Split:** PB7 split into PB7.1 to PB7.6 at pickup (frame; consent, new assistant, connect
  Claude; sources; build and interview; report and review; save, turn on, forget). PB7.2 also
  waits on V-Q5, where the app keeps a new assistant's folder.
- **Who wrote it:** Codex, in four passes (she stopped, as the brief said to, when one npm package
  was not cached offline; Claude fetched the pinned packages and she finished; then two fix-up
  passes from the security review and the Verifier). Claude briefed, reviewed, ran the gates
  outside her sandbox and committed. Claude's own review moved the command list to one file that
  the build script and the test both include.
- **Decisions made at pickup:** the reference app's stack and exact pins (Tauri 2.12.0,
  tauri-build 2.7.0, ts-rs 12.0.1, Vue 3.5.43, Tabler 1.6.1, Vite 8.3.1, Vitest 5.0.2); dev port
  5180; the identifier `dev.personabuilder.desktop` is a placeholder until signing (PB-D3) and
  the app keeps no data under it; a placeholder icon (the real one is PB8's); no `core:`
  permission until a screen needs one (PB7.4 adds the event ones).
- **Dependency intake:** new crates `tauri` `=2.12.0`, `tauri-build` `=2.7.0` (Apache-2.0 OR
  MIT) and `ts-rs` `=12.0.1` (MIT), the same pins the reference app runs; Cargo.lock adds 334 locked versions
  and changes no existing one. Licenses in the tree are permissive (MIT, Apache-2.0, BSD,
  ISC, Zlib, Unicode-3.0), plus five MPL-2.0 crates used unchanged. npm (exact pins, all MIT
  except TypeScript, Apache-2.0, and the Tauri packages, Apache-2.0 OR MIT): `vue`,
  `@tabler/core`, `@tauri-apps/api`; dev `@tauri-apps/cli`, `@vitejs/plugin-vue`,
  `@vue/test-utils`, `happy-dom`, `typescript`, `vite`, `vitest`, `vue-tsc`. Every lockfile entry
  has a sha512 hash and none is a git source. pnpm 10 and later don't run dependencies' install
  scripts (pnpm 10.0.0 release notes), hence the version floor.
- **Verified:** red first; Codex's mutation checks, each red and restored by `cmp`: an extra
  `core:default`, a command in one list only, global Tauri on, file drops on, the other app's dev
  port allowed, the error's own text shown, a Tauri import in a screen, an empty parity row, the
  dev switch reverted, a renamed plugin dependency, Tauri in `builder/api`, a pnpm below 10.
  `ci-local.sh` green after the security fixes (756 Rust tests, 12 screen tests, typecheck and
  build, the Python suites, fmt, clippy, shellcheck, the keystore cross-checks) and again on the
  final code, `5db2692` (the fix-up commits were squashed into it before the first push, to drop
  a checkout path an early commit carried).
  Live: the app built with the screens embedded (`--features tauri/custom-protocol`), opened, and
  stayed up 8 seconds with nothing on stderr. `/security-review` (sub-agent): no high; a fixed
  checkout path in the brief (removed) and two lows (the dev switch, old pnpm), fixed. Verifier
  (Fable): all eight claims verified, three low should-fixes fixed; a repeat Verifier (Opus) on
  the fixes.
- **Not verified:** what the Welcome screen looks like, in light and dark (owed to the project owner's next
  walk; screen control is not a runner's); `pnpm -C builder/ui tauri dev`; a build on Windows or
  Linux.
- **Known limits, for later slices:** the navigation guard allows only the macOS origin
  (`tauri://localhost`); Windows and Linux use `http://tauri.localhost`, so a build for them needs
  it added. Tauri always lets its internal channel fetch through, outside the app's command list.
  From the repeat Verifier (low, not fixed): `ci-local.sh` stops with no message if `pnpm
  --version` itself fails; the plugin check misses an underscore spelling (`tauri_plugin_x`); the
  brief names the reference app once (a public open-source product, not jargon, so kept).

## PB4.3b: refresh, test the change, then keep or undo it (2026-10-25, lane: V1)

- **What:** `persona-builder refresh apply <member_dir> <CHANGE>` takes an approved change on a
  current version, applies its diffs strictly, and checks the result again as a draft is checked.
  Then it answers every test the change names and every active test, on the old version and the
  new, through the person's own Claude Code (one call per batch of about 40, never the right
  answer). It grades written answers in a second call that sees only the right answer and the
  answer, under labels that don't say which version wrote it (a shared answer is graded once). It
  saves the measurement first, then keeps the change (`decisions.md` staged and renamed, the tests
  made `active`) or undoes it (`decisions.md` never written, the tests stay `new`). Either way
  `measurements.md` gains a `## <date>: refresh (<id>)` entry. A rerun makes no model call and
  finishes what is left. The report, and `persona-builder report`, show a growth line. New:
  `builder/grow/src/apply.rs`, `apply_reply.rs`, `prompts/answer.md`, `prompts/grade.md`;
  `diff::added_lines`; `draft::write_measurements`, `write_decisions`; `build::batches` generic.
  Brief: `plans/briefs/PB4.3b-refresh-apply.md`.
- **Who wrote it:** Codex, in three passes (the slice, finishing after she stopped on a
  contradiction in the brief, the gate fix-ups); Claude briefed, reviewed, ran the gates outside
  her sandbox and committed. Her stop was right: a choice test's right answer is one of its
  options, so it is in the prompt by design. The blindness test now checks rubric answers and
  signals for the sentinel, and that no prompt line carries `expected` or `match`.
- **Verified:** `ci-local.sh` green on `dd7bebe` (750 Rust tests, 0 failed, plus the Python
  suites, fmt, clippy, shellcheck and the keystore cross-checks). Codex ran 25, 46 and 12 mutation
  checks across the three passes, each red and restored by `cmp`. `/security-review` (sub-agent):
  two medium, no high, both fixed. Verifier (Fable) pass with two should-fixes, fixed in
  `dd7bebe`; a repeat Verifier (Opus) on `dd7bebe` passed and reverted six of the new fixes, each
  going red.
- **Not verified:** a live run against a real Claude Code (the tests use a stub `claude`), so how
  well a real model answers and grades is unmeasured.
- **Follow-up, not built:** the repeat Verifier's low should-fixes. A trailing `\r` on an added
  diff line passes the hidden-character check (it reads as a line ending; hand-edited records only).
  The duplicate-approval test checks that it errs before any call, not which error. `finish` re-reads the proposal instead of
  being handed the checked one. Also still open from PB4.3a: a later `build` drops kept refresh
  edits.

## PB4.3a: refresh, draft the change, then approve or reject it (2026-10-25, lane: V1)

- **What:** the first half of a refresh. `persona-builder refresh propose <member_dir>` sends up to
  40 of the oldest tests made from the person's answers (`new` cases) and the current
  `decisions.md` to their own Claude Code, which answers edits (an area, a new standing rule or a
  replacement for one existing point, a plain reason, the tests it serves). The code, never the
  model, builds the new `decisions.md` and one unified diff per edit (`file` is always
  `decisions.md`), proves each diff applies in order, and saves the proposal privately under
  `growth/`. `refresh show` prints it; `refresh approve|reject` writes the person's answer
  (`role: subject`). Nothing touches the member's folder: applying, re-measuring and
  keep-or-revert are PB4.3b. New modules in `builder/grow`: `version.rs` (a proposal's `base` is
  the git blob id of `decisions.md`), `diff.rs` (unified diffs and a strict apply, no fuzz),
  `edit.rs` (edits into `decisions.md`'s shape), `refresh.rs`. The build's cached, checked,
  retry-once `Asker` is public, with `with_misfit` for the caller's own closing sentence. Brief:
  `plans/briefs/PB4.3a-refresh-propose.md`.
- **Who wrote it:** Codex, in one pass and a fix-up pass (her sandbox can't bind sockets, so
  Claude ran the full gate); Claude split PB4.3, briefed, reviewed and committed.
- **Decisions made at pickup:** PB4.3 split into PB4.3a and PB4.3b (one run each, smaller
  Verifier passes); the model answers structured edits, not a diff, so no model text decides a
  path or a hunk; one proposal change per edit, the diffs applied in order; one open proposal at a
  time, a stale one (its base no longer current) never blocks; approve needs a current base and
  every named test still `new`; drafts the person rejected, or that were tested and undone, go
  back into the prompt so the next draft differs (the cached reply cannot repeat).
- **Checks on every new line:** one line, at most 400 characters, no hidden character, the
  member's filters, and no run of 6 or more of the person's words from the tests or their answers;
  a misfit is told back once, and what the person sees names counts and places only. A stored
  proposal with a hidden character (other than `<` and CR in a diff) is never printed, and never
  saved in the first place.
- **Verified:** `ci-local.sh` green on the fix-up head, run outside Codex's sandbox at 2 test
  threads (Mimi's load gate was at throttle), and again on the rebased head `b2cc5de` at 4 (704
  Rust tests, 0 failed, plus the Python suites, fmt, clippy, shellcheck and the Windows and Linux
  keystore checks). Codex's 53 mutation probes on the
  first pass and three on the fix-ups each went red and were restored. `/security-review`: no high
  or medium findings. Verifier (Fable): pass, five should-fixes and four nits, all fixed in
  `6d8345e` (a change that changes nothing, a proposal saved that `show` refuses, the refresh's
  own words after two misfits, CRLF files, reverted drafts repeating from the cache); a delta
  Verifier pass (Opus) on the fix-ups, and one (Opus) on the rebased head.
- **Not verified:** a live run against a real Claude Code (tests use a stub `claude`); strict
  red-first order for every test (Codex reports some supplemental tests first ran after the code
  and were then proven by mutation).
- **Follow-up, not built:** a later `build` rewrites `decisions.md` from history and drops kept
  refresh edits (noted in PB4.3b's spec; not sliced yet).

## PB4.2: kept answers, the growth loop's records and store (2026-10-25, lane: V1)

- **What:** every answer the person gives their assistant (a pick, a correction, a mark, a rule) can
  now be kept on their own computer as a growth-loop signal, and becomes at once a case (a test the
  assistant must pass at its next refresh), in the files `docs/spec/growth-loop.md` sets. A new
  crate `builder/grow` (`persona_builder_grow`): the five records of the spec as typed Rust that
  refuse unknown fields and check every schema rule serde cannot (`record.rs`); the rules that span
  records, 2, 3, 4, 5, 7, 8 and 9, for PB4.3 to call (`rules.rs`); the private store under the
  state folder's `growth/` (append-only `signals.jsonl` and `cases.jsonl`, one never-rewritten JSON
  file per proposal, approval and measurement, all under the state lock; rule 8 checked at write
  time) (`store.rs`); and `keep.rs`, which checks one answer (the filters, length limits, hidden
  characters, the member's own areas) and keeps it with no model call. New commands `persona-builder
  keep <member_dir> --signal <file>` and `persona-builder cases <member_dir>`. Ingest gains
  `read_private_optional_at`, size-capped reads and `private_folder_exists`; the build's
  hidden-character check moved to `persona_builder_steps::text_check` so the build, its report and
  kept answers share one list. Brief: `plans/briefs/PB4.2-kept-answers.md`.
- **Who wrote it:** Codex, in three passes and a fix-up pass (her sandbox cannot commit in a lane
  worktree, so Claude committed); Claude briefed, reviewed and resolved the rebase onto PB4.1.
- **Decisions made at pickup:** every signal makes exactly one `rubric` case whose `expected` is
  the signal's `wanted` (the signal has no options, so no `choice` case can be made without a
  model); ids are `<prefix>-<day>-<NNN>`, as the spec's examples; `persona` is the member's slug,
  which must fit the spec's pattern; parity is pinned both ways (the examples round-trip through
  Rust, every schema pattern and enum equals its Rust constant, and golden files Rust writes are
  validated by the Python schema test).
- **Verified:** `ci-local.sh` green after the rebase; about 130 new tests across the grow crate,
  the CLI, ingest and steps, each red first; Codex's mutation checks per claim (over 200, every one
  caught). Live proof on an invented member in a throwaway home: `cases` with none, `keep` writes
  0600 files in a 0700 `growth/`, a personal hit is refused without echoing the words, `cases`
  then counts one.
- **Gates:** `/security-review` (a sub-agent): no high or medium, four lows, all fixed. Verifier
  (Fable): pass with four should-fixes, all fixed. Delta Verifier (Opus): pass with three
  should-fixes, all fixed. The brief's last section lists all thirteen fixes.
- **Not verified:** nothing calls the proposal, approval and measurement writers yet (PB4.3 does);
  the app (PB7) that writes the signal file does not exist yet.
- **Skills Review:** ✅ tdd-skill (red first, mutations) · ✅ security-and-hardening (private I/O,
  no echo, size caps) · ✅ plain-language (every refusal and summary line) · ✅ code-quality (review
  plus two Verifier passes) · ⚠️ source-driven-development (no outside library; the spec is the
  source).

## PB4.1: "How well I know you", the plain report (2026-10-25, lane: V2)

- **What:** every build now ends with a plain report instead of the old "passed the test on" line:
  **Strong**, **Getting there** or **Thin**, overall and per kind of decision; for each area that
  is not Strong, what would strengthen it (the interview, adding the claude.ai chats, time); and,
  with fewer than 30 held-out questions, the honest line that there was too little to measure (plus
  the interview pointer while the history is under 150 answered questions, since the interview
  refuses above that). It is made from the build's own counts and test, with no model call and no
  record text. The rules reuse the repo's thresholds: `build::live`, `build::THIN`,
  `interview::THIN_BELOW`, `build::MIN_HELD`. It is kept as `report.json` in the state folder (0600
  in 0700, the private writer), and the new `persona-builder report <member_dir>` prints it again;
  it refuses a changed file (unknown fields, wrong schema or date, a hidden character in a name)
  before printing anything. New module `builder/build/src/report.rs`; all the words are in
  `text.rs`. Brief: `plans/briefs/PB4.1-how-well-i-know-you.md`.
- **Who wrote it:** Codex (back from her usage limit), from the brief, in four passes: the build,
  review cleanups, the gate fixes, the delta fixes. Claude wrote the brief, reviewed each pass and
  ran the gates.
- **Decisions made at pickup:** the report replaces `text::live` (removed; nothing else used it).
  `serde` derive added to `builder/build` (already in `Cargo.lock` through `builder/core`: no new
  crate). *At the gates:* a category that `decisions.md` lists live but that has no training rows
  is a Strong area too (examples 0), so the two never disagree. A category name that is empty,
  over 100 characters or holds a hidden character stops the build before any paid call.
- **Verified:** 14 new unit tests (`builder/build/tests/report.rs`) and 3 new CLI tests plus one
  changed (`builder/cli/tests/build.rs`). Codex saw each red on its assertion first; where a test
  pinned behaviour that already existed, she broke the code on purpose, saw the red, and put it
  back. The mutations the Verifier listed (the half rule `>=` → `>`, `<` → `<=` at 5 examples, the
  claude.ai condition, Strong over Thin, the test score's fields, the 150 boundary, the empty-area
  guard) all fail a test now. `ci-local.sh` green on the build Mac after the gate fixes (513 Rust,
  152 Python tests), and on the final head (514 Rust, 152 Python).
- **Gates:** `/security-review` (a sub-agent): no high or medium. One low (below) and one note: a
  hidden character in a category name printed at build time and then made every `report` fail;
  fixed. Verifier (Fable): pass, no must-fix; should-fixes in (the live-only areas, two missing
  pins). Delta Verifier (Opus): pass; her two should-fixes in (one more pin, the name check before
  spending); her ordering nit is accepted: live-only areas come after the tally's areas.
- **Not verified:** a live build on a real history; every run here used the stub Claude and the
  invented member.
- **Known limits:** the report file is read whole before its 64 KiB limit applies
  (`read_private_optional` has no bounded read). The folder is 0700 and must be the user's own, so
  only the user could plant a huge file; recorded, not changed. The build says the history is thin
  twice (the line before the build and the report's own).

## PB6.2: GitHub sign-in on Windows and Linux (2026-10-24, lane: V2)

- **What:** the GitHub token now lives in the computer's own password store everywhere, not only
  on a Mac. A new crate `builder/keystore` (`persona_builder_keystore`) opens Windows' Credential
  Manager or the Secret Service on Linux through `keyring-core`; the save crate's new
  `SystemStore` (in `keychain.rs`) reads, writes, reads back and forgets the token through it, and
  `Keychain` uses it off macOS. The macOS path is unchanged. No store (a Linux machine with no
  keyring running) is a plain refusal that offers keeping the assistant on this computer; there
  is no file fallback. Every *this Mac* in the save wording and the CLI's `save` help now says
  *this computer*; `MAC_ONLY` is gone; `KEYCHAIN_FAILED` names no OS store; `NEEDS_GIT` gives
  `xcode-select --install` on macOS only and `https://git-scm.com/downloads` elsewhere.
  `ci-local.sh` checks the keystore crate for `x86_64-pc-windows-msvc` and
  `x86_64-unknown-linux-gnu` when those targets are installed. Brief:
  `plans/briefs/PB6.2-other-os-sign-in.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed at
  pickup).
- **Decisions made at pickup:** `keyring` 4 is now a front for `keyring-core` plus one crate per
  store, so the stores are named directly, each only on its own OS, pinned `=`. The stores get their
  own crate because the save crate cannot be checked for another OS from a Mac: `ring` (under
  `ureq`'s TLS) compiles C that needs that OS's system headers. Windows credentials are kept
  `Local` (this computer), not the store's default `Enterprise`, which roams with a domain
  profile. Linux uses the pure-Rust `zbus` store with `rt-async-io-crypto-rust` (no tokio, no
  OpenSSL); its Secret Service session is encrypted. An error that carries the stored bytes
  (`BadEncoding`, `BadDataFormat`) is wiped before it drops.
- **Dependency intake:** `keyring-core =1.0.1`, `windows-native-keyring-store =1.1.0` (default
  features off: `search` only adds `regex`), `zbus-secret-service-keyring-store =1.0.1`. Names
  checked against crates.io. All three are new (2025) with one owner, who is also a co-owner of
  `keyring` itself; 3 to 5.5 million downloads each (crates.io, read 2026-10-24). Under them:
  `secret-service` 5.2.0 and `zbus` 5.19.0 on Linux (128 crates there in all), `windows-sys`
  0.61.2 on Windows (7 crates there, the keystore's own dependencies included). Every licence is permissive (MIT, Apache-2.0, Unlicense, Unicode-3.0;
  one MIT OR Apache-2.0 OR LGPL-2.1-or-later). The build scripts are the usual ones (`libc`,
  `rustix`, `serde`, `proc-macro2` and the like). The standard library cannot reach either store,
  and Windows has no command that reads a saved password back, so the macOS approach (driving the
  OS's own command) does not carry over; on Linux it would mean `secret-tool`, an extra program
  (whether desktops ship it was not checked). `Cargo.lock` is in this change.
- **Verified:** 16 new tests, plus one live test that is ignored (for a hand run on a Windows or
  Linux desktop): 3 in the keystore crate, 9 for `SystemStore` over keyring-core's mock
  store and a test store that keeps something other than it was given, 4 on the wording; 2 CLI help tests changed. Before
  the gates, 16 of the 17 then written were red on their assertion first against stubs; the
  keystore's round trip over the mock passed on the stub too, since both built entries the same way. 11 mutations, each restored and checked with `cmp`, all
  failed: among them the read back left out, the token check left out, no store reported as the
  generic failure, and a type error in each OS's `open`, which the Windows and the Linux
  `cargo check` each caught (so both OS branches really compile there). `ci-local.sh`
  green on the build Mac (151 Python, 423 Rust tests, 1 ignored; both cross-target checks ran).
- **Gates:** `/security-review` (a sub-agent): nothing exploitable; four lows. The Verifier
  (Fable): pass, no must-fix; one should-fix (on Linux a store with no default collection, as on
  WSL, or a dismissed unlock prompt reaches `KEYCHAIN_FAILED`, which offered no way out) and two
  wrong counts in this record. Fixed after the gates: `KEYCHAIN_FAILED` also offers keeping the
  assistant on this computer only; a loaded value the Keychain would refuse is not used as a token
  (security low 1); the counts. A new test (red first) and a new assertion (red under a mutation). The other three lows are recorded
  below, not changed.
- **Not verified:** a live sign-in on a Windows and a Linux desktop (V-Q2); neither OS's store
  has run here, only compiled. On Linux, the zbus store's docs warn about headless machines and
  WSL (no default collection): there the refusal is expected, not proven.
- **Known limits:** on Linux, `secret-service` keeps unwiped copies of the token in memory while
  it encrypts and decrypts (upstream); in a session with no keyring running, any program of the same
  user could claim the Secret Service name and receive the token (the Secret Service's own trust
  model); a locked keyring waits on its unlock dialog with no time limit, as `secret-tool` does, so
  a machine with no one to answer it would hang the command.
- **Out of scope (left for later slices):** the README's "You need a Mac", PB8's macOS-only installer,
  PB7's builds for other OSes, and the *this Mac* lines in `builder/agent` and `builder/ingest`.

## PB3.3: the build, label, distil, the blind backtest, write the member (2026-10-25, lane: V1)

- **What:** `persona-builder build <member_dir>` runs `bootstrap/PROMPT.md` steps 8 to 11 through
  the person's own Claude Code, one tool-less call at a time inside one session (the state lock),
  in `builder/build` (`build.rs`, `draft.rs`, the fixed prompts in `src/prompts/`). Label: the
  training records and the held-out questions (never their answers) in parts of at most 300 records
  and 400,000 bytes, each reply rebuilt per kind and mode and checked by `check-labels`' rules, then
  `tally`. Distil: one call per codebook group with rows, structured one-line points checked against
  the brief's limits, rendered by the code, none sharing a 6-word run with the corpus or tripping a
  filter. Backtest: skipped below 30 held-out questions; else batches of about 40, each question with
  the draft and its 6 closest training precedents; `score`; live at 5 or more High picks with at
  least 90% right. Then `decisions.md` from the template's own lines and the dated
  `measurements.md` entry (the template's examples removed, new entries before `## Reports`), both
  checked by the filters, staged beside the member's files and renamed last. Every checked reply is
  kept in the state folder's `build/` under a hash of the step, model, instructions, shape and first
  prompt, and re-checked before reuse. Brief: `plans/briefs/PB3.3-build.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed at
  pickup). Stacked on `lane-v1/PB3.4` (the crate it joins), so it lands after PB3.4.
- **Decisions made at pickup:** `<member_dir>` like every other command; distil replies come back as
  structured points, not Markdown, so the shape is checked and the code writes the Markdown; parts
  also capped by bytes so a part of long records never overruns a model's context; no
  `decisions-draft.md` file (the draft lives in memory, the replies in `build/`); the shingle check
  runs on the model's points, never the template's fixed lines (which a person's own history about
  this kit could match); calls run one at a time.
- **Verified:** 26 library tests (`builder/build/tests/build.rs`) and 4 CLI tests against a stub `claude` (a small Python script
  that answers each step from the records in its prompt, with a per-call plan for bad, quoting or
  failed replies): the fixed system prompts and schemas in every call's argv and nothing of the
  person's there, no held-out answer in any prompt (a sentinel in the held-out notes), the steps in
  order, the progress lines, one retry and the problem named, two misfits stop, a failed call stops
  and the rerun reuses all four paid replies (a damaged kept reply is asked for again), the cap
  stops at the second call, under 30 held-out questions no held-out labels and no backtest, nothing
  to train on and a category in no group refused before any call, the draft keeping every template
  line in order, the measurements entry, the filter refusal, the staged write, links refused. The
  tests were written alongside the code, so they were proved by mutation: 39 mutations, one per claim,
  each restored and checked with `cmp`; 37 failed a test at once, one survived (a multiSelect pick
  given as one string, a missing case, then added and caught), and one survived for good (the order
  of the two renames: there is no seam to stop a run between them). `bash scripts/ci-local.sh` green
  (481 Rust, 151 Python tests). Live: the real binary against the real Claude Code 2.1.292 on Mimi,
  an invented member in an empty home: "isn't signed in", no ledger, no `build/`, nothing spent.
- **Gates:** `/security-review` (a sub-agent): no high or medium; four lows (hidden characters in
  points, staged files a killed run leaves, an unbounded distil prompt, a linked `briefs/`). The
  Verifier (Fable): pass, no must-fix; three should-fixes (pre-flight the member folder before
  spending, no held-out labels without a test, every category in a group). All seven fixed, each with
  a test proved by a mutation (10 more mutations, all caught); see the brief's *After the gates*.
  The delta Verifier (Opus): pass, no must-fix; her three should-fixes (tag and other invisible
  characters, the distil prompt's own `<`, a test for a linked `decisions.md`) are in, each proved by a
  mutation (3 more, all caught).
- **Known limits:** a distil group whose prompt is over 700,000 bytes (after long words are cut to
  600 characters) stops the build rather than being split, after the labels are paid for (they are
  kept); 700,000 bytes may still be past a model's context for some text. The member folder being
  writable is not checked before spending. The two renames are not one atomic step. The same-day
  rerun adds a second dated entry. `build/` is never swept of replies an older codebook or corpus
  left. The lens step's shingle check (documents) is not built.
- **Not verified:** a successful live build (no signed-in Claude Code on Mimi); the quality of the
  labels, the distilled model and the backtest on a real person's history (owed to PB8).

## PB3.4: the interview when the history is thin (2026-10-24, lane: V1)

- **What:** a new crate `builder/build` (`persona_builder_build`) for the build's steps that call a
  model; PB3.3's build joins it. `persona-builder interview draft <member_dir>` counts the answered
  questions in the merged corpus and, below 150, has the person's own Claude Code draft twenty
  questions in the round-questions shape, saved to `interview-questions.json` in the state folder.
  `persona-builder interview answer <member_dir> --answers <file>` writes
  `decisions-interview.jsonl` (`question:interview-NN`, project `interview`, always training, through
  the filters), which `merge` already keeps in training. `round` gains `check_questions` (its own
  rules, no answers). `.gitignore` un-ignores `builder/build/` (every `build/` is ignored). Brief:
  `plans/briefs/PB3.4-interview.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed at
  pickup).
- **Decisions made at pickup:** no record goes in the prompt (only the codebook's areas and kinds of
  decision); category counts come only from an optional `--labels` file, since labelling follows the
  interview; each header must be a kind of decision and the questions numbered 1 to 20 in order; the
  saved file keeps only `round`'s keys; `round`'s deterministic timestamp, not "now"; `<member_dir>`
  like every other command, not the spec's `<member.json>`.
- **Verified:** red first (13 library tests against stub bodies, 7 CLI tests against a missing
  command), then green; 21 mutations, one per claim, all caught, each file restored and checked with
  `cmp`. Live: the real binary against the real Claude Code 2.1.292 on Mimi in an empty home said
  "isn't signed in", created no state folder and spent nothing.
- **Gates:** `/security-review` (a sub-agent): no high or medium; one low (the waiting-questions
  check ran before the lock), fixed by checking again under the lock. The Verifier (Fable): pass, no
  must-fix; should-fixes applied: kept answers are not replaced without `--replace`, the CLI test now
  pins the weighted prompt, and the help and the next-step line say the answers file goes in the
  private folder.
- **Known limits:** the template's second trigger ("fewer than 10 in a category the principal cares
  about") is not built: the spec names only the 150 rule, and a history at 150 or more gets no
  interview, even with `--labels`. The second waiting-questions check (a race) has no test.
- **Not verified:** a successful live draft (no signed-in Claude Code on Mimi); the questions'
  quality on a real person's areas (owed to PB8).

## PB6.1: GitHub sign-in's client ID (2026-10-25, lane: V2)

- **What:** the registered OAuth App's client ID is compiled in (`CLIENT_ID` in
  `builder/save/src/github.rs`), so `save choose … github`, `save choose … here` and `save push`
  go on to ask for sign-in instead of stopping at "isn't set up". The two CLI tests that expected
  `NOT_SET_UP` now expect `SIGN_IN_FIRST`. Brief: `plans/briefs/PB6.1-client-id.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed at
  pickup).
- **Decisions made at pickup:** the `Option` and the `PERSONA_BUILDER_GITHUB_CLIENT_ID` build-time
  override stay, so a fork with no app of its own sets `None` and gets the same plain refusal; no
  test runs `save sign-in`, because it would call GitHub. Default `gitleaks` does not flag the
  line (checked with the line staged), so it carries no `gitleaks:allow` marker.
- **Verification:** red first: both tests failed on their assertion, printing `NOT_SET_UP`; green
  after the change; the red run doubles as the mutation check (`None` makes both fail).
  `bash scripts/ci-local.sh` green (431 Rust, 151 Python tests). The tests stay offline: with a
  temporary `HOME`, `/usr/bin/security list-keychains` shows only the System keychain and the
  lookup exits 44 (no token), checked by hand. `/security-review` (sub-agent): no findings.
  Verifier (Fable): pass, nothing must-fix; her should-fix (these stale notes) is in.
- **Supersedes:** the PB2.1 and PB6 records' notes that the client ID is unset and waits on V-Q1.
- **Source:** GitHub, *Authorizing OAuth apps* § Device flow: the device code request takes
  `client_id` and `scope`, and "The `client_secret` is not needed for the device flow".
- **Not verified (owed, attended):** the live walk: `save sign-in` through GitHub's device page,
  `save choose … github <name>` on an invented assistant (the repo reads back private), `save push`,
  and a public repo refused.

## PB3.1: the build's steps that need no model, in Rust (2026-10-24, lane: V1)

- **What:** a new crate `builder/steps` (`persona_builder_steps`) ports eight commands of
  `crew_decisions.py` at parity: `chunk`, `check-labels`, `tally`, `distil-in`, `score`, `similar`,
  `shingle` and `round`, each a `persona-builder <cmd> <member_dir>` command with the Python's flags.
  Every file is `NAME` or `FOLDER/NAME` inside the member's state folder: `private.rs` gains
  `read_private_at`, `write_private_at` and `place`, and `.json` joins the private file endings. The
  ingest crate makes public what the steps share (`privacy::searchable`, `mine::outcome`,
  `mine::Mark`, `corpus::exact_numbers`, a `dates` module). Brief: `plans/briefs/PB3.1-build-steps.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed at
  pickup).
- **Decisions made at pickup:** the accepted differences in the brief (files not paths, every JSON
  input checked whole, odd types refused, `check-labels` reporting a bad label instead of crashing,
  an empty codebook list counted as missing, `similar`'s weights summed in byte order); the parity
  test runs the binary against the Python, so it covers the commands and the library together.
- **Found at pickup:** CPython 3.12's `sum()` of floats is compensated (Neumaier); a plain running
  sum differed from it in about a quarter of 200,000 random trials, so `similar` uses the same sum.
  The brief's first draft claimed the Python takes other scripts' digits in `round`'s date; it
  does not (`strptime` refuses them), so that difference was dropped.
- **Verified:** `builder/cli/tests/steps_parity.rs`: about 50 runs per case, three cases, two
  members, stdout, exit codes and every written file byte for byte, with modes. The tests were
  written with the code, so they were proved by mutation instead of a red first: 25 mutations, one
  per claim, each restored and checked with `cmp`; 20 failed the parity test at once, 5 survived (no
  fixture reached them), and after five fixtures were added all 25 failed it. A unit test pins the
  compensated sum (a plain sum fails it). `builder/steps/tests/rules.rs`: 11 tests of every refusal's
  words, a sentinel word never in an error, and the Python's own exit code beside each.
  `builder/ingest/tests/private_at.rs`: 5 tests, each mutation-checked. `bash scripts/ci-local.sh`
  green (426 Rust, 151 Python tests).
- **Gates:** `/security-review` (a sub-agent): no high or medium finding; three lows (the state
  folder itself unchecked, two `expect`s a direct library caller could reach, model-written label
  keys echoed in errors). The Verifier (Fable): pass with should-fixes, no must-fix (two more
  accepted differences, `similar` reading a field the Python does not, the panics, a link message,
  copies). All applied except the echoed keys (parity, recorded in `.ai/SECURITY.md`); the new tests
  went red first. The delta pass (Opus): pass with should-fixes, wording and one helper, all applied.
  `bash scripts/ci-local.sh` green again (431 Rust tests).
- **Not verified:** a Python older than 3.12 as the oracle (its float `sum()` is not compensated, so
  `similar`'s order can differ at near-ties); the steps on a real person's corpus (owed to PB8).
- **Known limits:** `chunk` leaves older chunk files beyond the ones it writes, as the Python does;
  PB3.3 must take the chunk list from the library, never the folder.

## PB3.2: the Claude driver (2026-10-24, lane: V1)

- **What:** a new crate `builder/agent` (`persona_builder_agent`), the one road every model call
  takes. The agent list (one entry, `claude-code`); finding Claude Code (absolute `PATH` folders,
  then the native installer's and Homebrew's places, since an app opened from the Finder gets a
  short PATH); a check that it runs and is signed in (only `loggedIn` is read from
  `claude auth status --json`); one tool-less `claude -p` call (argv modelled on Flightdeck's
  `draft`: safe mode, no tools, no MCP servers, no session saved, a JSON schema, `--max-budget-usd`)
  with the person's records on stdin only, run in an empty 0700 folder of its own, killed with its
  whole process group at its time limit; a ledger line per call in `runs.jsonl` (tokens, the CLI's
  own cost estimate, the outcome, never any text); a daily cap in estimated dollars (default 50, 5
  per call), each call on the ledger at its whole budget before it starts; one build per assistant at a time
  (an `flock` on the state folder). `private.rs` gains `lock_state`, `private_folder` and
  `read_private_optional`. The CLI gains `persona-builder agents`, `claude check` and
  `spend <member-dir>`. PB3 was split into PB3.1 to PB3.4 at this pickup. Brief:
  `plans/briefs/PB3.2-claude-driver.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit until 2026-10-30 (probed again
  at dispatch).
- **Decisions made at pickup:** `--bare` is not used, because it never reads the person's own login
  (the headless docs); the work folder must be empty before every call, since without `--bare` a
  `-p` run loads the working folder's `.claude/settings.json` and `.mcp.json`; the session markers
  `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT` are removed from the child's environment and nothing is
  added; a call that is cut off or answers unreadably is recorded at its whole budget, so the cap
  never under-counts; a damaged ledger stops all spending; the cap's defaults (50 a day, 5 a call)
  are this slice's choice and can be changed by flag.
- **Verified:** 40 new tests (30 in the crate, 7 in `ingest`, 3 in the CLI), 37 red on their
  assertion first; 3 (plain folder names, a folder that can't be searched, a budget given back)
  were proved by a mutation instead against stub `claude` scripts: the exact argv, the prompt on stdin and nowhere in the
  argv, the work folder as the current folder, the environment, every reply shape (ok, error, exit
  1, signed out, no output, unreadable, over the output cap), the cut-off killing a grandchild, every
  refusal running nothing and writing no line, the cap (full, exact, yesterday, two calls racing), a
  damaged ledger, the lock. 20 mutations, each restored and checked with `cmp`: 18 failed a test at
  once; one pattern did not apply and one survived (a missing test, added), and both then failed.
  Restored files must be touched before the next build, or cargo reuses the mutated one.
  Live: `persona-builder claude check` against the real Claude Code 2.1.292 on Mimi said "isn't
  signed in" (its own `auth status` reads `loggedIn: false` there), and the exact call argv run by
  hand against the real CLI was accepted flag for flag and answered `is_error: true` with "Failed to
  authenticate…", the shape the driver reads as signed out; the work folder stayed empty.
- **Gates:** `/security-review` (a sub-agent): nothing exploitable; one medium (a build killed
  mid-call left that call's spend uncounted) and four lows (planted settings and an open assistant
  folder, the argv readable by other users, an API key in the environment, a group kill after a
  failed `waitid`). The Verifier (Fable): pass with no must-fix; three should-fixes (a released
  budget untested, the same mid-call crash, a prompt opening with `/` read as a command). All fixed
  in the brief's *After the gates*, each with a test; 11 more mutations, all failed a test. The delta
  Verifier pass: no must-fix; her two should-fixes (an invisible character before the `/`, and the
  time limit after a failed `waitid`) are in, the first with a test; the second has none (a failing
  `waitid` on a live child could not be produced).
- **Not verified:** a successful live call (no signed-in Claude Code CLI on Mimi); owed to an
  attended walk (PB8 covers it). `total_cost_usd` is the CLI's own estimate.
- **Known limits:** the ledger is rewritten whole on each call (like the Python `record`); the cap
  is per assistant, not per person.

## PB2.3: mine the claude.ai export, in Rust (2026-10-24, lane: V1)

- **What:** `persona_builder_ingest` gains `claudeai`, ported from `crew_decisions.py`'s
  `export_files` to `cmd_claudeai_mine`: reading the export (a zip, a folder or a bare
  `conversations.json`, in memory, never unpacked), `inspect` (field names, types and counts),
  `titles` (the review file of every chat, keeping marks already made, with opening words or
  `[filtered]` for untitled chats), `mark` and `mine`, giving the same printed lines, review TSV
  and JSONL bytes as the Python. The CLI gains `persona-builder claudeai inspect | titles | mark |
  mine`; the review (`claudeai-review.tsv`) and the corpus (`decisions-claudeai.jsonl`, the names
  `install.sh` uses) are plain names in `~/.claude/state/<slug>/`, so `write_private` and
  `read_private` now take `.tsv` names too. `text_blocks` and `own_message` moved out of the Claude
  Code miner so both share them. Brief: `plans/briefs/PB2.3-claudeai-export.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit (until 2026-10-30) when the
  brief was dispatched, so the brief was implemented at the same bar and then corrected to match
  what testing proved.
- **Decisions made at pickup:** ten accepted differences, each with a test that also shows what the
  Python does (the brief lists them): a stricter timestamp grammar (16 forms only Python reads,
  measured on 3.12.14); odd types refused; a uuid holding a tab or line break refused (Python
  writes a broken review file); links, pipes, repeated zip names, other compression methods,
  encryption, a zip over 2 GB or over 100,000 entries refused; UTF-8 only; `mark` and `mine` need
  an existing review; plain file names; `--title` read by the `regex` crate; `--holdout` checked as
  `mine` checks it; `inspect` shows only the message senders' values, control characters escaped.
  The `zip` crate keeps one entry per name, so `validate_header_consistency` runs before any read
  (zip 9.0.5 has no public duplicate check).
- **Dependency intake:** `zip` =9.0.5 (zip-rs/zip2, MIT; crate first published 2014; 9.0.5 is a
  week old, chosen over 9.1.0, published hours before pickup), default features off plus
  `deflate-flate2`, and `flate2` =1.1.10 with `rust_backend` (miniz_oxide, pure Rust). Why: nothing
  in the lock read zip archives. The lock adds adler2, bitflags, cfg_aliases, crc32fast,
  miniz_oxide, nix, simd-adler32 and typed-path. Build scripts: crc32fast's (runs
  `rustc --version`) and nix's (`cfg_aliases`), both harmless. RustSec has four `zip` advisories,
  all about extraction (fixed in 9.0.0; nothing here extracts). `cargo audit` is not installed, so
  no advisory scan of the new tree was run.
- **Verified:** parity runs the real Python on 12 invented cases (zip, folder and bare file; a
  nested zip; untitled chats; every shared timestamp form; text and content blocks; the filters
  for both test members; a hand-edited review with odd marks, extra columns and CRLF; Unicode;
  odd shapes for `inspect`), through `inspect`, `titles`, a run of `mark` selectors and 7 `mine`
  split settings, comparing stdout, review bytes and JSONL bytes. The timestamp grammar is checked
  against the Python's own `utc` on 59 strings. 24 mutations, each restored and checked with `cmp`,
  all failed a test; the two review-fix tests went red on the code before the fix.
  `bash scripts/ci-local.sh` passed (151 Python tests, 365 Rust tests; after the review fixes 151 and 367).
- **Known limits:** the export is held in memory (up to 2 GB of JSON plus its parsed tree). The
  100:1 ratio can be beaten by a central directory that overstates the compressed size; the 2 GB
  read cap still holds. The 100,000-entry cap bounds the walk, not memory: the `zip` crate parses
  the whole central directory first (about 350 bytes per entry, measured by the Verifier), so a
  crafted zip under 2 GB listing tens of millions of entries can use many gigabytes before it is
  refused. `inspect` prints key names, which a crafted export could fill with words (the Python does
  too); only control characters are escaped, so Unicode format characters print as they are.
- **Gates:** `/security-review` (a sub-agent): nothing exploitable; two low findings, both fixed
  (`ec58dac`): `sender` values at any depth, and no cap on the zip itself. The Verifier (Fable):
  passed `25ff91e` with no must-fix, after about 70 two-engine round trips and a 538-string
  timestamp fuzz (every mismatch was a form only Python reads); her three should-fixes were gaps in
  the brief's list of differences, now listed. A delta pass on `ec58dac` passed with no must-fix; her two should-fixes are in
  `1301d04`: the sender check is by place, since a key named `chat_messages[].sender` forged the
  path, and the entry cap's limit is stated above.
- **Not verified:** a real claude.ai export, and a multi-gigabyte one (the attended walk in PB8
  covers a real export).
- **Skills Review (PB2 complete: PB2.1 to PB2.4 landed):**
  - ✅ tdd-skill: each test file red first; 24 mutations, each caught.
  - ✅ security-and-hardening: zip caps, links and pipes refused, errors that name places only;
    both review findings fixed.
  - ✅ source-driven-development: the `zip` crate's source, RustSec and Python 3.12's own behaviour
    were read or measured before each decision. The first draft assumed `zip` refuses repeated
    names; the source showed it merges them.
  - ⚠️ plain-language: the messages Persona Builder writes are plain, but the `zip` crate's own
    error text still reaches the person inside "can't be read (…)".
  - ⚠️ codex division: Codex was at a hard limit, so Claude built PB2.3 directly, as the rule allows.
  - ✅ project-config: `scripts/ci-local.sh` with parity required, run with at most 2 test workers
    while the machine was loaded.

## PB2.4: merge and split the corpus, in Rust (2026-10-24, lane: V1)

- **What:** `persona_builder_ingest` gains `corpus` (`merge`, `split`, `read_records`, `discover`),
  ported from `crew_decisions.py`'s `cmd_merge`, `cmd_split` and `read_jsonl`: the same records in the
  same order, the same JSONL bytes, the same printed lines. `private.rs` gains `read_private`, which
  reads a corpus file through the same `O_NOFOLLOW` folder walk as `write_private` and creates
  nothing. The CLI gains `persona-builder merge <member_dir>` (named inputs, or the state folder's
  `decisions-*.jsonl` sorted into history, interview and rounds as `install.sh` does) and
  `persona-builder split <member_dir>`, both with `--dry-run`. Every file is a plain name in
  `~/.claude/state/<slug>/`. The Python's `read_jsonl` now splits at `\n` only: with `splitlines` a
  record holding U+2028 or U+0085 (which `json.dumps` writes raw) failed to read. Brief:
  `plans/briefs/PB2.4-merge-and-split.md`.
- **Who wrote it:** Codex, from the brief and one fix round; Claude wrote the brief and this record
  and reviewed every line.
- **Decisions made at pickup:** the Python's order of checks is kept, so an error lands where the
  Python's would. Python reads in text mode, which turns `\r\n` and a lone `\r` into `\n`; Rust does
  the same (Codex found this one and stopped, as the brief asked). Three differences are deliberate,
  each with its own test: only whole numbers are copied (Python and serde_json write floats
  differently; `-0` is refused); JSON serde_json refuses (`NaN`, a lone surrogate, deep nesting) is
  an error; an `id`, `timestamp` or `kind` that is not a string is refused. On any error Rust writes
  nothing (the Python's `split` can leave a questions file behind).
- **Dependency intake:** none.
- **Verified:** every new test file went red on its assertion first. Parity runs the real Python on
  11 invented cases (two machines, the interview, rounds, filters tightened since mining, ties, Unicode
  line separators, odd key orders and duplicate keys, CRLF and bare CR) with both test members and 6
  split settings, then runs the Python `split` on each result, comparing every file's bytes and the
  printed lines. Four mutations, each restored and checked with `cmp`, each failing a test: no bare-CR
  translation, `read_private` following a file link, interview and round records moving the cutoff,
  no duplicate check. `bash scripts/ci-local.sh` passed (151 Python tests, 330 Rust tests).
- **Known limits:** `split` writes its two files one after the other, so a failure between them can
  leave a new questions file beside an old answers file. Discovery skips file names that aren't
  UTF-8. Reading follows the same walk as writing, so a link anywhere under the home folder is
  refused.
- **Gates:** `/security-review` (a sub-agent): no findings; its two notes (fields the filters don't
  read, and `split` not re-filtering) match the Python, and the files stay in the state folder.
  The Verifier (Fable): passed on `ea7ac86` with no must-fix or should-fix findings. Besides
  checking each claim, she ran 124 checks of her own through both engines (ties, holdout rounding,
  escapes, whole-number extremes, duplicates across modes, mixed line ends), all identical. Her
  notes: Rust reports a bad timestamp before a later file's error (both exit 1, nothing written);
  discovered names pass the name rule by construction; discovery sorts by bytes where `install.sh`
  uses the shell's order (it only decides which copy of a duplicate wins).
- **Not verified:** a merge of real corpora (the attended walk in PB8 covers it).

## PB2.2: mine Claude Code transcripts, in Rust (2026-10-24, lane: V1)

- **What:** `persona_builder_ingest` gains `mine` and `folders`, ported from `crew_decisions.py`'s
  `cmd_mine` and `cmd_folders`: the same records in the same order, the same JSONL bytes
  (`json.dumps(ensure_ascii=False)`), the same printed lines. The corpus goes only into
  `~/.claude/state/<slug>/` (default `decisions-claude-code.jsonl`), 0600 in 0700 folders, through
  `O_NOFOLLOW` directory descriptors and an atomic `renameat` (`src/private.rs`, the crate's only
  `unsafe`). The CLI gains `persona-builder mine <member_dir>` (with `--dry-run`, which writes
  nothing) and `persona-builder folders <member_dir>`. `forget`'s state-folder check moved to
  `member::private_state_dir`, shared with `mine`. The Python's `folders` now breaks project-name ties
  by folder name. Brief: `plans/briefs/PB2.2-mine-claude-code.md`.
- **Who wrote it:** Codex, from the brief and two fix rounds; Claude wrote the brief and this record
  and reviewed every line. Codex also made the one-line fix to PB6's flaky `ureq_wire` test (an
  accepted socket inherits the listener's non-blocking flag on macOS, so a loaded full run sometimes
  failed with `WouldBlock`).
- **Decisions made at pickup:** CPython's own rules are ported where Rust's differ: truthiness,
  `str.isspace` (it also strips U+001C to U+001F), `splitlines` (it also splits at U+2028, U+0085
  and the rest), `errors="ignore"` decoding (drop, never U+FFFD), half-to-even rounding. Where the
  Python raises on a malformed record, Rust stops with `<file>:<line>: a transcript record has an
  unexpected shape (<key>)`, never quoting record text, and writes nothing. Six differences are
  deliberate and each has its own test: links in the history refused; lines `serde_json` can't parse
  (`NaN`, a lone surrogate, deeper than 128) skipped; odd types the Python only tolerates by accident
  refused; `--holdout` limited to 0 to 1; names that aren't UTF-8 refused; a `*.jsonl` entry that
  isn't a file refused.
- **Dependency intake:** `sha1_smol` 1.0.1 (BSD-3-Clause, no dependencies, no build script, no
  network, files or processes), for the record ids the Python makes with `hashlib.sha1`; an id only,
  never a security control. No `cargo audit` scan: it is not installed.
- **Verified:** every test file went red on its assertion first. Parity runs the real Python as the
  oracle on 17 fixture cases built with `tests/crew_fixtures.py` (new `tests/crew_mine_fixtures.py`)
  and 7 split settings, comparing the JSONL bytes and the printed lines; the helpers are checked
  against CPython over every code point (`isspace`), invalid UTF-8 sequences, every line separator and
  every code point below U+0100 (JSON escaping). One disagreement surfaced (an empty `mark_word` kept
  an empty mark) and was fixed. Five mutations, each restored and checked with `cmp`, each failing a
  test: `isspace` without U+001C to U+001F, `splitlines` without U+2028, ordinary rounding, the folder
  walk without `O_NOFOLLOW`, empty marks kept. `bash scripts/ci-local.sh` passed (150 Python tests,
  312 Rust tests).
- **Known limits:** an existing `~/.claude/state` or slug folder keeps its mode (as in the Python);
  the file is still 0600. A project folder swapped for a link between listing and reading is not
  caught (same user only). The summary prints timestamp text from the transcripts.
- **Gates:** `/security-review` (a sub-agent): no findings; its three notes below the bar are in
  Known limits. The Verifier (Fable): her first pass on `1d35cf0` ran out of run time before reporting;
  her second pass, on `a306628`, passed with no must-fix or should-fix findings. She proved the `unsafe`
  file code, the dry run writing nothing, the state folder check, error text, live parity and the
  Python tie-break test, and found no real person in the diff. Her four notes: the parity tests skip
  without Python unless `PERSONA_BUILDER_REQUIRE_PARITY=1` (`ci-local.sh` sets it); an existing
  folder keeps its mode (Known limits); no folder fsync after the rename (durability only); a slug
  may hold a line break, and still stays inside the state folder.
- **Not verified:** a run on a real Claude Code history (the attended walk in PB8 covers it).

## PB2.1: the privacy filters and the privacy check, in Rust (2026-10-24, lane: V1)

- **What:** a new crate, `builder/ingest` (`persona_builder_ingest`), since core keeps to the
  filesystem and this one runs `git ls-files` and `git check-ignore`. It compiles a member's filters
  (`filters`), checks a mined record (`privacy`, for PB2.2 to PB2.4), and checks a folder (`check`),
  ported from `crew_decisions.py`. The CLI gains `persona-builder privacy-check <folder>`, and
  `save push` now runs the real check in place of the "can't run the privacy check yet" stop.
  The save and its check read one file list (`persona_builder_save::git::folder_files`): every file
  the save commits, at any depth, untracked and git-ignored files included. Brief:
  `plans/briefs/PB2.1-privacy-check.md`. PB2 was split into PB2.1 to PB2.4 at pickup.
- **Who wrote it:** Codex, from the brief and three review rounds; Claude wrote the brief and the
  docs, and reviewed every line.
- **Decisions made at pickup:** the `regex` crate, case-insensitive, as the engine. It has no
  look-around and no backreferences (its own docs), so a filter using one is refused in plain words,
  never rewritten; the template's filters use neither. The template's filters are compiled into the
  binary. Hit lines and the summary match the Python byte for byte, `1 hits` included.
- **Dependency intake:** `regex` 1.13.1 (MIT or Apache-2.0, the version Flightdeck pins) with
  `regex-automata` 0.4.18, `regex-syntax` 0.8.11 (both MIT or Apache-2.0) and `aho-corasick` 1.1.5
  (Unlicense or MIT). None has a build script; none touches the network, files or processes. `libc`
  0.2.191 was already in the lock. No `cargo audit` scan: it is not installed.
- **Verified:** every test file went red on its assertion first. Parity runs the real Python as the
  oracle: 12 invented records through `filtered` and `privacy`, and every case of the Python
  `PrivacyCheck` tests, each in a plain folder (the walk) and a git folder (tracked files), plus a
  path-order case, a two-line codebook string, and the walk's ignore list (a repo whose index is
  corrupt, so `ls-files` fails while `check-ignore --no-index` works). No case disagreed. Twelve
  mutations, each restored and checked with `cmp`, each failing a named test: secret winning a
  shared line, secret checked first, case-insensitive, `private.json` skipped, `O_NOFOLLOW`, the `._`
  junk rule, the template comparison, the first hit only for a `member.json` string, ignored paths
  dropped, a missing record key, `save push` using the save's file list (an untracked, ignored file
  with a hit is still refused), and `folder_files` skipping `private.json`. Two missed at first and
  got the last two parity cases. `bash scripts/ci-local.sh` passed.
- **Known limits:** the two engines differ on some Unicode text, proven by the Verifier: Rust's `\w`
  and `\b` count combining marks as word characters and Python's do not, so a number or word followed
  by a combining accent in decomposed text (`1234567` then U+0301, `hoa` then U+0301) is a hit in
  Python and not in Rust; Python's `\s` matches U+001C to U+001F and Rust's does not; an email whose
  first letter carries a combining accent is a hit in Rust only. Not fixed in this slice. The save
  lists the folder twice, once for the check and once for the commit, and `hash-object` reads the
  files again, so a file written between them by the same user is pushed unchecked. File names are
  not matched against the filters. A `member.json` key that trips a filter is named by its parent,
  but a child hit's path still holds that key (as in the Python). Python's `$` also matches before a
  final newline; Rust's does not (no template pattern uses `$`). The CLI's check, like the Python,
  lists only tracked files in a clone, so an untracked file there is not checked; `save push` does
  not have that gap. A file swapped for a link inside a listed folder between listing and reading is
  refused at the last step only (`O_NOFOLLOW` covers the file, not its parent folders; same user).
  `core.fsmonitor` is turned off for the two git calls, but that is not tested.
- **Gates:** `/security-review` (a sub-agent): no high-confidence findings; its three low notes are
  in Known limits. The Verifier (Fable) passed `2a00fde`; her one should-fix, to record the proven
  engine differences, is the Known limits text above.
- **Not verified:** a real `save push` past the check (it needs V-Q1's client ID: PB6.1).

## PB5: turn it on and off, the claude.ai Project pack, forget (2026-10-24, lane: V2)

- **What:** `persona-builder on <member dir>` copies the assistant's `SKILL.md` to
  `<claude home>/skills/<slug>/`, writes `<claude home>/persona-builder/<slug>/RULES.md`, and adds
  one `@` import of it to `CLAUDE.md` inside `<!-- persona-builder:begin/end <slug> -->` markers.
  A record (`persona-builder/installed.json`) lists every folder and file added with the exact text
  written, so `off` removes exactly that, leaves anything the user changed (and says so), and gives
  back `CLAUDE.md` byte for byte. One assistant on per Claude Code folder; a failed `on` rolls back.
  `status` reports the line, each file and whether the member changed since. `pack <member dir>
  --out <dir>` writes `instructions.md` plus one `decisions-<kind>.md` per kind of question in
  `decisions.md` (deterministic). `forget <member dir> [--yes]` deletes only
  `<home>/.claude/state/<slug>`, after a preview. Brief: `plans/briefs/PB5-turn-on-off.md`.
- **Design calls (within the plan, not asked):** "confirmed area" means a kind of question that has
  a section in the committed `decisions.md` (PB4's review edits that same file, so a dropped area
  drops out of the pack). The `CLAUDE.md` import points at a generated `RULES.md` rather than a
  member file, so moving the repo does not break it. `--claude-home` defaults to
  `$CLAUDE_CONFIG_DIR`, else `~/.claude`. Sources (Claude Code memory and skills docs, the claude.ai
  Projects help page) are cited in the brief.
- **Who wrote it:** Codex, from the brief, in three parts plus four fix rounds; Claude reviewed it.
  No new dependency.
- **Rebased onto PB6:** `builder/cli/src/main.rs` was merged by hand (PB5's commands became one
  function each, as PB6's are); a Verifier pass on the resolution found every PB5 line and every
  PB6 line unchanged. `ci-local.sh` green on the rebased head.
- **Verified:** every test went red on its assertion first; mutations on the line break before an
  appended block, the changed-file comparison, the record's containment check, the dropped `Where`
  section, blank areas and `forget`'s path comparison each turned a named test red.
  `bash scripts/ci-local.sh` passed with all Rust steps. Live, with the built binary in a scratch
  home: `on`, a second assistant refused, `status`, `off` (CLAUDE.md byte-identical, nothing left),
  `pack`, `forget` preview, delete, and a second run.
- **Gates:** the security review found a name holding a line break and `@~/…` could put any file's
  import into every session through `RULES.md` (high), and that a shared `SKILL.md` could carry
  hooks or shell lines (medium). Both are fixed: names are one plain line without `@ < > { } [ ]
  ( ) # \` or backticks, and a skill must be named for its slug and ask for no hooks,
  `allowed-tools` or `` !` `` lines. The Verifier (Fable) passed the rest and found a symlinked
  `skills/` folder was refused (fixed), plus nits: a refresh's message after a user edit (fixed), a
  record listing `CLAUDE.md` (refused now), and a test named for rollback that tested an early
  refusal (renamed). A delta Verifier pass confirmed those six fixes and found three more
  (brief items 7 to 9), fixed in `a6006a5`: a `skills` link that points back into the Claude Code
  folder is refused, front-matter keys are checked against an allowlist, and a refresh names why a
  file was left. A second security review and Verifier on that commit both found that a lone
  carriage return hid a second key from the allowlist (YAML breaks a line there, Rust's line split
  does not; Claude Code parses front matter with Bun's YAML parser, read from its binary). Fixed in
  `ff82b8f` (items 10 to 12): a `SKILL.md` holding a lone carriage return, a Unicode line break or
  another control character is refused, so is a tab-indented front-matter line, and the overlap
  refusal has its own message. A third Verifier pass on `ff82b8f`: PASS, no findings in the diff.
  Every fix in both rounds was red first, and a mutation of each turned its test red.
- **Known limits:** `on` with `skills` linked back into the Claude Code folder creates
  `skills/<slug>/` through the link before it is refused, then removes it again (the disk ends as
  it began); the CLI reads `HOME` two ways since PB6 landed beside it (an empty `HOME` is refused
  by `on`, `off`, `status` and `forget` but not by `save`), for a later cleanup; `off` refuses rather than reports when the user replaced a recorded folder with
  a symlink; a dangling-symlink `CLAUDE.md` and `pack --out` under a missing folder give the
  system's own error. The installed skill still calls the Python `similar` and `record` commands;
  a later slice moves it to `persona-builder`.
- **Not verified:** that a real Claude Code session loads the import and the skill (owed to the
  first attended run, PB8), and a real claude.ai Project built from the pack.

## PB6: saving, to an existing repo, a private GitHub repo, or this Mac only (2026-10-24, lane: V1)

- **What:** a new crate, `builder/save` (`persona_builder_save`), because the core library keeps
  to the filesystem. It holds every sentence a person sees while saving (`text`), a `Secret` type
  that hides a token from `Debug` and zeroes it on drop, GitHub's device-flow sign-in with the
  granted-scope check, creating a repo private and reading it back as private, GitHub's address
  rules, the token store in the macOS Keychain, git plumbing that commits an assistant's folder
  without touching anyone's work tree, the saved choice in `<state>/save.json`, and the flow
  (`detect`, `options`, `choose_*`, `save_now`). The CLI gains `persona-builder save options |
  sign-in | sign-out | choose (local | here | github) | push`. Brief: `plans/briefs/PB6-saving.md`.
- **Who wrote it:** Codex, in separate test and implementation passes for each half; Claude wrote
  the brief, the person-facing texts and the docs, and reviewed every test and line.
- **Decisions made at pickup:** the network, git and Keychain code is a crate of its own, so core's
  "filesystem only" rule stays true. Sign-in is an OAuth app's device flow with the `repo` scope,
  the only scope that can make a private repo; the client ID is unset until V-Q1 is answered, and
  sign-in says it is not set up. A save to a person's own repo goes to a `persona/<slug>` branch
  of its own (the guided build's step 14 also saves to a branch, `crew/<slug>`), built from a
  scratch index, so their index and work tree never change; if that branch is checked out
  anywhere, the save is refused rather than moving it. The GitHub backup is a bare repo in the state folder, pushed with the
  token as an environment header scoped to `https://github.com/`, the person's git config off.
  Every push address must read back private from GitHub's API before every push; any other host
  is refused, since nothing else can be confirmed. The privacy check is a seam: the CLI's `push`
  refuses with "can't run the privacy check yet" until PB2 plugs the Rust check in.
- **Dependency intake:** `ureq` 3.4.2 (rustls, no default features; MIT or Apache-2.0; the same
  pin Flightdeck uses), and `base64` 0.23.1, `libc` 0.2.191 and `zeroize` 1.9.1, which were
  already in ureq's tree. New in the lock: ureq's tree (rustls, ring, webpki-roots,
  rustls-pki-types, ureq-proto, http, httparse, percent-encoding, untrusted, subtle, log,
  getrandom, cc, utf8-zero). `ring` builds C and assembly through `cc` at build time. `cargo
  audit` is not installed, so no advisory scan was run.
- **Sources:** GitHub's docs on the device flow, OAuth scopes, the repos REST API and API
  versions, and actions/checkout's git auth header, all read 2026-10-24 and cited in the brief
  and the code.
- **Verified:** every test file went red before its code existed (part 1: 65 tests on stubs;
  part 2: 51 tests on stubs plus the review fixes' new files red on their assertions), then green:
  135 Rust tests in the two packages, and `bash scripts/ci-local.sh` passed. Mutation checks, each
  anchored, restored from a backup and confirmed with `cmp`, and each failing a named test, not a
  build: 26 in all (bearer only on API calls; both private fields; the exact host; the slow-down
  step; the exact `repo` scope; no redirects; the expiry stop; the device page's host; the
  Keychain alphabet; the read-back after create; the git config off for the backup repo, both
  variables; the credential helper off; the cleared environment; `private.json` skipped; links
  refused; the credential hidden in errors; `--no-gpg-sign`; the privacy check run; the remote
  check run; the backup's exact origin; every push URL on GitHub; a repo `detect` did not offer;
  the remote-name rule; the 0700 folder; a local choice sending nothing). Two missed at first; two
  tests were added, and both now fail when their line is removed. The live Keychain test (a throwaway
  `persona-builder.test` item) passed and left nothing behind. The CLI was driven by hand in a
  scratch home: the options with and without a repo around `~/.claude`, every refusal, and a
  `save.json` carrying `--upload-pack=…` as its remote refused before git ran. A build with an
  invented client ID reached github.com over rustls and turned GitHub's real `404 {"error":"Not
  Found"}` into the plain "sign-in didn't work" line.
- **Test harness, fixed on review:** on this Mac `python3` is a pyenv shim, which macOS will not
  run as a `#!` interpreter, so the git wrapper now resolves the real interpreter; APFS refuses
  non-UTF-8 file names, so that case is skipped where the filesystem refuses it; one test deleted
  the folder holding its own record. Codex stopped on that one rather than bend the code.
- **Gates:** `/security-review` (run as a sub-agent): no high-confidence findings; two low notes
  kept below. The Verifier (Fable) failed the first pass on one must-fix, that the token alphabet had
  no `_` and so would refuse real `gho_…` tokens. That was a misread: the alphabet's literal is
  `._~:+/=-`, whose second character is `_`. Realistic `gho_` fixtures now pin it. Her
  should-fixes, all fixed test-first:
  a save moved the person's `persona/<slug>` branch when it was checked out (now refused); a poll
  reply with status 400, as RFC 8628 sends, was treated as unexpected; a relative `state_dir` was
  accepted. Her second pass, on those fixes, passed with one should-fix, also fixed test-first: a
  400 reply carrying `access_token` was taken as the token; a token is now read only from a 200
  reply (one test, which fails when the status check is removed).
- **Known limits:** an existing state folder keeps its mode (only one Persona Builder creates gets
  0700), and `backup.git` takes the umask's modes inside it. The folder is committed whole except
  `private.json` and `.DS_Store`: `.gitignore` is not read, so PB2's privacy check must cover any
  other file kept there. Copies of the token outside `Secret` (form fields, the git header, the
  child's environment, visible to the same user while git runs) are not zeroed. The Keychain item
  can be read by any process of the same user through `/usr/bin/security`. A lost `backup.git` on
  a second Mac makes the next push a non-fast-forward, refused, with no recovery yet; a git failure
  after GitHub made the repo leaves the repo, so a retry under the same name is refused. A file swapped for a link between listing and hashing would be read through the link
  (same-user only). The person's own `pre-push` hook runs on a save to their repo, with Persona
  Builder's cleared environment. A non-GitHub remote cannot be used at all. The check that the
  `persona/<slug>` branch is not checked out runs before the branch moves, so a checkout in
  between still moves it (same-user only). A worktree whose folder was deleted but not pruned
  still counts as having the branch checked out, so the save is refused until `git worktree prune`
  runs; the message does not say so yet.
- **Not verified:** a real sign-in, repo creation and push (they need V-Q1's client ID and PB2's
  privacy check: PB6.1, card `51dkJ0YE`, an attended walk); Linux (the Keychain is macOS only).

## PB1: the member format in Rust, the neutral base, one field pack (2026-10-24, lane: V1)

- **What:** `builder/core` reads and writes an assistant's files the way the Python tooling does:
  `member.json` with `private.json` laid over (`member`), JSON written byte for byte as Python's
  `json.dumps(indent=2)` (`json`), a new `crew/<slug>/` folder filled from the template as
  `install.sh` fills it, without a kit, and removed again if filling it fails (`crew`), the dial in `autonomy.md` (`autonomy`) and field
  packs (`fields`). The CLI gains `persona-builder member show <member.json>`. The template's
  `IDENTITY.md` and `autonomy.md` are rewritten in plain words, keeping every dial key and its
  state; `crew/_fields/general.md` is the first field pack. Brief:
  `plans/briefs/PB1-member-format.md`.
- **Who wrote it:** Codex, from the brief; Claude reviewed it and wrote the user-facing text.
- **Dependency intake:** `serde` 1 (derive) and `serde_json` 1 (`preserve_order`), both MIT or
  Apache-2.0 and the standard Rust JSON pair. They add `serde_core`, `serde_derive`, `itoa`,
  `memchr`, `zmij` (serde_json's float formatter, same author), and `indexmap` with `hashbrown`,
  `equivalent` and `foldhash`. None of them touches the network, files or processes. No
  `cargo audit` scan was run because it is not installed.
- **Verified:** every test went red on its assertion first. The parity tests run the real Python:
  Rust's `read_member` matches `crew_decisions.py`'s byte for byte on three invented members and
  gives the same error on two invalid ones. `new_member` matches `install.sh --no-history` file for
  file and byte for byte on four invented cases, one of them non-ASCII. Five mutations each turned
  their test red: the ASCII escaping, the private-filter join, the placeholder order, the skipped
  `README.md` and the cleanup on failure. `bash scripts/ci-local.sh` passed with all Rust steps; it
  sets `PERSONA_BUILDER_REQUIRE_PARITY=1`, so a missing `python3`, `git` or `bash` fails the run
  instead of skipping the parity tests.
- **Gates:** `/security-review` found three low issues; the two cheap ones are fixed (the target
  folder is created with `create_dir`, and entry types come from `file_type()`). The Verifier
  (Fable) found that parity holds for configs in the template's shape but not beyond it. Fixed: a
  nested `member.json` is now copied unfilled, as `install.sh` does, and a non-string filter in
  `member.json` is refused instead of being replaced. Two `IDENTITY.md` sentences overclaimed: one
  said a change is kept only once approved (that step comes in PB4), and one promised no backup.
  Both now say only what is true today.
- **A miss caught by the gate:** the first `autonomy.md` text said "passwords", which the
  template's own secret filter flags, so the privacy-check tests failed. It now says "logins".
- **Known limits (documented in the code):** the writer puts keys in the struct's order with
  unknown keys last, drops `null` optional fields and empty codebook lists, and reads numbers as
  `f64` or `i64`. Python keeps all of these as they were. `slugify` matches `install.sh` under
  `LC_ALL=C`; in a UTF-8 locale `tr` may lowercase some non-ASCII letters differently. Rust keeps
  CRLF line endings and does not copy a file's executable bit, while `install.sh` normalizes the
  first and keeps the second. The template has none of these cases.
- **Not verified:** a kit (`install.sh --kit`), which `new_member` does not cover yet.

## PB0: the Rust workspace and its checks (2026-10-23, lane: V2)

- **What:** a virtual Cargo workspace at the root (edition 2024, stable, no toolchain file) with
  `builder/core` (package `persona-builder`, library `persona_builder`, one `VERSION` constant) and
  `builder/cli` (binary `persona-builder`, clap derive, `--version` and `--help` only).
  `scripts/ci-local.sh` runs `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D
  warnings` and `cargo test --locked` after the existing steps, and prints a skip line when `cargo`
  is missing. `.ai/PROJECT.md`, `README.md` and `CONTRIBUTING.md` name the new folders and the Rust
  requirement. `target/` was already ignored. Brief: `plans/briefs/PB0-rust-workspace.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit (until 2026-10-29), so the
  brief was implemented by hand at the same bar.
- **Dependency intake:** `clap` 4.6 with `derive` (MIT or Apache-2.0, the standard Rust argument
  parser); the lock adds only clap's own tree (anstream, anstyle, clap_lex, heck, strsim, syn,
  proc-macro2, quote, windows-sys on Windows). No network, file or process access of its own.
  `cargo audit` is not installed here, so no advisory scan was run.
- **Verified:** red first (`builder/cli/tests/version.rs` failed on its assertion against an empty
  `main`), then green; a mutation (core `VERSION` set to `9.9.9`) turned it red and was restored with
  `cmp`. `bash scripts/ci-local.sh` passed with cargo on `PATH` (all three Rust steps ran) and with
  `~/.cargo/bin` removed from `PATH` (it printed the skip line and passed).
- **Not verified:** a Linux or Windows build; a contributor machine with an older stable Rust than
  1.98 (edition 2024 needs 1.85 or newer).
- **Landed by V1 (2026-10-24):** rebased onto `main` (one README table conflict, both rows kept),
  re-authored to the project's noreply identity, and pushed to `main` as a fast-forward so the
  committer stays noreply too. `bash scripts/ci-local.sh` passed again with and without `cargo` on
  `PATH`.

## PB4.0: the growth loop spec (2026-10-23)

- **What landed:** `docs/spec/growth-loop.md`, five JSON Schemas and five invented examples in
  `docs/spec/growth-loop/`, a standard-library validator for exactly the keywords the schemas use
  (`tests/schema_subset.py`, which refuses any other keyword anywhere in a schema), and
  `tests/growth_loop_spec_test.py`, wired into `scripts/ci-local.sh`. Brief:
  `plans/briefs/PB4.0-growth-loop-spec.md`.
- **Who built it:** implemented directly from the brief; the usual implementer was at a usage limit.
- **Decisions made at pickup:** no new dependency (a small validator instead of `jsonschema`, so the
  kit still needs only Python and shellcheck); `additionalProperties: false` everywhere, so any shape
  change is a new version; the file-path pattern avoids lookaround so a Rust implementation can compile
  it; a case's status changes by appending the whole case again (last line wins).
- **Review:** an independent review passed it with should-fixes, all applied: rule 3 now requires the
  results to cover every case the proposal names (an omitted new case can no longer pass), versioning
  matches the closed schemas, rejected proposals get no measurement, a stale `base` is never applied,
  and the doc lists the checks the schemas cannot make. Security review: no findings.
- **Verified:** red first (validator assertions failed, spec files missing), then green; seven mutation
  checks each failed the test and were restored (`cmp`); the reference `jsonschema` 4.x library, in a
  throwaway environment, agreed on all five schemas and examples and on the path probes;
  `bash scripts/ci-local.sh` green.
- **Not verified:** the patterns under a Rust or JavaScript regex engine (they use only portable
  syntax; PB4 will run them in Rust).
