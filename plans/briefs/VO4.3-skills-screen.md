# VO4.3 brief: the skills screen

VO3 and VO4.1 built the skills commands: `voice skills list|agree|withdraw|refresh <member_dir>`.
A new skill on this computer waits until the person says yes once (gate Q1), and `refresh` copies
the agreed skills, with private details taken out, into the assistant's private mirror. This slice
puts the same four actions **in the app**, per assistant: every skill on this computer with its
state (those waiting for a yes first), a yes or a stop per skill, and a refresh whose report is
shown as plain text. The app calls the same library as the CLI, and the parity test lists the
pairs.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md` (the webview rows and the mirror row),
`CONTRIBUTING.md`, `builder/ui/README.md`, and `plans/briefs/VO4.1-skill-mirror.md` §§ 1 and 8.10.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/design-guide/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (every sentence
a person reads), `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only (an assistant "Pebble" with slug `pebble`, a principal
"Rowan Vale", a company "Pebble Freight", skills `notes` and `writing`), no real person, no fixed
paths, no real secret in a fixture. Commit nothing; leave every change in the working tree.

Out of scope: refreshing at convene (VO4.2), anything that reads the mirror, a bulk "say yes to
all" button (Q1 is one yes per skill, on purpose), `voice anonymize --out` in the app.

## 1. Decisions already made (do not change them)

- **Where it lives:** a new card `Skills.vue` on the Save step's page, after `ClaudeAiFiles` and
  before `Forget` in `VoiceApp.vue` (`:key="`skills-${assistant.slug}-${resets}`"`). No new rail
  step, so `steps.ts` and the rail tests don't change.
- **No consent check, as the CLI:** the skills commands touch only the anonymizer's files and the
  mirror, never the history, so like the CLI they need no history permission. Each command finds
  the assistant with `listed(home, &slug)` and takes `private::lock_state(home, &member.slug)` for
  the whole command, as the CLI does.
- **Only fixed sentences cross as failures** (`.ai/SECURITY.md`: a failure reaches the screens only
  as a fixed sentence, never an error's own text or a path). Every `Err` is mapped with
  `.map_err(|_| failure(CONST))` or the mapping in § 4. The skill rows and the refresh report lines
  are data the person's own CLI already prints to them, so they cross as data (the CLI's exact
  text), never inside a `Failure`.
- **One mirror-removal step, shared.** The withdraw block that removes the mirror in
  `builder/cli/src/skills.rs` (lines 113-150 today) moves into `voice_ingest::skills` (§ 2) and
  both the CLI and the api call it. The CLI's output does not change by one byte: its existing
  tests are the guard.
- **Command names** (appended, in this order, at the end of every list in § 5): `list_skills`,
  `agree_skills`, `withdraw_skills`, `refresh_skills`.
- **The list order:** waiting for a yes first, then every other skill; within each group, the
  CLI's order (by id). Each row carries `number`, its 1-based position in `voice skills list`
  (the CLI's order), because a refresh line can say "number N in voice skills list".

## 2. The shared mirror removal (`builder/ingest/src/skills.rs` or `skills/mirror.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// There was no mirror.
    NotThere,
    /// The mirror was removed.
    Removed,
    /// It couldn't be removed; its manifest was dropped (as today).
    NotRemoved,
}
/// After a withdrawal: remove the assistant's mirror so nothing withdrawn stays in it.
/// The caller holds `private::lock_state(home, slug)`.
pub fn remove_after_withdraw(home: &Path, slug: &str, found: &Found, rules: &Rules) -> Removal
```

Its body is today's CLI block, unchanged in behaviour: `private_folder_exists` → `Ok(false)` is
`NotThere`; else `private_folder`, `check_output(&path, home, &found.skills, &found.protected,
rules.projects())`, `remove_copy`; `Ok(true)` is `Removed`; anything else calls `drop_manifest` on
`home/.claude/state/<slug>/mirror` and is `NotRemoved`. The CLI's withdraw calls it only when a
record was withdrawn, and prints exactly what it prints today for `Removed` and `NotRemoved`
(returning `FAILURE` for `NotRemoved`) and nothing for `NotThere`. `pub use` it from `skills`.

## 3. The api (`builder/api/src/skills.rs`, new; `pub mod skills; pub use skills::*;` in `lib.rs`)

Each command has `foo_in(home: &Path, request) -> Result<Dto>` and `foo(request)` (home from
`assistants::home`), as the other api modules. Every request DTO has `#[serde(deny_unknown_fields)]`;
every DTO derives as the others in `dto.rs` and is `#[ts(export)]`.

**DTOs (in `builder/api/src/dto.rs`):**

```rust
pub struct SkillsRequest { pub slug: String, pub ids: Vec<String> }   // agree and withdraw
pub enum SkillState { Waiting, Agreed, Moved, NotOffered, NeverCopied } // serde camelCase strings
pub struct SkillRow {
    pub id: String,        // e.g. "claude:notes"
    pub name: String,      // skill.name
    pub place: String,     // skill.shown, the `~/…` path exactly as `voice skills list` prints it
    pub state: SkillState,
    pub agreed_on: Option<String>, // Some(date) only for Agreed
    pub number: u32,       // position in `voice skills list`, 1-based
}
pub struct Skills { pub skills: Vec<SkillRow>, pub notes: Vec<String> }
pub enum MirrorAfter { NotThere, Removed, NotRemoved }                 // camelCase strings
pub struct SkillsWithdrawn { pub skills: Skills, pub mirror: MirrorAfter }
pub struct SkillsRefreshed {
    pub lines: Vec<String>, // `copied.lines` split on '\n', empty lines dropped, in order
    pub copied: u32, pub agreed: u32, pub waiting: u32,
    pub new: u32, pub updated: u32, pub unchanged: u32, pub removed: u32,
}
```

`notes` carries only the discovery notes that are one of the three known sentences (the two in
`skills.rs` and `PLUGIN_NOTE`); any other string is dropped. Counts use `history::count` (or a copy
of it) so they saturate at `u32::MAX`.

**Commands:**

- `list_skills(SlugRequest) -> Skills`: find, lock, `Rules::for_member`, `discover`,
  `Agreements::load`; map each skill with `agreements.state(skill, &rules)`; order as § 1.
- `agree_skills(SkillsRequest) -> Skills`: as the CLI's agree: every id must be one `discover`
  found, else `Err(failure(GONE))` and nothing is saved; an empty `ids` is `Err(failure(NONE))`;
  date `voice_agent::ledger::local_day(SystemTime::now())`; `agree` each, `save`; then return the
  fresh list (discover again under the same lock).
- `withdraw_skills(SkillsRequest) -> SkillsWithdrawn`: empty `ids` is `Err(failure(NONE))`;
  `withdraw` each, `save`; if any record was withdrawn, `remove_after_withdraw`, else `NotThere`;
  return the fresh list and the removal.
- `refresh_skills(SlugRequest) -> SkillsRefreshed`: `skills::refresh(&home, &member, &date)`; map
  `Ok` as above (the summary, mirror-path and waiting lines the CLI prints after `copied.lines` are
  not in `lines`: the screen writes its own from the counts). Map `Err` as § 4.

**Fixed sentences** (consts in `skills.rs`; `FIND` and `BUSY` reuse the crate's existing ones):

| Const | Sentence | When |
| --- | --- | --- |
| `FIND` | the existing one | `listed` fails |
| `BUSY` | the existing one | `lock_state` returns `private::BUSY` |
| `SKILLS_READ` | `Voice couldn't look through the skills on this computer. Close it and open it again.` | any other error before the change: the lock, the rules, `discover`, loading agreements |
| `SKILLS_SAVE` | `Voice couldn't save your choice. Check there's space on this computer, then try again.` | `Agreements::save` fails |
| `GONE` | `That skill isn't on this computer any more. Look again.` | an agree id `discover` didn't find |
| `NONE` | `Choose a skill first.` | empty `ids` |

## 4. Refresh failures

`skills::refresh` returns masked `Error::Invalid` sentences that end with one outcome. Map by the
outcome only, never by passing the text through:

| The error | Failure sentence |
| --- | --- |
| `Invalid(text)` with `text.ends_with(MIRROR_STALE)` | `Voice couldn't bring your assistant's copy of your skills up to date, so it marked the copy as out of date and left its files alone. To see what needs fixing, run voice skills refresh in a terminal.` |
| `Invalid(text)` with `text.ends_with(MIRROR_REMOVED)` | `Voice couldn't bring your assistant's copy of your skills up to date, so it removed the copy. To see what needs fixing, run voice skills refresh in a terminal.` |
| `private::BUSY` (from the lock) | `BUSY` |
| anything else | `Voice couldn't bring your assistant's copy of your skills up to date, and it may be out of date. To see why, run voice skills refresh in a terminal.` |

## 5. The app (`builder/app`)

Add the four commands, at the end, in the same order, to: `src/command_names.rs` `COMMANDS`,
`src/main.rs` `generate_handler!`, `capabilities/default.json` (`allow-list-skills`, …), the literal
list in `tests/lockdown.rs`, and `PARITY` in `builder/api/src/parity.rs`:
`list_skills → [["skills","list"]]`, `agree_skills → [["skills","agree"]]`,
`withdraw_skills → [["skills","withdraw"]]`, `refresh_skills → [["skills","refresh"]]`.
Each in `src/commands.rs` exactly as `review_status` (`#[tauri::command(async)]\npub fn <name>(`).
Build once so `permissions/autogenerated/<name>.toml` exist; they are part of the change. Run
`cargo test` so ts-rs writes the new `builder/ui/src/types/generated/*.ts`; they are part of the
change too.

## 6. The UI (`builder/ui`)

**Service layer:** `listSkills(request: SlugRequest): Promise<Skills>`,
`agreeSkills(request: SkillsRequest): Promise<Skills>`,
`withdrawSkills(request: SkillsRequest): Promise<SkillsWithdrawn>`,
`refreshSkills(request: SlugRequest): Promise<SkillsRefreshed>` in `backend.ts`'s `Backend`,
`app/tauri-backend.ts` (`invoke('<command>', { request })`), `fakeBackend()` in
`test-support.ts`, and the table in `app/tauri-backend.test.ts`.

**`src/screens/Skills.vue`** (export it from `src/index.ts`), `defineProps<{ assistant: Assistant }>()`,
mirroring `TurnOn.vue` and `ClaudeAiFiles.vue`: a `<section class="card mt-4"><div class="card-body p-5">`,
`loading` / `loadError` with a **Try again** button, `busy` disabling every button while any call
runs, `message` (`alert-success`) and `error` (`alert-warning`, `role="alert"`), every failure through
`failureMessage(failure, <fallback>)`. All text through `{{ }}` only. Words in a new
`src/screens/skills/words.ts` (as `panels/words.ts`).

- Title `Your skills`. Under it: `Your assistant can keep its own copy of the skills on this
  computer, with private details taken out. It copies only the skills you say yes to.`
- Loading: the spinner pattern with `Looking for the skills on this computer…`. Load fallback:
  `Voice couldn't look through the skills on this computer. Try again.`
- Empty: `No skills found on this computer.`
- When any row is `waiting`: a line `<n> skill waits for your yes.` / `<n> skills wait for your yes.`
- A `table table-sm table-vcenter card-table` in an `overflow-auto` wrapper, one row per skill:
  the name (bold), under it the place (`text-secondary`, small); the state words; the action.
  | State | Words | Button |
  | --- | --- | --- |
  | `waiting` | `Waiting for your yes` | `Say yes` → `agreeSkills({ slug, ids: [id] })` |
  | `agreed` | `You said yes on <date>` | `Stop copying` → `withdrawSkills({ slug, ids: [id] })` |
  | `moved` | `Moved since you said yes. Say yes again to copy it.` | `Say yes` |
  | `notOffered` | `A claude.ai skill. It's copied only if you say yes.` | `Say yes` |
  | `neverCopied` | `Never copied: it's on your never-copy list.` | none |
  A button's accessible name includes the skill: `aria-label="Say yes to <name>"` /
  `"Stop copying <name>"`. Each row shows its `number` as a small muted `No. <n>` after the name.
- After agree or withdraw, the rows are replaced by the returned list. Agree's `message`:
  `Your assistant may copy it now. Bring its copy up to date to add it.` Withdraw's by `mirror`:
  `notThere` → `Your assistant won't copy it any more.`; `removed` → `Your assistant won't copy it
  any more, and its copy of your skills was removed so nothing you stopped stays in it. Bring the
  copy up to date to make it again.`; `notRemoved` → `error`: `Your assistant won't copy it any
  more, but Voice couldn't remove its copy of your skills, so it may still hold what you stopped.
  To see why, run voice skills refresh in a terminal.` Fallbacks: `Voice couldn't save your choice.
  Try again.`
- `Bring the copy up to date` (`btn btn-primary`), → `refreshSkills({ slug })`. While it runs, the
  spinner pattern with `Bringing your assistant's copy up to date…`. On success: `Copied <copied>
  of <agreed> skills.` and `Since the last refresh: <new> new, <updated> updated, <unchanged>
  unchanged, <removed> removed.` (as `message` when `copied === agreed`, else as `error`), then the
  report `lines` in a `<pre class="mt-3 mb-0">` as `{{ lines.join('\n') }}` when there are any. Then
  reload the list. Fallback: `Voice couldn't bring your assistant's copy up to date. Try again.`
- `notes` under the table, each a `text-secondary` small paragraph.

## 7. Tests first

Red before green for each, then green; mutation-proof assertions (exact sentences, exact requests).

**Rust, `builder/api/tests/skills.rs`** (temp home, `new_assistant_in`, then skills folders with a
`SKILL.md`, as `builder/cli/tests/common/skills.rs` builds them):
1. List: three skills (`notes` waiting, `writing` agreed, one on `never_copy`): waiting first, then
   the CLI's id order; each `number` equals its line number in `voice skills list` (assert by
   running the list through the ingest calls, or a fixed expectation); `agreed_on` only on agreed;
   a fresh `~/.claude/state/pebble` folder is created as the CLI does.
2. Agree an unknown id: `GONE`, `skills.json` unchanged. Agree `[]` and withdraw `[]`: `NONE`.
   Agree a waiting skill: its row is `agreed` with today's date.
3. Withdraw after a good refresh: `removed`, the mirror is gone. Withdraw with no mirror:
   `notThere`. Withdraw an id never agreed: `notThere`, the mirror kept. Withdraw with
   `~/.claude/skills/release.key` linked into the mirror (VO4.1's `denied` fixture): `notRemoved`,
   the linked file keeps its text, the manifest is gone.
4. Refresh: counts and `lines` match what `voice skills refresh` prints for the same home (the
   per-skill lines); a refused skill gives `copied < agreed` and an `Ok`.
5. Refresh failures: an unknown key in `private.json` → the `MIRROR_STALE` row's sentence; a
   project listed inside the mirror → the anything-else sentence or the stale one, whichever
   `refresh` returns (assert the exact one); a held lock → `BUSY`.
6. **No path or private name in any failure:** for every failure above and a listed project whose
   `.claude/skills` is mode 000, the message contains neither the temp home's path nor `pebble`
   nor `Pebble Freight`, and equals one of § 3/§ 4's sentences.
7. `builder/cli/tests/anonymize.rs` and `skills_refresh.rs` stay green untouched (the shared
   removal changed nothing in the CLI).

**App:** `lockdown.rs` and `app_parity.rs` updated as § 5 and green.

**UI, `src/screens/Skills.test.ts`** (`screen`, `fakeBackend`, `deferred`, `button`, as
`ClaudeAiFiles.test.ts`): the spinner then rows, `listSkills` called exactly once with
`{ slug: 'example' }`; waiting rows first as the backend returns them; each state's words and
button (none for `neverCopied`); `Say yes` calls `agreeSkills` with exactly
`{ slug: 'example', ids: ['claude:notes'] }`, the rows replaced and the message shown; `Stop
copying` with each `mirror` value shows its exact sentence (the `notRemoved` one as a warning);
every button disabled while a call is pending; refresh's summary, the `lines` in the `<pre>`, the
warning when `copied < agreed`; empty state; the load error with a backend `{ message }` and the
fallback for `new Error('private detail')`, and Try again; a skill named
`<img src=x onerror=alert(1)>` renders as that literal text and no `img` exists. `VoiceApp.test.ts`:
the Save page shows the `Your skills` card. `src/screens/skills/words.test.ts` for the plural line.

## 8. Records (V1 writes these, not Codex)

`.ai/SECURITY.md` (the webview failure row, and a sentence in the mirror row), `.ai/PROJECT.md`
(the api row), `builder/ui/README.md` (the screens sentence), the slice record.

## 9. Verify and report

`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p voice-ingest -p voice-cli -p voice-api -p voice-app`, then
`pnpm -C builder/ui typecheck`, `pnpm -C builder/ui test`, `pnpm -C builder/ui build`, then
`bash scripts/ci-local.sh` (its macOS sandbox tests can't run inside your own sandbox; say so if
that is where it stops). Report each command and its result, the red output you saw before each
fix, and every file you changed.
