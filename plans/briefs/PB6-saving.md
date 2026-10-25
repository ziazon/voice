# PB6 brief: saving an assistant (an existing repo, a private GitHub repo, or this Mac only)

Persona Builder builds a person's AI assistant from their own Claude history. PB1 gave the core
library (`builder/core`, `persona_builder`) the member format. This slice lets a person choose where
their assistant is kept and then save it there:

- **Found:** their AI setup already sits in a git checkout with a GitHub remote (for example
  `~/.claude`, or the folder holding the assistant). Offer to save there (recommended) or keep it on
  this Mac only.
- **Not found:** *Save it to a private GitHub repo* is selected, *Keep it on this Mac only* sits
  beside it, and one line tells a person with no GitHub account that a free one keeps a backup
  (`https://github.com/signup`).
- GitHub sign-in is GitHub's **device flow**; the token lives in the macOS Keychain only. A new repo
  is created private and **read back as private before the first push**. Before **every** push the
  privacy check runs and every push address is confirmed to be a private GitHub repo; anything else
  is refused.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`,
`bootstrap/PROMPT.md` steps 2 and 14 (how a person saves today), `builder/core/src/member.rs`,
`builder/core/src/crew.rs` (`shown_path`), `builder/core/src/error.rs`, `builder/cli/src/main.rs`,
`builder/core/tests/common/mod.rs` (the scratch-folder helper to copy). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

## Sources (read 2026-10-24; cite them in doc comments where the code depends on them)

- Device flow: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
  (*Device flow*). `POST https://github.com/login/device/code` with `client_id` and `scope`; the reply
  has `device_code`, `user_code`, `verification_uri`, `expires_in` (default 900) and `interval`
  (minimum seconds between polls). Poll `POST https://github.com/login/oauth/access_token` with
  `client_id`, `device_code` and `grant_type=urn:ietf:params:oauth:grant-type:device_code`. **No
  client secret** is needed. Errors: `authorization_pending` (keep polling), `slow_down` (add 5
  seconds to the interval; a new `interval` may be given), `expired_token`, `access_denied`,
  `unsupported_grant_type`, `incorrect_client_credentials`, `incorrect_device_code`,
  `device_flow_disabled`. Send `Accept: application/json` to get JSON back.
- Scopes: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps.
  Creating a **private** repo needs `repo`. The granted scopes are in the `X-OAuth-Scopes` response
  header, and "users can edit their scopes", so check them rather than trusting the request.
- Repos: https://docs.github.com/en/rest/repos/repos. `POST /user/repos` (`name`, `private`,
  `description`, `auto_init`, `has_issues`, `has_projects`, `has_wiki`; 201 created, 422 validation
  failed). `GET /repos/{owner}/{repo}` returns `private` (boolean) and `visibility` (string).
  Headers `Accept: application/vnd.github+json` and `X-GitHub-Api-Version: 2026-03-10`
  (https://docs.github.com/en/rest/about-the-rest-api/api-versions lists `2026-03-10` and
  `2022-11-28`).
- Git over HTTPS with a token: `actions/checkout` (`src/git-auth-helper.ts`) sets
  `http.https://github.com/.extraheader` to `AUTHORIZATION: basic <base64("x-access-token:<token>")>`.
- Git, checked by hand on git 2.56: `git remote get-url --push --all <remote>` applies
  `pushInsteadOf`; `read-tree`, `hash-object -w`, `update-index --index-info` (mode `0` removes a
  path), `write-tree`, `commit-tree` and `update-ref <ref> <new> ""` (fails if the ref exists) all
  work in a bare repo with `GIT_INDEX_FILE` pointing at a scratch index, and touch no work tree.
  `GIT_CONFIG_COUNT` needs git 2.31 and `GIT_CONFIG_GLOBAL` 2.32.

## Rules

- **New crate `builder/save`** (package `persona-builder-save`, library `persona_builder_save`). The
  core library's rule is "no I/O beyond the filesystem", so the network, git and Keychain code lives
  here; core is not changed. The crate, its `Cargo.toml` and the workspace member are already added,
  with `ureq =3.4.2` (rustls), `base64 =0.23.1`, `zeroize =1.9.1`, `serde`, `serde_json` and
  `persona-builder` fetched and in `Cargo.lock`. **Add no other dependency** and no
  dev-dependency. Pass `--offline` to cargo if it tries the registry.
- Reuse `persona_builder::error::{Error, Result}` everywhere: `Error::Io` for filesystem errors,
  `Error::Invalid(<plain message>)` for everything else. Every message a person can see is one of
  the texts in §1, exactly.
- Public repo: invented names only (`Wren` the assistant, `Rowan Vale` the principal, GitHub owner
  `rowan-vale`, ids `4242`). No real person, no fixed home path: every test path is under a scratch
  folder. Never a real GitHub call, a real Keychain write or a real token in a test (the one
  `#[ignore]` Keychain test in §4 is run by hand).
- No `unsafe`. No shell (`sh -c`) anywhere: every child is `Command` with an argument list.
- A token never appears in argv, in a log, in an error message, in `Debug` output or in a file.
- `cargo fmt`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked` and
  `bash scripts/ci-local.sh` pass. Tests may bind `127.0.0.1` and run `git`.
- Do not touch `scripts/`, `bootstrap/`, `crew/`, `docs/`, `.ai/` or `README.md` (Claude writes the
  docs).

## 1. `src/text.rs`: every sentence a person sees

Exactly these (`pub const` or `pub fn … -> String`); nothing else in the crate or the CLI may build
a person-facing sentence of its own.

```rust
pub const CHOICE_GITHUB: &str = "Save it to a private GitHub repo";
pub const CHOICE_LOCAL: &str = "Keep it on this Mac only";
pub fn choice_existing(shown: &str) -> String // "Save it in {shown}, with the rest of your AI setup"
pub const NO_GITHUB_ACCOUNT: &str = "No GitHub account? A free one keeps a backup of your assistant: https://github.com/signup. Or keep it on this Mac only.";
pub fn sign_in_prompt(uri: &str, code: &str) -> String // "Go to {uri} and enter this code: {code}"
pub const SIGN_IN_WAITING: &str = "Waiting for you to approve Persona Builder on GitHub…";
pub fn signed_in(login: &str) -> String // "Signed in to GitHub as {login}."
pub const SIGNED_OUT: &str = "Signed out of GitHub on this Mac. To remove Persona Builder's access completely, open Settings on GitHub, then Applications, then Authorized OAuth Apps.";
pub const NOT_SET_UP: &str = "GitHub sign-in isn't set up in this copy of Persona Builder yet. You can keep your assistant on this Mac only for now.";
pub const SIGN_IN_CANCELLED: &str = "GitHub sign-in was cancelled. Nothing was changed.";
pub const SIGN_IN_EXPIRED: &str = "The GitHub code ran out before it was used. Start signing in again for a new code.";
pub fn sign_in_failed(code: &str) -> String // "GitHub sign-in didn't work (GitHub said: {code}). Try again later, or keep your assistant on this Mac only."
pub const SIGN_IN_FIRST: &str = "Sign in to GitHub first.";
pub const NO_REPO_PERMISSION: &str = "GitHub didn't give Persona Builder permission to make private repos, so it can't save there. Sign in again and allow it, or keep your assistant on this Mac only.";
pub fn repo_public(full: &str) -> String // "{full} is public, so anyone could read your assistant. Persona Builder won't save it there. Make the repo private on GitHub, or keep your assistant on this Mac only."
pub fn repo_unconfirmed(full: &str) -> String // "Persona Builder couldn't confirm that {full} is private, so it didn't save there. Check that you're signed in to the GitHub account that owns it."
pub const NOT_ON_GITHUB: &str = "This repo isn't on GitHub. Persona Builder can only check that GitHub repos are private, so it won't save there. Keep your assistant on this Mac only, or save it to a private GitHub repo.";
pub fn repo_name_taken(name: &str) -> String // "GitHub wouldn't make a repo called {name}. You may already have one with that name: pick another."
pub const BAD_REPO_NAME: &str = "A repo name can use letters, numbers, dots, dashes and underscores, up to 100 characters.";
pub const NOT_OFFERED: &str = "Persona Builder can save only to a GitHub repo it found around your AI setup.";
pub const CHOOSE_FIRST: &str = "Choose where to save your assistant first.";
pub const KEPT_LOCAL: &str = "Your assistant is kept on this Mac only, so there's nothing to send.";
pub fn privacy_hits(places: &str) -> String // "The privacy check found something private, so nothing was sent. Look at: {places}"
pub const PRIVACY_NOT_BUILT: &str = "This copy of Persona Builder can't run the privacy check yet, so nothing was sent.";
pub fn links_in_folder(rel: &str) -> String // "Persona Builder won't save a folder that holds links to other places: {rel}"
pub fn saved_github(full: &str) -> String // "Saved to {full} on GitHub."
pub fn saved_existing(shown: &str, branch: &str) -> String // "Saved to {shown}, on the branch {branch}."
pub const UP_TO_DATE: &str = "Nothing had changed, so the saved copy was already up to date.";
pub const NEEDS_GIT: &str = "Persona Builder needs Git, which this Mac doesn't have yet. To get it, open the Terminal app and type: xcode-select --install";
pub const GIT_TOO_OLD: &str = "Persona Builder needs Git 2.32 or later. Update Git, then try again.";
pub fn push_failed(detail: &str) -> String // "GitHub didn't take the save: {detail}"
pub const MAC_ONLY: &str = "Saving to GitHub works only on a Mac for now.";
pub const KEYCHAIN_FAILED: &str = "Persona Builder couldn't use the Keychain on this Mac, so it didn't keep your GitHub sign-in.";
pub const GITHUB_UNREACHABLE: &str = "Persona Builder couldn't reach GitHub. Check the internet connection, then try again.";
```

## 2. `src/secret.rs`

`pub struct Secret(String)`: `Secret::new(String)`, `expose(&self) -> &str`, `Drop` zeroizes
(`zeroize::Zeroize`), `Debug` prints `Secret(hidden)`. No `Display`, no `Clone`, no `Serialize`.

## 3. `src/github.rs`: the HTTP seam, the device flow, the API

### The seam

```rust
pub enum Method { Get, Post }
pub struct HttpRequest<'a> {
    pub method: Method,
    pub url: String,
    /// Static values only, so a token cannot be put in a header by mistake.
    pub headers: Vec<(&'static str, &'static str)>,
    pub form: Vec<(&'static str, String)>,          // sent form-encoded when non-empty
    pub json: Option<serde_json::Value>,            // sent as JSON when Some (never both)
    pub bearer: Option<&'a Secret>,                 // becomes `Authorization: Bearer <token>`
}
pub struct HttpResponse { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
// header names lowercased
pub trait Http { fn send(&self, request: &HttpRequest<'_>) -> Result<HttpResponse>; }
pub struct UreqHttp { /* ureq::Agent */ }
impl Default for UreqHttp
```

`UreqHttp`: `ureq::Agent::config_builder().http_status_as_error(false).max_redirects(0)
.timeout_global(Some(30 s)).user_agent(format!("persona-builder/{}", persona_builder::VERSION))`.
Read the body with `.with_config().limit(1 MiB).read_to_vec()`. Any transport error (connect,
timeout, body over the limit) → `Error::Invalid(GITHUB_UNREACHABLE)`; never put the URL or ureq's
message in the error. A redirect comes back as its 3xx response, not followed.

### Endpoints and the client id

```rust
pub struct Endpoints { login: String, api: String }
impl Endpoints {
    pub fn github() -> Self;                       // "https://github.com", "https://api.github.com"
    pub fn new(login: &str, api: &str) -> Result<Self>;
}
pub const SCOPE: &str = "repo";
pub const API_VERSION: &str = "2026-03-10";
/// Persona Builder's registered GitHub OAuth App (device flow on). None until it is registered.
const CLIENT_ID: Option<&str> = None;
pub fn client_id() -> Option<&'static str> // option_env!("PERSONA_BUILDER_GITHUB_CLIENT_ID").or(CLIENT_ID)
```

`Endpoints::new` accepts each base only as `https://<host>` (no userinfo, port, path, query or
fragment; one trailing `/` dropped) or `http://127.0.0.1:<port>` (tests). Anything else →
`Error::Invalid("not a GitHub address")` (an internal message; the CLI never takes endpoints from
argv or the environment, only `Endpoints::github()`).

### `Github`

```rust
pub struct Github<'a> { http: &'a dyn Http, endpoints: Endpoints, client_id: String }
pub struct DeviceCode { pub user_code: String, pub verification_uri: String,
                        pub interval: u64, pub expires_in: u64, device_code: Secret }
pub struct Account { pub login: String, pub id: u64 }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct RepoRef { pub owner: String, pub name: String }
impl RepoRef { pub fn full(&self) -> String /* "owner/name" */ }

impl<'a> Github<'a> {
    pub fn new(http: &'a dyn Http, endpoints: Endpoints, client_id: &str) -> Result<Self>;
    pub fn start_sign_in(&self) -> Result<DeviceCode>;
    pub fn finish_sign_in(&self, code: &DeviceCode, sleep: &mut dyn FnMut(Duration)) -> Result<Secret>;
    pub fn account(&self, token: &Secret) -> Result<Account>;
    pub fn create_private_repo(&self, token: &Secret, name: &str) -> Result<RepoRef>;
    pub fn confirm_private(&self, token: &Secret, repo: &RepoRef) -> Result<()>;
}
```

- `new`: the client id is 1 to 64 ASCII letters, digits and `.`; else `Error::Invalid(NOT_SET_UP)`.
- Requests to `login` send `Accept: application/json`; requests to `api` send
  `Accept: application/vnd.github+json` and `X-GitHub-Api-Version: API_VERSION`. Only `api`
  requests ever carry `bearer`. Login requests carry the client id in the form, never a bearer.
- `start_sign_in`: `POST <login>/login/device/code`, form `client_id`, `scope=repo`. Needs 200 and a
  JSON object with string `device_code` (1–256 chars, no control characters), string `user_code`
  (1–32 chars of `A-Z`, `0-9`, `-`), string `verification_uri` that starts with `<login>/`, and
  optional `interval` (1–60, default 5) and `expires_in` (1–1800, default 900). Anything else →
  `sign_in_failed("unexpected reply")`.
- `finish_sign_in`: before each poll call `sleep(interval)`, adding it to a running total; once the
  total would pass `expires_in`, stop with `SIGN_IN_EXPIRED` without polling again. Poll
  `POST <login>/login/oauth/access_token`, form `client_id`, `device_code`,
  `grant_type=urn:ietf:params:oauth:grant-type:device_code`. A reply with string `access_token`:
  it must be 1–255 bytes of `A-Za-z0-9._~:+/=-` (letters, digits and `. _ ~ : + / = -`) (else `sign_in_failed("unexpected reply")`) → return
  it as a `Secret`. A reply with string `error`: `authorization_pending` → poll again;
  `slow_down` → interval = the reply's `interval` if it is larger than the current one plus 5, else
  current + 5; `expired_token` → `SIGN_IN_EXPIRED`; `access_denied` → `SIGN_IN_CANCELLED`; any other
  code made only of `a-z` and `_`, at most 40 chars → `sign_in_failed(code)`; anything else →
  `sign_in_failed("unexpected reply")`. A non-200 status → `sign_in_failed("unexpected reply")`.
- `account`: `GET <api>/user` with the bearer. 401 → `SIGN_IN_FIRST`. 200: the `x-oauth-scopes`
  header, split on `,` and trimmed, must contain exactly `repo` as one item, else
  `NO_REPO_PERMISSION`; the body needs string `login` (GitHub's owner rule below) and integer `id`
  > 0. Anything else → `GITHUB_UNREACHABLE`.
- `create_private_repo`: `valid_repo_name(name)` or `BAD_REPO_NAME`. `POST <api>/user/repos`, JSON
  `{"name": name, "private": true, "description": "My AI assistant, saved by Persona Builder",
  "auto_init": false, "has_issues": false, "has_projects": false, "has_wiki": false}`. 201 → owner
  from `owner.login` and name from `name` (both must pass the rules below) → then
  `confirm_private` on it, and return it only if that passes. 422 → `repo_name_taken(name)`. 401 →
  `SIGN_IN_FIRST`. 403 → `NO_REPO_PERMISSION`. Other → `GITHUB_UNREACHABLE`.
- `confirm_private`: `GET <api>/repos/{owner}/{name}`. 200 with `private == true` **and**
  `visibility == "private"` → `Ok`. 200 otherwise → `repo_public(full)`. Any other status (301
  renamed, 401, 403, 404, …) → `repo_unconfirmed(full)`.

### Repo names and remote addresses

```rust
pub fn valid_owner(owner: &str) -> bool;      // 1–39 of A-Za-z0-9 and '-'
pub fn valid_repo_name(name: &str) -> bool;   // 1–100 of A-Za-z0-9 . _ -; not "." or ".."; not ending ".git"
pub fn suggested_repo_name(slug: &str) -> String; // "{slug}-assistant"
pub fn github_repo_of(url: &str) -> Option<RepoRef>;
```

`github_repo_of` accepts exactly these shapes, the host compared case-insensitively and exactly
`github.com` (so not `github.com.example`, `www.github.com` or `gist.github.com`), with one optional
`.git` and one optional trailing `/` after the name: `https://github.com/O/N`,
`https://<userinfo>@github.com/O/N`, `git@github.com:O/N`, `ssh://git@github.com/O/N`. The owner and
name must pass the rules above. Every other string (`http://`, a port, a local path, `file://`,
extra path segments, a query) → `None`.

### Seams under test (`tests/github.rs`, with a scripted fake `Http` in `tests/common/mod.rs`)

The fake answers from a queue of `HttpResponse`s and records each request's method, URL, headers,
form, JSON and exposed bearer. It is the only boundary mocked. Expected values are literals.

1. `start_sign_in` sends the client id and `scope=repo` to `/login/device/code` with no bearer, and
   returns the code; each malformed reply (missing `device_code`, bad `user_code`, a
   `verification_uri` on another host, `interval` 0 or 61) is refused.
2. `finish_sign_in`: pending → pending → token returns the token after sleeps `[5, 5, 5]`; `slow_down`
   without and with a larger `interval` (sleeps `[5, 10]` and `[5, 30]`); `access_denied`,
   `expired_token`, `device_flow_disabled` give their texts; `expires_in: 12` with interval 5 polls
   twice and then gives `SIGN_IN_EXPIRED` without a third request; a token with a space or a quote is
   refused.
3. `account`: scopes `"repo, read:org"` passes; `"public_repo"` and `"repo:status"` give
   `NO_REPO_PERMISSION`; a missing header gives it too; 401 gives `SIGN_IN_FIRST`. The bearer is sent
   and the request carries `X-GitHub-Api-Version: 2026-03-10`.
4. `create_private_repo` sends exactly the JSON above, then a GET for the created repo; a create
   that answers 201 but whose read-back says `"private": false` or `"visibility": "internal"` is an
   error (`repo_public`), and so is a read-back of 404 (`repo_unconfirmed`); 422 gives
   `repo_name_taken`; a bad name sends no request at all.
5. `confirm_private` with every status above.
6. `github_repo_of` on a table of at least 20 accepted and refused strings; `valid_repo_name` and
   `valid_owner` edges (100 vs 101 chars, `.`, `..`, `x.git`).
7. **`UreqHttp` on the wire** (`tests/ureq_wire.rs`): a one-request server on `127.0.0.1:0` in a
   thread, reading the raw request and writing a canned reply. It proves: the bearer arrives as
   `Authorization: Bearer <token>` and nowhere else in the request; the form is
   `application/x-www-form-urlencoded`; a JSON body is `application/json`; a 302 reply comes back as
   status 302 with no second request; a 404 comes back as a response, not an error; a body over
   1 MiB gives `GITHUB_UNREACHABLE`; a closed port gives `GITHUB_UNREACHABLE`; the user agent is
   `persona-builder/<VERSION>`.
8. `Endpoints::new` refuses `http://example.com`, `https://github.com/path`,
   `https://user@github.com`, `https://github.com:8443`, and accepts `http://127.0.0.1:4567` and
   `https://github.com/`.

## 4. `src/keychain.rs`: where the token lives

```rust
pub trait TokenStore {
    fn load(&self) -> Result<Option<Secret>>;
    fn save(&self, token: &Secret) -> Result<()>;
    fn forget(&self) -> Result<()>;
}
pub const ACCOUNT: &str = "persona-builder";
pub struct Keychain { service: String }
impl Keychain {
    pub fn github() -> Self;                               // service "persona-builder.github"
    pub fn with_service(service: &str) -> Result<Self>;    // 1–64 of a-z 0-9 . -
}
pub fn storable(token: &Secret) -> bool;                    // 1–1024 bytes of A-Za-z0-9._~:+/=-
pub fn store_line(service: &str, token: &Secret) -> Result<Secret>;
pub struct MemoryStore { /* Mutex<Option<String>> */ }      // for tests and previews
impl MemoryStore { pub fn new() -> Self }                   // and Default
```

- On macOS, `Keychain` runs `/usr/bin/security` with `env_clear()` (then only `HOME`), stderr to
  null:
  - `load`: `find-generic-password -s <service> -a persona-builder -w`, stdin null. Exit 0 → stdout
    with one trailing `\n` removed, as a `Secret`; exit 44 → `None`; else `KEYCHAIN_FAILED`.
  - `save`: refuse a token that is not `storable` (`KEYCHAIN_FAILED`); spawn `security -i` and write
    `store_line` to its stdin, never argv. `store_line` is
    `add-generic-password -U -s <service> -a persona-builder -w "<token>"\n`, built in one
    `String::with_capacity` of the exact size and returned as a `Secret`. Then `load` must return
    the same value, else `KEYCHAIN_FAILED`.
  - `forget`: `delete-generic-password -s <service> -a persona-builder`; exit 0 or 44 → `Ok`.
- Not on macOS: every method → `MAC_ONLY` (`#[cfg(target_os = "macos")]` split).
- `MemoryStore` returns a fresh `Secret` copy on `load`.

Seams under test (`tests/keychain.rs`): the exact `store_line` bytes for an invented token;
`storable` refuses empty, 1025 bytes, a space, `"`, `\`, a newline; `with_service` refuses
`Persona`, `a b`, 65 chars; `MemoryStore` round trip and forget. One `#[ignore]` test,
`live_keychain_round_trip`, uses `Keychain::with_service("persona-builder.test")`: save, load,
forget, load is `None`. Claude runs it by hand with `--ignored`.

## 5. `src/git.rs`: running git

```rust
pub struct Git { program: PathBuf }
impl Git {
    pub fn system() -> Self;                // "git", found on PATH
    pub fn at(program: PathBuf) -> Self;    // tests: a wrapper script
    pub fn check_version(&self) -> Result<()>;
    pub fn find_repo(&self, folder: &Path) -> Result<Option<FoundRepo>>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundRepo { pub root: PathBuf, pub git_dir: PathBuf, pub remotes: Vec<Remote> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote { pub name: String, pub push_urls: Vec<String> }
pub enum Env { User, Own }
```

- Every git child: `Command::new(&program)`, `env_clear()`, then `HOME` and `PATH` from the parent,
  `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, stdin null unless written. **`Env::User`** (a person's own
  repo) also passes `SSH_AUTH_SOCK` through when set, so their own keys work. **`Env::Own`** (the
  backup repo this crate made) adds `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null` and
  `-c credential.helper=` before the subcommand, so no config of the person's applies.
- `check_version`: `git --version`; not found → `NEEDS_GIT`; below 2.32 → `GIT_TOO_OLD`.
- `find_repo(folder)`: a folder that does not exist → `None`. `git -C <folder> rev-parse
  --show-toplevel --absolute-git-dir` (`Env::User`); exit non-zero → `None`. `remotes` are
  `git -C <root> remote` in its order, each with `git -C <root> remote get-url --push --all <name>`.
- A failing git step's error is `push_failed(<detail>)` for a push and `Error::Invalid("git <subcommand>
  failed")` otherwise, where detail is git's last non-empty stderr line, cut to 300 chars, with every
  occurrence of the token and of its base64 credential replaced by `[hidden]`.

### Committing a folder without touching anyone's work tree

```rust
pub struct Identity { pub name: String, pub email: String }
pub struct Committed { pub commit: String, pub changed: bool }
pub fn commit_folder(git: &Git, env: Env, git_dir: &Path, branch: &str, folder: &Path,
                     prefix: &str, who: &Identity, message: &str) -> Result<Committed>;
```

1. List `folder` recursively in sorted order: regular files only. A symlink anywhere →
   `links_in_folder(<path relative to folder>)`. Skip every file named `private.json` and
   `.DS_Store`, at any depth. A relative path that is not UTF-8 or holds a control character or a
   backslash → `Error::Invalid("unsupported file name")`.
2. A scratch folder made with mode 0700 under `std::env::temp_dir()`, removed on every path (a
   `Drop` guard); `GIT_INDEX_FILE=<scratch>/index` for steps 3–6.
3. `parent` = `git --git-dir=<git_dir> rev-parse --verify -q refs/heads/<branch>^{commit}` (none is
   fine). `read-tree <parent>` or `read-tree --empty`.
4. Blob ids from `hash-object -w --no-filters --stdin-paths` (absolute paths on stdin). One
   `update-index --add --index-info` on stdin: first `0 <40 zeros>\t<path>` for every path
   `ls-tree -r -z --name-only <parent> -- <prefix>/` lists, then `100644 <blob>\t<prefix>/<rel>` for
   every file. Files outside `<prefix>/` are kept as they were.
5. `write-tree`. If there is a parent and the tree equals `<parent>^{tree}` → return
   `Committed { commit: parent, changed: false }`.
6. `commit-tree <tree> [-p <parent>] -F -` with the message on stdin and `GIT_AUTHOR_NAME`,
   `GIT_AUTHOR_EMAIL`, `GIT_COMMITTER_NAME`, `GIT_COMMITTER_EMAIL` from `who`. Then
   `update-ref refs/heads/<branch> <new> <parent or "">`.

`branch` must be `main` or `persona/<slug>` with a slug of `a-z0-9-`; `prefix` must be `crew/<slug>`.
Anything else → `Error::Invalid("bad branch or folder")`.

### Pushing

```rust
pub fn push(git: &Git, env: Env, git_dir: &Path, remote: &str, branch: &str,
            token: Option<&Secret>) -> Result<()>;
```

`git --git-dir=<git_dir> push --porcelain <remote> refs/heads/<branch>:refs/heads/<branch>`. With
`token` (only ever passed with `Env::Own`), the child also gets `GIT_CONFIG_COUNT=1`,
`GIT_CONFIG_KEY_0=http.https://github.com/.extraheader`,
`GIT_CONFIG_VALUE_0=AUTHORIZATION: basic <base64("x-access-token:" + token)>`. Never a token in
argv, never a credential in a URL, never `--force`.

Seams under test (`tests/git.rs`): real `git` in scratch repos, through a wrapper `Git::at` script
that appends its argv and its environment's variable *names* to a log file (the log path written
literally into the script, since the env is cleared) and then `exec`s the real git (absolute path
found once with `command -v git`):

1. `find_repo` on a non-repo, a missing folder, a work tree with two remotes (order kept, a
   `pushInsteadOf` applied), a subfolder of a work tree (root is the top).
2. `commit_folder` into a bare repo: the first commit holds exactly the files (no `private.json`,
   no `.DS_Store`, nested folders kept); a second run with no change gives `changed: false` and the
   same commit; a run after deleting one file and editing another removes and updates them and keeps
   an unrelated `other/keep.md` that was already on the branch; a symlink is refused naming it; the
   person's work tree, index and `HEAD` in a non-bare repo are byte-for-byte unchanged (`git status
   --porcelain` and `rev-parse HEAD` before and after); the scratch folder is gone afterwards; the
   author and committer are `who`.
3. `push` to a `file://` bare remote: the branch arrives; with a token the env names include
   `GIT_CONFIG_VALUE_0` and no argv line contains the token; with `Env::User` and no token the env
   names hold no `GIT_CONFIG_*`; with `Env::Own` argv starts with `-c credential.helper=`; only
   names from the allowlist above ever reach the child (assert the logged names are a subset of
   it).
4. A push that fails (remote missing) gives `push_failed(...)` with the token replaced by `[hidden]`
   when git's stderr would hold it (use a wrapper that prints the token to stderr and exits 1).

## 6. `src/record.rs`: the choice, kept in the state folder

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
pub enum SaveRecord {
    Local,
    Existing { repo: PathBuf, remote: String, branch: String },
    Github { owner: String, name: String, branch: String },
}
pub fn read_record(state_dir: &Path) -> Result<Option<SaveRecord>>;
pub fn write_record(state_dir: &Path, record: &SaveRecord) -> Result<()>;
```

The file is `<state_dir>/save.json`, written as `{"version": 1, "mode": …}` with
`persona_builder::json::to_python_json`. A missing `version`, a version other than 1, an unknown
key or a bad value → `Error::Invalid("save.json: not a save record")`. The state folder: a symlink
is refused (`Error::Invalid("state folder is a link")`); missing → created with mode 0700 (parents
with `create_dir_all`, the last one by `DirBuilder` with mode 0700). Write: a temp file
`save.json.tmp-<pid>` opened `create_new` with mode 0600 and `O_NOFOLLOW`, written, synced, renamed
over `save.json`. Read: opened with `O_NOFOLLOW`, at most 64 KiB.

Tests (`tests/record.rs`): each variant round-trips and the bytes are pinned for one; modes 0600 and
0700; a symlinked `save.json` and a symlinked state folder are refused; version 2 and an unknown key
are refused.

## 7. `src/flow.rs`: options, choosing, saving

```rust
pub trait PrivacyCheck { fn hits(&self, folder: &Path) -> Result<Vec<String>>; }   // "file:line", never text
pub struct NotBuiltYet; // impl PrivacyCheck: always Err(Error::Invalid(PRIVACY_NOT_BUILT))
pub trait RemoteCheck { fn confirm(&self, token: &Secret, push_urls: &[String]) -> Result<()>; }
pub struct GithubRemotes<'a> { pub github: &'a Github<'a> }
// confirm: empty list → NOT_ON_GITHUB; every url: github_repo_of → None → NOT_ON_GITHUB; Some → confirm_private

pub struct MemberPaths { pub member: persona_builder::member::Member, pub dir: PathBuf, pub state_dir: PathBuf }
impl MemberPaths { pub fn load(member_json: &Path, home: &Path) -> Result<Self>; }
// read_member; dir = member.json's folder; state_dir = member.state_dir with a leading "~/" replaced by home

pub enum Choice { Existing { root: PathBuf, remote: String, label: String }, Github { label: String }, Local { label: String } }
pub struct SaveOptions { pub choices: Vec<Choice>, pub selected: usize, pub note: Option<&'static str> }
pub fn detect(git: &Git, home: &Path, member_dir: &Path) -> Result<Vec<FoundRepo>>;
pub fn options(found: &[FoundRepo], home: &Path) -> SaveOptions;

/// `github` is None when this build has no client id: every step that needs GitHub then fails
/// with NOT_SET_UP, after the steps before it (so `save_now` still runs the privacy check first).
pub struct Saver<'a> { pub git: &'a Git, pub github: Option<&'a Github<'a>>, pub tokens: &'a dyn TokenStore }
pub enum Saved { KeptLocal, Pushed { message: String } }
impl Saver<'_> {
    pub fn choose_local(&self, member: &MemberPaths) -> Result<()>;
    pub fn choose_existing(&self, member: &MemberPaths, home: &Path, root: &Path, remotes: &dyn RemoteCheck) -> Result<SaveRecord>;
    pub fn choose_github(&self, member: &MemberPaths, name: &str) -> Result<RepoRef>;
    pub fn save_now(&self, member: &MemberPaths, home: &Path, privacy: &dyn PrivacyCheck, remotes: &dyn RemoteCheck) -> Result<Saved>;
}
```

- `detect`: candidates `<home>/.claude`, then `member_dir`; `find_repo` each; drop duplicates by
  root; keep a repo only when it has a **usable remote**: `origin` if every one of its push URLs
  passes `github_repo_of`, else the first remote in order whose push URLs all pass. No network.
- `options`: found repos → one `Existing` per repo (label `choice_existing(shown)`, `shown` from
  `persona_builder::crew::shown_path(root, home)`, falling back to the absolute path), then `Local`;
  `selected` 0; `note` `None`. None found → `[Github, Local]`, `selected` 0, `note`
  `Some(NO_GITHUB_ACCOUNT)`.
- `choose_local`: writes `SaveRecord::Local`.
- `choose_existing`: `github` or `NOT_SET_UP`; `git.check_version()`; `detect(...)` must contain a repo whose root equals
  `root` canonicalized, else `NOT_OFFERED`; token from `tokens.load()` or `SIGN_IN_FIRST`;
  `github.account(token)`; `remotes.confirm(token, <that remote's push URLs>)`; write
  `Existing { repo: <root>, remote, branch: "persona/<slug>" }` and return it.
- `choose_github`: `github` or `NOT_SET_UP`; `valid_repo_name` or `BAD_REPO_NAME`; `check_version`; token or `SIGN_IN_FIRST`;
  `account`. If the record is already `Github`, `confirm_private` that repo and keep it (no second
  repo). Else `create_private_repo`. Then the backup repo `<state_dir>/backup.git`: if missing,
  `git init -q --bare` (`Env::Own`), and set `origin` to `https://github.com/<owner>/<name>.git`
  (`remote add`, or `remote set-url` when it exists). Write `Github { owner, name, branch: "main" }`.
- `save_now`, in this order (each step's refusal sends nothing):
  1. `read_record`; none → `CHOOSE_FIRST`; `Local` → `Saved::KeptLocal`.
  2. `privacy.hits(member.dir)`: an error is returned as is; any hits →
     `privacy_hits(<first 20 joined by ", ">[" and N more"]>)`.
  3. `github` or `NOT_SET_UP`; `check_version`; token or `SIGN_IN_FIRST`; `account` → `Identity { name: login, email:
     "<id>+<login>@users.noreply.github.com" }`.
  4. Existing: `find_repo(repo)` must still have that root and that remote, else `NOT_OFFERED`;
     `Env::User`; no token for the push. Github: `<state_dir>/backup.git` must exist, and
     `git remote get-url origin` must be exactly `https://github.com/<owner>/<name>.git`, else
     `repo_unconfirmed(full)`; `Env::Own`; the token for the push.
  5. `remotes.confirm(token, <git remote get-url --push --all <remote>>)`.
  6. `commit_folder(..., prefix "crew/<slug>", message "Save <member.name>")`.
  7. `push(...)` (also when `changed` is false: an earlier push may have failed).
  8. `Pushed { message }`: `saved_github(full)` or `saved_existing(shown, branch)`, plus
     `" " + UP_TO_DATE` when nothing changed.

Tests (`tests/flow.rs`), with the fake `Http`, `MemoryStore`, real git and `file://` remotes (an
`Existing` repo gets a GitHub `origin` plus a repo-local `pushInsteadOf` to the `file://` remote; the
test's `RemoteCheck` records the URLs it was asked about and approves; one test uses `GithubRemotes`
to show a `file://` push URL is refused with `NOT_ON_GITHUB`):

1. `detect` and `options` for: nothing found; `~/.claude` in a repo with a GitHub origin; a repo
   whose only remote is a local path (dropped); the same repo reached from both candidates (one
   entry).
2. `save_now` refuses in order: no record; `Local` → `KeptLocal` and no git or HTTP call at all;
   privacy hits (25 of them → 20 shown and "and 5 more"), with no HTTP request made; `NotBuiltYet`
   → `PRIVACY_NOT_BUILT` with no HTTP request; no token; a public repo (the check refuses) with
   nothing committed (branch absent afterwards).
3. A full existing-repo save: the `file://` remote receives `persona/wren` holding exactly
   `crew/wren/…` minus `private.json`, the person's branch and work tree untouched, the commit
   authored `rowan-vale <4242+rowan-vale@users.noreply.github.com>`; a second save says
   `UP_TO_DATE`.
4. A full GitHub-mode save: `choose_github` creates the repo through the fake (POST then GET) and the
   bare backup with the right `origin`; a second `choose_github` makes no POST; `save_now` pushes
   `main` (the test points `backup.git`'s `pushInsteadOf` at a `file://` remote after
   `choose_github`); an `origin` edited to another URL gives `repo_unconfirmed`.

## 8. The CLI: `persona-builder save …`

Add `persona-builder-save` to `builder/cli/Cargo.toml`. Subcommands under `save`, all using
`Git::system()`, `UreqHttp`, `Endpoints::github()`, `Keychain::github()`, `HOME` for home, and
printing the §1 texts:

- `save options <member.json>`: one line per choice, `1. <label>` with ` (recommended)` after the
  selected one, then the note on its own line when there is one.
- `save sign-in`: `client_id()` or fail with `NOT_SET_UP`. Print `sign_in_prompt`, then
  `SIGN_IN_WAITING`; `finish_sign_in` with `std::thread::sleep`; `account`; `Keychain::save`; print
  `signed_in(login)`.
- `save sign-out`: `Keychain::forget`; print `SIGNED_OUT`.
- `save choose <member.json> local | here <folder> | github <name>`: prints the result in one plain
  line (`Your assistant will be kept on this Mac only.`, `Your assistant will be saved in {shown}.`,
  `Your assistant will be saved to {owner/name} on GitHub, a private repo.` — add these three to
  `text.rs` as `CHOSE_LOCAL`, `chose_existing`, `chose_github`). Without a client id, `here` and
  `github` fail with `NOT_SET_UP` before anything else.
- `save push <member.json>`: `save_now` with `NotBuiltYet` (PB2 replaces it with the Rust privacy
  check) and `GithubRemotes` (built only when there is a client id; without one, `Saver.github` is
  `None` and a `RemoteCheck` that is never reached). Prints `KEPT_LOCAL` or the message.

Errors print as today (`persona-builder: <message>`, exit 1). CLI tests (`builder/cli/tests/save.rs`,
`HOME` set to a scratch folder; never `sign-in`, `sign-out` or a GitHub call): `save options` with
nothing found prints the GitHub and Mac choices and the note; `save choose … local` then
`save push …` prints `KEPT_LOCAL`; a `save.json` written as `Github` makes `save push` fail with
`PRIVACY_NOT_BUILT` (proving the privacy check runs before any sign-in or network);
`save choose … github x` fails with `NOT_SET_UP` while no client id is built in.
(PB6.1 has since built the client id in, so these now ask for sign-in: `plans/briefs/PB6.1-client-id.md`.)

## Done when

All the tests above went red on their assertion first and are green; `cargo fmt --check`, clippy
and `bash scripts/ci-local.sh` pass. Report: each file you added or changed, the red you saw for
each test file, and anything in this brief you could not do as written (stop and say so rather than
changing a rule).

## Delta after the second Verifier pass (2026-10-24)

One should-fix, test-first, nothing else changes:

- **A token only from a 200 reply.** `Github::finish_sign_in` (`builder/save/src/github.rs`) accepts
  status 200 or 400 from the poll, then reads `access_token` before `error` whatever the status. A
  `400 {"access_token":"gho_…"}` is therefore stored as the token. RFC 8628 §3.5 sends errors with
  400 and a token only on success, so: read `access_token` only when the reply's status is 200. A
  400 must carry `error` and goes through the existing `match`; a 400 with `access_token` and no
  `error` fails with `sign_in_failed("unexpected reply")`, and one with both is handled by its
  `error`. Leave 200 replies (GitHub sends `authorization_pending` with 200), `start_sign_in`, the
  slow-down and expiry rules exactly as they are.
- **Test** (in `builder/save/tests/verifier_fixes.rs`, beside `poll_400_denied_is_cancelled`, same
  helpers): `poll_400_with_token_is_unexpected`: start reply, then
  `reply(400, json!({"access_token": TOKEN, "token_type": "bearer", "scope": "repo"}))` (the file's fixture constant);
  the result's error string equals `text::sign_in_failed("unexpected reply")`, and
  `http.assert_drained()`. It must fail on its assertion before the fix (today it returns `Ok`).
