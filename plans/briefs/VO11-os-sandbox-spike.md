# VO11 brief: the OS sandbox spike (a Seatbelt profile for Claude Code, and a probe)

Persona Builder (soon Voice) calls the person's own Claude Code as a child process
(`builder/agent/src/claude.rs`): a tool-less `claude -p` with its own profile folder
(`CLAUDE_CONFIG_DIR`). Today nothing at the operating-system level stops that child from reading
any file the person can read, or from connecting anywhere. This spike lands **a macOS Seatbelt
profile for that child**, **tests that prove what the profile allows and refuses**, and **a probe
script** a person runs to check their own Claude Code inside it (the attended walk). It does
**not** wire the profile into `claude.rs`: that is the next slice, after a question about where the
login lives is answered.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/source-driven-development/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`
(every sentence a person reads), `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths (find the repo from the
file's own location), and **no real secret in any fixture**: canary files hold the text `canary`
and nothing else. Add no dependency (Rust or Python). Python must run on 3.9. Commit nothing; leave
every change in the working tree.

**Your own sandbox:** you run inside a Seatbelt sandbox, and macOS refuses a sandbox inside a
sandbox (`sandbox-exec: sandbox_apply: Operation not permitted`, measured). The Rust tests in § 2
therefore cannot run for you. Build them so they compile and pass `cargo clippy`, run
`cargo test -p persona-builder-agent --test sandbox_profile` once to see the skip message (§ 2.1),
and say so in your summary. V1 runs them outside the sandbox. The Python tests in § 4 run anywhere.

## What was measured on macOS 27.0.1 (V1, 2026-10-31), so you don't re-derive it

- The last matching rule wins: a `(deny file-read* …)` placed **after** `(allow file-read*
  (subpath ROOT))` denies a `.env` inside ROOT; placed before, it does not.
- Seatbelt checks the real path: a symlink inside an allowed folder that points outside is
  denied. (Corrected after the first dispatch, § 6: its regexes ignore letter case, on a
  case-sensitive volume too, so the letter classes in § 1 are a backup, not the control.)
- A hard link inside an allowed folder to a file outside it **is readable** (Seatbelt checks the
  path, not the file). Record only; nothing to build.
- `claude --version` and `claude auth status --json` run with `(deny default)`, `(import
  "system.sb")`, `process-exec`/`process-fork`, reads of the system folders, the program folder and
  the profile folder, and writes to the profile folder only. Broad `file-read-metadata` and
  `sysctl-read` are not needed.
- `claude -p` also needs `CLAUDE_CODE_TMPDIR` set to a folder it may write (else `EPERM … open
  '/tmp/claude-<uid>'`); `TMPDIR` alone does not do it.
- `(allow network-outbound (remote ip "localhost:<port>"))` lets a child reach that port only: a
  listener on another localhost port, and `1.1.1.1:443`, are refused, and DNS fails (the
  `system-network` helper in `system.sb` is not called, so `mDNSResponder` is out of reach). `curl`
  reached `api.anthropic.com` through an HTTP CONNECT proxy on the allowed port. Claude Code
  follows `HTTPS_PROXY`; it does not support SOCKS (`https://code.claude.com/docs/en/network-config`).
- `claude auth login --claudeai` inside the profile fails `Failed to start OAuth callback server`
  unless the child may listen on localhost; with `(allow network-bind network-inbound (local ip
  "localhost:*"))` it starts and waits for the browser. SBPL takes a conditional: `(if (string=?
  (param "SIGN_IN") "yes") (allow …))` works.
- The profile reaches no Keychain: `system.sb` does not allow `com.apple.SecurityServer`. Claude
  Code keeps its login in the Keychain (keyed to `CLAUDE_CONFIG_DIR`) and, "when the Keychain
  rejects the write", in `.credentials.json` in that folder, mode 0600
  (`https://code.claude.com/docs/en/authentication`). So inside this profile the login is the
  file in the profile folder. Whether that is acceptable is a question for the owner; the profile
  is built this way for the spike.

## 1. The profile: `builder/agent/sandbox/claude-code.sb`

A new file, SBPL, commented in plain words. Parameters (each passed with `-D NAME=value`, every one
always given): `PROGRAM_DIR` (the folder holding the real `claude` binary, links resolved),
`PROFILE_DIR` (Claude Code's profile folder), `RUN_DIR` (this run's own temporary folder, also the
working folder), `PROXY_PORT` (the egress proxy's port on localhost), `SIGN_IN` (`yes` or `no`).
Paths are absolute and canonical (no links), which the caller guarantees.

In this order:

1. `(version 1)`, `(deny default)`, `(import "system.sb")`.
2. `(allow process-exec process-fork)`, `(allow signal (target same-sandbox))`,
   `(allow process-info* (target same-sandbox))`.
3. Reads: `(allow file-read* …)` of `(subpath "/usr")`, `(subpath "/System")`,
   `(subpath "/Library")`, `(subpath "/private/var/db")`, `(subpath "/private/etc")`,
   `(subpath "/dev")`, and the `PROGRAM_DIR`, `PROFILE_DIR` and `RUN_DIR` subpaths. Nothing under
   the home folder other than those parameters.
4. Writes: `(allow file-write* …)` of the `PROFILE_DIR` and `RUN_DIR` subpaths and
   `(literal "/dev/null")`.
5. Network: `(allow network-outbound (remote ip (string-append "localhost:" (param "PROXY_PORT"))))`
   and nothing else outbound.
6. Sign-in only: `(if (string=? (param "SIGN_IN") "yes") (allow network-bind network-inbound
   (local ip "localhost:*")))`.
7. **The secrets deny list**, after every allow so it wins: `(deny file-read* file-write* …)` with
   one regex per entry of the built-in list in `builder/core/src/deny.rs` (`NAMES` and
   `SEQUENCES`), matched as a whole path component, **both letter cases** (write each letter as a
   class, `[Ee][Nn][Vv]`, since SBPL regexes have no case flag):
   - names: `.ssh`, `.aws`, `.gnupg` (the folder and everything in it); `.env*`; `.envrc`;
     `.npmrc`; `.netrc`; `.pgpass`; `*.pem`; `*.key`; `*.p12`; `*.keychain`; `*.keychain-db`;
     `.credentials.json`;
   - sequences: `.config/gh` (and inside), `.docker/config.json`, `Library/Keychains` (and inside).
   A name entry matches a component exactly: `.env*` denies `.env`, `.env.local`, `.environment`;
   `*.key` must not deny `monkey.txt`, `keys.md` or `a.keyring`; `docker/config.json` (no dot)
   and `.docker/other.json` stay readable. Same rules as `deny.rs`.
8. **The child's own login**, last: `(allow file-read* file-write* (literal (string-append (param
   "PROFILE_DIR") "/.credentials.json")))`, so the deny list's `.credentials.json` holds
   everywhere except Claude Code's own file in its own profile folder. Comment why.

No `mach-lookup` beyond `system.sb`'s, no `file-read-metadata` beyond the reads above, no
`sysctl-read` beyond `system.sb`'s. If something you can measure needs more, stop and say so
rather than widening it.

## 2. The tests: `builder/agent/tests/sandbox_profile.rs`

`#![cfg(target_os = "macos")]`. The profile is read from
`concat!(env!("CARGO_MANIFEST_DIR"), "/sandbox/claude-code.sb")` and run with
`/usr/bin/sandbox-exec -f <profile> -D …` (that absolute path, never `PATH`). Use the existing
`Scratch` helper in `tests/common/mod.rs` (it canonicalizes, which the profile needs); add a
`mod common;` the way the other tests do. Programs run inside: `/bin/cat`, `/usr/bin/touch`,
`/usr/bin/curl`, `/usr/bin/nc`. One helper runs a program under the profile with every parameter
given (defaults: a scratch `PROGRAM_DIR`, `PROFILE_DIR`, `RUN_DIR`, a `PROXY_PORT` the test
chooses, `SIGN_IN=no`) and returns the exit status and output. Each `PROGRAM_DIR`/`PROFILE_DIR`/
`RUN_DIR` is its own scratch folder, and a further scratch folder stands for "the rest of the home
folder".

### 2.1 The nested-sandbox skip

Before each test, run `/usr/bin/sandbox-exec -p "(version 1)(allow default)" /usr/bin/true`. If
that fails, the test is already inside a sandbox: print `sandbox_profile: skipped, already inside a
sandbox (macOS refuses a sandbox inside one)` to stderr and return, **unless**
`PERSONA_BUILDER_REQUIRE_SANDBOX=1` is set, in which case panic with the same words. Add
`PERSONA_BUILDER_REQUIRE_SANDBOX=1` to the `cargo test` line in `scripts/ci-local.sh` beside
`PERSONA_BUILDER_REQUIRE_PARITY=1`, on macOS only (`[ "$(uname -s)" = Darwin ]`), and update its
`echo` line to match.

### 2.2 What the tests prove (one test per bullet; red first against an empty profile, then green)

- A file in `RUN_DIR`, `PROFILE_DIR` and `PROGRAM_DIR` is readable (`cat` prints `canary`).
- A file in the "rest of home" folder is refused, and so is a symlink in `RUN_DIR` that points to
  it.
- `touch` creates a file in `RUN_DIR` and in `PROFILE_DIR`; `touch` in `PROGRAM_DIR` and in the
  "rest of home" folder is refused.
- Every deny-list entry, as a canary inside `RUN_DIR`, is refused for reading **and** for writing
  (`touch` on a new name in a denied folder: `.ssh/new`, `.aws/new`, …), in lower case and in
  upper case on disk (`.ENV.LOCAL`, `REAL.PEM`, `LIBRARY/KEYCHAINS/x`; macOS folders are
  case-insensitive, so create the upper-case spellings in a separate scratch folder). The list of
  canaries is a `const` in the test; a second test asserts that
  `persona_builder::deny::DenyList::builtin().denied_relative(path)` is true for every canary, so
  the two lists cannot drift apart silently (add `persona-builder` as a dev-dependency only if the
  agent crate's normal dependency on it does not already make it usable from tests; it does).
- The near misses stay readable: `monkey.txt`, `keys.md`, `a.keyring`, `docker/config.json`,
  `.docker/other.json`, `envelope.txt`.
- `.credentials.json` directly in `PROFILE_DIR` is readable and writable; `.credentials.json` in
  `RUN_DIR`, and in a subfolder of `PROFILE_DIR`, is refused.
- Network: the test runs a `std::net::TcpListener` on `127.0.0.1:0` that answers one request with
  `HTTP/1.0 200 OK\r\n\r\nok` and passes its port as `PROXY_PORT`; `curl -sS -m5
  http://127.0.0.1:<port>/` inside prints `ok`. A second listener on another port is refused
  (curl fails, and the listener saw no connection, checked with a non-blocking accept after the
  child exits). `curl -m5 http://[::1]:<other port>/` is refused too.
- Sign-in: with `SIGN_IN=no`, `nc -l 127.0.0.1 <free port>` exits at once with an error; with
  `SIGN_IN=yes` it is still running after 500 ms (then kill it and wait for it). Pick the free port
  by binding a `TcpListener` to `127.0.0.1:0`, reading its port, and dropping it.
- The profile refuses to load without `SIGN_IN` (or any other parameter): `sandbox-exec` exits
  non-zero before running the program. If SBPL turns out to treat a missing parameter as empty
  rather than an error, assert what it does and say so in your summary.

## 3. The probe: `scripts/claude_sandbox_probe.py`

A person runs it on a Mac to see their own Claude Code inside the profile. Python 3.9, standard
library only, no network except through its own proxy. Module docstring in plain words: what it
does, that it spends a little of the person's Claude usage for `ask` only, and that it never prints
the sign-in link's contents beyond what Claude Code itself prints in `sign-in`, a reply, or any
credential.

```
python3 scripts/claude_sandbox_probe.py --profile <folder> status
python3 scripts/claude_sandbox_probe.py --profile <folder> sign-in
python3 scripts/claude_sandbox_probe.py --profile <folder> ask
python3 scripts/claude_sandbox_probe.py --profile <folder> sign-out
```

- **`--profile` is required** (so the person's real Persona Builder profile is never touched by
  accident). It must be an absolute path; if it does not exist it is made with mode 0700; if it is
  a link, or not a folder, the probe stops with `That profile folder is a link or not a folder.`
  A folder with group or other bits is tightened to 0700. It is canonicalized before use.
- **Finding Claude Code:** the first runnable `claude` in the absolute folders of `PATH`, then
  `~/.local/bin/claude`, `/opt/homebrew/bin/claude`, `/usr/local/bin/claude` (as `claude.rs`
  `fallbacks`); links resolved with `os.path.realpath`; `PROGRAM_DIR` is the resolved binary's
  folder. None found: `Claude Code isn't installed: no claude on PATH or in the usual places.`
- **The run folder:** `tempfile.mkdtemp(prefix="voice-sandbox-")`, canonicalized, mode 0700,
  removed at the end whatever happens. It is the child's working folder, `CLAUDE_CODE_TMPDIR` and
  `TMPDIR` (with a trailing `/`).
- **The egress proxy:** a class in the script, started on `127.0.0.1:0` in a daemon thread before
  the child and stopped after. HTTP `CONNECT` only, to port 443 only, to a host in the allow list
  only: `api.anthropic.com`, `platform.claude.com`, `claude.ai` (from
  `https://code.claude.com/docs/en/network-config`). Compare lower-cased with one trailing dot
  removed; refuse IP literals (v4 and v6, bracketed or not), anything else in the request line, a
  header block over 8 KiB, or one not complete within 10 seconds. Refusals answer `HTTP/1.1 403
  Forbidden` (`405` for a method other than CONNECT) and close. An allowed host is dialed with
  `socket.create_connection((host, 443), timeout=10)` through an injectable `connect` function
  (the tests replace it), then `HTTP/1.1 200 Connection established` and bytes relayed both ways
  until either side closes. It counts allowed and refused connections per host name and logs
  nothing else (no bytes, no headers).
- **The child's environment is built from nothing:** `HOME`, `USER`, `LOGNAME` (from `pwd` for the
  current user), `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `LANG` (copied if set, else `en_US.UTF-8`),
  `CLAUDE_CONFIG_DIR=<profile>`, `CLAUDE_CODE_TMPDIR=<run>`, `TMPDIR=<run>/`,
  `HTTPS_PROXY` and `https_proxy` = `http://127.0.0.1:<port>`,
  `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, and for `sign-in` only `BROWSER=/usr/bin/false`.
  Nothing else from the probe's own environment reaches it (so no `ANTHROPIC_API_KEY`,
  `CLAUDE_CODE_OAUTH_TOKEN`, base-URL override, `CLAUDECODE` marker or outer proxy).
- **The command:** `/usr/bin/sandbox-exec -f <repo>/builder/agent/sandbox/claude-code.sb -D
  PROGRAM_DIR=… -D PROFILE_DIR=… -D RUN_DIR=… -D PROXY_PORT=… -D SIGN_IN=<yes|no> <claude> …`,
  with `cwd=<run>`. The repo is found from the script's own location.
- **`status`:** runs `--version` (30 s), then `auth status --json` (30 s), and reads only
  `loggedIn` (the object can hold an email address, never printed). Prints
  `Claude Code <version> runs inside the sandbox.` and `Signed in inside the sandbox: yes|no.`
  A failure prints `Claude Code doesn't run inside the sandbox (exit <n>).` and the last line of
  its error output with the home folder shown as `~`, cut to 200 characters.
- **`sign-in`:** runs `auth login --claudeai` with `SIGN_IN=yes`, stdin inherited from the
  person's terminal (so a pasted code reaches it), stdout and stderr through a pipe that the probe
  copies line by line to its own stdout (the profile cannot write to the terminal device). 10
  minutes at most, then the child's whole process group is killed (start it with
  `start_new_session=True`). First prints `Sign in on Claude's page. If no browser opens, copy the
  link Claude Code prints below.` Afterwards it runs `status`, and says whether
  `<profile>/.credentials.json` exists (`The login is kept in the profile folder's own file.` or
  `The login is not in the profile folder's own file.`), never its contents.
- **`ask`:** one tool-less call with the same flags as `claude.rs` (`-p`, `--model haiku`,
  `--safe-mode`, `--no-session-persistence`, `--tools ""`, `--output-format json`,
  `--setting-sources project,local`, `--strict-mcp-config`, `--mcp-config '{"mcpServers":{}}'`,
  `--permission-mode dontAsk`, `--max-budget-usd 0.05`), prompt on stdin: `Reply with the single
  word: ok`. 2 minutes at most. Prints `Answered: yes|no.`, `It said ok: yes|no.` (the result,
  trimmed and lower-cased, equals `ok`), and `Cost: $<total_cost_usd, 4 places>.`; on an error
  reply, `Claude Code said: <result, cut to 200 characters>` only when `is_error` is true (those
  are Claude Code's own failure sentences, such as `Not logged in · Please run /login`). Never the
  reply otherwise.
- **`sign-out`:** runs `auth logout` inside the sandbox (30 s) and prints `Signed out inside the
  sandbox.` or the failure line as in `status`.
- **Every command** ends with the proxy's counts, one line per host, sorted:
  `Network: <host> allowed <n>, refused <m>.`, or `Network: nothing.` Exit 0 when the command did
  what it says, 1 otherwise.

## 4. The probe's tests: `tests/claude_sandbox_probe_test.py`

`unittest`, offline, Python 3.9, importing the script the way `tests/crew_hardening_test.py`
imports `scripts/`. Add `claude_sandbox_probe` to the `for t in …` list in `scripts/ci-local.sh`.
Prove, each red first:

- The proxy: a listed host (lower and upper case, with a trailing dot) gets `200` and its bytes
  relayed both ways through a fake `connect` returning one end of a `socket.socketpair()`; an
  unlisted host, an IP literal (`1.1.1.1`, `[::1]`, `::1`), port 80, a missing port, and a
  malformed request line get `403`; `GET` gets `405`; a header block over 8 KiB is refused without
  calling `connect`; the counts match what happened.
- The environment: with poison variables set in `os.environ` (`ANTHROPIC_API_KEY`,
  `CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_BASE_URL`, `CLAUDE_CODE_USE_BEDROCK`, `CLAUDECODE`,
  `HTTPS_PROXY=http://elsewhere`), the built environment holds exactly the keys in § 3 and none of
  their values; `BROWSER` only for `sign-in`.
- The command: exact argv for `status`, `sign-in`, `ask` and `sign-out` (the `/usr/bin/sandbox-exec`
  path, every `-D`, `SIGN_IN=yes` only for `sign-in`, the `ask` flags in § 3).
- Finding Claude Code: `PATH` order, relative `PATH` entries skipped, the fallbacks, links
  resolved, nothing found.
- The profile folder: made 0700 when missing, tightened when loose, a link refused, a file
  refused, a relative path refused.
- Reading replies: `loggedIn` read without printing any other field (an `email` in the fixture
  never appears in the output); the `ask` summary for a good reply, an `is_error` reply, and
  output that isn't JSON; the home folder shown as `~` in a failure line.
- The run folder is removed after a command that raises.

Use invented values only; a fake `claude` for any test that runs one is a shell script in a temp
folder, and no test runs `sandbox-exec` (those belong to § 2).

## 5. Done means

- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`, and
  `python3 tests/claude_sandbox_probe_test.py` pass for you; `shellcheck scripts/ci-local.sh`
  passes.
- Your summary lists each file, what the Rust tests do when run inside your sandbox, and anything
  in § 1 you could not do as written.

## 6. Fixes after V1's review (second dispatch)

1. **`status` when signed out.** `claude auth status --json` exits 1 when signed out and still
   prints its JSON (measured, inside and outside the sandbox; `claude.rs` reads it the same way).
   In `check_status`, read `loggedIn` from the output whatever the exit code; only when the output
   has no boolean `loggedIn` does the existing failure path run (and it uses the exit code and
   stderr as now). Add a test: a fake `auth status` that prints `{"loggedIn": false}` and exits 1
   gives `Signed in inside the sandbox: no.` and exit 0.
2. **The version line.** `claude --version` prints `2.1.292 (Claude Code)`, so the line reads
   `Claude Code 2.1.292 (Claude Code) runs…`. Print only the first word of its output, and only if
   it is two to four dot-separated runs of digits (as `claude.rs` `valid_version`); otherwise the
   failure path. Update the test.
3. **The profile's comment on letter case.** Seatbelt's regexes ignore letter case on macOS 27,
   on a case-insensitive and on a case-sensitive volume alike (V1 measured: `\.pem$` denied
   `UP.PEM` on a case-sensitive disk image, while `plain.txt` stayed readable). Keep the letter
   classes, and change the comment above the deny list to say they keep the rules right if a
   later macOS starts matching case. No test change: on this macOS a test can't tell the two apart.

Same rules as before: no commit, no new dependency, `-j 2`.

## 7. Fixes after the security review and the Verifier (third dispatch)

Both found the system reads too wide: `(subpath "/dev")` lets the child open the person's own
terminal devices (`/dev/ttys*` are the person's, mode `crw--w----`, so a child could read keystrokes
typed in another window), and `/Library`, `/private/var/db` and `/usr/local` (inside `/usr`) hold
the unified log, crash reports, network settings and Intel Homebrew's data. The Verifier measured
that `claude --version`, `auth status --json` and a signed-out `-p` still run without them.

**The profile (`claude-code.sb`):**

1. Reads: replace `(subpath "/usr")` with the subpaths `/usr/bin`, `/usr/lib`, `/usr/libexec`,
   `/usr/sbin`, `/usr/share`; remove `(subpath "/Library")`, `(subpath "/private/var/db")` and
   `(subpath "/dev")`; add the literals `/dev/null`, `/dev/zero`, `/dev/random`, `/dev/urandom`,
   `/dev/dtracehelper` and `(subpath "/dev/fd")`. Keep `/System`, `/private/etc` and the three
   parameter folders. Update the comment.
2. Above the sign-in rule, a comment: Seatbelt's `localhost` filter also lets the child listen on
   every address while signing in (V1 measured a listener on `*:<port>` reachable through the
   Mac's own network address); the sign-in is short and started by the person, and this is
   recorded as a known limit.

**The tests (`sandbox_profile.rs`):**

3. Rename the `home` scratch folder to `outside` (it is not the home folder).
4. A test `refuses_the_persons_terminals_and_system_logs`: for each path in a list (every
   `/dev/ttys*` entry that exists, `/dev/console`, `/private/var/db/diagnostics`,
   `/Library/Logs/DiagnosticReports`, `/Library/Preferences/SystemConfiguration/preferences.plist`,
   `/usr/local/etc`), when the test process itself can open it read-only (a file or device: open
   and drop; a folder: `read_dir`), assert that inside the sandbox it is refused (`/bin/cat` on a
   file or device with a 2-second limit, killing it on timeout and counting that as a failure;
   `/bin/ls` on a folder). Paths the test process can't open are skipped. Print how many were
   checked to stderr.
5. In `only_sign_in_can_listen`, also assert that with `SIGN_IN=no` a listener on all addresses
   (`nc -l <port>`, no address) exits at once with an error.

**The probe (`claude_sandbox_probe.py`) and its tests:**

6. After every child ends, normally or not, kill its whole process group (so nothing it started
   outlives the run), then wait for it.
7. `--profile`: refuse the home folder and every folder above it (`Use a throwaway folder for the
   profile, not your home folder.`). A folder that exists and is not empty must hold the marker
   file `.voice-sandbox-probe` that the probe writes into a new or empty folder; otherwise refuse
   with `That folder isn't empty and wasn't made by this probe. Use a new folder.`
8. Every line the probe prints from the child (the failure line, `Claude Code said:`, each copied
   sign-in line) has control characters removed (C0 except tab, DEL, and C1), so the child can't
   send terminal escape sequences.
9. The proxy takes at most 16 connections at once (more are closed at once and counted as
   refused); it counts at most 32 distinct names, the rest as `(other)`; a relayed tunnel closes
   after 120 seconds with no bytes either way.
10. At the end, print the counts first, then remove the run folder; if removal fails, print
    `The run folder could not be removed: <path with the home folder as ~>.` and exit 1 instead of
    raising.

Tests for 6 to 10, each red first. Same rules: no commit, no new dependency, `-j 2`.
