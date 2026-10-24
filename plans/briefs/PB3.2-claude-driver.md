# PB3.2 brief: the Claude driver

Written by Claude (lane V1) for Codex, 2026-10-24. Spec: lane V's `PB3.2` entry, split from PB3 of the
Persona Builder plan (PB-D6: Claude only, behind an agent list; PB-D9: the user's own Claude Code,
never a login of ours). Read the repo's `AGENTS.md` first, then `plans/briefs/PB2.2-mine-claude-code.md`
for the crate and test style. Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`): each test goes
red on its assertion before its code exists, then green. Security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`. Never commit; Claude commits. Run cargo with
`CARGO_NET_OFFLINE=true`. **No new dependency:** use `serde_json` and `libc` at the versions the
workspace already pins (`libc = "=0.2.191"`).

**Built by Claude directly:** Codex was at a hard usage limit (until 2026-10-30) when this brief was
dispatched, so Claude implemented it at the same bar. Two places differ from the first draft, as built:
the busy sentence lives with `lock_state` in `builder/ingest/src/private.rs` (the lock is taken there),
and the output's collection gets a one-second grace after the child exits, so a child that finishes
just at its time limit is not counted as cut off.

## What this slice does

A new crate `builder/agent` (package `persona-builder-agent`, library `persona_builder_agent`) that later
slices (PB3.3, the build; PB3.4, the interview; PB7, the app) use for every model call:

1. **The agent list:** the agents Persona Builder can drive. One entry now.
2. **Check:** Claude Code is installed and signed in, in plain words when it is not.
3. **One call:** a tool-less `claude -p` with a JSON schema, a time limit and a per-call budget.
4. **The ledger:** one line per call (tokens, the estimated cost, the outcome), never any content.
5. **The daily cap:** no call starts that could take today's estimated spend past the cap.

Plus two small CLI commands, `persona-builder agents` and `persona-builder claude check`, and
`persona-builder spend <member-dir>`. Nothing here labels, distils or interviews: that is PB3.3/PB3.4.

## Sources (checked 2026-10-24, cite them in doc comments where the code relies on them)

- Headless mode, `https://code.claude.com/docs/en/headless`: `--output-format json` answers one JSON
  object with `result`, `session_id`, usage and `total_cost_usd` ("client-side estimates [that] can differ
  from your actual bill"); with `--json-schema` the answer is in `structured_output`, and "if a run ends
  without valid structured output … the response has no `structured_output` field"; "when a failure
  happens inside the run, such as missing authentication, Claude Code prints the failure as the result on
  stdout"; piped stdin is capped at 10 MB; without `--bare`, a `-p` session runs the hooks in the working
  directory's `.claude/settings.json` and connects the servers in its `.mcp.json`; `--bare` never reads
  OAuth or the Keychain, so it cannot use the person's own login and is **not** used.
- `claude --help` (2.1.292): `--safe-mode` starts "with all customizations (CLAUDE.md, skills, installed
  plugins, hooks, MCP servers, custom commands and agents, …) disabled"; `--tools ""` disables all tools;
  `--max-budget-usd` (print mode only); `claude auth status --json` prints an object with a boolean
  `loggedIn`.
- Setup, `https://code.claude.com/docs/en/setup`: the native installer's launcher is
  `~/.local/bin/claude`; Homebrew's cask is `claude-code`; "The free claude.ai plan does not include Claude
  Code access".
- The precedent for the argv and the child handling: Flightdeck's tool-less call,
  `desktop/core/src/draft_run.rs` (`tool_less_call`) and `desktop/core/src/runtime.rs`
  (`run_captured_checked`, `settle`, `drain`, `kill_group`, `has_exited`), in `ziazon/flightdeck`. Port
  the child-process part (below); do not depend on Flightdeck.

## Decisions (made, do not reopen)

### Crate layout

- `builder/agent/Cargo.toml`: `persona-builder` (core, for `Error`/`Result`),
  `persona-builder-ingest` (for the private state folder), `serde_json` (with `preserve_order`, as
  the others), `libc = "=0.2.191"`. Add it to the workspace `members`, and add
  `persona-builder-agent = { path = "../agent" }` to the CLI.
- Modules: `agents.rs` (the list), `child.rs` (run a child with a time limit), `claude.rs` (find, check,
  call), `ledger.rs` (the ledger, the cap, the session), `text.rs` (every user-facing sentence, as
  `builder/save/src/text.rs` keeps them). Errors are `persona_builder::error::Error::Invalid(<plain
  sentence>)` throughout, as the other crates do.

### 1. The agent list (`agents.rs`)

```rust
pub struct Agent { pub id: &'static str, pub name: &'static str }
pub const AGENTS: &[Agent] = &[Agent { id: "claude-code", name: "Claude, through your own Claude Code" }];
pub fn agent(id: &str) -> Option<&'static Agent>;
```

`persona-builder agents` prints one line per agent: `claude-code  Claude, through your own Claude Code`
(id, two spaces, name). Exit 0.

### 2. Finding Claude Code (`claude.rs`)

`pub fn find(path: Option<&OsStr>, home: &Path) -> Result<Claude>`: the first match, in this order:
each **absolute** directory in `PATH` (relative entries and empty entries are skipped, so a `.` in PATH
never runs a `claude` from the current folder), then `~/.local/bin/claude`, `/opt/homebrew/bin/claude`,
`/usr/local/bin/claude` (an app opened from the Finder gets a short PATH). A match is a path whose
metadata, **following links** (the native launcher and Homebrew's are links), is a regular file with any
execute bit. None found: `text::NOT_INSTALLED`.

`pub struct Claude { program: PathBuf, timeout: Duration }`, with `Claude::at(program)` (tests),
`with_timeout(Duration)`, and `program()`. Default timeout: 15 minutes (a 300-record labelling call can
take minutes; the call is killed after this).

### 3. The child process (`child.rs`)

Port Flightdeck's `run_captured_checked` with its `settle`, `drain`, `kill_group` and `has_exited`
(`waitid` with `WNOWAIT`), keeping their comments' reasoning: the child runs in its own process group
(`process_group(0)`); stdin is written on its own thread and then closed; stdout is read on a thread up to
a cap and the rest discarded so the child never blocks; **stderr is `Stdio::null()`** (never read, never
shown: it can echo prompt text); every way out kills the whole group with `SIGKILL` before reaping, so a
grandchild cannot outlive the call. Signature:

```rust
pub(crate) enum Ran { Finished { status: ExitStatus, stdout: Vec<u8> }, CutOff, NotStarted }
pub(crate) fn run(command: &mut Command, input: Option<&[u8]>, timeout: Duration, cap: u64) -> Ran;
```

`CutOff` covers both the time limit and output that could not be collected. Stdout cap: 16 MiB.

**The environment:** the child inherits the person's own environment (that is how their own login,
proxy and config reach it), **minus `CLAUDECODE` and `CLAUDE_CODE_ENTRYPOINT`**, which mark a process as
running inside a Claude Code session; Persona Builder may itself be started from one (the guided build),
and its calls must run as a fresh, top-level CLI. No variable is added. No token is ever read, set or
passed by Persona Builder.

### 4. Check (`Claude::check`)

`pub fn check(&self, home: &Path) -> Result<Ready>`, `pub struct Ready { pub version: String }`:

1. `claude --version`, current directory `home`, no stdin, 30-second limit. Not exit 0, or cut off:
   `text::DOES_NOT_RUN`. `version` is the first whitespace-separated word of stdout, which must match
   `^[0-9]+(\.[0-9]+){1,3}$`; otherwise `text::DOES_NOT_RUN`.
2. `claude auth status --json`, same directory and limit. Parse stdout as a JSON object and read only
   `loggedIn`. `true`: ready. `false`, missing, or the command fails: `text::SIGNED_OUT`. **Never print,
   keep or log any other field** (the object can carry an email address).

`persona-builder claude check` runs `find` then `check` and prints `text::READY` with the version
(`Claude Code <version> is installed and signed in.`), exit 0; or the plain sentence on stderr, exit 1.

### 5. The session: the lock, the work folder and the cap (`ledger.rs`)

A build holds one `Session` for its whole run; every call goes through it.

```rust
pub struct Cap { daily_usd: f64, call_usd: f64 }
impl Cap {
    pub const DEFAULT_DAILY_USD: f64 = 50.0;
    pub const DEFAULT_CALL_USD: f64 = 5.0;
    pub fn new(daily_usd: f64, call_usd: f64) -> Result<Cap>; // both finite, 0 < call_usd <= daily_usd <= 1000
}
pub struct Session { /* home, slug, lock, work folder, cap, clock, Mutex<State { reserved: f64 }> */ }
impl Session {
    pub fn open(home: &Path, slug: &str, cap: Cap) -> Result<Session>;
    pub fn open_with_clock(home: &Path, slug: &str, cap: Cap, clock: Box<dyn Fn() -> SystemTime + Send + Sync>) -> Result<Session>;
    pub fn spent_today(&self) -> Result<f64>;
}
```

- **One build per assistant at a time.** `open` takes an exclusive, non-blocking `flock` on a descriptor
  of the member's state folder (`~/.claude/state/<slug>`, created 0700 if missing), held until the
  session drops. Taken already (`EWOULDBLOCK`): `text::BUSY`. Add to `builder/ingest/src/private.rs`, reusing
  its `state_directory` walk (no links followed, 0700):
  - `pub struct StateLock(OwnedFd)` and `pub fn lock_state(home, slug) -> Result<StateLock>`;
  - `pub fn private_folder(home, slug, name) -> Result<PathBuf>`: a plain-named subfolder of the state
    folder, created 0700 through `openat`/`mkdirat` with `O_NOFOLLOW` (a link there is refused), its
    mode checked to be 0700 and its owner the current user;
  - `pub fn read_private_optional(home, slug, name) -> Result<Option<Vec<u8>>>`: `None` when the state
    folder or the file does not exist, otherwise exactly `read_private`.
- **The work folder** is `private_folder(home, slug, "agent")`. Every call runs there, and every call
  first checks that it is **empty** (`read_dir` yields nothing); otherwise `text::WORK_FOLDER_NOT_EMPTY`
  and nothing runs. This is what keeps a planted `.claude/settings.json`, `.mcp.json` or `CLAUDE.md` out
  of the call.
- **The day** is the person's local calendar day: `local_day(SystemTime) -> String` (`YYYY-MM-DD`) via
  `libc::localtime_r` on the Unix seconds. The clock is `SystemTime::now` unless a test gives one.
- **Reserve, then call, then record**, all under the session's mutex except the call itself:
  1. `spent_today` = the sum of `cost_usd` over ledger lines whose `day` is today; `reserved` = the
     budgets of calls in flight in this session.
  2. If `spent_today + reserved + call budget > daily_usd`: `text::cap_reached(spent_today, daily_usd)`;
     nothing runs and nothing is written.
  3. Otherwise `reserved += budget`, run the call, then append its line and `reserved -= budget`.
  The call's budget is `Cap::call_usd`, passed to `--max-budget-usd`.

### 6. The ledger

`runs.jsonl` in the state folder, written only with `write_private` (read the whole file with
`read_private_optional`, append one line, write it back: the same pattern as the Python `cmd_record`).
One compact JSON object per line, keys in this order:

```json
{"at":"2026-10-24T22:10:03Z","day":"2026-10-24","agent":"claude-code","step":"label","model":"sonnet",
 "outcome":"ok","seconds":12.4,"input_tokens":1200,"output_tokens":300,"cache_creation_input_tokens":0,
 "cache_read_input_tokens":0,"cost_usd":0.0123,"cost_from":"claude"}
```

- `at` is UTC to the second; `seconds` the call's wall time to one decimal.
- Tokens are read from the reply's `usage` object (each a non-negative integer; a missing or odd one is
  0). `cost_usd` is the reply's `total_cost_usd` when it is a finite number ≥ 0 (`cost_from: "claude"`);
  **when the call was cut off, or its reply is unreadable or has no usable `total_cost_usd`, the line
  records the whole call budget** (`cost_from: "budget"`), because money may have been spent and the cap
  must not under-count.
- `outcome` is one of `ok`, `error` (a readable reply with `is_error: true`, or a non-zero exit), `signed-out`,
  `no-output` (a successful reply without a usable `structured_output`), `cut-off`, `unreadable`
  (stdout not one JSON object). A child that never started spends nothing and writes **no** line.
- **Never** the prompt, the system prompt, the schema, the reply, `result`, `session_id`, or any text from
  them.
- A ledger line that does not parse, or whose `day` or `cost_usd` is missing or of the wrong type:
  `text::LEDGER_DAMAGED` (refuse to spend; the cap cannot be known). Lines are never rewritten or dropped.

`persona-builder spend <member-dir> [--daily-cap-usd <n>]` reads the member's slug (the CLI's
`checked_member`), and prints `text::spend(spent_today, daily_usd)`, exit 0; it takes the lock like a
build (so it says `BUSY` during one). Default cap 50.

### 7. One call (`Claude::call`)

```rust
pub struct Call<'a> { pub step: &'a str, pub system: &'a str, pub prompt: &'a str, pub model: &'a str, pub schema: &'a serde_json::Value }
pub struct Reply { pub output: serde_json::Value, pub usage: Usage, pub cost_usd: f64 }
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64, pub cache_creation_input_tokens: u64, pub cache_read_input_tokens: u64 }
impl Claude { pub fn call(&self, session: &Session, call: &Call) -> Result<Reply>; }
pub const DEFAULT_MODEL: &str = "sonnet";
```

Checks before anything runs (each a plain error, nothing spent, no ledger line): `step` matches
`^[a-z][a-z0-9-]{0,39}$`; `model` matches Flightdeck's `valid_model` (lower-case letters, digits, dots and
dashes, starting with a letter or digit, at most 64); `prompt` is not blank and at most 8 MiB
(`MAX_PROMPT`, under the CLI's 10 MB stdin cap); `system` is not blank and at most 32 KiB; `schema` is a
JSON object. Then the work-folder check, then the reservation.

The argv, exactly and in this order (the `schema` serialised compactly):

```
claude -p --system-prompt <system> --model <model> --safe-mode --no-session-persistence --tools ""
  --output-format json --json-schema <schema> --setting-sources project,local --strict-mcp-config
  --mcp-config {"mcpServers":{}} --permission-mode dontAsk --max-budget-usd <budget with 2 decimals>
```

`--no-session-persistence` also matters for the person's history: without it, every build call would
become a Claude Code transcript that the next `mine` reads back. The prompt (the person's records) goes
**only on stdin**, never in the argv. Current directory: the work folder. Environment: § 3.

The reply:
- `Ran::NotStarted`: `text::DOES_NOT_RUN`, no line.
- `Ran::CutOff`: line `cut-off`, `text::cut_off(minutes)`.
- stdout not one JSON object: line `unreadable`, `text::FAILED`.
- `is_error: true` and `result` (a string) contains `/login`, `authenticate`, or starts with `Failed to
  authenticate` (Flightdeck's `could_not_sign_in` markers): line `signed-out`, `text::SIGNED_OUT`.
- exit not 0, or `is_error` not `false`: line `error`, `text::FAILED`.
- no `structured_output`, or it is not an object or an array: line `no-output`, `text::FAILED`.
- otherwise: line `ok`, `Ok(Reply)`. The output is untrusted: the caller validates it (PB3.3).

### 8. The sentences (`text.rs`, plain language, no tool words)

```
NOT_INSTALLED   Persona Builder works through your own Claude Code, and it isn't on this Mac yet. Install it (https://code.claude.com/docs/en/setup), sign in with your own Claude account, then try again. Claude Code needs a paid Claude plan; the free plan doesn't include it.
DOES_NOT_RUN    Claude Code is on this Mac but didn't start. Open it once on its own to check it works, then try again.
SIGNED_OUT      Claude Code isn't signed in. Open Claude Code, sign in with your own Claude account, then try again.
READY           Claude Code {version} is installed and signed in.
BUSY            Persona Builder is already building this assistant. Wait for that to finish, then try again.
WORK_FOLDER_NOT_EMPTY  Persona Builder's working folder for this assistant ({path}) has files in it that it didn't put there, so it stopped. Empty that folder, then try again.
LEDGER_DAMAGED  Persona Builder's record of what it has spent for this assistant can't be read, so it won't spend anything more. The record is {path}.
cap_reached     Today's spending limit for this assistant is reached: about ${spent:.2} of ${cap:.2}, as Claude Code estimates it. The build can carry on tomorrow, or you can raise the limit.
spend           Today, about ${spent:.2} of the ${cap:.2} daily limit, as Claude Code estimates it.
cut_off         Claude Code was still working on this step after {minutes} minutes, so Persona Builder stopped it. Try again; if it happens again, try a smaller step.
FAILED          Claude Code couldn't finish this step. Open Claude Code and check it answers you, then try again.
```

## Tests (all red first)

Stub `claude` programs are `#!/bin/sh` scripts the tests write into a fresh temp folder (0755), never
committed binaries. Each stub records its argv (one per line), its stdin, its current directory and
whether `CLAUDECODE` reached it into files beside it, then prints a canned reply. Helper in
`builder/agent/tests/common/mod.rs`. Tests set `HOME` to a temp folder (`state_dir` resolves under it).

1. `agents`: the list has one entry `claude-code`; `agent("other")` is `None`; the CLI prints the line.
2. `find`: PATH order; a relative and an empty PATH entry are skipped even when they hold a `claude`; a
   link to an executable file is found; a non-executable file and a directory named `claude` are not;
   the fallback `~/.local/bin/claude` is found when PATH has none; nothing found is `NOT_INSTALLED`.
3. `check`: ready (version parsed); `--version` failing, printing junk, or hanging past a short limit is
   `DOES_NOT_RUN`; `loggedIn: false`, missing, or non-JSON is `SIGNED_OUT`; an `email` field in the stub's
   status is never in any output.
4. `call` argv: the exact argv above (assert the whole list); the prompt arrives on stdin byte for byte
   and appears nowhere in the argv; the current directory is `<state>/agent`; `CLAUDECODE` and
   `CLAUDE_CODE_ENTRYPOINT` (set in the test process) do not reach the child; another variable does.
5. `call` replies, one test each: ok (output, usage, cost returned; the line's fields); `is_error` true;
   exit 1 with a valid object; signed out (`result` containing `/login`); no `structured_output`;
   `structured_output` a string; stdout not JSON; stdout over the cap; a hang past a 1-second limit
   (`cut-off`, recorded at the budget); a stub that starts a background `sleep 30` and writes its pid,
   then hangs: after the cut-off that pid is gone (`kill -0` fails). Each asserts the ledger line, and
   that no line holds any prompt or reply text (grep the ledger for a marker word put in both).
6. Before-run refusals write no line and never start the stub (the stub's argv file is absent): bad
   step, bad model, blank or 8 MiB + 1 prompt, non-object schema, a file in the work folder, a `.mcp.json`
   in the work folder, the work folder replaced by a link.
7. The cap: with a fixed clock, ledger lines for today summing 46.00 and a call budget of 5 refuse with
   `cap_reached` (the message carries 46.00 and 50.00); lines from yesterday do not count; 44.99 runs.
   Two threads calling at once with 41 spent and a cap of 50: exactly one runs (the reservation).
8. The ledger: a damaged line (bad JSON; `cost_usd` a string) refuses with `LEDGER_DAMAGED` and runs
   nothing; the file is 0600 and the state folder 0700; an existing ledger's lines are kept in order.
9. The lock: a second `Session::open` for the same slug while one is open is `BUSY` (in-process: `flock`
   locks belong to the open file description, so a second open conflicts); after drop it opens; the CLI
   `spend` prints the sentence, and `BUSY` while a test holds a session.
10. `private_folder`: a link where the folder should be is refused; a folder with mode 0755 is refused.

Every test that goes green gets a mutation check from Claude afterwards; write the tests so a removed
check fails them.

## Not in this slice

The prompts, labelling, distilling, the backtest, the interview (PB3.3, PB3.4); any UI (PB7); any API-key
option (PB-ANT); other agents. No live call to the real `claude` in the tests.

## Report back

The files changed, the test count, `cargo clippy --all-targets -- -D warnings` and `cargo test -p
persona-builder-agent -p persona-builder-ingest -p persona-builder-cli` results, and anything in this
brief you could not do as written (with why).
