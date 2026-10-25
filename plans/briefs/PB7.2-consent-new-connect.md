# PB7.2 brief: your permission, a new assistant, connect Claude

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB7.2` entry (PB7 of the Persona Builder
plan, PB-D9) and V-Q5's answer: a new assistant's folder is `~/.claude/crew/<slug>/`, with a
*Show in Finder* button. Read the repo's `AGENTS.md` and `.ai/PROJECT.md` first, then
`plans/briefs/PB7.1-app-frame.md` (the frame and the bridge this slice grows),
`builder/core/src/crew.rs` (`slugify`, `check_name`, `new_member`, `copy_template`, `fill`,
`fill_json`), `builder/core/tests/install_parity.rs`, `builder/agent/src/claude.rs` (`find`,
`check`), `builder/agent/src/text.rs`, `builder/agent/src/ledger.rs` (`local_day`),
`builder/ingest/src/private.rs` (`write_private`, `read_private_optional`), `builder/api/`,
`builder/app/` and `builder/ui/`. Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security:
`~/.codex/skills/security-and-hardening/SKILL.md`; structure: `~/.codex/skills/frontend/SKILL.md` and
`~/.codex/skills/tabler-ui/SKILL.md`; looks: `~/.codex/skills/design-guide/SKILL.md`; every sentence a
person sees: `~/.codex/skills/plain-language/SKILL.md` (the sentences below are final: use them
word for word); review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true` and pnpm with `--offline`. **No new dependency** in any
`Cargo.toml` or `package.json` beyond the path dependencies named below. If you think one is needed,
stop and say so.

## What this slice does

A person opens the app, sees their assistants (or none), makes a new one, gives their permission
for it to read their history, and checks that their own Claude Code is installed and signed in.
The same four things work from the `persona-builder` command.

## Decisions (made, do not reopen)

### 1. The template inside the binary (`builder/core`)

The installed app has no copy of the kit, so the template's files are compiled in.

- New `builder/core/src/template.rs`, `pub mod template;` in `lib.rs`:

  ```rust
  /// `crew/_template/`, compiled in, without its top-level README.md (install.sh deletes it).
  /// Paths are relative to the template folder, `/`-separated. `tests/template_embedded.rs`
  /// checks this list against the folder, file for file and byte for byte.
  pub const FILES: &[(&str, &str)] = &[
      ("IDENTITY.md", include_str!("../../../crew/_template/IDENTITY.md")),
      // … every file under crew/_template/ but the top-level README.md, sorted by path,
      // the briefs/ files included ("briefs/backtest-brief.md", …)
  ];
  ```

- In `crew.rs`, the per-file rule moves into one private function used by both copy paths, so
  the folder copy and the embedded copy can't drift:
  `fn filled(relative: &Path, text: &str, values: &[(&str, &str)]) -> Result<String>`: the
  top-level `member.json` is filled with `fill_json` and written with `to_python_json` (as now);
  a `member.json` below the top level is returned unchanged (as now: byte for byte); every other
  file is `fill(text, values)`. The top-level README.md skip stays in `copy_template`.
- `new_member` keeps its signature and behaviour. Add
  `pub fn new_member_embedded(target: &Path, new: &NewMember<'_>) -> Result<Member>`: the same
  exists check, slugs, values, folder creation, read-back and remove-on-failure as `new_member`
  (factor the shared part out; don't copy it), writing each `template::FILES` entry (creating
  `briefs/` as needed) through `filled`.

### 2. Where assistants live (`builder/core`)

New `builder/core/src/assistants.rs`, `pub mod assistants;`:

- `pub fn crew_dir(home: &Path) -> PathBuf`: `home/.claude/crew`.
- `pub const SHOWN_REPO: &str = "~/.claude";` the `{{REPO}}` value for an assistant the app makes
  (so its files say `~/.claude/crew/<slug>/`).
- `pub fn assistant_dir(home: &Path, slug: &str) -> Result<PathBuf>`: refuses (`Error::Invalid`) a
  `slug` that is not exactly `slugify(slug)` (so only `a-z`, `0-9` and single inner `-`), else
  `crew_dir(home).join(slug)`. Every request from the webview goes through this.
- `pub struct Listed { pub dir: PathBuf, pub member: Member }` and
  `pub fn list(home: &Path) -> Result<Vec<Listed>>`: no `crew_dir` → empty. Otherwise each entry
  whose name is a valid slug (by the rule above; `_template`, `_fields` and dot-names are skipped)
  and holds a `member.json` that `read_member` accepts and whose `slug` equals the folder name, sorted by the
  member's `name` (case-insensitive), then slug. Folders that fail any of those are skipped, not
  errors. A link to a folder is followed (a person may link an assistant from a copy of the kit).
- `pub struct New<'a> { pub name: &'a str, pub pronouns: &'a str, pub principal: &'a str, pub principal_pronouns: &'a str }`
  and `pub fn make(home: &Path, new: &New<'_>, date: &str) -> Result<Listed>`: checks the four
  values with `crew::check_name` (make it `pub`) using the `what` words `"name"`, `"pronouns"`,
  `"your name"`, `"your pronouns"`; `slugify(name)` (none: `Error::Invalid`); refuses when
  `assistant_dir` already exists (`symlink_metadata`, so a dangling link counts) with
  `Error::Exists(slug)`: add that variant to `error.rs`; then `new_member_embedded` into it with
  `repo_shown: SHOWN_REPO` and `date`. Nothing is read from or written to the state folder here.

### 3. Your permission (`builder/core` wording, `builder/api` record)

New `builder/core/src/consent.rs`, `pub mod consent;`. The wording, final:

```rust
pub const VERSION: u32 = 1;
pub struct Section { pub heading: &'static str, pub lines: &'static [&'static str] }
pub const TITLE: &str = "Before Persona Builder reads anything";
pub const SECTIONS: &[Section] = &[
    Section { heading: "What it reads", lines: &[
        "The messages you typed to Claude Code on this computer, including your answers to its questions.",
        "Your claude.ai chats, only if you download them and add them yourself.",
    ] },
    Section { heading: "What stays on this computer", lines: &[
        "Your history, and everything Persona Builder makes from it, stays in your Claude folder on this computer.",
        "It leaves out messages that look private, such as health, family, legal or money matters and account numbers. It can miss some.",
    ] },
    Section { heading: "What goes to Claude", lines: &[
        "To build your assistant, Persona Builder sends parts of your history to Claude through your own Claude Code, signed in to your own Claude account. The terms of your Claude account apply, as they do to anything else you send through Claude Code.",
        "This uses your own Claude plan's usage, and a build can use a lot of it. Persona Builder stops for the day when it reaches a daily limit, and you can stop it at any time.",
    ] },
    Section { heading: "Nothing else", lines: &[
        "Persona Builder sends nothing anywhere else. A copy goes to GitHub only if you choose that later.",
        "You can change your mind: Forget deletes the history it read and everything it made from it.",
    ] },
];
/// SHA-1 (hex) of the wording as `plain_text()` gives it. A test fails when the words change and
/// this does not: change the words, bump VERSION, then update this.
pub const WORDING_SHA1: &str = "<computed by you>";
pub fn plain_text() -> String  // TITLE, a blank line, then each heading and its lines, a blank line between sections, ending "\n"
```

The record lives in the assistant's private state folder (`~/.claude/state/<slug>/`), never in its
crew folder: `consent.json`, `{"version":1,"agreed_on":"YYYY-MM-DD"}` (the local date, from
`persona_builder_agent::ledger::local_day(SystemTime::now())`), written with
`persona_builder_ingest::private::write_private(home, slug, "consent.json", …)` and read with
`read_private_optional`. It counts as agreed only when it parses with exactly those two keys,
`version == consent::VERSION` and a `YYYY-MM-DD` date; anything else (missing, damaged, an older
version) reads as not agreed, never as an error.

### 4. The api (`builder/api`)

`Cargo.toml` gains path dependencies `persona-builder` (`../core`) and `persona-builder-ingest`
(`../ingest`). New module `src/assistants.rs` (or `src/lib.rs` if it stays short) with the commands.
Each command has a testable form taking `home` (and `path` for Claude) and a zero-argument form the
Tauri app calls, reading `HOME` (missing: the `Failure` below) and `PATH` from the environment.

DTOs in `src/dto.rs` (answers as now; **requests** add `#[serde(deny_unknown_fields)]`):

```rust
pub struct Failure { pub message: String }            // every command's error; Serialize only is enough, but derive as the others
pub struct Assistant { pub slug: String, pub name: String, pub principal: String, pub consent: Consent }
pub struct Consent { pub agreed: bool, pub agreed_on: Option<String> }
pub struct ConsentText { pub version: u32, pub title: String, pub sections: Vec<ConsentSection> }
pub struct ConsentSection { pub heading: String, pub lines: Vec<String> }
pub struct NewAssistantRequest { pub name: String, pub pronouns: String, pub principal: String, pub principal_pronouns: String }
pub struct SlugRequest { pub slug: String }
pub struct ClaudeStatus { pub ready: bool, pub message: String, pub note: Option<String> }
```

Commands (`Result<T, dto::Failure>`; a `Failure`'s message is always one of the sentences below,
never an error's own text and never a path):

| Command | Does | Errors |
| --- | --- | --- |
| `list_assistants() -> Vec<Assistant>` | `assistants::list`, each with its consent | any error: "Persona Builder couldn't look for your assistants. Close it and open it again." |
| `new_assistant(NewAssistantRequest) -> Assistant` | `assistants::make` with today's local date; `consent` not agreed | `Invalid` from a name check: "<Field> can be up to 64 characters on one line, without @ < > { } [ ] ( ) # \\ or `.", Field being "Your assistant's name", "Its pronouns", "Your name", "Your pronouns"; no letter or digit in the name: "Your assistant's name needs at least one letter or number."; `Exists`: "You already have an assistant called <name>. Pick another name, or open that one.", <name> being the existing assistant's `name` when its `member.json` reads, else the slug; anything else: "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again." |
| `show_assistant_folder(SlugRequest) -> ()` | `assistant_dir`, must be an existing folder; opens it with the system's file browser: macOS `/usr/bin/open <dir>`, Linux `xdg-open <dir>`, Windows `explorer <dir>` (fixed program, the folder the only argument, `cfg(target_os)`), not waited on beyond spawn | bad slug or no folder: "Persona Builder couldn't find that assistant. Close it and open it again."; spawn failure: "Persona Builder couldn't open the folder." |
| `consent_text() -> ConsentText` | the wording from `consent` | none |
| `agree_consent(SlugRequest) -> Consent` | the slug must name a listed assistant; writes the record; returns it read back | bad slug or not listed: the "couldn't find that assistant" sentence; write failure: "Persona Builder couldn't keep your answer. Check there's space on this computer, then try again." |
| `claude_check() -> ClaudeStatus` | `claude::find(PATH, home)` then `check(home)` | never fails: ready gives `ready: true`, `message: text::ready(version)`, `note: Some(text::API_KEY_SET)` when the key is set; an `Error::Invalid` whose text is exactly `NOT_INSTALLED`, `DOES_NOT_RUN` or `SIGNED_OUT` gives `ready: false` and that text; anything else `ready: false` and `text::FAILED` |

The testable forms take what the environment would give: `list_assistants_in(home)`, `new_assistant_in(home, request)`, `agree_consent_in(home, request)`, `show_assistant_folder_in(home, request, open: impl FnOnce(&Path) -> std::io::Result<()>)` (the opener injected) and `claude_check_in(home, path: Option<&OsStr>, fallbacks: &[PathBuf])` (through `claude::find_in`; the zero-argument form passes `claude::fallbacks(home)`).

`src/parity.rs` gains one row per command, in this order (the same order everywhere below):
`app_info` (as now), `list_assistants` → `[["assistants"]]`, `new_assistant` → `[["new"]]`,
`show_assistant_folder` → `[["assistants"]]` (the CLI prints each folder; the app opens it),
`consent_text` → `[["consent", "show"]]`, `agree_consent` → `[["consent", "agree"]]`,
`claude_check` → `[["claude", "check"]]`.

### 5. The CLI (`builder/cli`)

- `persona-builder new <NAME> --principal <YOUR NAME> [--pronouns <P>] [--principal-pronouns <P>]`,
  both pronouns defaulting to `they/them` (as `install.sh`). Help: "Make a new assistant in your
  Claude folder". Calls `assistants::make` with today's local date, then prints
  `Made <name> in ~/.claude/crew/<slug>/.` and
  `Next: persona-builder consent show, then persona-builder consent agree ~/.claude/crew/<slug>`.
  `Exists`: `You already have an assistant in ~/.claude/crew/<slug>/.` and exit 1.
- `persona-builder assistants`: help "List your assistants". One line each:
  `<name> (<slug>): ~/.claude/crew/<slug>/ · permission given <date>` or
  `… · permission not given yet`; none: `You have no assistants yet. Make one with: persona-builder new <name> --principal <your name>`.
- `persona-builder consent show`: help "Show what you agree to before Persona Builder reads your
  history"; prints `consent::plain_text()` then `Version <n>.`
- `persona-builder consent agree <member_dir>`: help "Agree, for one assistant"; reads the member
  (`checked_member`, as other commands) and writes the record as the api does; prints
  `Thank you. Permission given on <date>.`
- `persona-builder claude check` exists; unchanged.

### 6. The Tauri app (`builder/app`)

- `COMMANDS` becomes the seven names in the order above. The capability lists the seven
  `allow-…` entries; `generate_handler!` the seven `commands::…`; the lockdown test already holds
  all four lists together.
- Each new command is `#[tauri::command(async)]` and one call into `persona_builder_api` (its
  zero-argument form, or with the request): `async` keeps the 30-second Claude check and the file
  work off the main thread (tauri-macros 2.7.3 `src/command/wrapper.rs`: without it a command runs
  on the main thread; `async` on a sync function runs it on the async runtime). Requests are taken
  as one argument named `request`.
- No new permission, plugin or capability: the folder is opened by Rust, not the webview.

### 7. The screens (`builder/ui`)

Generated types (cargo test writes them): commit every new file in `src/types/generated/`.

`Backend` gains, typed only with generated types:
`listAssistants(): Promise<Assistant[]>`, `newAssistant(request: NewAssistantRequest): Promise<Assistant>`,
`showAssistantFolder(request: SlugRequest): Promise<void>`, `consentText(): Promise<ConsentText>`,
`agreeConsent(request: SlugRequest): Promise<Consent>`, `claudeCheck(): Promise<ClaudeStatus>`.
`tauri-backend.ts` maps each to `invoke('<snake name>', { request })` or `invoke('<snake name>')`.

A shared helper `src/failure.ts`: `failureMessage(error: unknown, fallback: string): string`
returns `error.message` only when `error` is a plain object whose `message` is a string (the
`Failure` shape the bridge rejects with), else `fallback`. Every screen uses it; none shows
anything else from an error.

The frame (`PersonaBuilder.vue`) keeps the side column and gains a view state:
`view: 'home' | 'new' | 'consent' | 'connect'` and `assistant: Assistant | null`.

- **Side column.** When an assistant is chosen, its name as a small heading above the list
  ("Making <name>"). Each step item: `consent` and `connect` are buttons (Tabler `btn btn-link`
  styled as list text, with `aria-current="step"` on the current one) once an assistant is
  chosen; `connect` stays disabled (`aria-disabled="true"`, muted "Needs your permission first")
  until that assistant's consent is agreed. With no assistant chosen they show "Choose an
  assistant first", `aria-disabled="true"`. The five later steps stay "Not here yet",
  `aria-disabled="true"`.
- **home**: `Welcome` (unchanged) then `screens/Assistants.vue`.
- **new**: `screens/NewAssistant.vue`.
- **consent**: `screens/Consent.vue` with the chosen assistant.
- **connect**: `screens/ConnectClaude.vue`.
- A "Your assistants" link-button above the main column on every view but home returns to home
  (and reloads the list).

**`Assistants.vue`** (emits `open(assistant)` and `make`):
- loading: spinner, visually hidden "Looking for your assistants…";
- error: alert "Persona Builder couldn't look for your assistants. Close it and open it again." (via `failureMessage` with that fallback);
- heading "Your assistants";
- none: "You haven't made an assistant yet.";
- each assistant a Tabler list-group item: its name (strong), "Learns from <principal>", and a
  badge-free status line: "Permission given" or "Needs your permission"; buttons "Open" (primary
  outline; emits `open`) and "Show in Finder" (secondary outline; calls `showAssistantFolder`,
  and on failure shows the message in a small alert in that item);
- under the list, the primary button "Make a new one" (emits `make`).

Opening an assistant goes to `consent` when not agreed, else to `connect`.

**`NewAssistant.vue`** (emits `made(assistant)` and `cancel`):
- heading "Make a new assistant";
- "Your assistant's name" (text, required; help "Pick any name. You'll call it by this name.");
- "Its pronouns" (select: they/them (default), she/her, he/him, Other…; Other shows a text field
  "Its pronouns");
- "Your name" (text, required; help "Your assistant learns how you decide, from your own history.");
- "Your pronouns" (same select and Other field, labelled "Your pronouns");
- buttons "Make it" (primary; while waiting disabled and reading "Making it…") and "Back";
- on failure, an alert with `failureMessage(error, "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again.")`; the form keeps what was typed.
- Leading and trailing spaces are trimmed before sending; the button stays disabled while a
  required field is empty.

On `made`, the frame chooses that assistant and goes to `consent`.

**`Consent.vue`** (prop `assistant`; emits `agreed(consent)` and `next`):
- loads `consentText()`; loading spinner "Loading…"; error alert "Persona Builder couldn't show
  this. Close it and open it again.";
- heading the wording's `title`; each section an `h3` and its lines as paragraphs;
- when `assistant.consent.agreed`: a muted line "You agreed on <agreedOn>." and the primary
  button "Next: connect Claude" (emits `next`);
- else the primary button "I agree" (while waiting disabled, "Saving…"), calling
  `agreeConsent({ slug })`; on success emit `agreed` with the answer (the frame stores it on the
  assistant and goes to `connect`); on failure an alert with `failureMessage(error, "Persona
  Builder couldn't keep your answer. Check there's space on this computer, then try again.")`.

**`ConnectClaude.vue`**:
- heading "Connect Claude"; the line "Persona Builder works through your own Claude Code, signed
  in to your own Claude account. It never asks for your Claude password.";
- on mount calls `claudeCheck()`; loading spinner, visually hidden "Checking Claude Code…";
- `ready`: a success alert with `message`, and `note` below it in a muted line when present;
- not ready: a warning alert with `message`, and the button "Check again" (re-runs the check,
  showing the spinner); the alert's text is `message` only;
- a rejected call (the bridge itself failing): the warning alert with
  "Persona Builder couldn't check Claude Code. Close it and open it again." and "Check again".
- Links in `message` (for example the setup page) are plain text, not links: the webview opens
  nothing.

Tabler classes only; no custom colors; no Tabler JavaScript; light and dark from the theme.
`index.ts` also exports `Assistants`, `NewAssistant`, `Consent`, `ConnectClaude`.

### 8. Docs

- `.ai/PROJECT.md`: extend the `builder/core/` row with "the template compiled in, and where the
  app keeps assistants (`~/.claude/crew/<slug>/`)"; the `builder/api/` row's tests gain
  `builder/api/tests/assistants.rs`.
- `builder/ui/README.md`: one line: the screens so far (assistants, a new one, permission, connect Claude).

## Tests (write each first and watch it fail on its assertion)

Rust:
- `builder/core/tests/template_embedded.rs`: `template::FILES` paths equal the files under
  `crew/_template/` minus the top-level README.md (walk it), sorted, and each entry's text equals
  the file's bytes.
- `builder/core/tests/install_parity.rs`: in `parity_case`, also run `new_member_embedded` into a
  second target with the same inputs and `repo_shown: "~/projects/voice"`, and require the same
  file set and bytes as `install.sh`'s folder (reuse the comparison; one helper, two calls).
- `builder/core/src/assistants.rs` unit tests (temp home): `make` writes `~/.claude/crew/<slug>/`
  whose SKILL.md names `~/.claude/crew/<slug>/`; `make` refuses an existing folder and a dangling
  link with `Exists`, leaving them untouched; bad names refused per field; `assistant_dir` refuses
  `../x`, `A`, `a--b`, `-a`, `a/b`, `` and `_template`; `list` skips a folder with no
  `member.json`, a damaged one, one whose slug differs from its folder, `_template`, and returns
  the rest sorted; no crew folder gives an empty list.
- `builder/core/src/consent.rs` unit test: `sha1(plain_text()) == WORDING_SHA1`, and
  `plain_text()` starts with the title and holds every line.
- `builder/api/tests/assistants.rs` (temp home, no real `HOME`): list empty; `new_assistant` then
  `list_assistants` shows it with `consent.agreed == false`; each failure sentence above for its
  case (bad name per field, no letter, exists, bad slug on show and agree); `agree_consent` writes
  `~/.claude/state/<slug>/consent.json` (mode 0600, folder 0700) and the listing then shows it
  agreed with today's date; a record with `"version":0`, extra keys, or bad JSON reads as not
  agreed; `show_assistant_folder_in` passes the canonical assistant folder to the opener and maps
  an opener error to its sentence; `claude_check_in` with a stub `claude` on a temp `PATH` (reuse
  the agent crate's stub style): ready, signed out, not installed (empty `PATH`, a temp home with
  no fallbacks: inject the fallbacks if `find` needs it, through `find_in`); DTO JSON is camelCase
  and requests refuse an unknown field.
- `builder/cli/tests/assistants.rs` (temp `HOME`): `new` then `assistants` then `consent agree`
  then `assistants` print the lines above; `new` twice exits 1 with its line; `consent show`
  prints the wording and `Version 1.`.
- `builder/cli/tests/app_parity.rs` runs as it does, over the new rows.
- `builder/app/tests/lockdown.rs` passes with the seven commands.

Vitest:
- `Assistants.test.ts`: loading, error (a rejection's own text never shown), none, a list of two
  in the given order with their status lines; "Open" and "Make a new one" emit; "Show in Finder"
  calls the backend with the slug and shows a `Failure` message in the item.
- `NewAssistant.test.ts`: the button disabled until both names are filled; "Other…" shows the text
  field and sends its value; it sends the trimmed request; "Making it…" while waiting; a `Failure`
  message shown, a non-`Failure` rejection shows the fallback; the typed values stay.
- `Consent.test.ts`: the title, the four headings and every line from the backend's text; "I agree"
  calls `agreeConsent({ slug })` and emits `agreed`; already agreed shows "You agreed on …" and
  "Next: connect Claude"; failure shows its message.
- `ConnectClaude.test.ts`: ready shows message and note; not ready shows message and "Check again",
  which calls the check again; a rejection shows the bridge sentence; no `<a>` in the screen.
- `PersonaBuilder.test.ts` (updated): no assistant: steps show "Choose an assistant first" or "Not
  here yet", all `aria-disabled`; making one goes to the consent view with "Making <name>"; connect
  disabled until agreed; after agreeing the connect view shows; "Your assistants" returns home.
- `failure.test.ts`: the `Failure` shape returns its message; a string, an `Error`, `null` and an
  object with a non-string `message` return the fallback.
- `tauri-backend.test.ts`: each method invokes its snake-case command with `{ request }` where it
  takes one.

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). List each with its red result:
1. drop one entry from `template::FILES` → template_embedded red;
2. change one word in `consent::SECTIONS` without touching `WORDING_SHA1` → consent test red;
3. let `assistant_dir` accept `..` (skip the slug check) → assistants unit test red;
4. make `new_assistant` pass the core error's own text to `Failure` → api test red;
5. accept a consent record with an older version → api test red;
6. add a command to `COMMANDS` only → lockdown red;
7. make `failureMessage` return `String(error)` → failure test red;
8. enable the connect step before consent → PersonaBuilder test red.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), the generated types are
committed-ready (no diff after `cargo test`), and the mutation list is reported. Don't commit;
Claude reviews, commits and runs the app.

## Fix-up pass (decided, do not reopen)

Claude's review of the first build:

1. **One reader and one writer of the permission record.** `builder/cli/src/assistants.rs` copies
   the api's `read_consent` and `valid_date`. Make `persona-builder-api` a normal dependency of
   `builder/cli` (it stays a dev-dependency too only if a test needs it there; otherwise move it),
   and have `assistants` and `consent agree` call `persona_builder_api::read_consent` and
   `persona_builder_api::write_consent`. Delete the CLI's `agreed_on` and `valid_date`.
2. **The wording hash without python3.** `consent.rs`'s test computes the SHA-1 with
   `sha1_smol` (`sha1_smol = "=1.0.1"`, already in the workspace through `builder/ingest`) as a
   dev-dependency of `builder/core`, not by running python3.
3. **The folder opener is waited for.** In `show_assistant_folder`, run the opener with
   `.status()` instead of `.spawn()` so no finished child is left behind; ignore its exit code
   (Windows' `explorer` exits 1 even when it opened the folder) and fail only when it cannot be
   started. Say so in a one-line comment.
4. **Help for `consent`.** The `Consent` variant of `Command` gets the doc comment
   `/// Your permission for Persona Builder to read your history`.
5. **`tauri-backend.test.ts`:** the request-row test calls the method once, without the
   `if (method === 'newAssistant') … else …` whose branches are the same; type the call through
   one cast instead, and drop the comment above it.

Mutation-check 1 again (make `read_consent` accept an older version → the api test and the CLI
test both red) with `cp`/`cmp`. Run `CARGO_NET_OFFLINE=true cargo fmt --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test -p persona-builder -p persona-builder-api -p persona-builder-cli -p persona-builder-app --locked`,
and `pnpm -C builder/ui typecheck`, `test` (offline). Don't commit.
