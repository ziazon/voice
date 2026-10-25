# PB7.6a brief: save a copy, from the app

Written by Claude (lane V2), 2026-10-25. Spec: lane V's `PB7.6a` entry, split from PB7.6 (PB7 of the
Persona Builder plan, § Saving, PB-D7, PB-D11). Read the repo's `AGENTS.md` and `.ai/PROJECT.md`
first, then `plans/briefs/PB7.2-consent-new-connect.md` (the bridge's rules this slice follows:
DTOs, `Failure`, testable `_in` forms, the parity table, the lockdown test, the screens' style),
`plans/briefs/PB6-saving.md`, `builder/save/src/` (all of it: `flow.rs`, `github.rs`, `text.rs`,
`keychain.rs`, `record.rs`), `builder/save/tests/common/mod.rs` (`FakeHttp`, `reply`, the `pb6`
fixtures), `builder/cli/src/main.rs` (`save_*`, `FolderPrivacy`, `UnavailableRemotes`),
`builder/api/`, `builder/app/` and `builder/ui/`. Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security: `~/.codex/skills/security-and-hardening/SKILL.md`;
structure: `~/.codex/skills/frontend/SKILL.md` and `~/.codex/skills/tabler-ui/SKILL.md`; looks:
`~/.codex/skills/design-guide/SKILL.md`; every sentence a person sees:
`~/.codex/skills/plain-language/SKILL.md` (the sentences below are final: use them word for word);
review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true` and pnpm with `--offline`. **No new dependency** in any
`Cargo.toml` or `package.json` beyond the path dependency named below. If you think one is needed,
stop and say so.

## What this slice does

A person opens an assistant's last step, *Save*, and sees where it can be kept, the recommended
place already picked. If they need GitHub, they sign in from the app: it shows GitHub's code,
opens GitHub's page in their browser, and waits (they can cancel). They keep their choice, then
save, and see the privacy check's result. They can sign out. The `persona-builder save …`
commands already do all of this; the app gains the same.

## Facts this rests on (checked)

- GitHub's device flow: the app shows the user code and asks the person to enter it at
  `https://github.com/login/device`, the `verification_uri` GitHub returns
  (docs.github.com, *Authorizing OAuth apps* → *Device flow*, read 2026-10-25). So the screens show
  that address as fixed text and never take a URL from GitHub's reply.
- `Github::start_sign_in` keeps the device code private (`DeviceCode.device_code` is a `Secret`);
  it must never reach the webview.
- `flow::options` lists at most the repo around `~/.claude` and the one around the assistant's
  folder (deduplicated), then GitHub when none, then *this computer only*; `selected` is 0.
- An assistant the app made lives in `~/.claude/crew/<slug>/` (V-Q5), so the repo around
  `~/.claude`, when there is one, is the one offered.

## Decisions (made, do not reopen)

### 1. The save crate (`builder/save`)

- `github.rs`: add
  `pub fn finish_sign_in_until(&self, code: &DeviceCode, wait: &mut dyn FnMut(Duration) -> Result<()>) -> Result<Secret>`:
  today's loop, with `wait(Duration::from_secs(interval))?` in place of `sleep(…)`, so a wait
  that returns an error stops it. `finish_sign_in` keeps its signature and becomes a one-line call
  of `finish_sign_in_until` with a `wait` that sleeps and returns `Ok(())`. Every existing test
  stays as it is.
- `text.rs`: `pub fn is_said(message: &str) -> bool`: true only for the crate's own sentences a
  person may see as a failure. Exactly equal to one of `NOT_SET_UP`, `SIGN_IN_CANCELLED`,
  `SIGN_IN_EXPIRED`, `SIGN_IN_FIRST`, `NO_REPO_PERMISSION`, `NOT_ON_GITHUB`, `BAD_REPO_NAME`,
  `NOT_OFFERED`, `CHOOSE_FIRST`, `NEEDS_GIT`, `GIT_TOO_OLD`, `NO_SAFE_STORE`, `KEYCHAIN_FAILED`,
  `GITHUB_UNREACHABLE`; or the output of `sign_in_failed`, `repo_public`, `repo_unconfirmed`,
  `repo_name_taken`, `privacy_hits`, `links_in_folder`, `push_failed`, `branch_in_use`. For the
  formatted ones, give each a private `(prefix, suffix)` constant pair that its formatter uses
  (`format!("{PREFIX}{arg}{SUFFIX}")`, so the output is byte for byte what it is today), and
  `is_said` accepts a message that starts with the prefix, ends with the suffix, is longer than
  both together and has no line break. The formatters' output must not change (the existing
  text tests prove it).

### 2. The api (`builder/api`)

`Cargo.toml` gains the path dependency `persona-builder-save` (`../save`). New module
`src/save.rs`, `pub mod save; pub use save::*;` in `lib.rs`.

**Move, don't copy:** `FolderPrivacy` (the `flow::PrivacyCheck` the CLI uses) and
`UnavailableRemotes` move from `builder/cli/src/main.rs` into `src/save.rs` as `pub` items; the
CLI uses them from there and its copies are deleted.

DTOs in `src/dto.rs` (answers as now; **requests** add `#[serde(deny_unknown_fields)]`):

```rust
pub struct SaveOptions {
    pub choices: Vec<SaveOption>,
    pub selected: u32,                  // flow::options' `selected`
    pub note: Option<String>,           // flow::options' note: today only text::NO_GITHUB_ACCOUNT
    pub chosen: Option<String>,         // the saved choice as a sentence, below
    pub signed_in: bool,                // a GitHub sign-in is kept on this computer
    pub suggested_name: String,         // github::suggested_repo_name(slug)
}
pub struct SaveOption { pub kind: SaveKind, pub label: String }
pub enum SaveKind { Existing, Github, Local }            // serde and ts: "existing" | "github" | "local"
pub struct SaveChoiceRequest { pub slug: String, pub choice: u32, pub name: Option<String> }
pub struct LinkRequest { pub link: Link }
pub enum Link { GithubSignup, GithubDevice }             // serde and ts: "githubSignup" | "githubDevice"
pub struct SignInCode { pub user_code: String }
pub struct Said { pub message: String }
```

`chosen` from `record::read_record(state_dir)`: `Local` → `text::CHOSE_LOCAL`; `Existing { repo, .. }`
→ `text::chose_existing(&flow::shown(&repo, home))`; `Github { owner, name, .. }` →
`text::chose_github("<owner>/<name>")`; none, or an error reading it → `None`.
`signed_in`: `tokens.load()` is `Ok(Some(_))`; anything else is `false` (never an error).

The fixed links, in Rust only:

```rust
const SIGNUP_PAGE: &str = "https://github.com/signup";
const DEVICE_PAGE: &str = "https://github.com/login/device";
```

opened with the system's opener: macOS `/usr/bin/open <url>`, Linux `xdg-open <url>`, Windows
`explorer <url>` (fixed program, the URL its only argument, `cfg(target_os)`), run with
`.status()` and its exit code ignored, as `show_assistant_folder` does. Factor the argv into
`link_opener_args(url) -> (&'static str, Vec<OsString>)` with a macOS unit test.

**The sign-in in progress.** One at a time, kept in Rust:

```rust
pub struct SignIns { state: Mutex<SignInState> }
struct SignInState { code: Option<DeviceCode>, cancel: Arc<AtomicBool> }
impl SignIns { pub const fn new() -> Self }   // or `new()` plus a `static` through `OnceLock`
pub fn sign_ins() -> &'static SignIns        // the one the zero-argument commands use
```

- start: sets the current `cancel` flag (an older wait stops), calls `start_sign_in`, then stores
  the new code with a fresh flag. Answers `SignInCode { user_code }` only.
- finish: takes the code out (none: the `START` failure), keeps a clone of its flag, releases the
  lock, then `finish_sign_in_until` with a `wait` that sleeps in steps of at most 250 ms through
  the injected `sleep` and returns `Err(Error::Invalid(text::SIGN_IN_CANCELLED.into()))` as soon as
  the flag is set (checked before each step and after the last). On a token: `github.account(&token)`
  (it refuses a token without the `repo` scope), then `tokens.save(&token)`, then
  `Said { message: text::signed_in(&account.login) }`. The token is never in an answer, a log or
  a `Failure`.
- cancel: sets the flag and drops any waiting code. Never fails.

Commands (`Result<T, dto::Failure>`). A `Failure`'s message is either a save-crate sentence that
`text::is_said` accepts (an `Error::Invalid` whose text passes) or the command's own fallback
below; never another error's text, never a path. Every command that takes a `slug` first finds it
among `assistants::list(home)` (as `agree_consent_in` does; not listed, or a bad slug: the `FIND`
sentence), then `flow::MemberPaths::load(&listed.dir.join("member.json"), home)`.

| Command | Does | Fallback |
| --- | --- | --- |
| `save_options(SlugRequest) -> SaveOptions` | `flow::detect` with `Git::system()`, then `flow::options` | `OPTIONS` |
| `open_link(LinkRequest) -> ()` | opens the fixed page for `link` | `LINK` for that page |
| `github_sign_in_start() -> SignInCode` | as above; no client ID gives `text::NOT_SET_UP` | `SIGN_IN` |
| `github_sign_in_finish() -> Said` | as above | `SIGN_IN` |
| `github_sign_in_cancel() -> ()` | as above | none |
| `github_sign_out() -> Said` | `tokens.forget()`, then `text::SIGNED_OUT` | `SIGN_OUT` |
| `save_choose(SaveChoiceRequest) -> Said` | recomputes `save_options`' choices and takes `choices[choice]` (out of range: `CHANGED`): `Local` → `Saver::choose_local` and `text::CHOSE_LOCAL`; `Existing { root, .. }` → `choose_existing` with that `root` and `GithubRemotes`, and `text::chose_existing(shown)`; `Github` → `choose_github` with `name` (missing: `text::BAD_REPO_NAME`), and `text::chose_github(full)`. `name` is ignored for the other two | `CHOOSE` |
| `save_push(SlugRequest) -> Said` | `Saver::save_now` with `FolderPrivacy` and `GithubRemotes` (no client ID: `UnavailableRemotes`, as the CLI); `KeptLocal` → `text::KEPT_LOCAL`, `Pushed { message }` → `message` | `SAVE` |

The api's own sentences, final:

```rust
const OPTIONS: &str = "Persona Builder couldn't check where your assistant can be kept. Close it and open it again.";
const CHANGED: &str = "The places to keep your assistant have changed. Look at them again, then choose.";
const SIGN_IN: &str = "GitHub sign-in didn't work. Try again later, or keep your assistant on this computer only.";
const START: &str = "Start signing in to GitHub again.";
const SIGN_OUT: &str = "Persona Builder couldn't sign you out of GitHub. Try again.";
const CHOOSE: &str = "Persona Builder couldn't keep your choice. Close it and open it again, then try again.";
const SAVE: &str = "Persona Builder couldn't save your assistant. Try again, or keep it on this computer only.";
// LINK, one per page:
"Persona Builder couldn't open your web browser. Go to https://github.com/signup yourself."
"Persona Builder couldn't open your web browser. Go to https://github.com/login/device yourself."
```

Testable forms take what the environment would give, with no `HOME`, keychain, network or real
browser: `save_options_in(home, request, git: &Git, tokens: &dyn TokenStore)`,
`open_link_in(request, open: impl FnOnce(&str) -> std::io::Result<()>)`,
`github_sign_in_start_in(sign_ins: &SignIns, github: Option<&Github<'_>>)`,
`github_sign_in_finish_in(sign_ins, github: Option<&Github<'_>>, tokens, sleep: &mut dyn FnMut(Duration))`,
`github_sign_in_cancel_in(sign_ins)`, `github_sign_out_in(tokens)`,
`save_choose_in(home, request, git, github: Option<&Github<'_>>, tokens)`,
`save_push_in(home, request, git, github: Option<&Github<'_>>, tokens)`.
The zero-argument forms read `HOME` (missing: that command's fallback), use `Git::system()`,
`Keychain::github()`, `UreqHttp::default()`, `Endpoints::github()` and `github::client_id()` (none:
`github` is `None`), `sign_ins()` and `std::thread::sleep`.

`src/parity.rs` gains, after `claude_check`, in this order: `save_options` → `[["save", "options"]]`,
`open_link` → `[["save", "options"], ["save", "sign-in"]]` (the CLI prints both pages),
`github_sign_in_start`, `github_sign_in_finish` and `github_sign_in_cancel` → `[["save", "sign-in"]]`,
`github_sign_out` → `[["save", "sign-out"]]`, `save_choose` → `[["save", "choose"]]`,
`save_push` → `[["save", "push"]]`.

### 3. The CLI (`builder/cli`)

Only the move in §2: `save_push` uses `persona_builder_api::FolderPrivacy` and
`persona_builder_api::UnavailableRemotes`. Every `persona-builder save …` output stays byte for byte
(the existing `save.rs` and `save_help.rs` tests prove it).

### 4. The Tauri app (`builder/app`)

`COMMANDS`, the capability's `allow-…` list, `generate_handler!` and the parity table gain the
eight names in the order above (15 in all). Each is `#[tauri::command(async)]` and one call into
`persona_builder_api`'s zero-argument form, taking a request as one argument named `request`. No
new permission, plugin or capability: Rust opens the browser, not the webview.

### 5. The screens (`builder/ui`)

Commit every new file in `src/types/generated/` (cargo test writes them).

`Backend` gains, typed only with generated types: `saveOptions(request: SlugRequest): Promise<SaveOptions>`,
`openLink(request: LinkRequest): Promise<void>`, `githubSignInStart(): Promise<SignInCode>`,
`githubSignInFinish(): Promise<Said>`, `githubSignInCancel(): Promise<void>`,
`githubSignOut(): Promise<Said>`, `saveChoose(request: SaveChoiceRequest): Promise<Said>`,
`savePush(request: SlugRequest): Promise<Said>`. `tauri-backend.ts` maps each to
`invoke('<snake name>', { request })` or `invoke('<snake name>')`.

**The frame (`PersonaBuilder.vue`).** `view` gains `'save'`. The `save` step becomes a button like
`consent` and `connect`, enabled as soon as an assistant is chosen (saving reads no history, so
it does not wait for permission); with no assistant it shows "Choose an assistant first",
`aria-disabled="true"`. Steps three to six stay "Not here yet". The save view shows
`screens/Save.vue` with the chosen assistant, keyed by its slug.

**`Save.vue`** (prop `assistant`). One Tabler card:

- heading "Keep a copy"; the line "Persona Builder can keep a copy of <name> somewhere safe, so you
  don't lose it if something happens to this computer.";
- on mount `saveOptions({ slug })`: spinner, visually hidden "Looking for places to keep <name>…";
  on failure a warning alert with `failureMessage(error, OPTIONS sentence)` and "Try again";
- when `chosen` is set, a muted line "Right now: <chosen>";
- the choices as radio buttons (`form-check`), labelled by `label`, the one at `selected` checked
  at first and followed by a muted " (recommended)";
- when `note` is set: the note as a paragraph, then the secondary outline button "Make a free
  GitHub account" (`openLink({ link: 'githubSignup' })`; on failure a small alert with the
  `Failure` message, fallback the signup `LINK` sentence);
- when the checked choice is `github`: a text field "Name for the new private repo", filled with
  `suggestedName`, trimmed before sending;
- when the checked choice is `existing` or `github` and `signedIn` is false: the sign-in panel
  below, and "Keep this choice" stays disabled;
- the primary button "Keep this choice" (while waiting disabled, "Keeping it…"):
  `saveChoose({ slug, choice: <checked index>, name: <the field, github only, else null> })`;
  success shows a success alert with `message` and reloads the options; failure a warning alert
  with `failureMessage(error, CHOOSE sentence)`;
- when `chosen` is set: the primary outline button "Save now" (while waiting disabled,
  "Saving…"): `savePush({ slug })`; success alert with `message`; failure a warning alert with
  `failureMessage(error, SAVE sentence)` (the privacy check's result arrives this way);
- when `signedIn`: a muted line "You're signed in to GitHub on this computer." and the link-style
  button "Sign out of GitHub": `githubSignOut()`, its `message` in an info alert, then `signedIn`
  false (reload the options).

**The sign-in panel** (inside `Save.vue`, or `screens/GithubSignIn.vue` emitting `signed-in`):

- idle: the line "To save to GitHub, sign in to your GitHub account first." and the primary
  button "Sign in to GitHub";
- the button calls `githubSignInStart()`, then at once `githubSignInFinish()`;
- waiting: "Go to github.com/login/device and enter this code:", the code in large monospace
  text (`data-code`), the secondary button "Open GitHub's page" (`openLink({ link: 'githubDevice' })`,
  failure shown as for the signup link with the device `LINK` fallback), a spinner with
  "Waiting for you to approve Persona Builder on GitHub…", and the link-style button "Cancel"
  (`githubSignInCancel()`, then back to idle; the finish call's rejection that follows is not
  shown);
- success: a success alert with `message`, then `signedIn` true (reload the options);
- failure (start or finish): a warning alert with `failureMessage(error, SIGN_IN sentence)` and
  the idle button again;
- leaving the screen while waiting calls `githubSignInCancel()` (`onBeforeUnmount`).

Addresses on screen are plain text, never `<a>`: the webview opens nothing. Tabler classes only;
no custom colors; no Tabler JavaScript; light and dark from the theme. `index.ts` also exports
`Save` (and `GithubSignIn` if you make it).

### 6. Docs

- `.ai/PROJECT.md`: the `builder/api/` row's tests gain `builder/api/tests/save.rs`.
- `builder/ui/README.md`: the screens line gains "save a copy".

## Tests (write each first and watch it fail on its assertion)

Rust:
- `builder/save/tests/text.rs` (extend): `is_said` is true for every constant listed in §1 and for
  each formatter's output with a sample argument; false for `"git push failed"`,
  `"state folder is a link"`, `"save.json: not a save record"`, `"bad branch or folder"`,
  `"unsupported file name"`, `"not a GitHub address"`, a listed sentence with `"\n"` appended, a
  bare prefix, and `""`.
- `builder/save/tests/verifier_fixes.rs` or `github.rs` (extend): `finish_sign_in_until` stops with
  the `wait`'s error before a second request when the `wait` errors after the first pending reply
  (the fake holds no more replies), and returns the token like `finish_sign_in` otherwise.
- `builder/api/tests/save.rs` (temp home, `FakeHttp` written for this file in the style of
  `builder/save/tests/common`, `MemoryStore`, the real `git` found with `command -v git` as the
  `pb6` fixtures do; no real `HOME`, keychain, network or browser):
  - options for an assistant made with `assistants::make`: with no repo anywhere, GitHub then local,
    `selected` 0, the note, `chosen` none, `signedIn` false, `suggestedName`; with `~/.claude` a git
    repo whose `origin` is a GitHub URL, the existing choice first and no note; after
    `save_choose_in` local, `chosen` is `CHOSE_LOCAL`; a token in the store gives `signedIn` true;
  - every command with an unlisted slug, `../x` and `""` answers the `FIND` sentence;
  - choose: local writes `save.json` and answers `CHOSE_LOCAL`; an index past the end answers
    `CHANGED`; github without a name answers `BAD_REPO_NAME`; github with no sign-in answers
    `SIGN_IN_FIRST`; github with no client ID (`None`) answers `NOT_SET_UP`;
  - push: with no choice answers `CHOOSE_FIRST`; local answers `KEPT_LOCAL`; a folder holding a
    private-looking line answers the `privacy_hits` sentence;
  - a non-said `Error::Invalid` gives the fallback, never its text: a damaged `save.json` in the
    state folder (`"save.json: not a save record"`) makes push answer `SAVE`;
  - sign-in: start answers the fake's user code and nothing else (serialize the answer: no
    `deviceCode`, no token); finish with the fake answering pending then a token, then the account
    with the `repo` scope, saves the token in the store and answers `signed_in(login)`; finish
    with nothing started answers `START`; a cancel from inside the injected `sleep` makes finish
    answer `SIGN_IN_CANCELLED` and makes no further request; a second start cancels a finish
    still holding the first code; a token without the `repo` scope is not saved and answers
    `NO_REPO_PERMISSION`;
  - sign-out empties the store and answers `SIGNED_OUT`;
  - `open_link_in` passes exactly `SIGNUP_PAGE` or `DEVICE_PAGE` to the opener and maps an opener
    error to that page's `LINK` sentence; a request `{"link":"https://example.com"}` and one with
    an extra key do not deserialize;
  - DTO JSON is camelCase, `SaveKind` and `Link` serialize as the strings above, and each request
    refuses an unknown field.
- `builder/api/src/save.rs` unit test (macOS): `link_opener_args` is `("/usr/bin/open", [url])`.
- `builder/cli/tests/save.rs` and `save_help.rs` pass unchanged.
- `builder/cli/tests/app_parity.rs` runs over the new rows; `builder/app/tests/lockdown.rs` passes
  with the 15 commands.

Vitest:
- `Save.test.ts`: loading, then the choices with the recommended one checked and marked; the note
  and "Make a free GitHub account" calling `openLink({ link: 'githubSignup' })`; the repo field
  only for github, filled with the suggestion, trimmed in the request; "Keep this choice" disabled
  for github or existing while signed out and the sign-in panel shown; it sends
  `{ slug, choice, name }` and shows the answer; "Save now" only once `chosen` is set, shows a
  success or a `Failure` message, and a non-`Failure` rejection shows the fallback; sign-out shows
  its message; the options failure shows its sentence, never a rejection's own text.
- sign-in tests (in `Save.test.ts` or `GithubSignIn.test.ts`): start shows the code and the fixed
  address text; "Open GitHub's page" calls `openLink({ link: 'githubDevice' })`; finish resolving
  shows its message and the panel goes; finish rejecting shows the `Failure` message; "Cancel"
  calls `githubSignInCancel` and the finish rejection that follows shows nothing; unmounting while
  waiting calls `githubSignInCancel`; no `<a>` anywhere in the screen.
- `PersonaBuilder.test.ts` (updated): with no assistant, the save step is disabled with "Choose an
  assistant first"; with one chosen (permission not given), save is enabled and opens the save
  view; steps three to six stay "Not here yet".
- `tauri-backend.test.ts`: each new method invokes its snake-case command, with `{ request }` where
  it takes one.

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). List each with its red result:
1. make `is_said` return true for any message → the text test and the api fallback test red;
2. put the device code in `SignInCode` (add a field from `DeviceCode`) → the api start test red;
3. stop checking the cancel flag after the last sleep step → the cancel test red;
4. let `save_choose` take a choice past the end as the last one → the `CHANGED` test red;
5. skip the listed-slug check in `save_options` → the `FIND` test red;
6. open GitHub's `verification_uri` instead of `DEVICE_PAGE` (or any URL from a request) → the
   `open_link_in` test red;
7. drop one new name from the capability only → lockdown red;
8. enable the save step with no assistant chosen → PersonaBuilder test red.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), the generated types are
committed-ready (no diff after `cargo test`), and the mutation list is reported. Don't commit;
Claude reviews, commits and runs the app.

## Fix-up pass (decided, do not reopen)

Claude's review of the Rust half:

1. **One lookup, one set of sentences.** `builder/api/src/save.rs` copies `FIND`, `failure` and
   `home` from `assistants.rs`, and its `member()` repeats the listed-slug lookup that
   `agree_consent_in` and `show_assistant_folder_in` each write out. In `assistants.rs` make
   `FIND`, `failure` and `home` `pub(crate)`, and add
   `pub(crate) fn listed(home: &Path, slug: &str) -> Result<assistants::Listed>` (the
   `assistant_dir` check, then the entry of `assistants::list` with that slug; any failure is
   `FIND`). `agree_consent_in`, `show_assistant_folder_in` and `save.rs`'s `member()` use it; delete
   the copies. Every answer stays the same (the existing api tests prove it).
2. **A bad client ID says so.** In `with_github`, a `Github::new` error goes through `map_error`
   (so `text::NOT_SET_UP` reaches the screen), not straight to the fallback.

## Fix-up pass 2 (decided, do not reopen)

Claude's review of the screens:

1. **A reload keeps what the person picked.** `Save.vue`'s `load()` sets `choice` to `selected`
   and `name` to `suggestedName` only on the first load. On a reload (after keeping a choice,
   signing in or signing out) `choice` stays as it is while it is still a valid index, else
   falls back to `selected`; `name` stays as typed. Test: pick *this computer only*, keep it,
   and after the reload that radio is still checked.
2. **No empty repo name.** With the github choice checked, "Keep this choice" is disabled while
   the trimmed name is empty. Test it.
