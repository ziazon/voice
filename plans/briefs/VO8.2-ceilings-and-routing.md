# VO8.2 brief: ceilings per source, and routing in the privacy guard

Persona Builder (soon Voice) asks a model in five places, all through the `Provider` trait VO8.1
added (`builder/agent/src/provider.rs`): the build's steps (`builder/build/src/build.rs`, `Asker`),
the interview draft (`builder/build/src/interview.rs`, `draft`), `refresh` and `apply`
(`builder/grow/src/refresh.rs`, `apply.rs`, through `Asker`) and panel turns
(`builder/panel-runner/src/turns.rs`). Today a caller hands a `Brief` straight to a provider.

This slice makes every brief **name the sources of data in it**, gives each source a **ceiling**
(how far the person lets that data go), and puts a **privacy guard** between the callers and the
providers: it picks a provider whose reach every ceiling in the brief allows, **seals** the brief
for that provider, and refuses, in plain words, when none is allowed. The provider **re-checks the
seal** before it sends anything. A caller can no longer reach a provider without the guard.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`, and
`plans/briefs/VO8.1-provider-interface.md` (the seam this builds on). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/project-config/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence a person sees).

This is a public repo: invented names only, no real person, no fixed paths. Add no dependency.
Commit nothing; leave every change in the working tree.

## 1. Sources and ceilings: `builder/core/src/ceilings.rs`, `persona_builder::ceilings`

Add `pub mod ceilings;` to `builder/core/src/lib.rs`. Plain data, no I/O.

```rust
/// Where a brief's data came from. Ordered as listed; `ALL` is that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source { ClaudeHistory, CrewFolder, Typed, SkillMirror, Mail, Calendar, Contacts, FamilySkills, LegalSkills }

/// How far a source's data may go. Ordered from the narrowest to the widest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ceiling { LocalOnly, Anthropic, AnyConsented }

/// Every source's ceiling: the defaults, with the person's own settings from member.json over them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ceilings { /* BTreeMap<Source, Ceiling> holding all nine */ }
```

Exact table (id, `name()` for people, `default_ceiling()`, `read_today()`):

| Source | id | name | default | read today |
|---|---|---|---|---|
| ClaudeHistory | `claude-history` | `your Claude history (your Claude Code sessions and your claude.ai export)` | Anthropic | yes |
| CrewFolder | `crew-folder` | `your assistant's own folder` | Anthropic | yes |
| Typed | `typed` | `what you type into Persona Builder (interview answers, notes on answers, a panel's question)` | Anthropic | yes |
| SkillMirror | `skill-mirror` | `the copy of your skills` | Anthropic | no |
| Mail | `mail` | `your mail` | LocalOnly | no |
| Calendar | `calendar` | `your calendar` | LocalOnly | no |
| Contacts | `contacts` | `your contacts` | LocalOnly | no |
| FamilySkills | `family-skills` | `your family skills` | LocalOnly | no |
| LegalSkills | `legal-skills` | `your legal skills` | LocalOnly | no |

Ceiling ids and words (`id()`, `words()`):

| Ceiling | id | words |
|---|---|---|
| LocalOnly | `local-only` | `on your own machines` |
| Anthropic | `anthropic` | `on your own machines or with Anthropic` |
| AnyConsented | `any-consented` | `with any provider you've agreed to` |

API: `Source::ALL: [Source; 9]`, `Source::id`, `Source::name`, `Source::default_ceiling`,
`Source::read_today`, `Source::from_id(&str) -> Option<Source>`; `Ceiling::id`, `Ceiling::words`,
`Ceiling::from_id(&str) -> Option<Ceiling>`; `Ceilings::defaults()`,
`Ceilings::get(&self, Source) -> Ceiling`, `Ceilings::is_set(&self, Source) -> bool` (the person
set it, as opposed to the default), and `Ceilings::for_member(&Member) -> Result<Ceilings>`, which
reads `member.extra["ceilings"]` (absent: the defaults).

### In member.json

The person's settings live in member.json as a top-level object, keys source ids, values ceiling
ids: `"ceilings": {"mail": "anthropic", "claude-history": "local-only"}`. It stays in
`Member::extra` (do **not** add a typed field to `Member`: the writer's key order and Python byte
parity depend on the struct). In `builder/core/src/member.rs`:

- `validate_config` checks `ceilings` when present: it must be a JSON object, every key a source
  id from the table, every value a ceiling id. The errors, exactly:
  - `member config: {source}: ceilings must be a JSON object`
  - `member config: {source}: ceilings names {key:?}, which isn't a source. The sources are claude-history, crew-folder, typed, skill-mirror, mail, calendar, contacts, family-skills, legal-skills.`
  - `member config: {source}: ceilings.{key} must be local-only, anthropic or any-consented`
- `read_member` refuses a `ceilings` key in private.json, before merging:
  `member config: private.json: ceilings go in member.json, not private.json`. (Ceilings hold no
  private data, and one file keeps what `ceilings set` writes the only setting.)
- The template (`crew/_template/member.json`) is **not** changed: absent means the defaults.
- The Python kit must still load a member.json that has `ceilings`. Add a case to an existing
  Python test (`tests/crew_hardening_test.py` or `tests/crew_decisions_test.py`, whichever already
  loads member configs from a scratch folder) proving it; if it fails, make the Python loader
  ignore `ceilings` (it does not route model calls) and say so in the report.

## 2. The guard: `builder/agent/src/guard.rs`, `persona_builder_agent::guard`

Add `pub mod guard;` to `builder/agent/src/lib.rs`.

### The brief names its sources

`Brief` (`provider.rs`) gains `pub sources: &'a [Source]` (from `persona_builder::ceilings`). Every
caller sets it (section 4). The guard refuses a brief whose `sources` is empty, before anything
else: `Error::Invalid("a step must name the sources of what it sends".into())`.

### Which reach each ceiling allows

```rust
/// Whether data under this ceiling may go where a provider of this reach sends it.
pub fn allows(ceiling: Ceiling, reach: Reach) -> bool
```

- `LocalOnly` allows `OwnMachines` only.
- `Anthropic` allows `OwnMachines` and `Anthropic`.
- `AnyConsented` allows `OwnMachines` and `Anthropic`, and `ThirdParty` **never yet**: no
  third-party provider and no consent record exist. Say so in a doc comment (a later slice adds
  the person's per-provider consent here). Do not add a consent parameter.

And `pub fn needed(reach: Reach) -> Ceiling`, the narrowest ceiling that allows a reach
(`OwnMachines` → `LocalOnly`, `Anthropic` → `Anthropic`, `ThirdParty` → `AnyConsented`), for the
refusal's sentence.

### The seal

```rust
/// A brief the guard checked for one provider. Only this module makes one.
pub struct Sealed<'a> {
    brief: &'a Brief<'a>,
    provider: &'static str,          // the id of the provider it was sealed for
    ceilings: Vec<(Source, Ceiling)>, // each source in the brief with the ceiling that applied
}
impl<'a> Sealed<'a> {
    pub fn brief(&self) -> &Brief<'a>;
    /// The provider's own re-check before it sends anything: the seal must be for this provider
    /// and every ceiling in it must allow this reach.
    pub fn check(&self, provider: &str, reach: Reach) -> Result<()>;
}
```

Fields stay private and there is no public constructor: nothing outside `guard` can make one.
`check` fails with `Error::Invalid("this brief wasn't sealed for this provider, so nothing was sent".into())`
when the id differs, and with the refusal sentence below when a ceiling does not allow the reach.

### The guard

```rust
/// The privacy guard: every model call goes through it.
pub struct Guard<'a> {
    providers: Vec<&'a dyn Provider>, // in order of preference
    ceilings: Ceilings,
}
impl<'a> Guard<'a> {
    pub fn new(providers: Vec<&'a dyn Provider>, ceilings: Ceilings) -> Self;
    /// The first provider, in order, whose reach every source's ceiling allows.
    pub fn route(&self, sources: &[Source]) -> Result<&'a dyn Provider>;
    /// Route, seal, and hand the sealed brief to that provider.
    pub fn call(&self, session: &Session, brief: &Brief) -> Result<Reply>;
}
```

- No providers: `Error::Invalid("no model provider is set up".into())`.
- `route` with none allowed: the refusal, naming the **first source in the brief's own order**
  whose ceiling does not allow the **first** provider's reach, and that provider:
  `Error::Invalid(text::kept_home(source, ceiling, provider.name(), provider.reach()))`.
- `call` checks the sources are not empty, routes, builds the `Sealed` with the ceiling each
  source has in `self.ceilings`, and calls `provider.call(session, &sealed)`. A refusal happens
  before any provider is called, so nothing is spent and no ledger line is written.

### The refusal, in `builder/agent/src/text.rs`

```rust
pub fn kept_home(source: Source, ceiling: Ceiling, provider: &str, reach: Reach) -> String
```

Exactly (one line):
`Nothing was sent. This step uses {source.name()}, which you keep {ceiling.words()}, and {provider} sends what it's given to {reach words}. To let it go there, run: persona-builder ceilings set <assistant folder> {source.id()} {needed(reach).id()}`

Reach words: `OwnMachines` → `your own machines`, `Anthropic` → `Anthropic`, `ThirdParty` →
`another company`.

When the ceiling is wide enough for that reach but the provider still isn't allowed (today only
`AnyConsented` against a `ThirdParty` provider: no consent exists yet), the sentence is instead:
`Nothing was sent. This step uses {source.name()}, and {provider} sends what it's given to another company, which you haven't agreed to.`

## 3. Providers take only a sealed brief

- `Provider::call` becomes `fn call(&self, session: &Session, sealed: &Sealed) -> Result<Reply>;`
  (update its doc comment: a provider re-checks the seal before it sends anything).
- `impl Provider for Claude` (`claude.rs`): the **first** statement of `call` is
  `sealed.check(AGENT_ID, REACH)?;`, before every other check, the stop flag, `session.start` and
  any child. Then `let call = sealed.brief();` and the body stays as it is (same argv, same ledger
  line).
- **Where Claude Code sends** (carried from VO8.1's reviews): the child inherits the person's
  environment, so a provider switch or a base-URL override would send briefs somewhere other than
  Anthropic while `reach()` says Anthropic. In `Claude::command` (used for every child: `--version`,
  `auth status`, sign-in and calls), remove from the child's environment every variable whose name
  starts with `CLAUDE_CODE_USE_` (today `CLAUDE_CODE_USE_BEDROCK`, `_VERTEX`, `_FOUNDRY`,
  `_MANTLE`, `_ANTHROPIC_AWS`), `ANTHROPIC_BASE_URL`, every name that starts with `ANTHROPIC_` and
  ends with `_BASE_URL` (today `ANTHROPIC_BEDROCK_BASE_URL`, `ANTHROPIC_BEDROCK_MANTLE_BASE_URL`,
  `ANTHROPIC_VERTEX_BASE_URL`, `ANTHROPIC_FOUNDRY_BASE_URL`, `ANTHROPIC_AWS_BASE_URL`), and
  `ANTHROPIC_FOUNDRY_RESOURCE`. Match on the names in `std::env::vars_os()` (a name that is not
  UTF-8 is left alone; none of these are). Keep the two session markers' removal. Do not touch
  proxy variables. Source: https://code.claude.com/docs/en/env-vars (read 2026-10-31). Put the
  list in one named function or constant with that doc link in its comment.
- `AGENTS` (`agents.rs`) is unchanged.

## 4. Every caller goes through a `Guard`, naming its sources

| Site | Change | Sources |
|---|---|---|
| `build/src/build.rs` `Asker` | holds `guard: &'a Guard<'a>` and `sources: &'a [Source]` instead of `provider`; `Asker::new(guard, sources, session, home, slug, model)`; `ask` calls `self.guard.call(self.session, &Brief { …, sources: self.sources })` | — |
| `build/src/build.rs` `build` | takes `guard: &Guard` | `pub const SOURCES: &[Source] = &[Source::ClaudeHistory, Source::Typed, Source::CrewFolder];` in `build.rs` (the corpus holds history, the interview's and rounds' answers, and the assistant's book) |
| `build/src/interview.rs` `draft` | takes `guard: &Guard`, calls `guard.call` | `pub const SOURCES: &[Source] = &[Source::CrewFolder];` (the book's kinds and counts only) |
| `grow/src/refresh.rs` `propose`, `grow/src/apply.rs` `measure` | take `guard: &Guard`; `Asker::new(guard, build::SOURCES, …)` | `build::SOURCES` (cases come from notes on the report, whose questions come from history) |
| `panel-runner/src/turns.rs` | `ask(guard: &Guard, session, system, prompt)` calls `guard.call` | `pub const SOURCES: &[Source] = &[Source::CrewFolder, Source::Typed];` (the seat's folder, other seats' posts, the question and steers) |

- The cache in `Asker` is unchanged: a reply kept by an earlier run is reused with no call. The
  cache key does not change.
- **Who builds the guard:** every place that today finds a `Claude` and passes `&claude` as a
  provider (`cli/src/main.rs`, `cli/src/assistants.rs`, `cli/src/panel.rs`, `api/src/building.rs`,
  `api/src/review.rs`, `api/src/assistants.rs`, `api/src/panels.rs`, and the tests) builds
  `Guard::new(vec![&claude], Ceilings::for_member(&member)?)` from the member it is working on, and
  passes `&guard`. Finding, checking and signing in stay on `Claude`.
- **Panels:** each seat is a different assistant with its own ceilings. `ClaudeTurns::new` also
  reads each seat's ceilings from that seat's member.json (use the same way the runner already
  finds a seat's member for its brief; read the code and reuse it), keeps them in a
  `BTreeMap<String, Ceilings>` beside `sessions`, and `turn` builds a `Guard` over `&self.claude`
  with that seat's ceilings. A seat whose member can't be read fails `new` with the existing
  `crate::text::assistant_problem(seat, …)` and a short reason added to `panel-runner/src/text.rs`:
  `its settings in member.json can't be read`. A refusal from the guard comes back as the turn's
  error with the guard's sentence, shown as today's errors are (extend `message`/`turn_error` so the
  `Nothing was sent.` sentence passes through instead of the generic failure).

## 5. Where the person changes a ceiling: `persona-builder ceilings`

In `builder/cli/src/main.rs`, a subcommand group beside the others:

- `persona-builder ceilings show <member_dir>`: reads the member (`read_member`), prints one line
  per source in `Source::ALL` order, tab-separated: `{id}\t{ceiling id}\t{set|default}\t{name}`,
  with ` (Persona Builder doesn't read this yet)` appended to the name when `read_today()` is false.
  Then one line per entry in `AGENTS`: `{agent name} sends what it's given to {reach words}.`
- `persona-builder ceilings set <member_dir> <source id> <ceiling id>`: an unknown source or
  ceiling id is refused with the same sentences as `validate_config` (with `{source}` the
  member.json path). It reads member.json **alone** (not merged with private.json, so nothing
  private is copied into it): refuse a link (`refuse_symlink`), parse, `Member::from_value`, set
  `extra["ceilings"][source] = ceiling` (creating the object), write it back with the same writer
  and the same safety the app already uses to rewrite member.json (find it; if it is only
  `write_member`, use it after `refuse_symlink`). Print
  `{source name} now stays {ceiling words}.` Setting a source to its default still writes it (the
  person chose it).
- Help text for both in the CLI's existing help style.
- The app gets no screen in this slice (a later card). Do not add an app command or a parity row.

## 6. Tests first

Write these first and run them red (they fail to compile or fail on their assertions), report
them, and stop. **Do not implement in this dispatch.** Existing tests whose only change is the
new `sources` field, `&Sealed`, `Guard` in place of a provider, or `Asker::new`'s arguments may be
updated for that and nothing else.

`builder/core` (unit tests in `ceilings.rs` and `member.rs`):
- The table above, row by row: id, name, default, read today; `from_id` round-trips every id and
  refuses `Mail`, `mail ` and `""`. Ceilings ordered `LocalOnly < Anthropic < AnyConsented`.
- `for_member` with no `ceilings` gives the defaults, none `is_set`; with
  `{"mail": "anthropic"}` mail is Anthropic and `is_set`, the rest default.
- Each of the three `validate_config` errors, exactly; private.json with `ceilings` is refused
  with its sentence; a member.json with `ceilings` survives `read_member` → `write_member` with
  the object unchanged.

`builder/agent/tests/guard.rs`, with an in-process fake provider (configurable id and reach;
records every sealed brief it gets; a counter):
- `allows`, all nine pairs, and `needed`, all three.
- Routing: providers `[third-party fake, anthropic fake, own-machines fake]` and a brief of
  `[ClaudeHistory]` (default Anthropic) goes to the Anthropic fake; with `[Mail]` (LocalOnly) to the
  own-machines fake; with `[ClaudeHistory, Mail]` to the own-machines fake.
- Refusal: only an Anthropic fake, a brief of `[CrewFolder, Mail]`: `call` fails with exactly
  `Nothing was sent. This step uses your mail, which you keep on your own machines, and Fake sends what it's given to Anthropic. To let it go there, run: persona-builder ceilings set <assistant folder> mail anthropic`,
  the fake was never called, and the session's ledger has no line.
- The person's setting counts: ceilings with `claude-history` set to `local-only` refuse a
  `[ClaudeHistory]` brief to the Anthropic fake.
- An empty `sources` is refused with its sentence and the fake is never called.
- `AnyConsented` never reaches a `ThirdParty` fake.
- The re-check: a provider whose `reach()` answers `Anthropic` to the guard's routing but checks
  the seal as `ThirdParty` (a fake that calls `sealed.check(id, Reach::ThirdParty)`) is refused
  by the seal; a fake that checks with another provider's id is refused with the not-sealed
  sentence. (This proves `check` itself, which the real provider calls.)

`builder/agent/tests/` (the stub Claude Code in `tests/common/`):
- Through a `Guard` over the stub, one call still succeeds with the same argv and one ledger line
  (extend the existing call test minimally).
- A `Guard` over the stub with `claude-history` set to `local-only` and a `[ClaudeHistory]` brief:
  refused, the stub never ran (it leaves no trace file, or however the common helpers detect a
  run), no ledger line.
- The environment: with `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX`,
  `ANTHROPIC_BASE_URL`, `ANTHROPIC_VERTEX_BASE_URL` and `ANTHROPIC_FOUNDRY_RESOURCE` set in the
  test's process environment for the child (follow how `builder/agent/tests/env.rs` already checks
  the child's environment), none reaches the stub, and an unrelated variable (`HTTPS_PROXY`) does.

`builder/build/tests/provider.rs` and `builder/grow`, `builder/panel-runner` tests:
- The interview's brief names `[CrewFolder]`; a build step's brief names `build::SOURCES`;
  `refresh` and `apply` briefs name `build::SOURCES`; a panel turn's brief names
  `[CrewFolder, Typed]` (the fakes record `sources`).
- A panel seat whose member.json sets `typed` to `local-only`: its turn fails with the
  `Nothing was sent.` sentence as the turn's error, and the stub never ran for it.

`builder/cli` (follow the existing CLI tests' style):
- `ceilings show` on a fixture member prints the nine lines and the provider line exactly.
- `ceilings set <dir> mail anthropic` writes `"ceilings": {"mail": "anthropic"}` into member.json,
  leaves private.json's contents out of it, prints `your mail now stays on your own machines or with Anthropic.`,
  and `show` then says `set` for mail; an unknown source and an unknown ceiling are refused with
  their sentences and member.json is unchanged.

## 7. Done means (second dispatch, implementing)

- The frozen tests pass unchanged; every existing test passes.
- `bash scripts/ci-local.sh` all passed (fmt, clippy `-D warnings`, every test, the Python tests).
  Inside your sandbox some `save` tests fail on the listener refusal; say which, and do not change
  them.
- `grep -rn 'provider.call\|\.call(session\|\.call(self.session' builder --include='*.rs'` finds
  model calls only inside `guard.rs` (and tests of `Sealed::check`).
- Report: the files changed, each test added, and anything this brief left unclear.

## 8. Fixes after review (third dispatch)

The security review (and V1's own reading) found that mail mined by the guided build can reach a
build brief unnamed. The guided build's `sources mine` writes sent mail (an `mbox` source in
private.json's `sources` list) to `decisions-src-<name>.jsonl` in the state folder; the guided
build's merge and the app's `discover` both fold every `decisions-*.jsonl` into `decisions.jsonl`,
which the app's `build` sends to Claude Code. Mail records carry no marker in the merged corpus
(their `project` is the source's own name). So:

- **The build names Mail when mail can be in its corpus.** In `builder/build/src/build.rs`, add
  `pub fn sources(home: &Path, member: &Member) -> Result<Vec<Source>>`: `SOURCES`, plus
  `Source::Mail` (appended last) when either the member's state folder holds a file whose name
  starts `decisions-src-` and ends `.jsonl` (list it with `persona_builder_ingest::private::list_private`,
  as `discover`'s callers do), or the member's merged config lists a source of kind `mbox` in
  `member.extra["sources"]` (consented or not). `build` uses it for its `Asker`. `refresh`,
  `apply` and the interview are unchanged: they send no corpus record (refresh and apply send
  the assistant's decisions, notes on answers and their cases; the interview sends kinds and
  counts).
- `Source::Mail.read_today()` becomes `true` (the build can send mail the guided build mined).
  Its name stays `your mail`. Update the core test row for mail and the CLI `ceilings show`
  expected line for mail accordingly (that is the only change allowed to those two frozen tests).
- **`ceilings set` writes safely.** Find the member with `checked_member` as the other member
  commands do. Read member.json with `O_NOFOLLOW` and check it is a regular file on the opened
  descriptor. Write it through a staged sibling (`create_new`, `O_NOFOLLOW`, member.json's own
  mode) that is synced and renamed over member.json: reuse the staged writer in
  `builder/build/src/draft.rs` (`Staged`), exposing one small `pub fn` there if needed, rather than
  writing a second one. Hold the same lock the other member-changing CLI commands take, if one
  exists; say in the report which you used or why none.
- **Tests first (red, then green):** a build whose state folder holds `decisions-src-mail.jsonl`
  (invented records) with mail at its default is refused with the `Nothing was sent.` sentence
  naming `your mail`, with no model call and no ledger line; the same build with
  `"ceilings": {"mail": "anthropic"}` runs and its briefs name `[ClaudeHistory, Typed,
  CrewFolder, Mail]`; a config listing an `mbox` source with no `decisions-src-*` file also adds
  Mail; a state folder with neither leaves `build::SOURCES`. For `ceilings set`: a member.json
  that is a link is refused and its target unchanged; after a set, no staged file is left in the
  folder and member.json keeps its mode.
- **The provider's name closes its aside** (the Verifier): Claude Code's name is
  `Claude, through your own Claude Code`, so `…, and {provider} sends…` leaves a dangling comma.
  The refusal becomes:
  `Nothing was sent. This step uses {source.name()}, which you keep {ceiling.words()}, and the provider for this step, {provider}, sends what it's given to {reach words}. To let it go there, run: persona-builder ceilings set <assistant folder> {source.id()} {needed(reach).id()}`
  and the no-consent one:
  `Nothing was sent. This step uses {source.name()}, and the provider for this step, {provider}, sends what it's given to another company, which you haven't agreed to.`
  `ceilings show`'s provider line becomes `{agent name}: sends what it's given to {reach words}.`
  Update every test that asserts these sentences to the new wording and nothing else.
