# PB7.4 brief: build, stop, spend and the interview

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB7.4` entry (PB7 of the Persona Builder
plan): `build` run in the background with its plain progress lines shown as they come, today's
spend against the cap, a stop that leaves the assistant as it was, and, when the history is thin,
the interview: drafted, shown four questions at a time, the answers kept, then put together and
built again. Read the repo's `AGENTS.md` and `.ai/PROJECT.md` first, then
`plans/briefs/PB7.1-app-frame.md`, `plans/briefs/PB7.2-consent-new-connect.md` and
`plans/briefs/PB7.3-sources.md` (the bridge, the screens and the rules this slice grows),
`builder/api/src/history.rs` (`readable`, `lock`, `failure`, `count`, the sentences, the `_in`
forms), `builder/api/src/assistants.rs` (`claude_check_in` and its message mapping),
`builder/api/src/save.rs` (`SignIns`, `sign_ins()`: the process-wide state pattern this slice
copies), `builder/agent/src/child.rs`, `builder/agent/src/claude.rs`, `builder/agent/src/ledger.rs`,
`builder/build/src/build.rs` (`build`, `Inputs`, `Built`), `builder/build/src/draft.rs`,
`builder/build/src/report.rs`, `builder/build/src/interview.rs`, `builder/steps/src/round.rs`, the
CLI's `build`, `spend`, `interview_draft` and `interview_answer` in `builder/cli/src/main.rs`, the
build crate's stub `builder/build/tests/fixtures/claude_stub.py` and the CLI's
`builder/cli/tests/build.rs` (how a stub `claude` is put on `PATH`), `builder/app/` and
`builder/ui/`. Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security:
`~/.codex/skills/security-and-hardening/SKILL.md`; structure: `~/.codex/skills/frontend/SKILL.md`
and `~/.codex/skills/tabler-ui/SKILL.md`; looks: `~/.codex/skills/design-guide/SKILL.md`; every
sentence a person sees: `~/.codex/skills/plain-language/SKILL.md` (the sentences below are final:
use them word for word); review bar: `~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true` and pnpm with `--offline`. **No new crate or package.**
`builder/api/Cargo.toml` gains two path dependencies already in the workspace,
`persona-builder-build = { path = "../build" }` and `persona-builder-steps = { path = "../steps" }`.
If you think anything else is needed, stop and say so.

## What this slice does

On the *Build* step the person sees what building has spent today against the daily limit, starts
the build, and watches its plain lines arrive while it works. They can stop it at any time: the
running Claude Code call is ended and their assistant's files stay exactly as they were. When their
history is thin, an interview card offers twenty questions drafted by Claude, shown four at a time;
their answers are kept as they go, then added to their history, and the assistant is built again.

## Decisions (made, do not reopen)

### 1. A stop for Claude Code calls (`builder/agent`)

- `child.rs`: `run` gains a last parameter `stop: Option<&AtomicBool>`. `Ran` gains `Stopped`.
  `settle` checks the flag on every pass of its loop (before the time-limit check): when it is set,
  it kills the group (`kill_group`), reaps the child (`child.wait()`), and the run answers
  `Ran::Stopped`. Every existing caller passes `None` except `Claude::call`.
- `claude.rs`: `Claude` gains `stop: Option<Arc<AtomicBool>>` (`None` from `at`) and
  `pub fn with_stop(self, stop: Arc<AtomicBool>) -> Claude`. In `call`, after the argument checks
  and before `session.start`, a set flag answers `Err(Error::Invalid(text::STOPPED.into()))` with
  no ledger line and no child. `call` passes the flag to `child::run`. `judge` maps `Ran::Stopped`
  to outcome `"stopped"`, no usage, `cost: None` (so the whole budget is counted: money may have
  been spent), result `Err(invalid(text::STOPPED))`. `ask` (the version and sign-in check) never
  stops.
- `text.rs`: `pub const STOPPED: &str = "Persona Builder stopped Claude Code, as you asked.";`
- The CLI never sets a stop (Ctrl-C ends the CLI as before).

### 2. One build path for the CLI and the app (`builder/build`)

New module `src/run.rs` (`pub mod run;` in `lib.rs`), the CLI's `build` body split in three so the
app runs exactly what the CLI runs:

```rust
pub struct Prepared { pub book: Book, pub codebook: String, pub corpus: Vec<u8>, pub answered: usize, pub filters: Filters }
pub fn prepare(member: &Member, member_dir: &Path, home: &Path, corpus_name: &str) -> Result<Prepared>
impl Prepared { pub fn inputs<'a>(&'a self, member: &'a Member, corpus_name: &'a str, model: &'a str) -> Inputs<'a> }
pub struct Checked { decisions: String, entries: …, report: Report }   // fields private; whatever draft::entries returns
pub fn check(member: &Member, member_dir: &Path, prepared: &Prepared, built: &Built, model: &str) -> Result<Checked>
pub fn save(member: &Member, member_dir: &Path, home: &Path, checked: Checked) -> Result<Report>
```

- `prepare`: today's lines in today's order: `book(member)`, `report::check_names`,
  `draft::read_codebook(member_dir)`, `draft::preflight(member_dir)`, `private::place` +
  `validate_name` + `read_private_at`, `interview::answered(read_records(…))`,
  `Filters::compile`.
- `check`: `draft::decisions`, the local day, `draft::entries`, `draft::check_private`,
  `report::report`, `report::check`. Nothing written.
- `save`: `draft::read_text` of the measurements, `draft::measurements`, `draft::write`, then
  `report::write`; answers the report.
- The CLI's `build` becomes: `member_and_home`, `run::prepare`, the cap, `claude::find` + `check`,
  `Session::open`, the thin line, `build::build` printing each line, `println!(CHECKING)`,
  `run::check`, `run::save`, then `written`, the rendered report and the spend line, as today. Its
  output does not change (the CLI's build tests prove it).

`interview.rs`:
- `pub const ANSWERS_FILE: &str = "interview-answers.json";` (the app's answers so far, in the
  round-answers shape).
- `pub fn weights_from_report(book: &Book, report: Option<&Report>) -> Result<Vec<(String, Option<usize>)>>`:
  `None` gives what `weights(book, None)` gives; a report gives each kind's count from the area of
  the same name (`examples`, which is the tally's `questions + messages`), 0 for a kind with no
  area, sorted as `weights` sorts. `weights` and this share one private helper over
  `Option<&[(String, usize)]>`.
- The CLI's `interview draft` without `--labels` now weights from the last report
  (`report::read`; an unreadable report counts as none), so the CLI and the app draft alike;
  `drafted`'s `weighted` is true when either is used.

### 3. The api (`builder/api`)

New module `src/building.rs` (`pub mod building; pub use building::*;`). In `history.rs`, make
`FIND`, `PERMISSION`, `BUSY`, `failure`, `readable`, `lock` and `count` `pub(crate)` and reuse them;
in `assistants.rs`, pull `claude_check_in`'s message choice into
`pub(crate) fn claude_message(error: Error) -> String` (the three consts pass through, anything
else `text::FAILED`) and use it there and below. Every command has a testable `_in` form and a
form reading `HOME` (and, where Claude Code runs, `PATH` and `claude::fallbacks(&home)`), as the
others do.

**The running builds.** As `SignIns`: `pub struct Builds { running: Mutex<HashMap<String, Arc<AtomicBool>>> }`,
`pub fn builds() -> &'static Builds` (a `OnceLock`), and a private guard that removes its slug's
entry when dropped. `build_assistant_in` first checks the slug with `assistants::assistant_dir`
(else `FIND`), then registers it: a slug already there answers `BUSY`. Only then the gate, the
lock and the rest, so a stop pressed at any moment after the build begins reaches it.

**Spend.** The app always uses `Cap::default()` ($50 a day, $5 a call) and
`claude::DEFAULT_MODEL`; changing the limit stays a CLI flag.

Sentences (each a `const` unless marked; a `Failure` is always one of these or one of the agent
crate's `NOT_INSTALLED`, `DOES_NOT_RUN`, `SIGNED_OUT`, `FAILED`, never an error's own text or a
path):

| Name | Sentence |
| --- | --- |
| `STOPPED` | Stopped. Your assistant is as it was before this build. What was already finished is kept, so building again carries on from there. |
| `NOTHING_YET` | There's nothing to build from yet. Read your history and put it together first, in Your history. |
| `SETTINGS` | Something in this assistant's settings stops the build. Use the persona-builder command to see what. |
| `BUILD_FAILED` | The build stopped before it finished, so your assistant is as it was. What it already finished is kept: build again to carry on. |
| `PRIVATE` | The new version had something in it that looked personal or secret, so nothing was saved and your assistant is as it was. Build again to try once more. |
| `LEDGER` | Persona Builder's record of what this assistant has spent can't be read, so it won't spend anything more. Use the persona-builder command to see what's wrong. |
| `BUILD_SAVE` | Persona Builder couldn't save your assistant. Check there's space on this computer, then try again. |
| `cap_reached(spent, cap)` (a `fn`) | Today's spending limit for this assistant is reached: about ${spent:.2} of ${cap:.2}, as Claude Code estimates it. Build again tomorrow to carry on. |
| `ENOUGH` | Your history already holds enough answers to build from. The interview is for a thinner history. |
| `ALREADY_DRAFTED` | Your questions are already drafted. Answer them, or draft different ones. |
| `DRAFT_FAILED` | Claude drafted questions that didn't fit, twice, so none were saved. Try again. |
| `NO_QUESTIONS` | Draft the questions first. |
| `ANSWER_BAD` | Persona Builder couldn't keep that answer. Close it and open it again. |
| `NO_ANSWERS` | Answer at least one question first. |
| `ALL_LEFT_OUT` | Every answer looked personal or secret, so none was kept. Answer without names or private details, then try again. |
| `ANSWERS_SAVE` | Persona Builder couldn't save your answers. Check there's space on this computer, then try again. |

DTOs in `dto.rs` (answers camelCase; requests `deny_unknown_fields`; counts `u32` through `count`;
the ones holding `f64` derive `PartialEq` without `Eq`):

```rust
pub struct Spend { pub spent_usd: f64, pub daily_cap_usd: f64 }
pub struct BuildProgress { pub line: String, pub spend: Spend }
pub struct BuildDone { pub answered: u32, pub held_out: u32, pub tested: bool, pub spend: Spend }
pub struct InterviewOption { pub letter: String, pub label: String, pub description: String }
pub struct InterviewQuestion { pub n: u32, pub kind: String, pub question: String, pub multi_select: bool, pub recommended: Option<String>, pub options: Vec<InterviewOption> }
pub struct InterviewAnswer { pub n: u32, pub picks: Vec<String>, pub own_words: String }   // also inside a request: Deserialize + deny_unknown_fields
pub struct InterviewStatus { pub answered: u32, pub thin_below: u32, pub questions: Vec<InterviewQuestion>, pub answers: Vec<InterviewAnswer>, pub kept: bool }
pub struct DraftInterviewRequest { pub slug: String, pub replace: bool }
pub struct InterviewAnswersRequest { pub slug: String, pub answers: Vec<InterviewAnswer> }
pub struct InterviewKept { pub kept: u32, pub unanswered: u32, pub private_left_out: u32 }
```

Reading the interview files (private helpers):
- *questions*: `interview-questions.json` through `read_private_optional`, parsed as an object and
  passed through `round::check_questions`; absent, unparseable or refused all read as no questions
  (so drafting again can replace a damaged file). Each becomes an `InterviewQuestion`: `kind` is
  its `header`, options lettered `A` to `D` in order, `recommended` the letter or `None`.
- *answers*: `interview-answers.json` the same way; anything unreadable reads as no answers. Each
  entry of its `answers` object becomes an `InterviewAnswer` (`pick` as one letter or a list,
  `own_words` or `""`), sorted by `n`.
- *answered*: `interview::answered` over `decisions.jsonl` read with `read_private_optional` (none
  is 0; unreadable is `SETTINGS`).

Commands (`Result<T, dto::Failure>`):

| Command | Does | Errors |
| --- | --- | --- |
| `build_assistant(SlugRequest, progress) -> BuildDone` | register (above); `readable`; no `decisions.jsonl` (`read_private_optional`) → `NOTHING_YET`; `run::prepare` with `assistants::assistant_dir`'s folder and `"decisions.jsonl"`; `claude::find_in` + `check` (with the registered flag through `with_stop`); `Session::open`; `build::build` with `prepared.inputs(…)`, each line sent as a `BuildProgress` with the spend read just then (`spent_today`, the last value kept if that read fails); then `text::CHECKING` sent the same way; `run::check`; **the flag read once more: set → `STOPPED`, nothing written**; `run::save`; answers `answered`, `held_out` (`built.held`), `tested` (`built.backtest.is_some()`) and the spend after | the gate's; `BUSY` from the registry or the lock (`Session::open` failing with `private::BUSY` text), any other `Session::open` failure `BUILD_SAVE`; `prepare`: `SETTINGS`; Claude Code: `claude_message`; `build::build`'s error, in this order: flag set → `STOPPED`; text equal to `build::text::NOTHING_TO_BUILD` → `NOTHING_YET`; `spent_today + cap.call_usd() > cap.daily_usd() + 1e-9` → `cap_reached`; `spent_today` failing → `LEDGER`; a `claude_message`-kept const → it; else `BUILD_FAILED`; `run::check`: `PRIVATE`; `run::save`: `BUILD_SAVE` |
| `stop_build(SlugRequest) -> ()` | sets the registered flag for the slug; no build running is `Ok(())` and changes nothing | none |
| `spend_today(SlugRequest) -> Spend` | `readable`; `Session::open(home, slug, Cap::default())`; `spent_today` | `BUSY` as above (a build or a read is running); `LEDGER`; other open failures `BUILD_SAVE` |
| `interview_status(SlugRequest) -> InterviewStatus` | `readable`; *answered*, *questions*, *answers*; `thin_below` is `interview::THIN_BELOW`; `kept` when `decisions-interview.jsonl` exists | `SETTINGS` |
| `draft_interview(DraftInterviewRequest) -> InterviewStatus` | `readable`; *answered* ≥ `THIN_BELOW` → `ENOUGH`; questions present and not `replace` → `ALREADY_DRAFTED`; `book`, the last report (`report::read`, unreadable as none), `weights_from_report`; Claude Code found and checked; `Session::open`; the same two checks again under the lock (as the CLI); `interview::draft`; under the session's lock `write_private` the questions (`pretty`) and, when `replace`, `remove_private` of `ANSWERS_FILE`; answers as `interview_status` | the gate's; `BUSY`; Claude Code: `claude_message`; the draft's error: cap test as above → `cap_reached`, `LEDGER`, a kept const, else `DRAFT_FAILED`; writes `ANSWERS_SAVE` |
| `save_interview_answers(InterviewAnswersRequest) -> InterviewStatus` | `readable`; `lock`; no questions → `NO_QUESTIONS`; more than 20 answers, a repeated `n`, an `n` with no question, a pick not among that question's letters, more than one pick on a single-choice question, or `own_words` over 2,000 characters → `ANSWER_BAD`; builds `{"answered": <local day>, "answers": {"<n>": {"pick": "A" or ["A","C"], "own_words": "…"}}}` (`pick` a string for single-choice, a list for multi; an answer with no picks and blank words is left out); checks it with `round::round(member, filters, interview::STEP, questions, answers, false)` (an error → `ANSWER_BAD`; the result is not kept); `write_private` `ANSWERS_FILE` (the whole set each time); answers as `interview_status` | `BUSY`, `SETTINGS`, `ANSWERS_SAVE` |
| `keep_interview_answers(SlugRequest) -> InterviewKept` | `readable`; `lock`; questions (none → `NO_QUESTIONS`) and answers (none → `NO_ANSWERS`); `interview::answer` with the member's filters; `write_private` `interview::OUT` (always in place of an earlier one: the answers file holds every answer); `kept` = records, `unanswered`, `private_left_out` = personal + secret | `interview::answer`'s `text::NO_ANSWERS` → `NO_ANSWERS`, `text::ALL_LEFT_OUT` → `ALL_LEFT_OUT`, other → `ANSWER_BAD`; writes `ANSWERS_SAVE` |

`parity.rs` gains, after `merge_histories`: `build_assistant` → `[["build"]]`, `stop_build` →
`[["build"]]`, `spend_today` → `[["spend"]]`, `interview_status` → `[["interview", "draft"]]`,
`draft_interview` → `[["interview", "draft"]]`, `save_interview_answers` →
`[["interview", "answer"]]`, `keep_interview_answers` → `[["interview", "answer"]]`.

### 4. The Tauri app (`builder/app`)

- `COMMANDS`, the capability's `allow-…` list and `generate_handler!` gain the seven names in the
  order above (30 in all). No new permission, plugin or capability.
- Each is `#[tauri::command(async)]`, one call into the api. `build_assistant` takes `request` and
  `on_progress: tauri::ipc::Channel<persona_builder_api::dto::BuildProgress>` and passes
  `&mut |p| { let _ = on_progress.send(p); }`, as the reads do.

### 5. The screens (`builder/ui`)

Commit every new generated type. `Backend` gains (generated types only):

```ts
buildAssistant(request: SlugRequest, onProgress: (progress: BuildProgress) => void): Promise<BuildDone>
stopBuild(request: SlugRequest): Promise<void>
spendToday(request: SlugRequest): Promise<Spend>
interviewStatus(request: SlugRequest): Promise<InterviewStatus>
draftInterview(request: DraftInterviewRequest): Promise<InterviewStatus>
saveInterviewAnswers(request: InterviewAnswersRequest): Promise<InterviewStatus>
keepInterviewAnswers(request: SlugRequest): Promise<InterviewKept>
```

`tauri-backend.ts`: `buildAssistant` makes a `Channel<BuildProgress>` as the reads do; the rest as
the others. `index.ts` also exports `Build`.

A money amount is shown as `$` and the number with two decimals (`toFixed(2)`). The spend line,
wherever it shows: "Today, about $<spent> of the $<cap> daily limit, as Claude Code estimates it."

**The frame (`PersonaBuilder.vue`).** The `build` step is a button like `sources`, enabled once
the chosen assistant's permission is given (same disabled text). `view` gains `'build'`.
`Sources.vue` gains, under its three cards, the primary button "Next: build your assistant",
emitting `next`; the frame goes to `build`. **The build keeps running while the person looks at
other steps:** the frame mounts `Build` the first time `view` becomes `build` for the open
assistant (a `buildOpened` ref, reset by `open`, `made` and `home`) and from then on shows it with
`v-show="view === 'build'"`, keyed by the slug. `Build` emits `running(boolean)`; while it is true
the "Your assistants" button is disabled and, under it, the muted line "Stop the build first to
choose another assistant." shows.

**`screens/Build.vue`** (prop `assistant`; emits `running`): heading "Build your assistant", the
line "Persona Builder sends your history to Claude through your own Claude Code, a part at a time,
and writes how you decide into your assistant. It uses your own Claude plan and stops for the day
at the limit below.", then two Tabler cards: `build/BuildRun.vue` and, below it,
`build/Interview.vue` (each prop `slug`). `BuildRun` emits `finished` after a build that answered;
`Build` then tells `Interview` to load its status again (a `key` bump). `Interview` emits
`rebuild`; `Build` calls `BuildRun`'s exposed `rebuild()` (`defineExpose`).

**`build/BuildRun.vue`**, card title "Build":
- on mount `spendToday`; the spend line under the title (no spinner; while loading, nothing). A
  failure shows in the card's alert (fallback `LEDGER`).
- primary button "Build my assistant" → `buildAssistant`; emits `running(true)` at the start and
  `running(false)` at the end, whatever the outcome. While it runs: the indeterminate
  `ReadProgress` (`progress` `null`, label "Building"); the lines received so far in a
  `ul.list-unstyled` with `aria-live="polite"` inside a `div` with `overflow-auto` and
  `style="max-height: 16rem"`, newest last; the spend line updated from each message's `spend`;
  and the button "Stop" (`btn btn-outline-danger`) in place of "Build my assistant".
- "Stop" → `stopBuild`; the button becomes "Stopping…", disabled, until the build answers. A
  rejection after "Stop" was pressed shows its message in an `alert alert-info` (fallback
  `STOPPED`); any other rejection in an `alert alert-danger` (fallback `BUILD_FAILED`). The lines
  stay visible under either.
- after it answers: the success alert "Done. Your assistant is built from <answered> answers to
  Claude's questions." followed, when `tested` is false, by " There weren't enough answers kept
  aside to test it yet, so it decides nothing on its own until it can be tested."; the spend line
  from `BuildDone.spend`; then the muted line "Next, see how well it knows you." The button reads
  "Build again".
- `rebuild()`: `mergeHistories` first (its failure shown with fallback PB7.3's `MERGE`, and the
  build not started), then the build as above.
- Numbers use `toLocaleString()`.

**`build/Interview.vue`**, card title "The interview":
- on mount `interviewStatus`; spinner, visually hidden "Checking your interview…"; failure
  fallback `SETTINGS`.
- shown in full only when `answered < thinBelow` or there are questions; otherwise the card shows
  only "Your history holds <answered> answers to Claude's questions, enough to build from."
- the line "Your history holds <answered> answers to Claude's questions. About <thinBelow> helps
  your assistant know you well. Answering twenty questions fills it out."
- no questions: primary button "Draft the questions" (`replace: false`), with the muted line
  "Claude drafts them from your areas of work, through your own Claude Code. It uses a little of
  your Claude plan." While drafting: the indeterminate `ReadProgress` labelled "Drafting" and the
  button "Drafting the questions…", disabled. Failure fallback `DRAFT_FAILED`.
- with questions: four at a time, pages 1 to 5, the line "Questions <first>–<last> of
  <count>". Each question in a `fieldset` with a `legend` holding the question; above it the muted
  small kind; its options as Tabler `form-check` radios (single choice) or checkboxes
  (`multiSelect`), each label "<label>" with the description in a muted line under it and, on the
  recommended one, a `badge bg-secondary-lt` "Suggested"; then a `textarea` labelled "Or in your
  own words (optional)", `maxlength="2000"`, `rows="2"`. A question left blank is fine: the muted
  line under the page says "Leave out any that don't fit your work."
- the answers start from `status.answers`. Buttons: "Back" (secondary; not on page 1), "Next"
  (primary; pages 1–4), "Keep my answers" (primary; page 5). "Back", "Next" and "Keep my answers"
  all first send every answer so far with `saveInterviewAnswers` (each answer `{ n, picks, ownWords }`,
  `picks` the chosen letters in option order); a failure keeps the page (fallback `ANSWERS_SAVE`).
  "Keep my answers" then calls `keepInterviewAnswers` and shows the success alert "Kept <kept>
  answers. <privateLeftOut> that looked private were left out." and the primary button "Add them
  to your history and build again", which emits `rebuild`. Fallback `ANSWER_BAD`.
- under the questions, the small secondary button "Draft different questions". It first shows the
  warning alert "This replaces these questions and your answers to them." with the buttons "Draft
  different questions" (`replace: true`) and "Keep these" (closes it).
- every button in the card is disabled while a call runs.

Tabler classes only; no custom colors; no Tabler JavaScript; light and dark from the theme; no
`<a>` anywhere in these screens. Every failure through `failureMessage(error, <fallback>)`.

### 6. Docs

- `.ai/PROJECT.md`: the `builder/api/` row gains `building.rs` (the build, its stop, spend and the
  interview) and its test `builder/api/tests/building.rs`; the `builder/build/` row gains `run.rs`
  (the build's steps shared by the CLI and the app).
- `builder/ui/README.md`: the screens line gains "build and the interview".

## Tests (write each first and watch it fail on its assertion)

Fixtures are invented (no real person): reuse the CLI build tests' member (`new_member` from
`crew/_template`), corpus builders and the build crate's `claude_stub.py` (Python 3 is already a CI
requirement); temp homes only, `PATH` passed in through the `_in` forms, never the process's.

Rust:
- agent (`tests/call.rs` or a new `tests/stop.rs`): a stub that writes its pid, starts a
  grandchild `sleep 30` and writes its pid too, then sleeps: setting the flag 200 ms into a call
  answers `STOPPED` within 5 seconds, both pids are gone (`kill(pid, 0)` fails), and the ledger's
  line for the call has outcome `stopped` and `cost_from` `budget`; a flag set before the call
  answers `STOPPED`, the stub never runs, and the ledger has no line.
- build: `run::prepare` + `build` + `check` + `save` give the same `decisions.md`,
  `measurements.md` and report as the CLI's build on the same fixture (the CLI's existing build
  tests still pass unchanged); `weights_from_report`: no report equals `weights(book, None)`; a
  report with areas orders kinds thinnest first and gives 0 to a kind with no area.
- CLI: `interview draft` with a report present and no `--labels` sends the prompt with the counts
  (the stub's saved stdin shows "past decisions").
- `builder/api/tests/building.rs`:
  - every command, for an assistant with no permission, answers `PERMISSION` and writes nothing;
    an unknown slug and `../x` answer `FIND`.
  - `build_assistant`: no `decisions.jsonl` → `NOTHING_YET`; with the stub, it writes
    `decisions.md` and `measurements.md`, answers `answered`, `held_out`, `tested`, and the
    progress lines equal the CLI's lines for the same fixture, from "Reading your history" to
    `CHECKING`, each with a `spend`; a second build while the registry holds the slug answers
    `BUSY`; with the state lock held, `BUSY`.
  - the stop: a stub that sleeps on its second call; once the second call has started (its stdin
    file exists), `stop_build_in` → the build answers `STOPPED` within 10 seconds, `decisions.md`
    and `measurements.md` are byte-for-byte as before, the registry no longer holds the slug, and
    a second build reuses the first call's kept reply (the stub's call count shows it).
  - a stop that arrives after the last call: drive it with a progress callback that calls
    `stop_build_in` on the `CHECKING` line → `STOPPED` and the member's files unchanged.
  - the cap: a ledger seeded with $49 spent today (written in the ledger's own line shape) →
    `cap_reached` with "about $49.00 of $50.00"; `spend_today` answers 49 and 50.
  - a signed-out stub → `SIGNED_OUT`'s sentence.
  - `draft_interview`: `ENOUGH` at 150 answered; with the stub, the questions saved and answered
    by `interview_status` lettered A–D; a second draft without `replace` → `ALREADY_DRAFTED`;
    with `replace` the answers file is gone; a damaged questions file reads as none and can be
    drafted over.
  - `save_interview_answers`: each `ANSWER_BAD` case in the table; a good set is saved and comes
    back from `interview_status`; single-choice saved as a string, multi as a list.
  - `keep_interview_answers`: `NO_ANSWERS` with none; kept answers written to
    `decisions-interview.jsonl` equal `interview::answer`'s records for the same files; an answer
    that looks private counts in `private_left_out`.
  - no `Failure` message in this file's tests contains the temp home's path.
  - DTOs camelCase (`spentUsd`, `heldOut`, `multiSelect`, `ownWords`); each request refuses an
    unknown field.
- `builder/cli/tests/app_parity.rs` and `builder/app/tests/lockdown.rs` pass with the new rows and
  the 30 commands.

Vitest:
- `BuildRun.test.ts`: the spend line from `spendToday`; a build shows the lines as they arrive and
  the spend from the last message, then the done line (with and without the untested sentence)
  and "Build again"; "Stop" calls `stopBuild`, shows "Stopping…", and the stopped rejection shows
  in `alert-info`; another rejection in `alert-danger` with the fallback for a non-`Failure`;
  `running` emitted true then false; `rebuild()` merges first and does not build when the merge
  fails.
- `Interview.test.ts`: enough history shows the short line only; thin with no questions shows
  "Draft the questions" and drafting shows the progress; questions show four per page with
  radios or checkboxes and the "Suggested" badge; "Next" sends every answer so far with letters
  in option order; page 5 "Keep my answers" saves, keeps and shows the kept line and the rebuild
  button, which emits `rebuild`; "Draft different questions" asks first and sends
  `replace: true`; a failed save keeps the page.
- `Build.test.ts`: `rebuild` from the interview starts the run's rebuild; `finished` reloads the
  interview.
- `PersonaBuilder.test.ts`: `build` disabled until permission; "Next: build your assistant" from
  Sources goes to the build view; while `running` is true "Your assistants" is disabled with the
  line, and moving to another step and back keeps the build's lines.
- `tauri-backend.test.ts`: each new method invokes its snake-case command; `buildAssistant` passes
  `{ request, onProgress }` with a `Channel` whose messages reach the callback.

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). Report each with its red result:
1. drop the flag check from `settle` → the agent stop test red;
2. count a stopped call at 0 instead of its budget → the agent ledger assertion red;
3. drop the flag read before `run::save` → the after-the-last-call api test red;
4. register the slug only after `Session::open` → the stop-early test red (add it: a stub whose
   `--version` sleeps 1 second; `stop_build_in` sent while it sleeps → `STOPPED`, no call made);
5. drop the guard's removal → the "registry no longer holds the slug" assertion red;
6. let `save_interview_answers` take a pick past the last option → its `ANSWER_BAD` test red;
7. drop the consent check from `readable` → api permission tests red;
8. add a command to `COMMANDS` only → lockdown red;
9. enable `build` before permission → PersonaBuilder test red;
10. mount `Build` with `v-if="view === 'build'"` → the keeps-the-lines test red.

## Not in this slice

- No setting for the daily limit in the app. No report screen (PB7.5). The CLI's Ctrl-C is
  unchanged. Closing the app mid-build ends the process; the call already sent is counted at its
  budget by the ledger, as today.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), `cargo fmt --check` and
`cargo clippy --workspace --all-targets --locked -- -D warnings` are clean, the generated types are
current (no diff after `cargo test`), and the mutation list is reported. Don't commit; Claude
reviews, commits and runs the app.
