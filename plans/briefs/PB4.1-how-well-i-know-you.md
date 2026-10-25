# PB4.1 brief: "How well I know you", the plain report

After every build, Persona Builder shows a plain report instead of raw numbers: **Strong**,
**Getting there** or **Thin**, overall and per area of decision, what would strengthen each area that
is not yet Strong, and an honest line when there were fewer than 30 held-out questions to test on.
The report is kept as a private file, so `persona-builder report <member_dir>` shows it again and
the app (PB7) can read it.

Read first: `AGENTS.md`, `.ai/SECURITY.md`, `builder/build/src/build.rs` (`Built`, `Counts`,
`live`, `THIN`, `MIN_HELD`), `builder/build/src/draft.rs` (`live_categories`, `ordered`),
`builder/build/src/text.rs`, `builder/build/src/interview.rs` (`answered`, `THIN_BELOW`),
`builder/ingest/src/private.rs` (`write_private`, `read_private_optional`),
`builder/cli/src/main.rs` (`build`, `member_and_home`), `builder/cli/tests/build.rs`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

No model call is added: the report is made only from what `build::build` already returns. It holds
counts, the codebook's own category names and fixed words, never a record's text.

## The rules (fixed; do not change a threshold)

- **Areas:** every `(category, Row)` in `built.tally` (the training rows per category; a category
  with no training rows is not in it), in the codebook's category order (`book.categories`), then
  any tally category the codebook does not list, in tally order. `other` is an area like any other.
  An area's `examples` is `row.questions + row.messages`.
- **An area's level**, checked in this order:
  1. **Strong** when there is a backtest and `build::live(score.categories.get(area).and_then(|t| t[0].as_ref()))`
     is true (5 or more High picks, at least 90% right). With no backtest, no area is Strong.
  2. **Thin** when `examples < build::THIN` (5).
  3. else **Getting there**.
- **Overall**, checked in this order:
  1. **Thin** when `answered < interview::THIN_BELOW` (150). `answered` is
     `interview::answered(records)`, the same number the build's thin-history line prints.
  2. **Strong** when there is a backtest, there is at least one area, and
     `strong_areas * 2 >= areas` (at least half).
  3. else **Getting there**.
- **What would strengthen an area** (only for Thin and Getting there; a Strong area has none), in
  this order:
  1. `interview`, when `answered < 150` or the area's `examples < 5`;
  2. `claudeai`, when `built.counts.claudeai == 0` (no claude.ai chats in the history);
  3. `time`, always.
- **Tested** is `built.backtest.is_some()` (30 or more held-out questions, `build::MIN_HELD`).

## The file: `report.json` in the member's state folder

A new module `builder/build/src/report.rs` (add `pub mod report;` to `lib.rs`). Add
`serde = { version = "1", features = ["derive"] }` to `builder/build/Cargo.toml` (already in the
workspace's `Cargo.lock` through `builder/core`: no new crate).

```rust
pub const REPORT: &str = "report.json";
pub const SCHEMA: &str = "persona-builder/report/1";
pub const REPORT_MOST: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level { Strong, GettingThere, Thin }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strengthen { Interview, Claudeai, Time }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test { pub right: usize, pub of: usize }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area { pub area: String, pub level: Level, pub examples: usize, pub strengthen: Vec<Strengthen> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,     // always SCHEMA
    pub built: String,      // the local day of the build, YYYY-MM-DD
    pub overall: Level,
    pub answered: usize,
    pub held: usize,
    pub test: Option<Test>, // None when untested; else the backtest's overall hits and n
    pub areas: Vec<Area>,
}
```

Functions (all `pub`):

- `report(book: &Book, built: &Built, answered: usize, day: &str) -> Report`: the rules above.
- `render(report: &Report) -> Vec<String>`: the plain lines (below), one per line, no trailing
  newline. Used by both the build and `persona-builder report`, so they print the same words.
- `write(home: &Path, slug: &str, report: &Report) -> Result<PathBuf>`: `serde_json::to_vec_pretty`
  plus a final `\n`, through `private::write_private(home, slug, REPORT, …)` (0600 in the 0700 state
  folder, atomic, never through a link). Check `REPORT` passes the private name check; if
  `validate_private_name` refuses `report.json`, stop and say so in your summary instead of
  widening it.
- `read(home: &Path, slug: &str) -> Result<Option<Report>>`: `private::read_private_optional`;
  `None` when there is none. Refuse (`Error::Invalid(text::REPORT_UNREADABLE.into())`) when the file
  is over `REPORT_MOST` bytes, is not valid JSON of this shape (unknown fields included), its
  `schema` is not `SCHEMA`, `built` is not `YYYY-MM-DD` digits and dashes, or any `area` is empty,
  over 100 characters, or holds a character `build::hidden` flags (make `hidden` `pub(crate)`; it
  catches control and invisible characters, so a changed file cannot write terminal escapes).

## The words (`builder/build/src/text.rs`, plain language, no jargon)

Add these, and use exactly these strings:

```rust
pub const REPORT_UNREADABLE: &str = "Your last report couldn't be read, so it isn't shown. Build your assistant again to make a new one.";

pub fn no_report(folder: &str) -> String {
    format!("There's no report yet. Build your assistant first: persona-builder build {folder}")
}
pub fn report_from(day: &str) -> String { format!("From the build on {day}:") }
```

`render` gives, in order:

1. The heading: `How well your assistant knows you: Strong.` / `… Getting there.` / `… Thin.`
2. The overall reason, one line:
   - Thin: `Your history holds {answered} answered questions. It takes about 150 to know you well.`
   - Strong or Getting there, tested: `It passed the test on {strong} of {areas} kinds of decision, and got {right} of {of} test questions right.`
   - Getting there, untested: `There's enough history to learn from, but too little to test yet.`
3. When untested, the honest line:
   `Only {held} questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own.`
   and, only when `answered < 150`, a second line:
   `The interview fills out a thin history: persona-builder interview draft`
   (the interview command refuses at 150 or more, so it is not offered then).
4. `By kind of decision:` then one line per area:
   - Strong: `- {area}: Strong.`
   - Getting there: `- {area}: Getting there. To strengthen it: {ways}.`
   - Thin: `- {area}: Thin, {examples} examples so far. To strengthen it: {ways}.`
     (`1 example so far` when `examples == 1`.)
   `{ways}` joins, with `; `, in the order of `strengthen`:
   - interview: `answer the interview (persona-builder interview draft)`
   - claudeai: `add your claude.ai chats (persona-builder claudeai)`
   - time: `keep using Claude, since it learns from every answer`
   With no areas (cannot happen after a build, but `read` may meet it), print `By kind of decision: none yet.`

## The CLI

- `persona-builder build`: after `draft::write` succeeds and the `text::written` line is printed,
  make the report (`report::report(&book, &built, answered, &local_day(now))`, the same day value
  the measurements entry uses: compute it once and pass it to both), `report::write` it, and print
  `render`'s lines. **Replace** the `text::live(...)` line with the report (the report's Strong
  areas say the same thing); remove `text::live` if nothing else uses it (grep first; also check
  the Python side never imports the string). A build that fails before `draft::write` leaves the
  last report as it was. The spend line stays last.
- New `persona-builder report <member_dir>` (`Command::Report { member_dir: PathBuf }`, doc comment
  `/// Show how well your assistant knows you, from its last build.`): `member_and_home`, then
  `report::read`; `None` prints `text::no_report(<member_dir as given>)` and exits 0; else print
  `text::report_from(&report.built)` then `render`'s lines.

## Tests: red first, then green

Write the tests, run them, and see each fail on its assertion (not on a compile error: add the
types and stub functions that `todo!()` first if you must), then make them pass.

`builder/build/tests/report.rs` (unit, constructing `Built`, `Score`, `Cell`, `Row`, `Counts`,
`Backtest` directly; a `Book` with categories `["scope", "methods", "tooling", "other"]`):

1. Untested, 44 answered: overall Thin, no area Strong, every area gets `interview`, and `time`.
2. Untested, 160 answered, an area with 8 examples: overall Getting there; that area Getting there
   with strengthen `[claudeai, time]` (no claude.ai in counts) and `[time]` once `counts.claudeai > 0`.
3. Area level: 4 examples is Thin, 5 is Getting there; live (5 of 5 High) is Strong; 5 High with
   4 right is not Strong; Strong has empty strengthen.
4. Overall Strong at 2 Strong of 4 areas with 200 answered and tested; Getting there at 1 of 4.
5. Order: codebook order, then a tally category the codebook lacks, last.
6. `render`: the exact lines for one tested and one untested report (the honest line present only
   untested; the interview pointer only under 150; `1 example so far`).
7. `write` then `read` round-trips; the file is mode 0600; `read` with no file is `None`;
   `read` refuses an unknown field, a wrong `schema`, an area holding `\u{1b}`, and a file over
   `REPORT_MOST`, each with `REPORT_UNREADABLE`. Use a temporary `HOME` folder as the existing
   tests do (`builder/build/tests/build.rs` shows how a state folder is made for an invented member).

`builder/cli/tests/build.rs`:

8. In `build_writes_how_they_decide_and_the_tests_result`, replace the `text::live` line in the
   expected list with `How well your assistant knows you: Thin.` and
   `- scope: Strong.`; assert `report.json` exists in the state folder with mode 0600 and parses
   with `overall: "thin"`, `held: 32`.
9. New `report_shows_the_last_build`: before a build, `persona-builder report <member>` prints
   `text::no_report`'s line and succeeds; after the build, it prints `From the build on ` and the
   same report lines the build printed.
10. New `build_with_too_few_held_out_questions_says_so_honestly`: `corpus(10)` (fewer than 30 held
    out): the output has the honest line and the interview pointer, and no area is Strong.

All fixtures stay invented (the existing Wren / Robin member). No real person, no fixed path, no
audio.

## Done when

- `cargo test -p persona-builder-build -p persona-builder-cli` and
  `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` are green, then
  `bash scripts/ci-local.sh` is green.
- No new crate in `Cargo.lock` (`git diff --stat Cargo.lock` shows no added package).
- Do not commit; leave the changes in the worktree. In your last message list the files changed,
  the red runs you saw (test name and the failing assertion), and anything in this brief you could
  not do as written.
