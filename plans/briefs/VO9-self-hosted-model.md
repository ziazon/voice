# VO9 brief: one self-hosted model adapter

Persona Builder (soon Voice) asks a model only through the privacy guard
(`builder/agent/src/guard.rs`), which hands a sealed brief to the first `Provider` whose reach every
source's ceiling allows (VO8.1, VO8.2). Today there is one provider, the person's own Claude Code
(`Reach::Anthropic`), so a step that carries data the person keeps on their own machines (mail, by
default) is refused.

This slice adds the **second provider: a model server on the person's own machines or home
network** (`Reach::OwnMachines`): Ollama, llama.cpp's `llama-server`, LM Studio, vLLM, or anything
else that speaks the OpenAI-style `POST /v1/chat/completions` with a JSON-schema reply. The person
sets it up per assistant with `persona-builder self-hosted set`; every place that builds a guard
puts it after Claude Code, so steps that may go to Anthropic go where they go today and only steps
with data kept home go to the server.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`,
`plans/briefs/VO8.1-provider-interface.md` and `plans/briefs/VO8.2-ceilings-and-routing.md` (the
seam this builds on). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/project-config/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (every
sentence a person sees), `~/.codex/skills/source-driven-development/SKILL.md`.

This is a public repo: invented names only (assistants `pebble`, `wren`), no real person, no fixed
paths. **Add no new crate**: `ureq` 3.4.2 (with `rustls`) is already in the lockfile through
`builder/save`; declare it in `builder/agent/Cargo.toml` exactly as save does
(`ureq = { version = "=3.4.2", default-features = false, features = ["rustls"] }`), and check
`Cargo.lock` gains no new package. Commit nothing; leave every change in the working tree.

## Sources (read 2026-10-31; cite them in the module's doc comment)

- Ollama's OpenAI compatibility: `response_format` is supported on `/v1/chat/completions`
  (https://docs.ollama.com/api/openai-compatibility), and `openai/openai.go` maps
  `response_format: {"type": "json_schema", "json_schema": {"schema": …}}` to its own `format`
  (https://github.com/ollama/ollama/blob/main/openai/openai.go).
- llama.cpp's server reads the same nested shape: `response_type == "json_schema"` takes
  `response_format.json_schema.schema` (`tools/server/server-common.cpp`,
  https://github.com/ggml-org/llama.cpp).
- LM Studio: "the JSON schema should be provided in the `json_schema` field of `response_format`"
  (https://lmstudio.ai/docs/developer/openai-compat/structured-output).
- ureq 3.4.2's `Config::default()` takes its proxy from `ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY`
  (`Proxy::try_from_env`), so the agent must set `.proxy(None)`; `Agent::with_parts(config,
  connector, resolver)` takes a custom `unversioned::resolver::Resolver`.

## 1. Shared brief checks: `builder/agent/src/provider.rs`

Move the provider-neutral checks out of `Claude::call` into one crate-private function, used by
both providers, with the **same error texts** as today:

```rust
/// The checks every provider makes on a brief before it spends anything.
pub(crate) fn check_brief(brief: &Brief) -> Result<()>
```

It checks, in this order: the step name (`valid_step`, moved here from claude.rs), the prompt
(non-blank, at most `MAX_PROMPT`), the system text (non-blank, at most `MAX_SYSTEM`), the schema
(a JSON object). Move `MAX_PROMPT` and `MAX_SYSTEM` here too and keep `claude::MAX_PROMPT` and
`claude::MAX_SYSTEM` as `pub use` re-exports (callers use them). `Claude::call` keeps its own
model check (`valid_model`) and the leading-`/` check (`opens_with_slash`), both Claude-only, and
runs them after `check_brief`.

Add one method to the `Provider` trait, with a default:

```rust
/// The model this provider runs for a brief that asks for `asked`. Claude Code runs the one
/// asked for; a model server runs the one it was set up with.
fn model<'b>(&'b self, asked: &'b str) -> &'b str {
    asked
}
```

## 2. The model server: `builder/agent/src/self_hosted.rs` (`pub mod self_hosted;` in lib.rs)

### Constants

```rust
pub const AGENT_ID: &str = "self-hosted";
pub const AGENT_NAME: &str = "A model server on your own machines";
pub const REACH: Reach = Reach::OwnMachines;
/// Saved in the assistant's state folder, 0600, beside limit.json.
pub const FILE: &str = "self-hosted.json";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const BODY_CAP: u64 = 16 * 1024 * 1024;
const MODELS_CAP: u64 = 1024 * 1024;
```

Add the entry to `agents::AGENTS` after Claude Code:
`Agent { id: self_hosted::AGENT_ID, name: self_hosted::AGENT_NAME, reach: self_hosted::REACH }`.
Update the module's doc comment ("Later providers join this list" stays true).

### Which addresses count as the person's own

```rust
/// This computer or the person's own network: loopback, the private ranges and link-local.
pub fn own_network(ip: IpAddr) -> bool
```

- IPv4 true: `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`.
  Everything else false, including `0.0.0.0/8`, `100.64.0.0/10` (carrier-grade NAT is shared with
  strangers), multicast, broadcast and every public address.
- IPv6 true: `::1`, `fc00::/7`, `fe80::/10`, and an IPv4-mapped address (`::ffff:a.b.c.d`)
  whose IPv4 part is true by the rule above. Everything else false, including `::`, `64:ff9b::/96`
  and every global address.
- Write the ranges by hand with bit masks; do not rely on unstable std methods.

### The saved server

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server { url: String, model: String }
impl Server {
    pub fn new(url: &str, model: &str) -> Result<Server>;
    pub fn url(&self) -> &str;
    pub fn model(&self) -> &str;
    /// Read without creating any state: `Ok(None)` when nothing is saved or the state folder
    /// doesn't exist. Damaged values are an error that names the file.
    pub fn load(home: &Path, slug: &str) -> Result<Option<Server>>;
    /// Atomically save as 0600. The caller holds the state lock.
    pub fn save(&self, home: &Path, slug: &str) -> Result<()>;
    /// Remove the saved server; `Ok(false)` when there was none. The caller holds the state lock.
    pub fn remove(home: &Path, slug: &str) -> Result<bool>;
}
```

Model `load`/`save` on `ledger::Cap::load`/`save` (`read_private_optional`, `write_private`,
`remove_private` from `persona_builder_ingest::private`; a missing state folder reads as `None`
the way `Cap::load` reads it as the default). The file is
`{"url": "http://192.168.1.20:11434", "model": "qwen3:8b"}`, read with a `serde`
`deny_unknown_fields` struct and checked again with `Server::new`; any failure is
`Error::Invalid(text::server_damaged(&path))` with `path` = `~/.claude/state/<slug>/self-hosted.json`
built as `Cap::load` builds its path.

`Server::new` rules. The address (`url`), at most 255 bytes, ASCII only:

- starts `http://` or `https://`, then an authority, then optionally a single `/` and nothing
  more. No path, query, fragment, user name or password (`@`), white space or control character.
- the authority is a host and an optional `:port` (digits only, 1 to 65535; no leading `+`).
- the host is a bracketed IPv6 literal (`[::1]`), an IPv4 literal (four decimal parts), or a
  name: dot-separated labels of ASCII letters, digits and `-`, each 1 to 63 long, not starting or
  ending with `-`, at most 253 in all. Letters are lower-cased when saved.
- an IP literal must be `own_network`; otherwise the error is `text::SERVER_NOT_OWN`. A name is
  allowed here (`localhost`, `studio.lan`, `model-box.local`) because it is checked again at every
  connection (below).
- every other failure is `text::BAD_SERVER_URL`.
- saved normalised: `<scheme>://<host>[:<port>]`, lower-case, no trailing `/`.

The model, 1 to 128 bytes: the first an ASCII letter or digit, the rest ASCII letters, digits or
`.`, `_`, `-`, `:`, `/`, `@` (`qwen3:8b`, `library/llama3.2:latest`,
`lmstudio-community/gemma-3-12b@q4_k_m`). Otherwise `text::BAD_SERVER_MODEL`.

### The connection: refuse any address outside the person's own network, at every connection

```rust
#[derive(Debug)]
struct OwnNetworkOnly;
impl ureq::unversioned::resolver::Resolver for OwnNetworkOnly { … }
```

It calls `DefaultResolver::default().resolve(uri, config, timeout)` and returns its addresses only
when **every** one is `own_network`; otherwise it returns
`ureq::Error::Other(Box::new(NotOwnNetwork))` (a private unit error type implementing
`std::error::Error`), so a name that resolves, or later re-resolves, to a public address is never
connected to. ureq connects only to the addresses the resolver returns, so the check and the
connection are the same addresses.

The agent, built once per `SelfHosted`:

```rust
let config = ureq::Agent::config_builder()
    .http_status_as_error(false)
    .max_redirects(0)
    .proxy(None)
    .timeout_global(Some(timeout))
    .user_agent(format!("persona-builder/{}", persona_builder::VERSION))
    .build();
ureq::Agent::with_parts(config, DefaultConnector::default(), OwnNetworkOnly)
```

No proxy (the person's data never goes through a proxy), no redirects (a redirect could point
anywhere), no cookies, no gzip feature. Map ureq errors: the `NotOwnNetwork` error (find it by
downcasting `Error::Other`; check with a test that it arrives that way) → outcome `refused`,
`text::SERVER_NOT_OWN`; `Error::Timeout(_)` → outcome `cut-off`, `text::server_cut_off(minutes)`
(minutes = the timeout's seconds rounded up, at least 1, as Claude does);
`Error::BodyExceedsLimit(_)` → outcome `unreadable`, `text::SERVER_FAILED`; any other error
→ outcome `unreachable`, `text::SERVER_UNREACHABLE`. Never put a ureq error's own text in a
message or the ledger.

### The provider

```rust
pub struct SelfHosted { server: Server, agent: ureq::Agent, timeout: Duration, stop: Option<Arc<AtomicBool>> }
impl SelfHosted {
    pub fn new(server: Server) -> SelfHosted;            // DEFAULT_TIMEOUT
    pub fn load(home: &Path, slug: &str) -> Result<Option<SelfHosted>>;   // Server::load, then new
    pub fn with_timeout(self, timeout: Duration) -> SelfHosted;           // rebuilds the agent
    pub fn with_stop(self, stop: Arc<AtomicBool>) -> SelfHosted;
    pub fn server(&self) -> &Server;
    /// The server answers and lists the model: `GET <url>/v1/models`, 30 s at most.
    pub fn check(&self) -> Result<()>;
}
impl Provider for SelfHosted { … }
```

`check`: same agent rules (build a second agent with `CHECK_TIMEOUT`, or the shorter of the two).
Status 200 and a JSON body (at most `MODELS_CAP`) with a `data` array of objects; the model must
equal one object's `id` string. Not listed → `text::no_model(model)`; any other answer
(non-200, unreadable, no `data` array) → `text::SERVER_FAILED`; transport errors map as above.

`Provider::call(&self, session, sealed)`, in this order:

1. `sealed.check(AGENT_ID, REACH)?` before anything else.
2. `check_brief(sealed.brief())?`.
3. The stop flag, when set: `Err(text::SERVER_STOPPED)`, nothing on the ledger, nothing sent.
4. `let started = session.start(AGENT_ID, brief.step, &self.server.model)?;` (so the daily
   limit governs this call as it does Claude's; the ledger line names the server's model).
5. `POST <url>/v1/chat/completions` with `Content-Type: application/json` and
   `Accept: application/json`, body exactly:

   ```json
   {"model": "<server model>",
    "messages": [{"role": "system", "content": "<brief.system>"},
                 {"role": "user", "content": "<brief.prompt>"}],
    "stream": false,
    "response_format": {"type": "json_schema",
                        "json_schema": {"name": "reply", "strict": true, "schema": <brief.schema>}}}
   ```

   No `tools`, `tool_choice`, `functions` or any other key: the model is handed no tool. The
   brief's own `model` (a Claude name) is not sent.
6. Read the body with `.with_config().limit(BODY_CAP).read_to_vec()`, then judge:
   - status not 200 → outcome `error`, `text::SERVER_FAILED`;
   - not a JSON object, or no `choices[0].message.content` string → `unreadable`, `SERVER_FAILED`;
   - `choices[0].message.tool_calls` present and not an empty array, or
     `choices[0].finish_reason` present, not null and not `"stop"` → `error`, `SERVER_FAILED`;
   - the content does not parse as JSON, or parses to something other than an object or an array
     → `no-output`, `SERVER_FAILED`;
   - otherwise `ok`: `Reply { output, usage, cost_usd: 0.0 }` with
     `usage.input_tokens = usage.prompt_tokens`, `output_tokens = usage.completion_tokens`
     (missing or not a `u64`: 0), both cache counts 0.
7. Always `session.finish(started, &Line { agent: AGENT_ID, step, model: <server model>, outcome,
   plan: None, seconds, usage, cost_usd: 0.0, cost_from: "self-hosted" })` before returning,
   on every path after step 4.

`fn model(&self, _asked)` returns the server's model. `id`, `name`, `reach` return the constants.

## 3. Words: `builder/agent/src/text.rs`

Exact texts (plain language; no tool words):

```rust
pub const SERVER_UNREACHABLE: &str = "Persona Builder couldn't reach your model server. Check it's running and that its address is right (`persona-builder self-hosted show`), then try again.";
pub const SERVER_NOT_OWN: &str = "Nothing was sent. Your model server's address leads outside this computer and your own network, so Persona Builder won't use it. Give it the address of a computer on your own network.";
pub const SERVER_FAILED: &str = "Your model server couldn't finish this step. Check it answers you and has the model loaded, then try again.";
pub const SERVER_STOPPED: &str = "Persona Builder stopped before asking your model server, as you asked.";
pub const BAD_SERVER_URL: &str = "A model server's address looks like http://192.168.1.20:11434 or http://localhost:1234: http or https, a computer's name or address, and a port, with nothing after it.";
pub const BAD_SERVER_MODEL: &str = "A model's name is letters and digits, and may have . _ - : / or @ in it, up to 128 in all, like qwen3:8b.";
pub const NO_STATE_FOR_SERVER: &str = "This assistant has no private folder yet, so there's nowhere to keep its model server. Run `persona-builder consent agree` first.";
pub const NO_SERVER: &str = "No model server is set up for this assistant, so a step with data you keep on your own machines doesn't run.";
pub const SELF_HOSTED_HINT: &str = "To run it without the data leaving your machines, set up a model server with `persona-builder self-hosted set`.";
/// Every fixed sentence a model-server call can fail with, for callers that pass only known
/// sentences on to a person.
pub const SERVER_MESSAGES: [&str; 4] = [SERVER_UNREACHABLE, SERVER_NOT_OWN, SERVER_FAILED, SERVER_STOPPED];
pub fn server_cut_off(minutes: u64) -> String  // "Your model server was still working on this step after {n} minute(s), so Persona Builder stopped waiting. Try again; if it happens again, try a smaller model or a smaller step."
pub const SERVER_CUT_OFF_START: &str = "Your model server was still working on this step after ";
pub fn server_damaged(path: &Path) -> String   // "The model server settings saved at {path} can't be read, so Persona Builder won't use them. Set them again with `persona-builder self-hosted set`, or remove them with `persona-builder self-hosted off`."
pub fn no_model(model: &str) -> String         // "Your model server answers, but it doesn't list the model {model}. Load it on the server, or give the name the server lists."
pub fn server(server: &Server) -> String       // "Model server: {url}, model {model}. Steps with data you keep on your own machines go to it. Before each step Persona Builder checks the address still leads to this computer or your own network."
```

`server_cut_off` uses "minute" for 1 and "minutes" otherwise, as `cut_off` does.

## 4. The guard: the order of providers and the hint

In `guard.rs`:

```rust
/// One assistant's providers, in the guard's order: Claude Code, then the person's model server
/// when one is set up. A step goes to the first one its data may reach.
pub fn providers<'a>(claude: &'a Claude, server: Option<&'a SelfHosted>) -> Vec<&'a dyn Provider>
```

`Guard::route`'s refusal, when **no** provider in the guard has `Reach::OwnMachines`, becomes
`format!("{} {}", <today's kept_home text>, text::SELF_HOSTED_HINT)`. With an own-machines
provider in the guard the refusal is unchanged (it can't happen today: `OwnMachines` takes every
ceiling). `Sealed::check`'s refusal is unchanged.

## 5. Every guard gets the server

Replace `Guard::new(vec![&claude], …)` with `Guard::new(guard::providers(&claude, server.as_ref()), …)`
at each of these, loading the server right before the guard:

- `builder/cli/src/main.rs`: the four in `build`, the interview draft, refresh and apply
  (`SelfHosted::load(&home, &member.slug)?`).
- `builder/api/src/building.rs` (build, interview draft) and `builder/api/src/review.rs` (refresh
  draft, refresh apply): `SelfHosted::load(home, &request.slug).map_err(|_| failure(SETTINGS))?`
  (`REVIEW_SETTINGS` in review.rs). Where the Claude provider gets `.with_stop(...)`, give the
  server the same stop flag.
- `builder/api/src/assistants.rs` `claude_message`: also pass on any message in
  `text::SERVER_MESSAGES` or starting with `text::SERVER_CUT_OFF_START`.
- `builder/panel-runner/src/turns.rs`, below.

### Panel turns

`ClaudeTurns::new` loads each seat's server (`SelfHosted::load(home, seat)`, with the same
`with_timeout(Duration::from_secs(600))` as Claude, and `with_stop` in `ClaudeTurns::with_stop`);
a damaged one is `crate::text::assistant_problem(seat, crate::text::BAD_SERVER)` with a new
`BAD_SERVER` reason in `builder/panel-runner/src/text.rs` worded like the existing
`BAD_SETTINGS`/`BAD_LIMIT` reasons ("its model server settings can't be read"; read those two
and match their grammar). Keep the per-seat `servers` map beside `sessions`.

**Every turn is checked against the narrowest ceilings across all the panel's seats**, not the
receiving seat's alone: a seat's post can carry its own folder's data into every other seat's
brief. Add to `builder/core/src/ceilings.rs`:

```rust
/// Per source, the narrower of the two ceilings; a source is set when either sets it.
pub fn narrowest(&self, other: &Ceilings) -> Ceilings
```

`ClaudeTurns` stores one `Ceilings` (the fold of every seat's with `narrowest`, starting from the
first seat's) and every `turn` builds `Guard::new(guard::providers(&self.claude,
self.servers[seat].as_ref()), self.ceilings.clone())`. `turn_error` also passes on
`text::SERVER_MESSAGES` and messages starting `text::SERVER_CUT_OFF_START`.

## 6. The build's cache and record name the provider

`builder/build/src/build.rs`, `Asker::ask`: route first, then key the cache by the provider:

```rust
let provider = self.guard.route(self.sources)?;
let model = provider.model(self.model);
let keyed = if provider.id() == claude::AGENT_ID { model.to_owned() } else { format!("{}/{model}", provider.id()) };
let name = cache_name(step, &keyed, system, schema, prompt);
```

Claude Code's keys stay exactly as today (a rebuild keeps reusing what the person already paid
for); another provider's never equal one of Claude's, because Claude's model names can't hold a
`/`. Do not change `cache_name` itself.

`Asker` records who answered: a `by: Vec<(String, String)>` of `(provider id, model)` in
first-use order, without repeats, pushed on every `ask` (reused or asked), with
`pub fn by(&self) -> &[(String, String)]`. `Built` gains `pub by: Vec<(String, String)>` filled
from it.

`builder/build/src/draft.rs` `entries`: the build backtest's Method line now says
`predicted each one blind through {who}, from decisions.md …` where `who` joins one phrase per
`built.by` entry with ` and `: `claude-code` → `the person's own Claude Code (model {m})`,
`self-hosted` → `a model server on the person's own machines (model {m})`, any other id →
`{id} (model {m})`. When `built.by` is empty, `who` is `the person's own Claude Code (model
{model})`, exactly today's words, so a Claude-only build writes the same entry as today.

## 7. The command line: `persona-builder self-hosted`

In `builder/cli/src/main.rs`, a `SelfHosted { #[command(subcommand)] command: SelfHostedCommand }`
command beside `Limit`, documented "Use a model server on your own machines for this assistant":

```rust
enum SelfHostedCommand {
    /// Show the model server set up for this assistant
    Show { member_dir: PathBuf },
    /// Use a model server on this computer or your own network
    Set { member_dir: PathBuf, #[arg(long)] url: String, #[arg(long)] model: String },
    /// Stop using a model server
    Off { member_dir: PathBuf },
}
```

- `show`: `member_and_home`, `Server::load`; prints `text::server(&server)` or `text::NO_SERVER`.
- `set`: `member_and_home`; `Server::new(url, model)?`; `SelfHosted::new(server).check()?` (before
  taking the lock: no network call holds it); then `lock_existing_state` (NotFound →
  `text::NO_STATE_FOR_SERVER`, as `limit_set` does), `save`, print `text::server`.
- `off`: lock as `set` does, `Server::remove`, print `text::NO_SERVER`.

`persona-builder agents` and `ceilings show` list the new provider through `AGENTS` already.

## 8. Tests first (red, then green)

Write each test, see it fail on its assertion where the API already exists, then make it pass.
Test servers are plain `std::net::TcpListener`s on `127.0.0.1:0` in a thread, answering canned
HTTP/1.1 responses with `Content-Length` and `Connection: close`, and recording each request's
method, path, headers and body. No new dev-dependency.

`builder/agent` (unit tests in `self_hosted.rs` where they need private parts; the rest in
`builder/agent/tests/self_hosted.rs`):

1. `own_network`: true for `127.0.0.1`, `10.1.2.3`, `172.16.0.1`, `172.31.255.255`,
   `192.168.1.10`, `169.254.1.1`, `::1`, `fd00::1`, `fe80::1`, `::ffff:192.168.1.10`; false for
   `8.8.8.8`, `172.32.0.1`, `100.64.0.1`, `0.0.0.0`, `255.255.255.255`, `224.0.0.1`, `::`,
   `2001:db8::1`, `::ffff:8.8.8.8`, `64:ff9b::808:808`.
2. `Server::new` accepts and normalises `http://127.0.0.1:11434`, `http://localhost:1234/`
   (saved without the `/`), `https://studio.lan:8443`, `http://[::1]:8080`,
   `http://Model-Box.local:11434` (lower-cased); refuses with `BAD_SERVER_URL`: `ftp://127.0.0.1`,
   `http://user:pw@127.0.0.1:1`, `http://127.0.0.1:11434/v1`, `http://127.0.0.1:11434?x=1`,
   `http://127.0.0.1:11434#x`, `http://127.0.0.1:0`, `http://127.0.0.1:65536`, `http://`,
   `http://-box.lan:1`, `http://box .lan:1`, a control character, 256 bytes; refuses with
   `SERVER_NOT_OWN`: `http://8.8.8.8:80`, `http://[2001:db8::1]:80`. Models: accepts the three
   examples above; refuses ``, ` qwen`, `a b`, `.hidden`, 129 bytes.
3. Save, load, remove: round trip, the file is mode 0600, absent → `None`, no state folder →
   `None` and nothing created; a file with an unknown field, bad JSON, or a public IP literal →
   `server_damaged` naming the path.
4. A call through `Guard::new(vec![&server], Ceilings::defaults())`: the request is
   `POST /v1/chat/completions`, `content-type: application/json`; the body's keys are exactly
   `model`, `messages`, `stream`, `response_format`; no `tools`, `tool_choice` or `functions`;
   `model` is the server's, not the brief's; `messages` are the system then the user text;
   `response_format` is as above with the brief's schema. The reply's output, usage
   (prompt/completion tokens) and `cost_usd == 0.0` come back; the ledger's one line has agent
   `self-hosted`, the server's model, outcome `ok`, `cost_usd` 0 and `cost_from` `self-hosted`.
5. The seal: a fake provider with id `fake` and reach `OwnMachines` whose `call` hands its own
   sealed brief to the server's `call` is refused, and the test server sees no request.
6. A server whose address is a public literal, made by struct literal in a unit test (bypassing
   `new`): the call is refused with `SERVER_NOT_OWN`, ledger outcome `refused`, and no connection
   is attempted (the resolver refuses before connecting; `8.8.8.8` is never dialled because the
   literal resolves without DNS). Plus a resolver unit test: `http://127.0.0.1:9/` resolves,
   `http://8.8.8.8:9/` is refused.
7. No proxy: the agent's `config().proxy()` is `None`.
8. A `302` with `Location` pointing at a second listener: `SERVER_FAILED`, outcome `error`, and
   the second listener gets nothing.
9. Failures, each with its ledger outcome: 500 (`error`); a body that isn't JSON (`unreadable`);
   no `choices` (`unreadable`); content `not json` (`no-output`); content `42` (`no-output`);
   non-empty `tool_calls` (`error`); `finish_reason` `length` (`error`); a body over
   `BODY_CAP` (`unreadable`); a closed port (`SERVER_UNREACHABLE`, `unreachable`); a server that
   accepts and never answers, with `with_timeout(1 s)` (`server_cut_off(1)`, `cut-off`).
10. The stop flag set: `SERVER_STOPPED`, no request, no ledger line.
11. The daily limit reached (a ledger already at the cap, as `tests/cap.rs` does): the cap's
    refusal, no request.
12. `check`: lists the model → `Ok`; doesn't → `no_model`; 404 → `SERVER_FAILED`.
13. Routing with `guard::providers(&stub_claude, Some(&server))` and the defaults: a brief with
    `[Source::Mail]` reaches the server only; `[Source::ClaudeHistory]` reaches Claude only. With
    `providers(&stub_claude, None)`, `[Source::Mail]` is refused with a message that starts
    `Nothing was sent.` and ends with `SELF_HOSTED_HINT`. Use the existing Claude stub helpers in
    `builder/agent/tests/`.
14. `AGENTS`: update `tests/find_check.rs` and `tests/provider.rs` for two providers, unique ids.

`builder/core`: `narrowest` takes the narrower per source and marks set when either set it.

`builder/build` (`tests/provider.rs` has an `Asker` harness): before changing anything, compute
`cache_name("label", "sonnet", "S", &json!({"type":"object"}), "P")` on today's code and pin that
literal in a test, so Claude's keys are proven unchanged. A fake provider with id `self-hosted`
gets a different key for the same inputs, and a reply cached under Claude's key is not reused when
the step routes to it. `Asker::by` records `(id, model)` once per provider. `draft::entries` with
`by` of Claude only writes today's words exactly; with Claude and the server, the joined words.

`builder/panel-runner`: seats `pebble` (member.json `"ceilings": {"crew-folder": "local-only"}`)
and `wren` (defaults), Claude only: a turn for `wren` is refused (`Nothing was sent.`), because
`pebble`'s ceiling now covers it; `narrowest` folding unit test; `turn_error` passes
`SERVER_FAILED` and a `server_cut_off(3)` message through.

`builder/api`: `claude_message` passes `SERVER_UNREACHABLE` and a `server_cut_off(2)` text
through.

`builder/cli` (follow the existing tests in `builder/cli/tests/`): `self-hosted set` against a
test server listing the model saves `self-hosted.json` (0600, normalised URL) and prints
`text::server`; a model it doesn't list prints `no_model` and saves nothing; a public literal is
refused and saves nothing; `show` before and after; `off` removes it.

## 9. Done means

- `bash scripts/ci-local.sh` passes (fmt, clippy with `-D warnings`, every test, the name guards).
- `Cargo.lock` adds no package (only `persona-builder-agent`'s dependency list changes).
- No real person, no fixed path, no network address outside `127.0.0.1` dialled by any test.
- Report: each file changed, each test added, and the red failure you saw for each before green.

## 10. Fixes after review (V1, 2026-10-31)

The security review (Opus) found one medium, and the Verifier (Fable, PASS) two should-fixes;
these close them.

### 10.1 This computer only, unless the person says their network (medium)

A private address is not proof of the person's own network: on café, hotel or office Wi-Fi a
stranger's device can answer as `studio.lan` (DNS or mDNS) or simply hold `192.168.1.20`, and the
briefs, mail included, would reach it in plain HTTP. So a server is **this computer only** by
default; another machine needs the person's explicit `--network`, and the words say what that
risks.

- `Server` gains `network: bool` (`pub fn network(&self) -> bool`); `Server::new(url, model,
  network)`. The saved file gains a required `"network": true|false`.
- `pub fn this_computer(ip: IpAddr) -> bool`: `127.0.0.0/8`, `::1`, and an IPv4-mapped address
  whose IPv4 part is in `127.0.0.0/8`. Nothing else.
- With `network == false`: an IP literal that is not `this_computer` is refused by `Server::new`
  with `text::SERVER_NOT_THIS_COMPUTER`, and the resolver (now `OwnNetworkOnly { network: bool }`)
  returns addresses only when every one is `this_computer`, else the same `NotOwnNetwork` error,
  mapped to `text::SERVER_NOT_THIS_COMPUTER`. With `network == true`: exactly today's
  `own_network` rule and `SERVER_NOT_OWN`.
- CLI: `self-hosted set` takes `--network` (a plain flag, "Allow a computer on your own network,
  not only this one").
- Texts, exactly (add `SERVER_NOT_THIS_COMPUTER` to `SERVER_MESSAGES`, which becomes 5 long):

  ```rust
  pub const SERVER_NOT_THIS_COMPUTER: &str = "Nothing was sent. Your model server's address isn't this computer, and you haven't said to use your network. To use another computer on your own network, set it up again with --network.";
  ```

  `text::server(&server)`, two forms:
  - not network: `Model server: {url}, model {model}, on this computer. Steps with data you keep on your own machines go to it, and before each step Persona Builder checks the address still leads to this computer.`
  - network: `Model server: {url}, model {model}, on your network. Steps with data you keep on your own machines go to it. Before each step Persona Builder checks the address is on a private network, but it can't tell your network from someone else's: on other Wi-Fi, a stranger's computer could answer at the same address. Before you use other Wi-Fi, turn it off with persona-builder self-hosted off.`
- Tests: `this_computer` table (true: `127.0.0.1`, `127.255.0.1`, `::1`, `::ffff:127.0.0.1`;
  false: `10.0.0.1`, `192.168.1.10`, `169.254.1.1`, `fd00::1`, `fe80::1`, `::ffff:192.168.1.10`,
  `8.8.8.8`); `Server::new("http://192.168.1.20:11434", m, false)` refused with
  `SERVER_NOT_THIS_COMPUTER` and accepted with `true`; `http://localhost:1234` accepted with
  `false`; the resolver with `network: false` refuses `http://10.0.0.1:9/` and accepts
  `http://127.0.0.1:9/`, with `network: true` accepts `http://10.0.0.1:9/`; the saved file round
  trips `network`, and a file without it is `server_damaged`; the CLI `set` without `--network`
  on `http://192.168.1.20:1` is refused and saves nothing, and both `show` forms print exactly.

### 10.2 A server that doesn't answer is unreachable, not slow (Verifier)

Set `.timeout_connect(Some(Duration::from_secs(10).min(timeout)))` on the agent, and map
`ureq::Error::Timeout(ureq::Timeout::Resolve | ureq::Timeout::Connect)` to outcome `unreachable`,
`text::SERVER_UNREACHABLE`; every other timeout stays `cut-off`. Test it in a unit test on
`transport_error` (no network: construct the errors directly), and keep the existing
never-answers listener test passing as `cut-off`.

### 10.3 A panel post is held to its length here, not only by the server (security review, low)

In `builder/panel-runner/src/turns.rs` `ask`, a `post` longer than 12,000 characters
(`chars().count()`) is treated like a missing one: `crate::text::UNREADABLE`, usage kept. The
schema's `maxLength` is only a request to a model server. Test with a fake provider whose post is
12,001 characters (refused) and one of exactly 12,000 (kept).
