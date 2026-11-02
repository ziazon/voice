# VO11 spike: an operating-system sandbox for Voice

Run 2026-10-31 on a Mac with macOS 27.0.1 and Claude Code 2.1.292. The question (VO plan,
gate Q12): App Sandbox or a Seatbelt profile for the broker and the `claude` child; whether
`claude -p` keeps its login inside it; an egress proxy for outbound connections. The answer decides
how the broker is packaged.

## The answer

1. **Seatbelt, one profile per child process, not App Sandbox.** The `claude` child runs under
   `builder/agent/sandbox/claude-code.sb` through `/usr/bin/sandbox-exec`, with the folders and the
   proxy port passed as parameters. App Sandbox stays out until the app is signed for release
   (PB-D3), and even then it does not do what the child needs (below).
2. **Siblings, never nested.** macOS refuses a sandbox inside a sandbox (measured:
   `sandbox-exec: sandbox_apply: Operation not permitted`). A sandboxed broker could not put
   `claude` in a tighter sandbox, because the child inherits its parent's. So an unsandboxed
   launcher (the app) starts each sandboxed piece itself: the `claude` child now (VO11.1), and
   later the broker's reading in a helper process of its own, allowed only the consented sources
   and no network (VO11.2).
3. **Egress goes through a proxy the launcher runs.** Seatbelt can't name hosts, but it can limit a
   child to one port on this computer. The profile allows only the proxy's port; the proxy allows
   only Claude Code's hosts. Claude Code follows `HTTPS_PROXY` and does not support SOCKS
   (`https://code.claude.com/docs/en/network-config`), so the proxy speaks HTTP `CONNECT`.
4. **The login.** Inside the profile, Claude Code can't reach the Keychain, so its login lives in
   `.credentials.json` in its own profile folder. Whether that is acceptable is a question for the
   owner; VO11.1 waits on the answer.

## What was measured

Each line was run on the Mac named above, with throwaway folders and canary files holding the word
`canary`. The landed tests (`builder/agent/tests/sandbox_profile.rs`) repeat the profile's part.

| Check | Result |
| --- | --- |
| `(deny default)` + `system.sb` + `process-exec`/`process-fork` + reads of the system, program and profile folders | `claude --version` and `claude auth status --json` run; so does a signed-out `-p` with reads cut to `/usr/bin`, `/usr/lib`, `/usr/libexec`, `/usr/sbin`, `/usr/share`, `/System`, `/private/etc` and a few device files (no `/Library`, `/private/var/db` or `/dev`) |
| `claude auth status --json` when signed out | prints `"loggedIn": false` and exits 1, inside the profile and outside it alike |
| The same without `process-exec` | `sandbox-exec` can't start `claude` |
| `claude -p` with only those rules | `EPERM … open '/tmp/claude-<uid>'`; setting `CLAUDE_CODE_TMPDIR` to a writable run folder fixes it (`TMPDIR` alone does not) |
| `claude -p` signed out, inside the full profile | runs to Claude Code's own `Not logged in · Please run /login` |
| Network allowed to TCP on `localhost:<proxy port>` only | the proxy port works (on `127.0.0.1` and `[::1]` alike: `localhost` covers both); a listener on another localhost port, `127.0.0.2`, UDP (to the proxy's port number too, once the rule says `tcp`: with `ip` a datagram got through), Unix sockets and `1.1.1.1:443` are refused; DNS fails (`system.sb`'s network helper is not called) |
| `curl` through a `CONNECT` proxy on that port | `api.anthropic.com` reached (HTTP 404, TLS fine); `example.com` refused by the proxy |
| Signed-out `claude -p` with `HTTPS_PROXY` set | two connections to `api.anthropic.com`, both through the proxy; with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, none |
| `claude auth login --claudeai` inside the profile | `Failed to start OAuth callback server`; with listening on localhost allowed (the profile's `SIGN_IN=yes`), it starts and waits for the browser |
| Listening while `SIGN_IN=yes` | the `localhost` filter also lets the child listen on **every** address: a listener on `*:<port>` was reachable through the Mac's own network address. The other rules tried either left it open (a later `deny` on inbound by `remote` address, `network*`) or refused every listener, the sign-in's too (`ip4 "127.0.0.1"`, `remote` on inbound alone) |
| A deny rule after an allow of the folder | a `.env`, `.ssh/…` or `.pem` inside an allowed folder is refused; the same rule placed **before** the allow is not (the last matching rule wins) |
| A symlink in an allowed folder pointing outside | refused (Seatbelt checks the real path) |
| A deny rule written in lower case (`\.pem$`) | denies `UP.PEM` too, on this Mac's volume and on a case-sensitive disk image alike: Seatbelt's regexes ignore letter case here; the profile still spells both cases, in case a later macOS stops |
| A hard link in an allowed folder to a file outside it | **readable** (Seatbelt checks the path, not the file) |
| The child making a hard link or a symlink to a file outside | the hard link is refused; the symlink is made but reading through it is refused |
| The person's own terminals (`/dev/ttys*`, theirs to read) and the system logs, with `/dev` and `/Library` allowed | **readable**, so the profile allows neither (the first draft did; the review caught it); the landed test checks every such path the test can open is refused |
| A missing profile parameter | `sandbox-exec` refuses to start (exit 65) |
| `sandbox-exec` inside a sandbox | refused (`sandbox_apply: Operation not permitted`) |

Not measured, owed to the attended walk (the probe, below): a signed-in `claude -p` inside the
profile, TLS from Claude Code itself through the proxy, the sign-in finishing inside the profile
and writing `.credentials.json`, and pasting the sign-in code when no browser opens. This Mac's
Claude Code had no login to test with, and a Keychain test item could not be written from the run.

## Why not App Sandbox

From Apple's documentation and its developer forums (read 2026-10-31):

- **The network entitlement is one switch.** `com.apple.security.network.client` is "a Boolean
  value indicating whether your app may open outgoing network connections"; the page has no host
  or port setting
  (`https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.network.client`).
  A proxy would still be needed, and the sandbox would still let the app connect anywhere.
- **Children inherit the whole app's sandbox.** "If a sandboxed process runs another program as a
  child process, that child process always inherits its sandbox from the parent"
  (`https://developer.apple.com/forums/thread/706390`). So `claude` would get the app's grants,
  not narrower ones, and a helper must carry exactly `app-sandbox` plus `inherit`
  (`https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app`).
- **Running the person's own Claude Code takes a pick and an extra entitlement.** "Your app can't
  run programs in locations outside its app bundle, sandbox container, or app group containers
  using the entitlements to access user-selected files"; the same page names
  `com.apple.security.files.user-selected.executable` for running one the person picks
  (`https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox`).
  Claude Code lives in Homebrew's folder or `~/.local/bin`, and PB-D9 is that Voice
  drives the person's own install, so the person would pick it in an Open panel, and it would then
  run with the whole app's sandbox.
- **Files outside the container need the person to pick them** in an Open panel, kept with a
  security-scoped bookmark (same page). Reading the Claude Code history would need that pick once.
- **Tauri's bundler signed every external binary with the app's own entitlements** when
  `https://github.com/tauri-apps/tauri/issues/16211` was filed (closed 2026-10-05; whether a fix
  shipped was not checked), so a helper with `inherit` alone may need a signing step of its own.
- It needs a signed app; signing and public release are out of scope (PB-D3).

App Sandbox may still suit the app itself at release, around a launcher that starts the Seatbelt
children; that is for the release plan, not this spike.

## The risks of Seatbelt, stated

- **Apple calls it unsupported.** The man page says `sandbox-exec` is deprecated, and Apple's
  developer support says the profile language "has never been documented for third-party use, and
  thus is not considered API" (`https://developer.apple.com/forums/thread/777509`). It still ships
  in macOS 27, and both Anthropic's sandbox runtime and OpenAI's Codex build on it
  (`https://github.com/anthropic-experimental/sandbox-runtime`,
  `https://github.com/openai/codex/blob/main/codex-rs/sandboxing/src/seatbelt_base_policy.sbpl`).
  The profile tests run in `scripts/ci-local.sh` on every Mac, so a macOS update that changes its
  behaviour fails the checks rather than going unnoticed.
- **A later route:** macOS 27 added `es_new_descendants_client` in Endpoint Security for
  sandboxing child processes (the same forum thread). It needs Apple's Endpoint Security
  entitlement, so it waits for signing. Not read further.
- **Hard links** inside an allowed folder reach the file they point to. The `claude` child is
  allowed only its own program, profile and run folders, which Voice makes; the risk
  matters for VO11.2, where the reader is allowed the person's consented folders.
- **The proxy is the only network control.** A bug in it is a way out; VO11.1 builds it in Rust
  with tests, `CONNECT` to port 443 of a listed name only.
- **The tunnel to Anthropic is opaque.** The proxy sees only the host name. A Claude Code that
  misbehaved could send what it can read to `api.anthropic.com` under someone else's account. The
  child has no tools, so a prompt can't make it do this; the risk is a broken or tampered program,
  and it is accepted.
- **The system's certificate service is closed.** The profile denies both `com.apple.trustd` and
  `com.apple.trustd.agent`, which could otherwise fetch certificates from outside the sandbox.
  On 2026-11-01, Claude Code 2.1.292 still reached `api.anthropic.com` through the proxy and
  returned `401 Invalid bearer token` for an invented token: its own certificate checks worked.
  `nscurl` failed its certificate checks under the same deny, so the deny took effect.
- **Signing in opens a listener to the network.** While `SIGN_IN=yes` (at most 10 minutes, started
  by the person), the child can listen on every address, not only this computer. Recorded with the
  owner's question about the login.
- **Programs the child may start.** Only the program itself may run, plus the link helper and
  `/bin/bash` while signing in. On 2026-11-01, Claude Code 2.1.292 ran `--version`, `auth status`,
  a signed-out `-p`, and `auth status` reading an invented `.credentials.json` with this limit;
  it needed no Keychain helper. Reading only the program file, rather than its folder, was enough.
- **A child can outlive its process group.** A process started with `setsid` can survive the
  group's kill. This is accepted because each launch has its own proxy, which stops with it
  (VO11.1c), leaving that process no way out.
- **The profile folder lasts.** The child may write it, and later runs read it: a `settings.json`
  planted there would be read by a later `claude`. VO11.1 passes `--setting-sources` to every
  call, as `claude.rs` does for `-p` today.

## What the profile allows

`builder/agent/sandbox/claude-code.sb`, with five parameters: `PROGRAM` (the real `claude` binary's
absolute path with links resolved), `PROFILE_DIR` (its `CLAUDE_CONFIG_DIR`), `RUN_DIR` (this call's
working and temporary folder), `PROXY_PORT`, and `SIGN_IN` (`yes` only while signing in).

- **Reads:** the system's own code and settings (`/usr/bin`, `/usr/lib`, `/usr/libexec`,
  `/usr/sbin`, `/usr/share`, `/System`, `/private/etc`), the device files `/dev/null`, `zero`,
  `random`, `urandom`, `dtracehelper` and `/dev/fd`, the program file, and the profile and run
  folders. Not the program's folder, the rest of the home folder, `/Library`, `/usr/local`, the
  system logs, or the person's terminals.
- **Writes:** the profile and run folders, and `/dev/null`.
- **Network:** out over TCP to the proxy's port on this computer only; listening only while signing in
  (on every address, see above).
- **Never, whatever the folders:** the secrets deny list of `builder/core/src/deny.rs`, in either
  letter case, with one exception: Claude Code's own `.credentials.json` directly in its profile
  folder.
- **Programs:** only the program itself, plus the link helper and `/bin/bash` while signing in.
- **No Keychain or system certificate service:** the profile allows no Keychain service and
  denies `com.apple.trustd` and `com.apple.trustd.agent`; Claude Code checks certificates itself.

## The probe, for the attended walk

`scripts/claude_sandbox_probe.py --profile <a throwaway folder> status|sign-in|ask|sign-out` runs
the person's own Claude Code under the profile, behind a small `CONNECT` proxy that allows
`api.anthropic.com`, `platform.claude.com` and `claude.ai` only, with an environment built from
nothing. It prints fixed lines: whether Claude Code runs, whether it is signed in, whether a one-word
`ask` came back `ok` and what it cost, and which hosts it reached, with control characters
removed from anything Claude Code printed. `--profile` is required and must be a new or empty
folder (or one the probe made before), never the home folder, so the person's real Voice
profile is never touched by accident.

## Wired in (VO11.1, 2026-11-02)

The owner chose the login file in Voice's own profile folder (V-Q10, option 1). Since VO11.1c,
`claude.rs` starts every `claude` (the check's two runs, each call, the sign-in) through
`/usr/bin/sandbox-exec` with this profile (`sandbox::Launch`, VO11.1b), each with a Rust `CONNECT`
proxy of its own (`egress::Egress`, VO11.1a), and an environment built from nothing. One change
from the plan above, measured on Mimi with Claude Code 2.1.292: a sandboxed `claude -p` leaves a
`claude-<uid>/` folder in `CLAUDE_CODE_TMPDIR`. The run folder is therefore a fresh folder per launch
(`runs/` beside the profile, removed when the launch ends), not the call's work folder, which
would refuse the next call. While signing in, Claude Code hands its link to a helper in the run
folder; the app checks it and opens it once with `/usr/bin/open`, and never shows it.

Not yet verified: a signed-in sign-in and ask inside the sandbox, on a person's own Mac. That is
the attended walk, through the app or with the probe above.

## Next

**The reader's profile (VO11.2a).** The reader is the shipped `voice` command. Its profile,
`builder/agent/sandbox/reader.sb`, is a template plus one rule per source the person agreed to,
each path passed as a `-D` parameter. It allows no network, other program, Keychain or certificate
service, and keeps the same secrets denies, with no login exception. Metadata-only reads on every
folder above each allowed path let the reader resolve paths, including through `/tmp`, without
listing those folders (other than `/`, which the system profile already lists) or reading their
other files (measured on 2026-11-02, macOS 27.0.1).
The caller gives the reader only stdin, stdout and stderr, as pipes or `/dev/null`, since `/dev/fd`
is readable inside the sandbox and a file it can't open by path can't be written through a
redirected stdout either.
A hard link inside a source still reaches its file; the reader refuses those itself from VO11.2b.
This library is not yet wired in: VO11.2b adds the reader command and VO11.2c connects the app.
