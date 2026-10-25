# PB7.3 brief: your history (Claude Code and claude.ai)

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB7.3` entry (PB7 of the Persona Builder
plan), plus the rule PB7.2 left: every app command that reads history refuses, in plain words, an
assistant without a current agreement, checked in Rust and never only by the screens. Read the
repo's `AGENTS.md` and `.ai/PROJECT.md` first, then `plans/briefs/PB7.1-app-frame.md` and
`plans/briefs/PB7.2-consent-new-connect.md` (the bridge, the screens and the rules this slice
grows), `builder/api/src/assistants.rs` (`read_consent`, the `failure` sentences, the `_in` test
forms), `builder/ingest/src/mine.rs` (`mine`, `folders`, `Mined`, `FolderRow`),
`builder/ingest/src/claudeai.rs` (`read_export`, `inspect`, `titles`, `mark`, `Selector`, `mine`,
`MINE_MARKS`), `builder/ingest/src/corpus.rs` (`discover`, `merge`),
`builder/ingest/src/private.rs` (`lock_state`, `state_directory`, `write_private`,
`read_private_optional`), the CLI's `mine`, `folders`, `claudeai_*` and `merge` in
`builder/cli/src/main.rs`, `builder/app/` and `builder/ui/`. Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security: `~/.codex/skills/security-and-hardening/SKILL.md`;
structure: `~/.codex/skills/frontend/SKILL.md` and `~/.codex/skills/tabler-ui/SKILL.md`; looks:
`~/.codex/skills/design-guide/SKILL.md`; every sentence a person sees:
`~/.codex/skills/plain-language/SKILL.md` (the sentences below are final: use them word for word);
review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true` and pnpm with `--offline`. **One new dependency, and only
this one:** `rfd = { version = "=0.17.2", default-features = false, features = ["gtk3"] }` in
`builder/app/Cargo.toml` (already in the local cargo cache; `Cargo.lock` gains it). No other
crate or package. If you think one is needed, stop and say so.

## What this slice does

After giving permission, the person opens *Your history*. They see which Claude Code folders
Persona Builder will read and which it leaves out, count first, then read. If they use claude.ai,
they choose their downloaded export in a window the app opens, see that it can be read, mark which
chats are theirs, and read those. Then they put it all together into the one history the build
uses. Long reads show a progress bar.

## Decisions (made, do not reopen)

### 1. Ingest (`builder/ingest`)

- `mine.rs`: `pub const DEFAULT_HOLDOUT: f64 = 0.15;` The CLI's `split_choice` uses it instead of
  the `"0.15"` string (parse only when `--holdout` is given).
- `mine.rs`: `pub fn mine_with_progress(member, filters, projects, home, split, progress: &mut dyn FnMut(usize, usize)) -> Result<Mined>`:
  today's `mine` body, calling `progress(0, total)` before the first folder and
  `progress(done, total)` after each project folder (excluded ones included), `total` being the
  number of project folders. `mine` becomes a one-line call with `&mut |_, _| {}`.
- `claudeai.rs`: the same for `mine`: `pub fn mine_with_progress(filters, export, review, split, progress: &mut dyn FnMut(usize, usize)) -> Result<MinedExport>`,
  counting conversations (every conversation in the export, marked or not); `mine` calls it with a
  no-op.
- `claudeai.rs`: `pub fn conversations(export: &Export) -> Result<usize>` (the length of the private
  `list`), and `pub fn is_mine(mark: &str) -> bool` (trimmed as `mine` trims, lower-cased, in
  `MINE_MARKS`); `mine` uses `is_mine` instead of its inline test.
- `private.rs`:
  - `pub const BUSY: &str` holding `lock_state`'s current "already working" text, and `lock_state`
    uses it.
  - `pub fn list_private(home: &Path, slug: &str) -> Result<Vec<String>>`: the state folder checked
    as `read_private_optional` checks it (through `state_directory(home, slug, false)`: never a
    link, only this user's), then its file names (regular files only, UTF-8 names only, others
    skipped), sorted; no state folder gives an empty list. Tests: files listed, a folder and a
    non-UTF-8 name skipped, no folder empty, a link in place of the state folder refused.
  - The CLI's `merge` replaces its inline `read_dir` with `list_private`.

### 2. Where the chosen claude.ai download is kept

The webview never names a path. When the person picks their download in the app's window, Rust
keeps where it is in the state folder as `claudeai-export.json`, `{"path":"<absolute path>"}`,
through `write_private`, and every later step reads it from there. A path that isn't valid UTF-8 is
refused with the `EXPORT_BAD` sentence. Forget already deletes the state folder, so this goes with
it. The review file and outputs keep the CLI's default names: `claudeai-review.tsv`,
`decisions-claude-code.jsonl`, `decisions-claudeai.jsonl`, `decisions.jsonl`; the split is always
`Split::Holdout(DEFAULT_HOLDOUT)`.

### 3. The api (`builder/api`)

New module `src/history.rs` (`pub mod history; pub use history::*;`). Every command has a testable
`_in(home, …)` form and a zero-argument form reading `HOME`, as PB7.2's.

**The gate**, one private function every command below calls first:
`fn readable(home, slug) -> Result<Member>`: `assistants::assistant_dir` must accept the slug and
`assistants::list` must hold it (else `FIND`); `member::private_state_dir(&member, home)` must be
`Some` (else `ELSEWHERE`); `read_consent(home, slug).agreed` must be true (else `PERMISSION`).
Writers then take `lock_state(home, slug)` for the whole command: an error whose `Error::Invalid`
text equals `private::BUSY` answers `BUSY`, any other `SAVE`.

Sentences (each a `const`; a `Failure` is always one of these, never an error's own text or a
path):

| Name | Sentence |
| --- | --- |
| `FIND` | (PB7.2's) Persona Builder couldn't find that assistant. Close it and open it again. |
| `PERMISSION` | Persona Builder needs your permission before it reads anything. Give it in the first step. |
| `ELSEWHERE` | This assistant keeps its history outside your Claude folder. Use the persona-builder command for it. |
| `BUSY` | Persona Builder is already working on this assistant. Wait for it to finish, then try again. |
| `SAVE` | Persona Builder couldn't save what it read. Check there's space on this computer, then try again. |
| `FOLDERS` | Persona Builder couldn't look through your Claude Code history. Close it and open it again. |
| `READ` | Persona Builder couldn't read your Claude Code history. Close it and open it again. |
| `EXPORT_GONE` | Persona Builder can't find your claude.ai download any more. Choose it again. |
| `EXPORT_BAD` | Persona Builder couldn't read this download. Check it's the file claude.ai sent you, then choose it again. |
| `NO_CHATS` | Choose at least one chat first. |
| `NOTHING` | There's nothing to put together yet. Read your Claude Code history or your claude.ai chats first. |
| `MERGE` | Persona Builder couldn't put your history together. Close it and open it again. |

DTOs in `dto.rs` (answers camelCase; requests also `deny_unknown_fields`; counts are `u32`,
saturating from `usize`):

```rust
pub struct ClaudeCodeFolder { pub project: String, pub conversations: u32, pub read: bool }
pub struct ClaudeCodeFolders { pub found: bool, pub folders: Vec<ClaudeCodeFolder> }
pub struct ReadRequest { pub slug: String, pub dry_run: bool }
pub struct ReadProgress { pub done: u32, pub total: u32 }
pub struct ReadCounts { pub questions: u32, pub held_out: u32, pub messages: u32, pub private_left_out: u32, pub first: Option<String>, pub last: Option<String> }
pub struct ChooseExportRequest { pub slug: String, pub folder: bool }
pub struct ExportStatus { pub name: String, pub ready: bool, pub chats: u32 }
pub enum ChatMark { Mine, NotMine, Unmarked }   // serde + ts-rs as "mine" | "notMine" | "unmarked"
pub struct Chat { pub uuid: String, pub title: String, pub created: String, pub messages: u32, pub mark: ChatMark }
pub struct ChatList { pub chats: Vec<Chat>, pub mine: u32, pub not_mine: u32, pub unmarked: u32 }
pub struct MarkChatsRequest { pub slug: String, pub mine: bool, pub uuids: Vec<String>, pub unmarked: bool }
pub struct ClaudeaiCounts { pub chats: u32, pub messages: u32, pub private_left_out: u32 }
pub struct MergeCounts { pub questions: u32, pub held_out: u32, pub messages: u32, pub duplicates: u32, pub private_left_out: u32 }
```

`private_left_out` is always `personal + secret`. `first`/`last` are the first ten characters of
the first and last record's `timestamp` (as `mine::summary` takes them), `None` when there are
none. A chat's `mark` is `Mine` when `claudeai::is_mine`, `Unmarked` when the trimmed mark is
empty, else `NotMine`. `ExportStatus.name` is the chosen path's last component (lossy).

Commands (`Result<T, dto::Failure>`):

| Command | Does | Errors |
| --- | --- | --- |
| `claude_code_folders(SlugRequest) -> ClaudeCodeFolders` | `home/.claude/projects`; missing → `found: false`, no folders; else `mine::folders` with the member's compiled filters, `read: !excluded`, in its order | `FOLDERS` |
| `read_claude_code(ReadRequest, progress) -> ReadCounts` | `mine_with_progress`; unless `dry_run`, under the lock, writes `decisions-claude-code.jsonl` (`to_jsonl`). No `~/.claude/projects` → `ReadCounts` of zeros, nothing written | `READ`; write: `SAVE` |
| `use_claudeai_export_in(home, slug, path: &Path) -> ExportStatus` (no zero-arg form; the app calls `use_claudeai_export(slug, path)`) | under the lock, writes `claudeai-export.json`, then answers as `claudeai_export_status` | `SAVE`, then that command's |
| `claudeai_export_status(SlugRequest) -> Option<ExportStatus>` | no `claudeai-export.json` → `None`; its path missing (`symlink_metadata` NotFound) → `EXPORT_GONE`; `read_export` + `inspect`: `ready` when `missing` is empty, `chats` from `conversations` | damaged record or any read error: `EXPORT_BAD` |
| `list_claudeai_chats(SlugRequest) -> ChatList` | as the CLI's `claudeai titles` with no project field: keeps marks from an existing `claudeai-review.tsv`, writes it under the lock, answers its rows | no export chosen or gone: `EXPORT_GONE`; read: `EXPORT_BAD`; write: `SAVE` |
| `mark_claudeai_chats(MarkChatsRequest) -> ChatList` | `uuids` empty and `unmarked` false → `NO_CHATS`; under the lock, reads the review file (none → `EXPORT_GONE`), `claudeai::mark` with `Selector { uuids, unmarked, ..Default }` and `"y"`/`"n"`, writes it, answers the rows | `SAVE` |
| `read_claudeai(SlugRequest, progress) -> ClaudeaiCounts` | as `claudeai mine`, under the lock: the export, the review file (none → `NO_CHATS`), `mine_with_progress`, writes `decisions-claudeai.jsonl`; `chats` is `read` | `EXPORT_GONE`, `EXPORT_BAD`, `SAVE` |
| `merge_histories(SlugRequest) -> MergeCounts` | as `merge` with no `--in`, under the lock: `list_private` → `corpus::discover(names, "decisions.jsonl")` (empty → `NOTHING`), each read with `read_private`, `corpus::merge`, writes `decisions.jsonl` | `MERGE`; write: `SAVE` |

Also `pub fn can_read(request: SlugRequest) -> Result<()>` (the gate alone, `_in` form too), which
the app calls before it opens the file window.

**Progress.** The `progress` parameter is `&mut dyn FnMut(dto::ReadProgress)`. The api forwards
ingest's `(done, total)` only when the whole percent (`done * 100 / total`, 100 when `total` is 0)
differs from the last one sent, so a read sends at most 101 messages; the first call always sends.

`parity.rs` gains, in this order after `claude_check`:
`claude_code_folders` → `[["folders"]]`, `read_claude_code` → `[["mine"]]`,
`choose_claudeai_export` → `[["claudeai", "inspect"]]`, `claudeai_export_status` →
`[["claudeai", "inspect"]]`, `list_claudeai_chats` → `[["claudeai", "titles"]]`,
`mark_claudeai_chats` → `[["claudeai", "mark"]]`, `read_claudeai` → `[["claudeai", "mine"]]`,
`merge_histories` → `[["merge"]]`.

### 4. The Tauri app (`builder/app`)

- `COMMANDS`, the capability's `allow-…` list and `generate_handler!` gain the eight names in the
  order above (15 in all). No new permission, plugin or capability: the progress channel's data
  fetch is exempt from the ACL in Tauri 2.12.0 (`webview/mod.rs`, `FETCH_CHANNEL_DATA_COMMAND`).
- Each is `#[tauri::command(async)]`. Reads take `request` and `on_progress: tauri::ipc::Channel<persona_builder_api::dto::ReadProgress>`,
  and pass `&mut |p| { let _ = on_progress.send(p); }` (a closed window must not fail the read).
- `choose_claudeai_export(request: ChooseExportRequest) -> Result<Option<ExportStatus>, Failure>`:
  `persona_builder_api::can_read(SlugRequest { slug })?` first, so no window opens for an assistant
  that can't read; then, on this worker thread (rfd 0.17.2 hands the panel to the main thread
  itself when the app is running, `backend/macos/utils.rs` `run_on_main`), an `rfd::FileDialog`
  with the title "Choose your claude.ai download" and, when `home/Downloads` is a folder, that as
  its starting folder. `folder: false`: `.add_filter("claude.ai download", &["zip", "json"]).pick_file()`;
  `folder: true`: `.pick_folder()`. Cancelled → `Ok(None)`; a choice →
  `persona_builder_api::use_claudeai_export(&slug, &path).map(Some)`. No parent window is set.
  This is the only function that touches rfd; put it in `src/commands.rs` with the others.

### 5. The screens (`builder/ui`)

Commit every new generated type. `Backend` gains (generated types only):

```ts
claudeCodeFolders(request: SlugRequest): Promise<ClaudeCodeFolders>
readClaudeCode(request: ReadRequest, onProgress: (progress: ReadProgress) => void): Promise<ReadCounts>
chooseClaudeaiExport(request: ChooseExportRequest): Promise<ExportStatus | null>
claudeaiExportStatus(request: SlugRequest): Promise<ExportStatus | null>
listClaudeaiChats(request: SlugRequest): Promise<ChatList>
markClaudeaiChats(request: MarkChatsRequest): Promise<ChatList>
readClaudeai(request: SlugRequest, onProgress: (progress: ReadProgress) => void): Promise<ClaudeaiCounts>
mergeHistories(request: SlugRequest): Promise<MergeCounts>
```

`tauri-backend.ts`: the reads make a `new Channel<ReadProgress>()` from `@tauri-apps/api/core`,
set `channel.onmessage = onProgress`, and `invoke('<snake>', { request, onProgress: channel })`;
the rest as PB7.2's. `index.ts` also exports `Sources`.

**The frame.** The `sources` step is a button like `consent` and `connect`, enabled once the chosen
assistant's permission is given (same disabled text as `connect`); `view` gains `'sources'`.
`ConnectClaude.vue` emits `next`; when the check is ready it shows the primary button
"Next: your history" under the alert; the frame goes to `sources`.

**`screens/Sources.vue`** (prop `assistant`): heading "Your history", the line "Your assistant
learns from what you've written to Claude. Persona Builder reads it on this computer and keeps it
here.", then three Tabler cards in order: `sources/ClaudeCode.vue`, `sources/Claudeai.vue`,
`sources/PutTogether.vue`, each with prop `slug`. One shared `sources/ReadProgress.vue` (prop
`progress: ReadProgress | null`, prop `label`): a Tabler `progress` with a `progress-bar` at the
percent, `role="progressbar"`, `aria-valuenow`, `aria-valuemin="0"`, `aria-valuemax="100"`,
`aria-label` = label; with `null` progress, the indeterminate bar (`progress-bar-indeterminate`).
Every failure goes through `failureMessage(error, <the card's fallback>)`, shown in an alert in
that card. Numbers use `toLocaleString()`.

**`ClaudeCode.vue`**, card title "Claude Code":
- on mount `claudeCodeFolders`; spinner, visually hidden "Looking at your Claude Code history…";
  fallback the `FOLDERS` sentence.
- `found: false` or no folders: "Persona Builder found no Claude Code history on this computer."
  and nothing else.
- else the line "It will read <n> folders (<c> conversations) and leave out <m> (<d>
  conversations)." (the counts summed from the list), and a Tabler table (`table table-sm
  table-vcenter`) inside a `div` with `overflow-auto` and `style="max-height: 16rem"`: columns
  "Project", "Conversations", "Read?", the last "Yes" or "Left out"; under it the muted line
  "Your assistant's settings decide which folders are left out."
- button "Count first" (secondary): `readClaudeCode({ slug, dryRun: true }, …)` with
  `ReadProgress` labelled "Counting"; then the counts line (below), and the primary button
  "Read my Claude Code history" (`dryRun: false`, label "Reading") appears; after the read, the
  counts line again starting "Read …" and a success alert "Done. Your Claude Code history is
  saved on this computer." Both buttons disabled while either runs. Fallback `READ`.
- Counts line: "<Counted|Read> <questions> answers to Claude's questions and <messages> other
  messages, from <first> to <last>. <privateLeftOut> that looked private were left out. <heldOut>
  answers are kept aside to test your assistant with." The "from … to …" part only when `first`
  is set.

**`Claudeai.vue`**, card title "claude.ai (if you use it)":
- the lines: "If you also chat on claude.ai, you can add those chats." and "On claude.ai, open
  Settings, then Privacy, and choose Export data. claude.ai emails you a link that works for 24
  hours. Download the file, then choose it here." (support.claude.com, "Export your Claude data",
  checked 2026-10-25.)
- on mount `claudeaiExportStatus`; spinner, visually hidden "Checking your download…". A
  rejection shows its message (fallback `EXPORT_BAD`) and still offers the choose buttons.
- buttons "Choose the download" (`folder: false`) and "Choose a folder" (`folder: true`), both
  secondary, and the muted line "If your Mac unzipped the download, choose the folder it made." A
  `null` answer (cancelled) changes nothing.
- a status: ready → "You chose <name>. Persona Builder can read it: <chats> chats." and the
  primary button "List the chats"; not ready → warning alert "You chose <name>, but it's missing
  parts Persona Builder reads. Download your data from claude.ai again, then choose the new file."
- "List the chats" → `listClaudeaiChats`; then `sources/ChatList.vue` (props `slug`, `list`; emits
  `update(list)` with each new `ChatList`).
- under the list, the primary button "Read the chats marked yours", disabled while
  `list.mine === 0` or a call runs; `readClaudeai` with `ReadProgress` labelled "Reading"; then the
  success alert "Done. Read <chats> chats: <messages> messages. <privateLeftOut> that looked
  private were left out." Fallback for all of these `EXPORT_BAD`.

**`ChatList.vue`**:
- the line "Mark the chats that are yours. Leave out any you wrote for someone else, or that
  aren't about how you work." and the count line "<mine> yours, <notMine> not yours, <unmarked>
  not marked yet."
- buttons "Mark all as yours" (`uuids` = every chat's uuid, `mine: true`) and "Mark the rest as not
  yours" (`unmarked: true`, `mine: false`), secondary, small.
- a search field "Find a chat" (filters by title, case-insensitive, on the screen only).
- the chats in a scrolling `div` (`overflow-auto`, `style="max-height: 24rem"`), a Tabler
  `list-group`: title (a title of exactly `[filtered]` shows "Title hidden: it looked private"; an
  empty one "No title"), the muted line "<created> · <messages> messages", and a Tabler `btn-group`
  of two small buttons "Yours" and "Not yours", the one matching `mark` with `active` and
  `aria-pressed="true"`. Clicking one calls `markClaudeaiChats({ slug, mine, uuids: [uuid], unmarked: false })`
  and emits `update` with the answer. Buttons disabled while a call runs. Failure alert in the
  component (fallback `EXPORT_BAD`).

**`PutTogether.vue`**, card title "Put it together":
- the line "When you've read everything you want, put it together into the one history your
  assistant learns from."
- primary button "Put it together" (while waiting "Putting it together…") → `mergeHistories`;
  then the success alert "Done. Your assistant will learn from <questions> answers to Claude's
  questions and <messages> other messages. <heldOut> answers are kept aside to test it with.
  <duplicates> repeats were left out." Fallback `MERGE`.

Tabler classes only; no custom colors; no Tabler JavaScript; light and dark from the theme; no
`<a>` anywhere in these screens.

### 6. Docs

- `.ai/PROJECT.md`: the `builder/api/` row gains `history.rs` (reading the histories) and its test
  `builder/api/tests/history.rs`; the `builder/app/` row names `rfd` as the native file window.
- `builder/ui/README.md`: the screens line gains "your history".

## Tests (write each first and watch it fail on its assertion)

Fixtures are invented (no real person): reuse the ingest tests' Claude Code session builders and
claude.ai export fixtures; temp homes only.

Rust:
- ingest: `mine_with_progress` calls `(0, n)` first and `(n, n)` last for n folders (one
  excluded); `claudeai::mine_with_progress` the same over conversations; `conversations` and
  `is_mine` (`"y"`, `" Yes "`, `"mine"` true; `"n"`, `""`, `"maybe"` false); `list_private` as in
  §1.
- `builder/api/tests/history.rs`:
  - every command, for an assistant with no permission, answers `PERMISSION` and writes nothing
    (the state folder's file list unchanged); with a version-0 `consent.json`, the same; an
    unknown slug and `../x` answer `FIND`.
  - `claude_code_folders`: no `~/.claude/projects` → `found: false`; two folders, one left out by
    the member's filters, with their conversation counts and `read`.
  - `read_claude_code`: dry run writes nothing and answers the counts; the read writes
    `decisions-claude-code.jsonl` (mode 0600) equal to `mine::to_jsonl(&mine::mine(…)?.records)`
    for the same home and split; progress starts at done 0 and ends at done == total, never more than 101 calls;
    with the state lock held (`lock_state`) it answers `BUSY`.
  - `use_claudeai_export_in` with a fixture export (a folder holding an invented
    `conversations.json` the test writes; no zip, so no Python and no new dev-dependency): `ready`, `chats`, `name`; then
    `claudeai_export_status` answers the same; the file moved away → `EXPORT_GONE`; a text file
    that isn't an export → `EXPORT_BAD`; none chosen → `None`.
  - `list_claudeai_chats` then `mark_claudeai_chats` (one uuid yours, then `unmarked` not yours)
    → the counts; the review file on disk matches; marks survive a second `list_claudeai_chats`;
    `NO_CHATS` for an empty request.
  - `read_claudeai` writes `decisions-claudeai.jsonl` with only the chats marked yours.
  - `merge_histories`: nothing read → `NOTHING`; after both reads, `decisions.jsonl` holds both
    and equals `to_jsonl` of `corpus::merge` over the same two files and split.
  - no `Failure` message in this file's tests contains the temp home's path.
  - DTOs camelCase; `ChatMark` serializes `"notMine"`; each request refuses an unknown field.
- `builder/cli/tests/app_parity.rs` and `builder/app/tests/lockdown.rs` pass with the new rows and
  the 15 commands; the CLI's existing `merge` tests pass on `list_private`.

Vitest:
- `ClaudeCode.test.ts`: loading; none found; the folder table and the summary line; "Count first"
  shows progress then the counts and reveals the read button; the read sends `dryRun: false` and
  shows "Done…"; buttons disabled while running; a `Failure` shown and a non-`Failure` rejection
  shows the fallback.
- `Claudeai.test.ts`: nothing chosen shows both choose buttons; a cancelled choice (`null`) keeps
  the earlier status; ready shows the name and chat count and "List the chats"; not ready shows
  the warning; the read button is disabled at `mine === 0` and shows "Done. Read …" after reading.
- `ChatList.test.ts`: the count line; "Yours" sends one uuid with `mine: true` and emits the
  answer; "Mark the rest as not yours" sends `unmarked: true`; the search filters by title; a
  `[filtered]` title shows "Title hidden: it looked private"; `aria-pressed` follows `mark`.
- `PutTogether.test.ts`: success line; `NOTHING` shown from a `Failure`.
- `ReadProgress.test.ts`: 40 of 80 renders 50%; `null` renders the indeterminate bar.
- `PersonaBuilder.test.ts`: `sources` disabled until permission; "Next: your history" from a
  ready connect check goes to the sources view.
- `tauri-backend.test.ts`: each new method invokes its snake-case command; the two reads pass
  `{ request, onProgress }` with a `Channel` whose messages reach the callback (mock the module as
  the file already does).

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). Report each with its red result:
1. drop the consent check from `readable` → api permission tests red;
2. let `read_claude_code` write on a dry run → api test red;
3. forward every progress call (no percent throttle) → api test red;
4. pass an ingest error's text to `Failure` in `claudeai_export_status` → the no-path test red;
5. drop `lock_state` from `mark_claudeai_chats` → its `BUSY` test red (add one);
6. add a command to `COMMANDS` only → lockdown red;
7. enable `sources` before permission → PersonaBuilder test red;
8. make `ChatList` send every uuid on one "Yours" click → ChatList test red.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), `cargo fmt --check` and
`cargo clippy --workspace --all-targets --locked -- -D warnings` are clean, the generated types are
current (no diff after `cargo test`), and the mutation list is reported. Don't commit; Claude
reviews, commits and runs the app.
