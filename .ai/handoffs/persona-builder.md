# Persona Builder: slice records

One record per slice, newest first. Paraphrase only; no real person.

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
