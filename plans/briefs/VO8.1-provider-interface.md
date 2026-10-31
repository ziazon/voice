# VO8.1 brief: the provider interface, Claude Code its first adapter

Persona Builder (soon Voice) asks a model in five places: the build's steps
(`builder/build/src/build.rs`, `Asker`), the interview draft (`builder/build/src/interview.rs`,
`draft`), `refresh` and `apply` (`builder/grow/src/refresh.rs`, `apply.rs`, through `Asker`) and
panel turns (`builder/panel-runner/src/turns.rs`, `ClaudeTurns`). Every one of them calls
`Claude::call` on the concrete `Claude` type in `builder/agent/src/claude.rs`. This slice puts one
`Provider` trait between them, so later slices can add other model providers (a self-hosted one
first, VO9) and route by where the data goes (VO8.2). **No behaviour changes:** same argv, same
ledger line, same texts, same outputs.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. Add no dependency.
Commit nothing; leave every change in the working tree.

## 1. The trait: `builder/agent/src/provider.rs`, `persona_builder_agent::provider`

Add `pub mod provider;` to `builder/agent/src/lib.rs`, and update that file's doc comment: the
providers Persona Builder drives for its model calls, behind one tool-less trait; today one, the
person's own Claude Code.

```rust
/// Where a provider sends what it is given. Ordered from the narrowest to the widest, so VO8.2
/// can compare a provider's reach with a source's ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    /// Never leaves the person's own machines (this Mac, or one on their own network).
    OwnMachines,
    /// Goes to Anthropic.
    Anthropic,
    /// Goes to a third party.
    ThirdParty,
}

/// One brief: what it is for (`step`, kept in the ledger), the instructions, the person's records,
/// the model and the shape the reply must have. It has no tools field: no provider can be handed
/// a tool.
pub struct Brief<'a> {
    pub step: &'a str,
    pub system: &'a str,
    pub prompt: &'a str,
    pub model: &'a str,
    pub schema: &'a Value,
}

pub struct Usage { /* moved unchanged from claude.rs, same derives and fields */ }
pub struct Reply { /* moved unchanged from claude.rs, same derives, fields and doc */ }

/// A model provider. A call is tool-less, spends against the session's daily cap and leaves one
/// ledger line; the reply's `output` is untrusted and the caller checks it.
pub trait Provider: Send + Sync {
    /// Its id, for the CLI, the app and the ledger.
    fn id(&self) -> &'static str;
    /// Its name, for people.
    fn name(&self) -> &'static str;
    /// Where it sends a brief.
    fn reach(&self) -> Reach;
    /// One call.
    fn call(&self, session: &Session, brief: &Brief) -> Result<Reply>;
}
```

`Brief` is today's `claude::Call`, renamed, **with its field names unchanged** (so every test that
builds one only changes the type's name). Move `Call`, `Usage` and `Reply` out of `claude.rs` into
`provider.rs`; do not leave re-exports or aliases behind in `claude.rs`. Update every import
(`ledger.rs` takes `Usage` from `crate::provider`).

## 2. Claude Code implements it: `builder/agent/src/claude.rs`

- Add `pub const AGENT_NAME: &str = "Claude, through your own Claude Code";` and
  `pub const REACH: Reach = Reach::Anthropic;` beside `AGENT_ID`.
- `impl Provider for Claude`: `id` is `AGENT_ID`, `name` is `AGENT_NAME`, `reach` is `REACH`, and
  `call` is today's `Claude::call` body, moved into the impl unchanged (the same checks, in the same
  order, the same argv, `session.start(AGENT_ID, …)`, the same ledger `Line`). Remove the inherent
  `Claude::call`; callers reach it through the trait.
- Finding (`find`, `find_in`, `fallbacks`, `profile_folder`), checking (`status`, `check`),
  signing in (`sign_in`, `sign_in_command`) and `at`, `with_timeout`, `with_stop`, `program`,
  `profile` stay inherent methods of `Claude`, unchanged. They are Claude Code's, not every
  provider's.
- `Claude` must be `Send + Sync` for the trait bound; it already is (a `PathBuf`, a `Duration`, an
  `Option<Arc<AtomicBool>>`). Change no field.

## 3. `AGENTS` gains each provider's reach: `builder/agent/src/agents.rs`

`Agent` gains `pub reach: Reach`. The one entry becomes
`Agent { id: claude::AGENT_ID, name: claude::AGENT_NAME, reach: claude::REACH }`. Update its doc
comment: the providers Persona Builder can drive, each with where its data goes.

Do **not** add `reach` to the app's DTO (`builder/api/src/dto.rs` `AgentInfo`, `app_info` in
`builder/api/src/lib.rs`) or to `persona-builder agents`' output in `builder/cli/src/main.rs`:
both stay byte-identical. VO8.2 decides what the person sees.

## 4. Every model caller takes `&dyn Provider`

| Site | Today | Change |
| --- | --- | --- |
| `build/src/build.rs` `Asker` | field `claude: &'a Claude`, `Asker::new(claude: &'a Claude, …)`, `self.claude.call(…, &Call {…})` | field `provider: &'a dyn Provider`, `Asker::new(provider: &'a dyn Provider, …)`, `self.provider.call(…, &Brief {…})` |
| `build/src/build.rs` `build` | `claude: &Claude` | `provider: &dyn Provider`, passed to `Asker::new` |
| `build/src/interview.rs` `draft` | `claude: &Claude`, `claude.call(…, &Call {…})` | `provider: &dyn Provider`, `provider.call(…, &Brief {…})` |
| `grow/src/refresh.rs`, `grow/src/apply.rs` | `claude: &Claude`, passed to `Asker::new` | `provider: &dyn Provider` |
| `panel-runner/src/turns.rs` `ClaudeTurns::turn` | asks `self.claude.call(…)` inline | Move the body that asks and turns the reply into a `Turn` into a private `fn ask(provider: &dyn Provider, session: &Session, system: &str, prompt: &str) -> Turn`; `turn` finds the session as today, then calls `ask(&self.claude, …)`. `ClaudeTurns` keeps its name, its `claude: Claude` field, `new` and `with_stop`. |

Callers that hold a `Claude` (`cli/src/main.rs`, `cli/src/assistants.rs`, `cli/src/panel.rs`,
`api/src/building.rs`, `api/src/review.rs`, `api/src/assistants.rs`, `api/src/panels.rs`, and the
tests) keep finding and checking Claude Code exactly as today and pass `&claude`, which coerces to
`&dyn Provider`. Change a caller only where the compiler requires it (an import of `Call`, `Usage`
or `Reply`, or a `use … provider::Provider` so `.call` resolves). Keep `claude::DEFAULT_MODEL`
where it is used today.

Do not change any user-facing text, any ledger field, `ingest`, `core`, the Python scripts, or
anything under `builder/ui`.

## 5. Tests first

Write these first and run them red (they fail to compile until `provider` exists), then
implement.

`builder/agent/tests/provider.rs`:
- `Claude::at(<any path>)`, used as `&dyn Provider`, reports `id() == "claude-code"`,
  `name() == "Claude, through your own Claude Code"` and `reach() == Reach::Anthropic`.
- `agents::agent("claude-code")` has the same id, name and reach as `Claude` reports through the
  trait; every entry in `AGENTS` has a distinct id.
- `Reach::OwnMachines < Reach::Anthropic < Reach::ThirdParty`.
- Through a `&dyn Provider` holding the existing stub Claude Code (`tests/common/mod.rs`), one call
  succeeds and leaves exactly one ledger line with `"agent": "claude-code"` (reuse the common
  helpers; this proves the trait path is the real path).

`builder/build/tests/provider.rs`, with an in-process `Fake` provider (no child process): it
records each `Brief` it gets (`step`, `system`, `prompt`, `model`, `schema`, copied into owned
values behind a `Mutex`), answers canned `Reply`s in order, and reports `id "fake"`,
`reach Reach::OwnMachines`. Use a real `Session` as `builder/build/tests/interview.rs` does
(`Session::open_with_clock` under a scratch home).
- `interview::draft` with the fake drives the interview through the trait: one brief, with the
  form's step name, the fixed instructions and the count's schema; the draft it returns is the
  fake's checked output. A first reply that does not fit makes exactly one more brief, whose prompt
  names the problem (as the stub-based tests already assert for Claude Code).
- A build step through the trait: `Asker::new(&fake, …).ask(…)` sends one brief carrying the
  step, the model given to `Asker::new`, the system, schema and prompt, returns the checked output
  with `reused == false`, and a second identical `ask` on a new `Asker` answers from the cache with
  no brief sent (`reused == true`, the fake's count unchanged). Set up the member's private folder
  the way `builder/build/tests/build.rs` does.

Every existing test must pass unchanged, except for imports and the type name `Call` becoming
`Brief`. In particular the argv assertions in `builder/agent/tests/call.rs` and the ledger
assertions stay as they are.

## 6. Done means

- `bash scripts/ci-local.sh` all passed (fmt, clippy with `-D warnings`, every test, the Python
  parity tests).
- `grep -rn 'claude::Call\|&Claude' builder --include='*.rs'` finds no model caller left taking
  `&Claude` (finding, checking and signing in may still name `Claude`).
- Report: the files changed, each test added, and anything this brief left unclear.
