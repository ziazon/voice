# VF3 brief: Claude Code keeps its own login file inside Voice's sandbox

`voice claude sign-in` with a Claude subscription (testers' builds, `--claudeai`) fails with
`SIGN_IN_FAILED` after the browser step: Claude Code can't save its login inside the sandbox. The
owner chose the fix (V-Q15 option 1, 2026-11-09): **let Claude Code start `/usr/bin/security`, keep
the Keychain closed, and let Claude Code fall back to its own `.credentials.json` in Voice's profile,
which the sandbox already allows.** Voice itself never reads, copies or passes the login. Around
that, four smaller pieces: keep the profile out of Time Machine, add a Claude sign-out, refuse the
Console sign-in on macOS until a later item (VF5) gives it a way to keep its key, and update the
docs.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md` (the rows on the app's Claude sign-in
and the sandbox), `CONTRIBUTING.md`, `docs/spikes/VO11-os-sandbox.md`, `plans/briefs/VO17-console-sign-in.md`
(the previous change to this code; follow its patterns). Code to read before writing any:
`builder/agent/sandbox/claude-code.sb`, `builder/agent/src/sandbox.rs`, `builder/agent/src/claude.rs`
(`ensure_profile`, `sign_in_with`, `status`, `login_args`), `builder/agent/src/child.rs`,
`builder/agent/src/text.rs`, `builder/agent/tests/sandbox_profile.rs`, `builder/agent/tests/profile.rs`,
`builder/agent/tests/launch.rs`, `builder/api/src/assistants.rs` (`claude_check_in`,
`claude_sign_in_in`, `ClaudeSignIns`), `builder/api/src/save.rs` (`github_sign_out_in`: the model for
the new sign-out), `builder/api/src/parity.rs`, `builder/api/src/dto.rs`,
`builder/api/tests/claude_sign_in.rs`, `builder/app/src/commands.rs`, `builder/app/tests/lockdown.rs`,
`builder/cli/src/main.rs` (`ClaudeCommand`, `claude_sign_in`, `save_sign_out`),
`builder/cli/tests/agent.rs`, `builder/cli/tests/app_parity.rs`,
`builder/ui/src/screens/ConnectClaude.vue` and its test, `builder/ui/src/backend.ts`,
`builder/ui/src/app/tauri-backend.ts` and its test, `builder/ui/src/test-support.ts`,
`scripts/ci-local.sh`.

Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence a person reads).

This is a public repo: invented names only, no real person, no fixed paths. **Commit nothing; leave
every change in the working tree.** Never print, log or assert on a secret's value; no test may
read the person's real Keychain or real `~/.claude`.

## Facts already checked (do not re-derive; cite them in comments where the code depends on them)

Measured on a Mac with macOS 27 and Claude Code 2.1.292, with the real `claude-code.sb`,
2026-11-08 and 2026-11-09:

1. Claude Code saves a subscription login through the Keychain by running `security` (by name, from
   `PATH`). The sandbox refuses to start it, Claude Code takes that as "try again later", skips its
   file fallback and fails: that is VF3.
2. Adding only `(allow process-exec (literal "/usr/bin/security"))` makes the subscription sign-in
   succeed. `security` starts but can't reach the Keychain (it exits 155, "The authorization was
   denied"), so Claude Code writes `<profile>/.credentials.json` (mode 0600). `auth status --json`
   then says `loggedIn: true`, `authMethod: "claude.ai"`, and a `-p` call answers. `auth logout`
   exits 0 and removes `.credentials.json`. Under that profile the person's own Claude Code
   Keychain item can't be found from inside the sandbox.
3. Claude Code's docs (code.claude.com/docs/en/authentication, read 2026-11-09): "When the Keychain
   rejects the write ... Claude Code stores your login in `~/.claude/.credentials.json` with file
   mode `0600` instead", kept under `CLAUDE_CONFIG_DIR` when that is set; and "A Console login that
   creates an API key fails until the Keychain is writable."
4. With the same one-line change, `claude auth login --console` asks for the `org:create_api_key`
   scope, creates an API key on the person's Console, then fails with "Failed to save API key to
   macOS Keychain" and exits 1: the person ends up not signed in, with an unused key left on their
   Console. So on macOS the Console sign-in must not start at all until VF5.
5. `/usr/bin/tmutil addexclusion <path>` (no `-p`) works without admin rights: exit 0, `tmutil
   isexcluded` then says `[Excluded]`, and it sets the extended attribute
   `com.apple.metadata:com_apple_backup_excludeItem` on the path.

## What to build

### 1. The sandbox may start the Keychain program, and nothing more (`claude-code.sb`)

- Add `(allow process-exec (literal "/usr/bin/security"))` next to the existing `process-exec`
  rules, **unconditionally** (sign-in and ordinary calls: Claude Code reads and refreshes its login on
  every call). Comment it with fact 2: why, and that the Keychain stays closed because the profile
  allows none of the Keychain's services and denies `Library/Keychains`.
- Change nothing else in the profile. In particular, add no `mach-lookup` allow and no Keychain path.
- Replace the comment above the `.credentials.json` allow so it says the file is Claude Code's own
  fallback when the Keychain refuses the write (fact 3).

Tests in `builder/agent/tests/sandbox_profile.rs`, written first and seen red:

- `starts_only_the_program_and_the_keychain_program` (rename and extend `runs_only_the_program` or
  add beside it): under the profile, `/usr/bin/security` starts (an exec of it does not fail with
  "Operation not permitted" / status 126 the way `/usr/bin/true` does), while `/usr/bin/true` and
  any other program still fail as today.
- `the_keychain_stays_closed`: under the profile, `/usr/bin/security add-generic-password -a
  voice-test -s <a fresh random service name> -w unused` exits non-zero, and
  `/usr/bin/security find-generic-password -s <that service>` exits non-zero; outside the sandbox,
  delete that service with `delete-generic-password` in a cleanup guard in case a write ever got
  through, and fail the test if the delete found one. Capture and discard both commands' output;
  never print it.
- Keep `only_the_profiles_own_login_is_allowed` passing unchanged.

These run `sandbox-exec`, which may not run inside your own sandbox: the file's `available()` guard
already skips them there. Say so in your report; the reviewer runs them outside.

### 2. The profile stays out of Time Machine (`claude.rs`, `ensure_profile`)

- On macOS, after `ensure_profile` has made or checked the profile folder, make sure it carries the
  `com.apple.metadata:com_apple_backup_excludeItem` attribute: read it with `libc::getxattr` (no
  process when it is already there); when missing, run `/usr/bin/tmutil addexclusion <folder>`
  (absolute path, fixed argv, no shell, stdin/stdout/stderr null, a short timeout). Exclude the
  profile folder (`.../Voice/claude-code`), not the whole app folder.
- A failure here (tmutil missing, non-zero exit, timeout) never fails `ensure_profile`: the sign-in
  and calls go on. It stays silent; the next `ensure_profile` tries again.
- Not on other systems (`#[cfg(target_os = "macos")]`).

Tests in `builder/agent/tests/profile.rs` (macOS only): a fresh profile under a temporary home
carries the attribute after `find`; a profile that already carries it is left alone (no tmutil
run: make that observable through a seam, for example a function that takes the tmutil path, and
point it at a stub that records it ran); a failing tmutil stub does not fail `find`.

### 3. A Claude sign-out

- `Claude::sign_out(&self, home)` in `claude.rs`: runs `auth logout` in the sandbox (`launch(home,
  false)`, the same `child::run` with a short timeout, nothing on stdin). Exit 0 is success. After
  success, if `<profile>/.credentials.json` still exists, return a fixed error (new `text.rs`
  constant) instead of deleting it: Voice never touches the login file itself.
- api: `claude_sign_out_in` and `claude_sign_out` in `assistants.rs`, modelled on
  `github_sign_out_in`. Refused (fixed message) while a Claude sign-in is running in
  `ClaudeSignIns`. Returns the fresh `ClaudeStatus` (as `claude_check` does), so the screen shows
  signed out.
- CLI: `voice claude sign-out` (`ClaudeCommand::SignOut`), printing one plain line on success.
- App: a `claude_sign_out` command in `commands.rs` and its `lockdown.rs` entry, a parity entry
  `claude_sign_out` ↔ `["claude", "sign-out"]` in `parity.rs`, the `backend.ts` and
  `tauri-backend.ts` call, and a **Sign out** button on `ConnectClaude.vue`, shown only when signed
  in, styled like the screen's other secondary action.
- Words (plain-language skill): the button says "Sign out". Success: "Signed out of Claude Code in
  Voice." The file-left-behind error: "Claude Code signed out, but its sign-in file is still in
  Voice's folder. Try signing out again." Refused during a sign-in: "Wait for the sign-in to finish
  or cancel it, then sign out."
- Tests: agent (a stub `claude` whose `auth logout` exits 0 and removes the file → `Ok`; exits 1 →
  error; exits 0 but leaves the file → the fixed error and the file is still there), api (refused
  while a sign-in runs; returns signed-out status), CLI (`claude sign-out` output and exit code),
  parity, lockdown, UI (button only when signed in; calls the backend; shows the result).

### 4. On macOS the Console sign-in does not start (until VF5)

- In `sign_in_with`, before launching anything: on macOS, `SignIn::Console` returns a fixed error
  (new `text.rs` constant), and no child starts. Comment it with fact 4.
- Words: "Signing in with a Console account doesn't work on a Mac yet. A later update adds it."
  In a testers' build, add: "Use Sign in with a Claude subscription instead." (two constants, or
  one built per build; the subscription wording must not exist in a build without
  `claude-subscription`, as VO17 already requires).
- `ConnectClaude.vue`: in a default build on macOS the Console button stays but the error shows as
  any other sign-in error; do not hide the screen.
- Tests: agent (Console on macOS returns the error and the stub never ran), api and CLI (the error
  reaches the person), UI (the error text shows). Keep the existing Console tests working on other
  systems or behind `cfg`, whichever the code already does for platform differences.

### 5. Docs

- `.ai/SECURITY.md`: the sandbox row says the child may start `/usr/bin/security` and why, that the
  Keychain stays closed (no Keychain services, `Library/Keychains` denied), that Claude Code keeps
  its login in `.credentials.json` (0600) in Voice's profile, which Voice excludes from Time Machine,
  and that `voice claude sign-out` removes it through Claude Code's own `auth logout`. Name the new
  tests.
- `.ai/DECISIONS.md`: one entry, VF3 (2026-11-09): option 1 of V-Q15 chosen by the owner (the login
  file reconfirmed, as V-Q10 chose on 2026-10-31); the RAM-volume and Voice-held-login options
  were rejected (memory cost; Voice would hold the login); Console waits for VF5.
- The module doc at the top of `claude.rs`: one line on the sign-in's storage.

## Done means

- Every test above written first and seen failing, then passing; `bash scripts/ci-local.sh` green
  (report anything it skipped, and why).
- `git diff --stat` touches only the files named here plus their tests.
- Your report lists: each test and what it proves, which tests were skipped in your sandbox, and
  anything in this brief you found wrong in the code (stop and say so rather than guessing).

## Fix-up 1 (2026-11-09): register the app command

The brief left out the app's registration files. They are in scope: register `claude_sign_out` in
`builder/app/src/command_names.rs`, `builder/app/src/main.rs` and `builder/app/capabilities/default.json`,
in the same order as the other lists, exactly as `github_sign_out` is registered. Then run
`bash scripts/ci-local.sh` to the end and report the result. Change nothing else. Commit nothing.

## Fix-up 2 (2026-11-09): two review findings

1. `builder/cli/src/main.rs` `claude_sign_in` repeats the agent's macOS Console rule
   (`!cfg!(target_os = "macos") || sign_in != claude::SignIn::Console`) to decide whether to print
   `SIGN_IN_WAITING`. Keep the rule in one place: add `pub fn starts(sign_in: SignIn) -> bool` (or
   a similarly named function) in `builder/agent/src/claude.rs`, use it in `sign_in_with`'s guard and
   in the CLI, and test it in the agent.
2. In `ClaudeCommand`, put `SignOut` after `SignIn`, so `voice claude --help` lists check, sign-in,
   sign-out.

Run the agent and CLI suites in both builds (default and `--features claude-subscription`) and
clippy with `-D warnings`. Change nothing else. Commit nothing.

## Fix-up 3 (2026-11-09): the Verifier's should-fixes

1. **`builder/agent/tests/sandbox_profile.rs` `the_keychain_stays_closed`:** the
   `find-generic-password` assertion is vacuous (the item never exists, so it fails with or
   without the bug). Make it real: outside the sandbox, create the throwaway item first
   (`security add-generic-password -a voice-test -s <fresh random service> -w unused`, output
   discarded), then assert the sandboxed `find-generic-password -s <that service>` exits non-zero,
   and delete it in the cleanup guard (the existing guard; it must now expect to find one). Keep the
   sandboxed `add-generic-password` assertion on a second fresh service as it is, with its own
   guard that fails if a delete finds one.
2. **`builder/api/src/assistants.rs` `claude_sign_out_in` holds `sign_ins.state` across up to
   ~90 s of child runs,** so `claude_sign_in_shutdown` (app quit) and a concurrent
   `claude_sign_in_in` block on the mutex. Add a separate "signing out" marker to `ClaudeSignIns`
   (for example an `AtomicBool` or a second variant in the state): take the lock only to check that
   no sign-in runs and set the marker, release it, run the sign-out and the status check, then clear
   the marker on every path (a drop guard). While the marker is set, `claude_sign_in_in` refuses at
   once with a fixed sentence: "Wait for the sign-out to finish, then sign in." and a second sign-out
   refuses with the existing "Wait for the sign-in to finish…" style sentence adapted: "Voice is
   already signing out." Put all three refusal sentences and the CLI/UI success sentence in
   `text.rs` (or the api's existing fixed-text place) rather than as literals, matching how the
   other fixed sentences are kept. Tests: sign-in refused while a sign-out runs; shutdown/cancel does
   not wait for a running sign-out (bounded, e.g. returns within 1 s while a stub logout sleeps);
   the marker is cleared after a failed sign-out.
3. **`.ai/SECURITY.md`, the new sandbox row:** add one sentence: `/usr/bin/security` carries
   private Apple entitlements and the base profile allows `com.apple.AppSSO.service-xpc`; the
   commands that reach it (`security platformsso …`) need an admin's authorization, which the
   sandboxed child can't get (no UI), measured refused on 2026-11-09.

Run the agent, api and CLI suites in both builds and clippy with `-D warnings`, and the UI suite if
you touched it. Change nothing else. Commit nothing.

## Fix-up 4 (2026-11-09): the repeat Verifier's findings

1. **`.ai/SECURITY.md`, the new sandbox row:** fix-up 3's sentence was wrong. In
   `/System/Library/Sandbox/Profiles/system.sb` the `com.apple.AppSSO.service-xpc` allow sits inside
   `(define (system-network) …)` (checked: lines 277-311), which `claude-code.sb` never calls, so the
   lookup falls under `(deny default)`. Replace the sentence with: "`/usr/bin/security` carries
   private Apple entitlements; `system.sb` defines an AppSSO allow only inside `system-network`,
   which this profile doesn't call, and `security platformsso …` was measured refused on
   2026-11-09."
2. **`builder/api/tests/claude_sign_in.rs` (around line 538):** the shutdown-does-not-wait test
   passes `claude_sign_in_shutdown(Duration::from_millis(100))` and checks it returns within 1 s, so
   a shutdown that waited on the sign-out marker up to its bound would still pass. Pass a wait of
   5 s and keep the 1 s check, so waiting on the marker fails it.

Run the api suite in both builds. Change nothing else. Commit nothing.
