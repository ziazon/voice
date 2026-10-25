# PB7.6b brief: turn it on, the claude.ai files, forget, from the app

Written by Claude (lane V2), 2026-10-25. Spec: lane V's `PB7.6b` entry, split from PB7.6 (PB7 of the
Persona Builder plan). Read the repo's `AGENTS.md` and `.ai/PROJECT.md` first, then
`plans/briefs/PB7.6a-save.md` (the slice before this one: the bridge's rules this slice follows,
DTOs, `Failure`, testable `_in` forms, the parity table, the lockdown test, the screens' style),
`plans/briefs/PB5-turn-on-off.md` (what `on`, `off`, `status`, `pack` and `forget` do), then
`builder/core/src/{install,install_record,pack,forget,assistants,member}.rs`,
`builder/ingest/src/private.rs` (`lock_state`, `private_folder`, `private_folder_exists`),
`builder/cli/src/main.rs` (`places`, `forget`, `pack`, `turn_on`, `turn_off`, `status`),
`builder/api/` (all of it, `tests/` included), `builder/app/` and `builder/ui/`. Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security: `~/.codex/skills/security-and-hardening/SKILL.md`;
structure: `~/.codex/skills/frontend/SKILL.md` and `~/.codex/skills/tabler-ui/SKILL.md`; looks:
`~/.codex/skills/design-guide/SKILL.md`; every sentence a person sees:
`~/.codex/skills/plain-language/SKILL.md` (the sentences below are final: use them word for word);
review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true` and pnpm with `--offline`. **No new dependency** in any
`Cargo.toml` or `package.json`. If you think one is needed, stop and say so. Public repo: invented
names only (the assistant **Ari** for **Robin**, a second assistant **Mo Lee**); no real person,
no fixed home path; every test works under a scratch folder and never touches the real `~/.claude`.

## What this slice does

The *Save* step gains three more cards under *Keep a copy*:

1. **Use it in Claude Code.** Whether this assistant (or another one) is on in Claude Code, and a
   button to turn it on, update it, or turn whichever one is on off. One assistant on at a time.
2. **Use it in claude.ai.** Make the claude.ai Project files in the assistant's private folder
   (`~/.claude/state/<slug>/claude-ai/`), list them, show them in Finder, and say how to add them
   to a Project.
3. **Forget.** Behind a confirmation that says what goes and what stays, delete the assistant's
   private folder: the history, the working files (the claude.ai files with them), the permission
   and the saving choice. The assistant's own folder and any copy on GitHub stay.

`persona-builder on|off|status|pack|forget` already do the work; the app gains the same.

## Facts this rests on (checked in the code)

- `install::turn_on`, `turn_off` and `status` take `Places { home, claude_home }`. `status` and
  `turn_on` fail when `claude_home` is not a folder. `status` is `None` with no record. The record
  (`install_record`) checks its slug with `block::validate_id` but **not its name**: a hand-edited
  `installed.json` can carry any name.
- `turn_on` of the same slug turns that one off first and refreshes; of a different slug it
  refuses. It rolls back on any error.
- `pack(member_dir, out)` refuses an `out` that exists and is not an empty folder, and refuses a
  member with no kinds of question with
  `"{name} has no kinds of question yet. Build {name} first, then make the pack"`. The template's
  `decisions.md` has none, so a new assistant the app made gets that refusal.
- `forget(member_dir, home, delete)` counts (and with `delete`, removes) the member's state folder,
  which must be exactly `~/.claude/state/<slug>`. That folder holds `consent.json`
  (`read_consent`), `save.json` (`persona_builder_save::record`) and everything else Persona
  Builder keeps for the assistant.
- `lock_state(home, slug)` takes an exclusive `flock` on the state folder (creating it 0700) and
  refuses with a fixed sentence when another build holds it.
- The CLI's `places()` picks `claude_home` as `$CLAUDE_CONFIG_DIR` when set and not empty, else
  `$HOME/.claude`. The assistants themselves always live in `$HOME/.claude/crew/` (V-Q5).

## Decisions (made, do not reopen)

### 1. Core and ingest: name two sentences so the api can recognise them

- `builder/core/src/pack.rs`: `pub fn no_kinds_yet(name: &str) -> String` returning exactly
  today's text; `pack` uses it. Output unchanged (the existing pack tests prove it).
- `builder/ingest/src/private.rs`: `pub const BUSY: &str` holding exactly today's `lock_state`
  refusal text; `lock_state` uses it. Output unchanged.

### 2. The api (`builder/api`)

New module `src/use_it.rs` (doc comment: "The save step's other sections: Claude Code on and off,
the claude.ai files, forget."), `pub mod use_it; pub use use_it::*;` in `lib.rs`.

**Move, don't copy:** add
`pub fn claude_home(home: &Path, config_dir: Option<&OsStr>) -> PathBuf` (a `config_dir` that is
`Some` and not empty, else `home.join(".claude")`). The CLI's `places()` uses it for its default
(its `--claude-home` flag still wins); its own copy of the rule is deleted. Make `opener_args` in
`assistants.rs` `pub(crate)` and reuse it.

DTOs in `src/dto.rs` (answers as now; **requests** add `deny_unknown_fields`):

```rust
pub struct ClaudeCodeStatus {
    pub state: OnState,
    pub on_name: Option<String>,   // the name of whoever is on (This or Other), shown safely, below
    pub on_slug: Option<String>,   // the record's slug (This or Other); sent back by turn_off
    pub out_of_date: bool,         // This only: status' out_of_date
    pub changed: bool,             // This only: the CLAUDE.md line is missing, or a recorded file is not Present
}
pub enum OnState { NotSetUp, Off, This, Other }   // serde and ts: "notSetUp" | "off" | "this" | "other"
pub struct TurnOffRequest { pub slug: String }    // the on_slug the person saw
pub struct ClaudeAiFiles { pub files: Vec<String> }
pub struct ForgetPreview { pub files: u32 }       // 0: nothing to forget
pub struct ForgetRequest { pub slug: String, pub files: u32 }  // the count the person confirmed
```

**This or Other.** A record is **This** only when its slug equals the request's slug **and**
`fs::canonicalize(record.member_dir)` equals `fs::canonicalize(listed.dir)`; any other record is
**Other** (an assistant of the same slug turned on from another folder, such as a copy of the kit,
is Other). `out_of_date` and `changed` are `false` unless This.

**A shown name.** `fn shown_name(name: &str) -> Option<String>`: `Some(name)` when
`persona_builder::crew::check_name("name", name)` passes, else `None`. Every record name that
reaches an answer or a sentence goes through it; `None` is written "Another assistant" at the start
of a sentence and "the assistant" elsewhere (the sentences below say which).

Commands (`Result<T, dto::Failure>`). As in PB7.6a: a `Failure`'s message is one of the sentences
below, never another error's text, never a path. Every command that takes a `slug` first finds it
with `assistants.rs`'s `listed(home, slug)` (any failure: `FIND`).

| Command | Does | Fallback |
| --- | --- | --- |
| `claude_code_status(SlugRequest) -> ClaudeCodeStatus` | `claude_home` not a folder: `NotSetUp`, the rest `None`/`false`. Else `install::status`: `None` → `Off`; a record → This or Other as above | `STATUS` |
| `turn_on(SlugRequest) -> Said` | `claude_home` not a folder: `NOT_SET_UP`. A record that is Other: `other_on(shown name, name)`, nothing changed. Else `install::turn_on(&listed.dir, places)` and `on(name)` | `on_failed(name)` |
| `turn_off(TurnOffRequest) -> Said` | `claude_home` not a folder or no record: `NOTHING_ON` (an answer, not a failure). A record whose slug differs from the request's: `CHANGED_ON`, nothing changed. Else `install::turn_off`: no `left` → `off(shown)`, else `off_left(shown)` | `OFF` |
| `claude_ai_files(SlugRequest) -> ClaudeAiFiles` | the files in `<state>/claude-ai/` (below); none or no folder: an empty list | `FILES` |
| `make_claude_ai_files(SlugRequest) -> ClaudeAiFiles` | under `lock_state`, the replace below, then the same listing | `PACK` |
| `show_claude_ai_files(SlugRequest) -> ()` | `private_folder_exists(home, slug, "claude-ai")` false: `MAKE_FIRST`. Else the canonical folder through `opener_args`, `.status()`, exit code ignored, as `show_assistant_folder` | `SHOW` |
| `forget_preview(SlugRequest) -> ForgetPreview` | `forget::forget(&listed.dir, home, false)`: its `files` (0 when it did not exist), as `u32` (saturating) | `FORGET` |
| `forget(ForgetRequest) -> Said` | a preview count of 0 (not there): `nothing_to_forget(name)`, nothing locked or created. Else `lock_state`; count again with `forget(.., false)`; a count different from the request's `files`: `FORGET_CHANGED`, nothing deleted; else `forget(.., true)` and `forgotten(files, name)`; drop the lock after | `FORGET` |

`lock_state`'s refusal (`Error::Invalid` equal to `persona_builder_ingest::private::BUSY`) answers
`busy(name)` in `make_claude_ai_files` and `forget`. `pack`'s refusal equal to
`pack::no_kinds_yet(&member.name)` answers `no_kinds(name)`.

**The claude.ai files, replaced whole.** The state folder is
`member::private_state_dir(&member, home)` (`None`: `PACK`). Holding the lock:
1. if `private_folder_exists(home, slug, "claude-ai-new")`, `remove_dir_all` it (a leftover);
2. `pack::pack(&listed.dir, &state.join("claude-ai-new"))`; on its error, stop (the old
   `claude-ai` is untouched);
3. if `private_folder_exists(home, slug, "claude-ai")`, `remove_dir_all` it;
4. `fs::rename` `claude-ai-new` to `claude-ai`.

**The listing** (`claude_ai_files` and the answer of `make_claude_ai_files`): when
`private_folder_exists(home, slug, "claude-ai")`, the names of the entries whose
`symlink_metadata` is a regular file and whose name is exactly `instructions.md` or
`decisions-<c>.md` with `<c>` 1 to 64 bytes of `a-z`, `0-9` and `-`; `instructions.md` first, then
the rest sorted by name. Anything else in the folder is never named.

The api's own sentences, final (`{name}` is the listed assistant's name; `{shown}` is a record's
shown name):

```rust
const STATUS: &str = "Persona Builder couldn't check Claude Code. Close it and open it again.";
const NOT_SET_UP: &str = "Claude Code isn't set up on this computer yet. Install it and sign in, then come back.";
const NOTHING_ON: &str = "No assistant is on in Claude Code.";
const CHANGED_ON: &str = "What's on in Claude Code has changed. Look again, then choose.";
const OFF: &str = "Persona Builder couldn't turn the assistant off. Close it and open it again, then try again.";
const FILES: &str = "Persona Builder couldn't look for the claude.ai files. Close it and open it again.";
const PACK: &str = "Persona Builder couldn't make the claude.ai files. Check there's space on this computer, then try again.";
const MAKE_FIRST: &str = "Make the claude.ai files first.";
const SHOW: &str = "Persona Builder couldn't open the folder.";
const FORGET: &str = "Persona Builder couldn't forget anything, and nothing was deleted. Close it and open it again, then try again.";
const FORGET_CHANGED: &str = "There's more to forget than when you looked. Look again, then confirm.";
fn on(name) -> "{name} is on. Start a new Claude Code session to use {name}."
fn on_failed(name) -> "Persona Builder couldn't turn {name} on, so nothing in your Claude folder was changed."
fn other_on(shown, name) -> "{shown} is on in Claude Code. Turn {shown} off first, then turn {name} on."
                          // shown None: "Another assistant is on in Claude Code. Turn it off first, then turn {name} on."
fn off(shown) -> "{shown} is off. Everything Persona Builder added is gone."
                // shown None: "The assistant is off. Everything Persona Builder added is gone."
fn off_left(shown) -> "{shown} is off. A few things in your Claude folder were changed after {shown} was turned on, so Persona Builder left them where they are."
                     // shown None: "The assistant is off. A few things in your Claude folder were changed after it was turned on, so Persona Builder left them where they are."
fn no_kinds(name) -> "{name} doesn't know any kinds of question yet. Build {name} first, then make the claude.ai files."
fn busy(name) -> "Persona Builder is already working on {name}. Wait for it to finish, then try again."
fn nothing_to_forget(name) -> "There's nothing to forget: Persona Builder hasn't kept anything for {name} yet."
fn forgotten(files, name) -> "Deleted {count}. {name}'s own folder is untouched."
                             // {count}: "1 file" for 1, else "{files} files"
```

Testable forms, with no `HOME`, `CLAUDE_CONFIG_DIR`, real `~/.claude` or real Finder:
`claude_code_status_in(home, claude_home, request)`, `turn_on_in(home, claude_home, request)`,
`turn_off_in(home, claude_home, request)`, `claude_ai_files_in(home, request)`,
`make_claude_ai_files_in(home, request)`,
`show_claude_ai_files_in(home, request, open: impl FnOnce(&Path) -> std::io::Result<()>)`,
`forget_preview_in(home, request)`, `forget_in(home, request)`. The zero-argument forms read `HOME`
(missing: that command's fallback; for `turn_on`, `on_failed` cannot be named, so `STATUS`) and
`claude_home(&home, env::var_os("CLAUDE_CONFIG_DIR").as_deref())`.

`src/parity.rs` gains, after `save_push`, in this order: `claude_code_status` → `[["status"]]`,
`turn_on` → `[["on"]]`, `turn_off` → `[["off"]]`, `claude_ai_files`, `make_claude_ai_files` and
`show_claude_ai_files` → `[["pack"]]`, `forget_preview` and `forget` → `[["forget"]]`.

### 3. The CLI (`builder/cli`)

Only the move in §2: `places()` takes its default from `persona_builder_api::claude_home`. Every
`persona-builder on|off|status|pack|forget` output stays byte for byte (the existing tests prove
it).

### 4. The Tauri app (`builder/app`)

`COMMANDS`, the capability's `allow-…` list, `generate_handler!`, the lockdown test's list and the
parity table gain the eight names in the order above (23 in all). Each is
`#[tauri::command(async)]` and one call into `persona_builder_api`'s zero-argument form, taking a
request as one argument named `request`. No new permission, plugin or capability.

### 5. The screens (`builder/ui`)

Commit every new file in `src/types/generated/` (cargo test writes them).

`Backend` gains, typed only with generated types: `claudeCodeStatus(request: SlugRequest):
Promise<ClaudeCodeStatus>`, `turnOn(request: SlugRequest): Promise<Said>`,
`turnOff(request: TurnOffRequest): Promise<Said>`, `claudeAiFiles(request: SlugRequest):
Promise<ClaudeAiFiles>`, `makeClaudeAiFiles(request: SlugRequest): Promise<ClaudeAiFiles>`,
`showClaudeAiFiles(request: SlugRequest): Promise<void>`, `forgetPreview(request: SlugRequest):
Promise<ForgetPreview>`, `forget(request: ForgetRequest): Promise<Said>`. `tauri-backend.ts` maps
each to `invoke('<snake name>', { request })`.

**The frame (`PersonaBuilder.vue`).** The save view shows, in this order, `Save`, `TurnOn`,
`ClaudeAiFiles` and `Forget`, each with the chosen assistant and keyed by its slug plus a counter
`resets`. On `Forget`'s `forgotten` event the frame sets the assistant's `consent` to
`{ agreed: false, agreedOn: null }` and adds one to `resets` (so every card reloads; *Connect
Claude* goes back to "Needs your permission first").

Each new screen is one Tabler card, root `<section class="card mt-4">`, `card-body p-5`, with the
same busy, alert and failure patterns as `Save.vue` (`failureMessage(error, <the Rust fallback
sentence>)`; a non-`Failure` rejection shows the fallback). Buttons are disabled while any call of
their card is running. No `<a>`, no custom colors, no Tabler JavaScript; light and dark from the
theme. `index.ts` exports all three.

**`screens/TurnOn.vue`** (prop `assistant`):
- heading "Use it in Claude Code"; the line "Turn {name} on, and Claude Code asks {name} before it
  asks you a question. One assistant can be on at a time.";
- on mount `claudeCodeStatus({ slug })`: spinner, visually hidden "Checking Claude Code…"; on
  failure a warning alert with the `STATUS` sentence fallback and "Try again";
- `notSetUp`: the `NOT_SET_UP` sentence as a paragraph, no button;
- `off`: "{name} is off in Claude Code." and the primary button "Turn {name} on" (busy
  "Turning it on…") → `turnOn({ slug })`;
- `this`: "{name} is on in Claude Code."; when `outOfDate`, "{name} has changed since you turned it
  on." and the primary button "Update {name} in Claude Code" (busy "Updating…") → `turnOn`; when
  `changed`, the muted line "Something Persona Builder added for {name} in your Claude folder was
  changed or removed."; the secondary outline button "Turn {name} off" (busy "Turning it off…") →
  `turnOff({ slug: onSlug })`;
- `other`: "{onName} is on in Claude Code. Only one assistant can be on at a time." (no `onName`:
  "Another assistant is on in Claude Code. Only one assistant can be on at a time.") and the
  secondary outline button "Turn {onName} off" (no `onName`: "Turn it off") → `turnOff({ slug:
  onSlug })`;
- after any of these calls: its `message` in a success alert, or its failure in a warning alert,
  then reload the status.

**`screens/ClaudeAiFiles.vue`** (prop `assistant`):
- heading "Use it in claude.ai"; the line "claude.ai can't read your Claude folder, so Persona
  Builder makes files you add to a claude.ai Project yourself.";
- on mount `claudeAiFiles({ slug })` (spinner, hidden "Looking for the claude.ai files…"; failure
  alert with the `FILES` fallback and "Try again");
- no files: the primary button "Make the claude.ai files" (busy "Making them…");
- files: "The files are ready:", the names as a list in monospace (`<code>` per item), the
  secondary outline button "Show in Finder" (`showClaudeAiFiles`; failure in a small warning alert,
  fallback `SHOW`), then the numbered steps:
  1. "In claude.ai, make a new Project."
  2. "Open instructions.md and paste what it says into the Project's instructions."
  3. "Add the other files to the Project's knowledge with the + button."

  then the muted line "If you change {name}, make the files again and replace them in the
  Project." and the primary outline button "Make them again" (busy "Making them…");
- making: `makeClaudeAiFiles({ slug })`; success replaces the list; failure a warning alert,
  fallback `PACK` (the `no_kinds` and `busy` sentences arrive this way).

**`screens/Forget.vue`** (prop `assistant`, emits `forgotten`):
- heading "Forget what {name} learned from"; the line "Forget deletes the history {name} learned
  from and Persona Builder's working files. {name} itself stays.";
- the danger outline button "Forget…" → `forgetPreview({ slug })` (busy "Checking…"; failure: a
  warning alert, fallback `FORGET`);
- preview of 0 files: "There's nothing to forget: Persona Builder hasn't kept anything for {name}
  yet." (Vue's own copy of the sentence) and the link-style button "Close";
- preview of N > 0 files, the confirmation:
  - "This deletes {count} from your Claude folder:" (`{count}`: "1 file" or "N files"), then a list:
    "the history {name} learned from"; "Persona Builder's working files, including the claude.ai
    files"; "your permission to read your history"; "your choice of where to keep a copy of
    {name}";
  - "These stay:", then a list: "{name}'s own folder, and everything Persona Builder wrote into
    it"; "any copy of {name} on GitHub"; "{name} in Claude Code, if it's on";
  - "You can't undo this.";
  - the danger button "Delete these files" (busy "Deleting…") → `forget({ slug, files: N })` with
    the previewed N, and the link-style button "Cancel" (back to the first state, nothing sent);
- success: its `message` in a success alert, back to the first state, and emit `forgotten`;
  failure: a warning alert, fallback `FORGET`, the confirmation closed (the `FORGET_CHANGED` and
  `busy` sentences arrive this way).

### 6. Docs

- `.ai/PROJECT.md`: the `builder/api/` row's tests gain `builder/api/tests/use_it.rs`.
- `builder/ui/README.md`: the screens line becomes "The screens so far: assistants, a new one,
  permission, connect Claude, save a copy, turn it on, the claude.ai files, forget."

## Tests (write each first and watch it fail on its assertion)

Rust, `builder/api/tests/use_it.rs` (a temp home as in `tests/assistants.rs`; assistants made with
`new_assistant_in`; `claude_home` = `<home>/.claude` unless a test says otherwise):
- **status:** a `claude_home` that does not exist → `NotSetUp`; nothing on → `Off`; after
  `turn_on_in` → `This` with `onName` "Ari", `onSlug` "ari", `outOfDate` and `changed` false; after
  appending a line to the assistant's own `SKILL.md` → `outOfDate` true; after removing the
  `CLAUDE.md` block's lines by hand → `changed` true; Mo Lee on, asked for Ari → `Other`, `onName`
  "Mo Lee", `outOfDate` false; **a copy of Ari's folder at `<home>/kit/crew/ari` turned on with
  `persona_builder::install::turn_on`, asked for the listed Ari → `Other`** (same slug, other
  folder).
- **on:** answers `on("Ari")`, the skill is in `<claude_home>/skills/ari/`; with Mo Lee on →
  `other_on("Mo Lee", "Ari")` and the Claude folder unchanged (compare a listing with file bytes
  before and after); with the kit copy on → `other_on` and unchanged; a `claude_home` that does not
  exist → `NOT_SET_UP`; turning on again after editing the assistant's `SKILL.md` refreshes it
  (`outOfDate` false after); a failure inside `turn_on` (make `<claude_home>/skills/ari` a folder
  Persona Builder did not add) → `on_failed("Ari")`, never the core's text, and no path in it.
- **off:** nothing on → `NOTHING_ON`; on, request slug `ari` → `off("Ari")` and status `Off`; a
  request slug that differs from the record's → `CHANGED_ON` and still on; a changed installed
  `SKILL.md` → `off_left("Ari")`; `shown_name` unit test (in `use_it.rs`'s own `#[cfg(test)]`):
  `"Ari"` → `Some`, `"Ari\n@~/.ssh/x"`, `""` and 65 characters → `None`.
- **claude.ai files:** a new assistant (template `decisions.md`) → `no_kinds("Ari")` and no
  `claude-ai` folder; with a `decisions.md` holding the areas `scope` and `writing-voice` (an
  invented body, as in `builder/core/src/pack_tests.rs`), make → `["instructions.md",
  "decisions-scope.md", "decisions-writing-voice.md"]`, the folder is 0700 and each file 0600,
  `claude_ai_files_in` gives the same list; remove the `scope` area and make again →
  `decisions-scope.md` is gone (the folder is replaced whole); with files made, a `decisions.md`
  with no areas → `no_kinds` and the old files still there; a leftover `claude-ai-new` folder is
  cleared; planted `notes.txt`, `secret.md` and a symlink `decisions-x.md` in `claude-ai` are never
  listed; with `lock_state(home, "ari")` held by the test, make → `busy("Ari")`;
  `show_claude_ai_files_in` before making → `MAKE_FIRST`, after → the opener gets the canonical
  `<home>/.claude/state/ari/claude-ai`, and an opener error → `SHOW`.
- **forget:** after agreeing to permission, keeping *this computer only*
  (`save_choose_in` local) and making the claude.ai files: preview counts every file in the state
  folder; `forget_in` with that count answers `forgotten(n, "Ari")`, the state folder is gone,
  `list_assistants_in` shows Ari with `consent.agreed` false, `save_options_in` shows `chosen`
  none, and every file in the assistant's own folder is byte-identical; with a wrong count →
  `FORGET_CHANGED` and nothing deleted; with the lock held → `busy("Ari")` and nothing deleted;
  nothing there → preview 0, and `forget_in` answers `nothing_to_forget("Ari")` and creates no
  state folder; `forgotten(1, …)` says "1 file".
- **every command** with an unlisted slug, `../x` and `""` answers `FIND` (`turn_off` has no
  listed-slug check: it is global; test its `CHANGED_ON` instead).
- **DTOs:** JSON is camelCase; `OnState` serializes as the strings above; `TurnOffRequest` and
  `ForgetRequest` refuse an unknown field.
- `claude_home` unit test: `Some("/x")` → `/x`; `Some("")` and `None` → `<home>/.claude`.
- `builder/core` pack tests and `builder/ingest` lock tests pass unchanged; the CLI's
  `turn_on.rs`, pack and forget tests pass unchanged; `builder/cli/tests/app_parity.rs` runs over
  the new rows; `builder/app/tests/lockdown.rs` passes with the 23 commands.

Vitest:
- `TurnOn.test.ts`: loading, then each state's text and button (`notSetUp` has none); "Turn Ari on"
  calls `turnOn({ slug: 'ari' })`, shows the message and reloads; `this` with `outOfDate` shows the
  update button and with `changed` the muted line; "Turn Ari off" sends `{ slug: onSlug }`;
  `other` without `onName` says "Another assistant…" and "Turn it off"; a `Failure` shows its
  message, a non-`Failure` rejection the fallback.
- `ClaudeAiFiles.test.ts`: no files → "Make the claude.ai files"; made → the list, the steps,
  "Show in Finder" calling `showClaudeAiFiles({ slug })` and "Make them again"; a make failure
  shows its message; no `<a>` in the screen.
- `Forget.test.ts`: "Forget…" calls `forgetPreview`; 0 shows the nothing-to-forget line and no
  delete button; N shows the count ("1 file" for 1), both lists and "You can't undo this.";
  "Delete these files" sends `{ slug, files: N }` with the previewed N; success shows the message
  and emits `forgotten`; "Cancel" sends nothing; a failure shows its message and closes the
  confirmation.
- `PersonaBuilder.test.ts` (updated): the save view shows the four cards' headings; a `forgotten`
  from `Forget` sets *Connect Claude* back to "Needs your permission first".
- `tauri-backend.test.ts`: each new method invokes its snake-case command with `{ request }`.

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). List each with its red result:
1. drop the `member_dir` comparison in This or Other → the kit-copy status test red;
2. let `turn_off` ignore a request slug that differs → the `CHANGED_ON` test red;
3. let `forget` ignore a count that differs → the `FORGET_CHANGED` test red;
4. list every regular file in `claude-ai` → the planted-files test red;
5. skip `lock_state` in `forget` → the busy forget test red;
6. remove the old `claude-ai` before packing instead of after → the "old files still there" test
   red;
7. drop one new name from the capability only → lockdown red;
8. send `files: 0` instead of the previewed N from `Forget.vue` → the Forget test red.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), the generated types are
committed-ready (no diff after `cargo test`), and the mutation list is reported. Don't commit;
Claude reviews, commits and runs the app.
