# PB7.5 brief: how well it knows you, and review

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB7.5` entry (PB7 of the Persona Builder
plan): the report (overall and per area, Strong / Getting there / Thin, what would help, the honest
line under 30 kept-back questions), and review: keeping an answer, how many tests the answers have
made, and a refresh: drafted, shown as a readable before-and-after with each reason, approved or
rejected, then tested with progress, saying whether it was kept or undone. Read the repo's
`AGENTS.md` and `.ai/PROJECT.md` first, then `plans/briefs/PB7.4-build-and-interview.md` (the
slice this one copies most: its patterns, its tests and its rules hold here unless this brief says
otherwise), `plans/briefs/PB4.1-how-well-i-know-you.md`, `plans/briefs/PB4.2-kept-answers.md`,
`plans/briefs/PB4.3a-refresh-propose.md` and `plans/briefs/PB4.3b-refresh-apply.md` (what the CLI
already does), `docs/spec/growth-loop.md`, then the code: `builder/build/src/report.rs`,
`builder/build/src/text.rs` (the report's CLI words), `builder/grow/src/{keep,refresh,apply,diff,store,text}.rs`,
the CLI's `report`, `keep_answer`, `cases`, `refresh_propose`, `refresh_show`, `refresh_answer` and
`refresh_apply` in `builder/cli/src/main.rs`, `builder/api/src/{building,history,assistants,dto,parity}.rs`,
`builder/api/tests/building.rs`, `builder/cli/tests/refresh.rs`, the stubs
`builder/grow/tests/fixtures/{refresh_stub,apply_stub}.py`, `builder/app/` and `builder/ui/`.
Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security:
`~/.codex/skills/security-and-hardening/SKILL.md`; structure: `~/.codex/skills/frontend/SKILL.md`
and `~/.codex/skills/tabler-ui/SKILL.md`; looks: `~/.codex/skills/design-guide/SKILL.md`; every
sentence a person sees: `~/.codex/skills/plain-language/SKILL.md` (the sentences below are final:
use them word for word); review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true` and pnpm with `--offline`. **No new crate or package.**
`builder/api/Cargo.toml` gains one path dependency already in the workspace,
`persona-builder-grow = { path = "../grow" }`. If you think anything else is needed, stop and say so.

## What this slice does

On *How well it knows you* the person reads their last build's report in plain words: how well
their assistant knows them overall and for each kind of decision, what would help where it is thin,
and, when too few questions were kept back to test it, that honestly. On *Review* they teach their
assistant: they write how they'd decide something (or where it got one wrong) and it is kept as a
test; they see how many tests their answers have made; and they ask for a refresh. Claude drafts a
change to how the assistant decides, the person reads each point as before and after with its
reason, and approves or rejects it. An approved change is tested on all their answers while they
watch, and it is kept only if every new test passes and none of the others gets worse; otherwise it
is undone and the assistant stays as it was.

## Decisions (made, do not reopen)

### 1. The growth loop crate (`builder/grow`)

The app must run exactly what the CLI runs, under the app's own lock (PB7.3a: a lock that never
recreates the state folder, with the permission checked again under it). So:

- `keep.rs`: split `keep` into two private helpers, `check(store, member, filters, input, now) ->
  Result<(Signal, Case)>` (everything before the lock today) and `save(store, signal, case, now) ->
  Result<Kept>` (everything after it). `pub fn keep` = `check`, `store.lock()`, `save`, as today, in
  that order. New `pub fn keep_locked(store, member, filters, input, now) -> Result<Kept>` = `check`
  then `save`, with the doc line "The caller holds the assistant's state lock."
- `refresh.rs`:
  - `pub fn waiting(store: &Store, member_dir: &Path) -> Result<Option<(Proposal, bool)>>`: the open
    change (today's private `pending` at the current `decisions.md` version) and whether it is
    approved; it runs `check_stored` on what it finds. `open` becomes
    `Ok(waiting(store, member_dir)?.map(|(p, _)| p))`.
  - split `answer` the same way: `pub fn answer_locked(store, member_dir, id, decision, now) ->
    Result<Approval>` holds today's body without the lock; `answer` = `store.lock()` then
    `answer_locked`.
- `apply.rs`:
  - `measure` gains a last parameter `progress: &mut dyn FnMut(&str)`. Only on the path that tests
    (not when a measurement already exists), it sends `text::testing_now(tests.len())` before the
    first answer batch for the current decisions, `text::testing_change(tests.len())` before the
    first batch for the changed ones, and, inside `grade_answers`, `text::CHECKING_ANSWERS` once
    before the first grading batch when there is at least one rubric item (pass `progress` into
    `grade_answers`).
  - `reason` becomes `pub fn reason(measurement: &Measurement) -> String` (unchanged body).
  - `pub struct Growth { pub kept: usize, pub undone: usize, pub passing: usize, pub of: usize }`
    and `pub fn growth_counts(store: &Store, since: Option<&str>) -> Result<Option<Growth>>`
    holding today's counting; `growth` becomes `growth_counts` mapped through `text::growth`. Its
    output does not change.
- `diff.rs`: `pub fn removed_lines(diff: &str) -> Vec<&str>`: each `-` row's content after the two
  file headers (`---`/`+++` are skipped as `added_lines` skips them), with one trailing `\r`
  removed unless the next row is `\ No newline at end of file`. `added_lines` is unchanged.
- `text.rs`:
  - `pub fn testing_now(n: usize) -> String`: "Testing your assistant as it is now on {n} tests."
    ("test" when n is 1)
  - `pub fn testing_change(n: usize) -> String`: "Testing it with the change on {n} tests." ("test"
    when n is 1)
  - `pub const CHECKING_ANSWERS: &str = "Checking the written answers against what you wanted.";`
- The CLI's `refresh apply` passes `&mut |line| println!("{line}")`, so it prints the same lines
  before its outcome; `keep`, `refresh approve` and `refresh reject` keep calling `keep` and
  `answer`. Every other CLI output is unchanged. If an existing CLI test breaks only because the
  new lines come first, change that assertion to look past them and say so in your report.

### 2. The api (`builder/api`)

New module `src/review.rs` (`pub mod review; pub use review::*;` in `lib.rs`). In `building.rs`,
make `open`, `spend` and `call_failure` `pub(crate)` and reuse them. Every command has a testable
`_in` form and a form reading `HOME` (and, where Claude Code runs, `PATH` and
`claude::fallbacks(&home)`), as the others do. The app always uses `Cap::default()` and
`claude::DEFAULT_MODEL`, as in PB7.4. The member folder is always `assistants::assistant_dir(home,
slug)`; the webview never names a path.

**The webview names only a change it was shown.** `answer_refresh` accepts an id only when it is
the change `refresh::waiting` finds and that change is not yet approved; `apply_refresh` only when
it is the waiting change and it is approved. Anything else is `CHANGE_GONE`, before any write or
any Claude Code call.

Sentences (each a `const`; a `Failure` is always one of these, one of `history.rs`'s `FIND`,
`PERMISSION`, `BUSY`, one of `building.rs`'s `LEDGER`, `BUILD_SAVE` and `cap_reached`, or one of the
agent crate's `NOT_INSTALLED`, `DOES_NOT_RUN`, `SIGNED_OUT`, `FAILED`; never an error's own text,
a path or a record's words):

| Name | Sentence |
| --- | --- |
| `REVIEW_SETTINGS` | Something in this assistant's settings stops the review. Use the persona-builder command to see what. |
| `GROWTH_DAMAGED` | Your assistant's kept answers can't be read, so nothing was changed. Use the persona-builder command to see what's wrong. |
| `KEEP_EMPTY` | Say what was being decided and what you'd want, then keep it. |
| `KEEP_BAD` | Persona Builder couldn't keep that answer. Check it and try again. |
| `KEEP_PRIVATE` | That looked personal or secret, so it wasn't kept. Leave out names and private details, then try again. |
| `NOT_BUILT_YET` | Build your assistant first: there's nothing to refresh yet. |
| `ALREADY_OPEN` | A change is already waiting for your answer. Approve or reject it first. |
| `REFRESH_FAILED` | Claude's draft didn't fit, twice, so nothing was saved. Try again. |
| `CHANGE_GONE` | That change isn't waiting for your answer any more. Close it and open it again. |
| `CHANGE_OLD` | Your assistant or your answers changed since this was drafted, so it can't be approved. Reject it, then draft a new one. |
| `ANSWER_SAVE` | Persona Builder couldn't save your answer. Check there's space on this computer, then try again. |
| `TEST_OLD` | Your assistant or your answers changed since you approved this change, so it wasn't tested and nothing changed. Use the persona-builder command to see what to do. |
| `TEST_FAILED` | The test stopped before it finished, so your assistant is as it was. What was already finished is kept: test again to carry on. |
| `APPLY_SAVE` | The change was tested, but the result couldn't be saved. Use the persona-builder command to finish it. |

DTOs in `dto.rs` (answers camelCase; requests `deny_unknown_fields`; counts `u32` through `count`;
enums `rename_all = "camelCase"` as `SaveKind` is; the ones holding `Spend` derive `PartialEq`
without `Eq`). Progress reuses PB7.4's `BuildProgress { line, spend }`.

```rust
pub enum ReportLevel { Strong, GettingThere, Thin }
pub enum ReportHelp { Interview, Claudeai, Time }
pub struct ReportTest { pub right: u32, pub of: u32 }
pub struct ReportArea { pub area: String, pub level: ReportLevel, pub examples: u32, pub help: Vec<ReportHelp> }
pub struct BuildReport { pub built: String, pub overall: ReportLevel, pub answered: u32, pub thin_below: u32, pub held_out: u32, pub test: Option<ReportTest>, pub areas: Vec<ReportArea> }
pub struct GrowthCounts { pub kept: u32, pub undone: u32, pub passing: u32, pub of: u32 }
pub struct ReportView { pub report: Option<BuildReport>, pub unreadable: bool, pub growth: Option<GrowthCounts> }
pub struct TestCounts { pub waiting: u32, pub in_use: u32, pub retired: u32 }
pub struct AreaTests { pub area: String, pub tests: TestCounts }
pub struct RefreshPoint { pub why: String, pub before: Vec<String>, pub after: Vec<String> }
pub struct RefreshChange { pub id: String, pub drafted: String, pub summary: String, pub points: Vec<RefreshPoint>, pub learned_from: u32, pub approved: bool }
pub struct ReviewStatus { pub built: bool, pub areas: Vec<String>, pub tests: TestCounts, pub by_area: Vec<AreaTests>, pub change: Option<RefreshChange> }
pub struct KeepAnswerRequest { pub slug: String, pub area: String, pub situation: String, pub wanted: String, pub assistant_said: String }
pub struct RefreshDrafted { pub change: Option<RefreshChange>, pub spend: Spend }
pub struct RefreshAnswerRequest { pub slug: String, pub id: String, pub approve: bool }
pub struct RefreshApplyRequest { pub slug: String, pub id: String }
pub struct RefreshApplied { pub kept: bool, pub reason: String, pub spend: Spend }
```

Private helpers:
- *status*: `book(member).categories` (error `REVIEW_SETTINGS`); `Store::open` (`REVIEW_SETTINGS`);
  `store.cases()` (`GROWTH_DAMAGED`); `tests` counts every case by status (`new` waiting, `active`
  in use, `retired` retired); `by_area` the same per category, in the categories' order, only areas
  with at least one case; `refresh::waiting(store, dir)`: an `Error::Invalid` whose text equals
  `persona_builder_grow::text::NOT_BUILT` gives `built: false` and no change, another error
  `GROWTH_DAMAGED`, else `built: true` and the change.
- *change(proposal, approved)*: `id`; `drafted` the first 10 characters of `created`; `summary`;
  one `RefreshPoint` per `changes` entry: its `why`, `diff::removed_lines` as `before`,
  `diff::added_lines` as `after` (each line owned, no other change); `learned_from` the number of
  `cases`; `approved`.

Commands (`Result<T, dto::Failure>`):

| Command | Does | Errors |
| --- | --- | --- |
| `show_report(SlugRequest) -> ReportView` | `readable`; `report::read`: `Ok(Some)` maps every field (`held_out` from `held`, `help` from `strengthen`, `thin_below` is `interview::THIN_BELOW`), `Ok(None)` no report, `Err` no report and `unreadable: true`; growth: `Store::open` then `apply::growth_counts(&store, Some(&report.built))` when there is a report, else `None` as `since`; any growth error gives `growth: None` | `readable`'s |
| `review_status(SlugRequest) -> ReviewStatus` | `readable`; *status* | *status*'s |
| `keep_answer(KeepAnswerRequest) -> ReviewStatus` | trim each text field; `situation` or `wanted` empty → `KEEP_EMPTY`; any of the three over 2,000 characters or `area` over 100 → `KEEP_BAD` (both before the lock); then `locked`; `area` not among the categories → `KEEP_BAD`; `Filters::compile` (`REVIEW_SETTINGS`); any non-empty field for which `filters.kind` answers → `KEEP_PRIVATE`; builds `{"kind": "correction" when assistant_said is non-empty else "instruction", "area", "situation", "wanted", "persona_said" (only when non-empty), "source": "review"}` and passes its bytes to `keep::keep_locked` with `store::utc_now()`; answers *status* | `readable`'s and `locked`'s; `keep_locked`'s error → `KEEP_BAD` |
| `draft_refresh(SlugRequest) -> RefreshDrafted` | `readable`; `Filters::compile` (`REVIEW_SETTINGS`); `claude::find_in` + `check`; `open`; the permission checked again (`PERMISSION`); `Store::open`; `refresh::waiting`: `NOT_BUILT` text → `NOT_BUILT_YET`, another error `GROWTH_DAMAGED`, a change → `ALREADY_OPEN` (all before any call); `refresh::propose` with `store::utc_now()`: `Nothing` → `change: None`, `Drafted(p)` → *change(p, false)*; `spend` after | Claude Code: `claude_message`; `open`'s; `propose`'s error → `call_failure(e, &session, REFRESH_FAILED)` |
| `answer_refresh(RefreshAnswerRequest) -> ReviewStatus` | `locked`; `Store::open` (`REVIEW_SETTINGS`); `refresh::waiting` (errors as in *status*) must give `(p, false)` with `p.id == id`, else `CHANGE_GONE`; `refresh::answer_locked` with `Approved` or `Rejected` and `store::utc_now()`; answers *status* | an approval's error → `CHANGE_OLD`; a rejection's → `ANSWER_SAVE` |
| `apply_refresh(RefreshApplyRequest, progress) -> RefreshApplied` | `readable`; `Filters::compile`; `claude::find_in` + `check`; `open`; the permission checked again; `Store::open`; `refresh::waiting` must give `(p, true)` with `p.id == id`, else `CHANGE_GONE`; `apply::measure` with each line sent as a `BuildProgress` carrying the spend read just then (the last value kept if that read fails, as `build_assistant` does); `apply::finish`; answers `kept` (outcome `Keep`), `reason` = `apply::reason(&measurement)` (never the stored `reason` text) and the spend after | Claude Code: `claude_message`; `open`'s; `measure`'s error, in this order: text equal to grow's `CHANGE_DOES_NOT_FIT`, `DECISIONS_CHANGED_TESTING` or `cases_moved(id)` → `TEST_OLD`; else `call_failure(e, &session, TEST_FAILED)`; `finish`'s error → `APPLY_SAVE` |

`parity.rs` gains, after `keep_interview_answers`: `show_report` → `[["report"]]`, `review_status` →
`[["cases"], ["refresh", "show"]]`, `keep_answer` → `[["keep"]]`, `draft_refresh` →
`[["refresh", "propose"]]`, `answer_refresh` → `[["refresh", "approve"], ["refresh", "reject"]]`,
`apply_refresh` → `[["refresh", "apply"]]`.

### 3. The Tauri app (`builder/app`)

- `COMMANDS`, the capability's `allow-…` list and `generate_handler!` gain the six names in the
  order above (36 in all). No new permission, plugin or capability.
- Each is `#[tauri::command(async)]`, one call into the api. `apply_refresh` takes `request` and
  `on_progress: tauri::ipc::Channel<persona_builder_api::dto::BuildProgress>`, as `build_assistant`.

### 4. The screens (`builder/ui`)

Commit every new generated type. `Backend` gains (generated types only):

```ts
showReport(request: SlugRequest): Promise<ReportView>
reviewStatus(request: SlugRequest): Promise<ReviewStatus>
keepAnswer(request: KeepAnswerRequest): Promise<ReviewStatus>
draftRefresh(request: SlugRequest): Promise<RefreshDrafted>
answerRefresh(request: RefreshAnswerRequest): Promise<ReviewStatus>
applyRefresh(request: RefreshApplyRequest, onProgress: (progress: BuildProgress) => void): Promise<RefreshApplied>
```

`tauri-backend.ts`: `applyRefresh` makes a `Channel<BuildProgress>` as `buildAssistant` does; the
rest as the others. `index.ts` also exports `Report` and `Review`. Money and the spend line as in
PB7.4 ("Today, about $<spent> of the $<cap> daily limit, as Claude Code estimates it."). Numbers use
`toLocaleString()`.

**The frame (`PersonaBuilder.vue`).** Every step now has a screen. With an assistant open, every
step is a button (drop the per-id lists and the "Not here yet" text); `report` and `review` are
enabled once permission is given, with the same disabled text as `build`. `view` gains `'report'`
and `'review'`. `Report` is mounted with `v-else-if="view === 'report' && assistant"` (so it reads
the report afresh each time). `Review` copies `Build`'s mounting: a `reviewOpened` ref (set when
`view` becomes `review`, reset with `buildOpened` by `open`, `made`, `home` and `forgotten`), then
`v-show="view === 'review'"`, keyed by the slug, so a test keeps running while the person looks
elsewhere. `Review` emits `running(boolean)`: while it is true, "Your assistants" is disabled and
the muted line "Wait for the test to finish to choose another assistant." shows (the build's line
stays as it is). `Build` emits `report` (below): the frame goes to `report`. `Report` emits
`build` and `next`: the frame goes to `build` or `review`.

**`build/BuildRun.vue`**: the muted line "Next, see how well it knows you." becomes the secondary
button "See how well it knows you", emitting `report`; `Build` passes it on as `report`.

**`screens/Report.vue`** (prop `assistant`; emits `build`, `next`): heading "How well it knows you".
On mount `showReport`; spinner with the visually hidden "Reading your report…"; failure in an
`alert alert-danger` (fallback `REVIEW_SETTINGS`).
- no report and not `unreadable`: "There's no report yet. Build your assistant first." and the
  primary button "Go to Build" (emits `build`).
- `unreadable`: `alert alert-warning` "Your last report couldn't be read, so it isn't shown. Build
  your assistant again to make a new one." and the same button.
- a report: one Tabler card.
  - the line "Overall:" then the level as a badge: Strong `badge bg-green-lt`, Getting there
    `badge bg-yellow-lt`, Thin `badge bg-secondary-lt`; words "Strong", "Getting there", "Thin".
  - muted: "From the build on <built>."
  - overall Thin: "Your history holds <answered> answered questions. It takes about <thinBelow> to
    know you well." Otherwise, with a test: "It passed the test on <strong> of <areas> kinds of
    decision, and got <right> of <of> test questions right." (`strong` counts Strong areas).
    Otherwise: "There's enough history to learn from, but too little to test yet."
  - no test: "Only <heldOut> questions were kept back to test your assistant, fewer than 30, so
    there's too little to measure yet. Until it can be tested, it decides nothing on its own." and,
    when `answered < thinBelow`, "The interview fills out a thin history." with the secondary
    button "Go to the interview" (emits `build`).
  - areas: none → "No kinds of decision yet." Otherwise a `table table-vcenter card-table` with the
    headings "Kind of decision", "How well" and "What would help"; each row: the area; the badge
    (as above) and, for Thin, a muted small line "<examples> example(s) so far" ("example" for 1);
    and, for an area that is not Strong, its `help` as a `ul.list-unstyled mb-0`: Interview "Answer
    the interview", Claudeai "Add your claude.ai chats", Time "Keep using Claude: it learns from
    every answer"; a Strong area shows the muted "Nothing needed".
  - growth, when present: "Since the build: <kept> change(s) kept and <undone> undone. Your
    assistant passes <passing> of <of> tests from your answers." ("change" for 1).
- under the card, when there is a report: the primary button "Next: review what it learns" (emits
  `next`).

**`screens/Review.vue`** (prop `assistant`; emits `running`): heading "Review", the line "Teach
your assistant from your own answers. Persona Builder turns what it learns into a change you can
read, and keeps it only if your assistant does at least as well on every test." On mount
`reviewStatus`; spinner with the visually hidden "Reading your answers…"; failure fallback
`REVIEW_SETTINGS`. Then three Tabler cards in a column, each given the `slug` and the current
`ReviewStatus`; each emits `changed(ReviewStatus)` and `Review` replaces its status with it.

**`review/TeachIt.vue`**, card title "Teach it something":
- the line "Tell it how you'd decide something, or where it got one wrong. Each answer becomes a
  test it has to pass."
- a form: `select` labelled "Kind of decision" (the `areas`, the first chosen); `textarea`s
  labelled "What was being decided", "What you'd want" and "What your assistant said, if it got
  this wrong (optional)", each `rows="2"`, `maxlength="2000"`. The primary button "Keep this
  answer", disabled while either of the first two is blank or a call runs.
- on success: `alert alert-success` "Kept. It's now one of the tests your assistant is checked on
  at its next refresh."; the three text fields are cleared, the area stays; emits `changed`.
  Failure fallback `KEEP_BAD`; the fields stay as typed.
- no areas (empty `areas`): only "Your assistant's settings name no kinds of decision yet."

**`review/Tests.vue`**, card title "Tests from your answers":
- all three counts 0: "You haven't given your assistant any answers to learn from yet."
- otherwise: "<waiting> waiting for a refresh, <inUse> in use, <retired> retired." and, under it, a
  `ul.list-unstyled` with one line per `byArea` entry, "<area>: <waiting> waiting for a refresh,
  <inUse> in use, <retired> retired."

**`review/Refresh.vue`**, card title "Refresh" (emits `changed` and `running`):
- no change:
  - the line "A refresh drafts a change to how your assistant decides, from the answers waiting
    for it. You read the change and approve or reject it first."
  - the primary button "Draft a change" (`draftRefresh`), disabled when `built` is false, when
    `tests.waiting` is 0, or while a call runs; under it the muted reason: not built "Build your
    assistant first."; nothing waiting "Keep an answer first: there's nothing new to learn from."
  - while drafting: the indeterminate `ReadProgress` labelled "Drafting" and the button "Drafting a
    change…", disabled. Then the spend line from `RefreshDrafted.spend`. `change: null` →
    `alert alert-info` "There's nothing new to learn from yet. Keep answering your assistant, then
    try again." A change → emits `changed` with the status reloaded (`reviewStatus`). Failure
    fallback `REFRESH_FAILED`.
- a change, not approved:
  - "Drafted <drafted>." (muted), the summary in bold, then "Learned from <learnedFrom> of your
    answers." (muted).
  - each point in order: a heading `h4` "<n>. <why>"; then a `row` of two `col-6`, headed by the
    small muted "Before" and "After"; each a `ul.list-unstyled` of its lines; an empty Before shows
    the muted "Nothing: this is new." and an empty After the muted "Taken out."
  - the primary button "Approve" and the secondary button "Reject" (`answerRefresh` with `approve`
    true or false), both disabled while a call runs. Approve → emits `changed` (the change now
    approved). Reject → `alert alert-info` "Rejected. Nothing changed. Your answers stay saved for
    the next refresh." and emits `changed`. Failure fallback `ANSWER_SAVE`.
- a change, approved:
  - "You approved this change. Next, Persona Builder tests it on all your answers: it keeps the
    change only if every new test passes and none of the others gets worse. It uses a little of
    your Claude plan." The points are shown as above, without the buttons.
  - the primary button "Test it" (`applyRefresh`); emits `running(true)` at the start and
    `running(false)` at the end, whatever the outcome. While it runs: the indeterminate
    `ReadProgress` labelled "Testing", the lines received so far in a `ul.list-unstyled` with
    `aria-live="polite"` inside a `div` with `overflow-auto` and `style="max-height: 16rem"`, the
    spend line from each message, and the button "Testing the change…", disabled.
  - kept: `alert alert-success` "Kept. Your assistant now works this way. <reason>"; undone:
    `alert alert-info` "Undone. Your assistant is as it was, and your answers stay saved for the
    next refresh. <reason>". The spend line from `RefreshApplied.spend`. Then it reloads the status
    (`reviewStatus`) and emits `changed`, keeping the outcome alert above "Draft a change". Failure
    in `alert alert-danger`, fallback `TEST_FAILED`; the lines stay.
- every button in the card is disabled while a call runs.

Tabler classes only; no custom colors; no Tabler JavaScript; light and dark from the theme; no
`<a>` anywhere in these screens. Every failure through `failureMessage(error, <fallback>)`. Text
from the report and the growth records is shown as text (Vue's escaping), never `v-html`.

### 5. Docs

- `.ai/PROJECT.md`: the `builder/api/` row gains `review.rs` (the report, kept answers and the
  refresh) and its test `builder/api/tests/review.rs`.
- `builder/ui/README.md`: the screens line gains "the report and review".

## Tests (write each first and watch it fail on its assertion)

Fixtures are invented (no real person): reuse the api build tests' assistant setup (an assistant
made in a temp home's `.claude/crew/<slug>/` with permission given), a `decisions.md` written in
the test (copy the shape `builder/cli/tests/refresh.rs` uses), and the grow crate's
`refresh_stub.py` and `apply_stub.py` (Python 3 is already a CI requirement); temp homes only,
`PATH` passed in through the `_in` forms, never the process's.

Rust:
- grow: `keep_locked` keeps the same signal and case `keep` keeps for the same input, and, called
  while the test holds the state lock, does not answer busy; `answer_locked` likewise for
  `answer`; `waiting` answers `(p, false)` for an unanswered change, `(p, true)` once approved, and
  `None` once measured; `removed_lines` answers the `-` rows of a diff with a replaced, an added
  and a removed line (and a CRLF file); `measure`'s progress gets, in order, the "as it is now"
  line, the "with the change" line and, with rubric tests, `CHECKING_ANSWERS`, and nothing when
  the measurement already exists; `growth_counts` matches what `growth` printed before.
- CLI: `refresh apply` prints the testing lines before "Kept. Your assistant now works this way."
- `builder/api/tests/review.rs`:
  - every command, for an assistant with no permission, answers `PERMISSION` and writes nothing;
    an unknown slug and `../x` answer `FIND`.
  - `show_report`: no report → `report: null`, `unreadable: false`; a damaged `report.json` →
    `unreadable: true`; a report (write one with `report::write`) maps every field, `thinBelow`
    150, `heldOut`, `help`; with a kept and an undone measurement after the build day, `growth`
    equals `growth_counts`.
  - `review_status`: no `decisions.md` → `built: false`; counts by status and by area in the
    categories' order, leaving out areas with none.
  - `keep_answer`: blank → `KEEP_EMPTY`; 2,001 characters → `KEEP_BAD`; an unknown area →
    `KEEP_BAD`; a field holding something the filters catch (use what the grow keep tests use) →
    `KEEP_PRIVATE` and nothing kept; a good answer with "what your assistant said" is kept as a
    `correction`, without it as an `instruction`, with `source` "review"; with the state lock held,
    `BUSY`; after forgetting between the check and the lock (as PB7.3a's tests do), `PERMISSION`.
  - `draft_refresh`: not built → `NOT_BUILT_YET`; no waiting tests → `change: null` and no call;
    with the stub, a change whose points' `before` and `after` equal the diff's removed and added
    rows; a second draft → `ALREADY_OPEN` with no call (the stub's call count shows it).
  - `answer_refresh`: an id that isn't the waiting change → `CHANGE_GONE` and no approval written;
    approving twice → `CHANGE_GONE` the second time; reject → the status has no change; approving
    after `decisions.md` changed → `CHANGE_GONE`.
  - `apply_refresh`: an unapproved change's id → `CHANGE_GONE` with no call; with `apply_stub.py`,
    kept → `kept: true`, `reason` equal to grow's `KEEP_REASON`, `decisions.md` changed, and the
    progress lines equal the CLI's testing lines, each with a `spend`; an undone run → `kept:
    false`, `decisions.md` byte-for-byte as before; a cap seeded at $49 → `cap_reached`.
  - no `Failure` message in this file's tests contains the temp home's path.
  - DTOs camelCase (`heldOut`, `thinBelow`, `gettingThere`, `inUse`, `byArea`, `learnedFrom`,
    `assistantSaid`); each request refuses an unknown field.
- `builder/cli/tests/app_parity.rs` and `builder/app/tests/lockdown.rs` pass with the new rows and
  the 36 commands.

Vitest:
- `Report.test.ts`: no report shows "Go to Build", which emits `build`; unreadable shows the
  warning; a Thin report shows the history line, the untested line and "Go to the interview"; a
  tested report shows the passed line; the areas table shows each level's badge class, the
  examples line on a Thin area ("1 example so far" and "3 examples so far"), the help lines and
  "Nothing needed"; growth singular and plural; "Next: review what it learns" emits `next`.
- `TeachIt.test.ts`: the button is disabled until both fields are filled; keeping sends `{ slug,
  area, situation, wanted, assistantSaid }`, shows the kept line, clears the fields, keeps the area
  and emits `changed`; a failure keeps the fields and shows the fallback for a non-`Failure`.
- `Tests.test.ts`: none, and the totals and per-area lines.
- `Refresh.test.ts`: "Draft a change" disabled with each reason; drafting shows the progress, then
  the nothing-new line or the change; the points show before and after with "Nothing: this is
  new." and "Taken out."; Approve and Reject call `answerRefresh` with `approve` true and false;
  the approved change shows "Test it", which shows the lines as they arrive and the spend from the
  last message, emits `running` true then false, and shows the kept or undone line with its
  reason; a failure shows the fallback in `alert-danger` with the lines kept.
- `Review.test.ts`: a child's `changed` replaces the status the others get.
- `BuildRun.test.ts`: "See how well it knows you" emits `report` after a build.
- `PersonaBuilder.test.ts`: `report` and `review` disabled until permission; "See how well it knows
  you" goes to the report and "Next: review what it learns" to review; while Review's `running` is
  true "Your assistants" is disabled with its line, and moving to another step and back keeps the
  test's lines.
- `tauri-backend.test.ts`: each new method invokes its snake-case command; `applyRefresh` passes
  `{ request, onProgress }` with a `Channel` whose messages reach the callback.

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). Report each with its red result:
1. drop the waiting-change id check from `answer_refresh` → its `CHANGE_GONE` test red;
2. let `apply_refresh` accept an unapproved waiting change → its `CHANGE_GONE` test red;
3. make `apply::reason` return `""` → the api kept-reason assertion red;
4. drop the filters check from `keep_answer` → the `KEEP_PRIVATE` test red;
5. make `keep_answer` call `keep` instead of `keep_locked` → the keep test red (busy);
6. drop the permission re-check after `open` in `apply_refresh` → a forget-between test red (add
   it, as PB7.4's build test does);
7. stop sending the "with the change" progress line → the grow progress test red;
8. swap `before` and `after` in *change* → the draft test red;
9. add a command to `COMMANDS` only → lockdown red;
10. enable `review` before permission → PersonaBuilder test red;
11. mount `Review` with `v-if="view === 'review'"` → the keeps-the-lines test red.

## Not in this slice

- No stop for a refresh's test: it writes nothing in the assistant's folder until it has finished,
  and closing the app mid-test leaves the assistant as it was (the call already sent is counted at
  its budget, as today). Finishing a test whose result couldn't be saved stays a CLI step
  (`refresh apply` with its id), as `APPLY_SAVE` says. No marks on the report's individual
  decisions (the spec's `verdict` signals) and no `pick` signals from the app yet: the app keeps
  `instruction` and `correction` only.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), `cargo fmt --check` and
`cargo clippy --workspace --all-targets --locked -- -D warnings` are clean, the generated types are
current (no diff after `cargo test`), and the mutation list is reported. Don't commit; Claude
reviews, commits and runs the app.
