# VO4.2 brief: refresh the skill mirror at convene

VO4.1 built each assistant's private skill mirror (`~/.claude/state/<slug>/mirror/`) and
`voice_ingest::skills::refresh`, which copies every skill the person agreed to into it, with private
details taken out, and writes `.manifest.json`. A refresh that fails leaves the mirror removed, or
keeps its files and drops the manifest so the copy reads as out of date (`.ai/DECISIONS.md`,
2026-11-01). This slice refreshes **each seat's mirror before a panel's first round**, in
`voice panel convene` and `voice panel resume` and in the app (both go through `voice_api::panels`
into `voice_panel_runner`). A refresh that fails never stops the panel: the panel goes on, and its
progress says so in one fixed line naming the assistant, never a skill's name or text, never the
error, never a path.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md` (the mirror rows and the panel rows),
`.ai/DECISIONS.md` § *2026-11-01: the skill mirror is fresh, absent, or marked out of date*,
`CONTRIBUTING.md`, and `plans/briefs/VO4.1-skill-mirror.md` § 1.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (the new
sentence), `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only (assistants "Pebble" and "Quill", slugs `pebble` and
`quill`, a principal "Rowan Vale", skills `notes` and `writing`), no real person, no fixed paths,
no real secret in a fixture. Commit nothing; leave every change in the working tree.

Out of scope: anything that reads the mirror (no panel turn reads it today; the first reader adds
the trust check from DECISIONS), a refresh on success line, the skills screen, `voice skills`
itself, a change to `refresh`'s own behaviour or messages, the UI package (the panel screen already
shows every progress `line` as it comes; `PanelRun.vue` needs no change).

## 1. Decisions already made (do not change them)

- **Where:** in `voice_panel_runner`, immediately before the `run(` call at the end of both
  `convene_named` and `resume` (`builder/panel-runner/src/run.rs`), after every validation and
  every early return. So a resume that refuses (`NEED_STEER`, `ANSWER_FIRST`, `NO_ASK`) or that
  only closes an expired panel and returns refreshes nothing. One place serves the CLI and the app,
  since both call `voice_api::panels::{convene_panel_in, resume_panel_in}`.
- **The lock is already held.** `refresh` requires its caller to hold
  `private::lock_state(home, slug)`. At that point every seat's state lock is held by this process:
  by `ClaudeTurns`' sessions (`holds_session`) or by `Seat::_session`
  (`seats::load_for`). Do not take another lock (a second `flock` in the same process would refuse
  as busy). Say so in a one-line comment at the call.
- **Which seats are refreshed:** a seat whose mirror is in use, by a new
  `voice_ingest::skills::mirror_in_use(home: &Path, slug: &str) -> bool` in
  `builder/ingest/src/skills/mirror.rs`, re-exported from `skills.rs` with the other mirror items.
  It is `true` when `Agreements::load(home, slug)` fails, or it holds at least one record, or
  `private::private_folder_exists(home, slug, "mirror")` is `Ok(true)` or `Err(_)`; otherwise
  `false`. So an assistant that never agreed to a skill and has no mirror gets no mirror folder,
  and after the panel starts every seat's mirror is fresh, absent, or marked out of date. Add
  `pub fn is_empty(&self) -> bool` to `Agreements` for it.
- **The date** passed to `refresh` is `voice_agent::ledger::local_day(SystemTime::now())`, as
  `voice_api::skills` does (the manifest's `made_on`). Not the panel `Clock`.
- **Failure is one fixed line, and the panel goes on.** Any `Err` from `refresh` (the error is
  dropped unread, never formatted, logged or saved) emits one progress event before the first
  round's "Round N is starting.":
  `Progress::SkillsNotRefreshed { round: u32, seat: String }`, where `round` is the round about to
  start (`state.round_no.saturating_add(1)`) and `seat` is `seat.member.name` (the person's
  name for the assistant; `RoundDone`'s failures show slugs; see § 5 for how it is cleaned). `refresh` itself already removed the mirror or dropped its
  manifest (VO4.1): do nothing more to the mirror here. Success emits nothing.
- **The sentence**, in `builder/panel-runner/src/text.rs`, as a `pub fn
  skills_not_refreshed(seat: &str) -> String` used by `text::progress` for the new variant:
  `Voice couldn't bring {seat}'s copy of your skills up to date, so the panel goes on without it.
  To see why, run voice skills refresh in a terminal.` (one line, exactly this text).
- **Seats are refreshed one at a time, in seat order**, before any round. No threads.
- `voice_api::panels::send` maps the new variant's `round` into `PanelProgress.round` like the
  others. The CLI prints the line as it prints every progress line. No change to exit codes.

## 2. The code

1. `builder/ingest/src/skills.rs`: `Agreements::is_empty`.
2. `builder/ingest/src/skills/mirror.rs`: `mirror_in_use` (doc comment: why each case counts), and
   its re-export.
3. `builder/panel-runner/src/run.rs`: the `Progress::SkillsNotRefreshed` variant, and a private
   `fn refresh_mirrors(home: &Path, seats: &[Seat], round: u32, progress: &mut dyn
   FnMut(Progress))` called right before `run(` in `convene_named` and `resume`. It loops the seats:
   `if voice_ingest::skills::mirror_in_use(home, &seat.member.slug)` then
   `if voice_ingest::skills::refresh(home, &seat.member, &date).is_err()` emit the event. Put
   `refresh_mirrors` in `seats.rs` instead if it reads better there (it is "before a run"); either
   way it is not public.
4. `builder/panel-runner/src/text.rs`: `skills_not_refreshed` and the new `progress` arm.
5. `builder/api/src/panels.rs`: the new arm in `send`.

## 3. Tests, red first

Write each test, run it and see it fail on its assertion before the code exists. Invented skills
only: `~/.claude/skills/notes/SKILL.md` (`Write short notes.\n`) and
`~/.claude/skills/writing/SKILL.md`; agree with `voice_api::agree_skills_in(home, SkillsRequest {
slug, ids: vec!["claude:notes".into()] })` (ids are `claude:<folder>`), or by writing
`~/.claude/state/<slug>/anonymizer/skills.json` the way the existing tests do.

`builder/api/tests/panels.rs` (the app path; it uses `panel_support::Home` and the Claude
substitute):

1. **Convene refreshes a seat with an agreed skill and leaves the other alone.** Pebble agrees to
   `notes`; no mirror exists before. After `convene_panel_in` closes: Pebble's
   `~/.claude/state/pebble/mirror/.manifest.json` exists and lists `notes` (and `SKILL.md`'s copy is
   there); Quill's `~/.claude/state/quill/mirror` does not exist; no progress line starts with
   `Voice couldn't bring`.
2. **Convene brings a stale mirror up to date.** Agree, refresh once with
   `voice_api::refresh_skills_in`, then change `notes/SKILL.md`'s text; convene; the manifest's hash
   for `notes/SKILL.md` changed and the copy holds the new text.
3. **A failed refresh never stops the panel.** Agree, refresh once, then make
   `~/.claude/state/pebble/anonymizer/placeholders.json` unreadable (mode `0o000`, restored at the
   end; skip when running as root, as `builder/cli/tests/skills_refresh.rs`
   `refresh_names_unreadable_placeholders_file` does). Convene: the panel closes; the progress
   lines, in order, include exactly
   `Voice couldn't bring Pebble's copy of your skills up to date, so the panel goes on without it. To see why, run voice skills refresh in a terminal.`
   before `Round 1 is starting.`, and that line appears once; no line contains `notes`,
   `placeholders` or `~/`; Pebble's mirror has no `.manifest.json` any more (its files may stay);
   the line's `round` is 1.
4. **Resume refreshes too.** Convene with Pebble agreed (closes), change `notes/SKILL.md`, resume
   with a steer: the manifest's hash changed.
5. **A refused resume refreshes nothing.** After a closed convene, change `notes/SKILL.md` and
   resume with no steer: it fails with `NEED_STEER` and the manifest is byte for byte unchanged.

`builder/cli/tests/panel.rs` (the CLI path):

6. `voice panel convene` with the unreadable placeholders file of test 3: exit 0, stdout holds the
   sentence for Pebble on its own line before `Round 1 is starting.`.

Unit tests in `builder/ingest/src/skills/mirror.rs`'s `tests` module for `mirror_in_use`: false
with nothing; true with one agreement; true with an unreadable or malformed `skills.json`; true
with a mirror folder and no agreements; and `Agreements::is_empty`.

`builder/panel-runner/src/text.rs` (or the runner tests): `text::progress` of the new variant is
the sentence exactly.

## 4. Done when

- `bash scripts/ci-local.sh` passes (fmt, clippy with `-D warnings`, every test).
- Every existing panel and skills test passes unchanged (the runner's fixture homes have no skills,
  so `mirror_in_use` is false for them and their progress is unchanged).
- Report: the files changed, each test and what it pins, and the ci-local result.

## 5. Review fix: the name is cleaned (security review, LOW)

Nothing checks `member.name` when it is read, and `private.json` can override it, so a name could
hold terminal escapes, a carriage return or a newline and fake extra progress lines in the CLI and
the app. Every other place that shows the name cleans it (`text::assistant_problem` uses
`voice_panels::clean(name, 64)`).

- `text::skills_not_refreshed(seat)` cleans the name itself, so every caller gets it:
  `voice_panels::clean(seat, 64)`, then every run of whitespace (newlines and tabs included, which
  `clean` keeps) becomes one space: `.split_whitespace().collect::<Vec<_>>().join(" ")`. The
  sentence is otherwise unchanged.
- Test, red first, in `text.rs`'s tests: `skills_not_refreshed("Peb\u{1b}[2K\rble\nRound 1 is
  done.\tX")` contains no `char::is_control` character and no newline, starts with
  `Voice couldn't bring Peb`, and ends with the sentence's fixed ending; and a 100-character name is
  cut to 64 characters plus `…`. The existing exact-sentence test stays.
- Nothing else changes.
