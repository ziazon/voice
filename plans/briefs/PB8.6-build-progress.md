# PB8.6: the build shows how far it got and how much is left

Code change. Rust in `builder/build`, `builder/api`, `builder/grow` and `builder/cli`; Vue and
TypeScript in `builder/ui`. Commit nothing: the reviewer commits. Skills:
`~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/frontend/SKILL.md` (states, light and dark), `~/.codex/skills/plain-language/SKILL.md`
(every string a person reads). Read the repo's `AGENTS.md` first: invented fixtures only, no real person
anywhere.

## What happened

Watching a real build, a person kept clicking *Run the build again* with no idea how many runs it would
take. The Build screen (`builder/ui/src/screens/build/BuildRun.vue`) shows an indeterminate bar while
running, then nothing but the lines and the failure. Two things are missing:

1. **How far it got.** The build (`build::build` in `builder/build/src/build.rs`) has three kinds of
   paid step: label parts, distil groups, test batches. It only sends text lines (`progress: &mut dyn
   FnMut(&str)`), so the screen can't draw a real bar.
2. **Whether running again can help.** PB8.5 made a misfit show its reason
   (`text::misfit`, `builder/build/src/text.rs`) and keeps each rejected reply as
   `rejected-<cache stem>-<secs>-<try>.json` in the state folder's `build/` (`Asker::ask_inner`). A part
   that stopped the build before says nothing different the next time, and still invites another run.

## The changes

### 1. The build counts its steps (`builder/build/src/build.rs`)

Add, next to `build`:

```rust
/// One line the build shows, and how far it has got: the steps finished (reused ones too) of the
/// steps it holds in all.
pub struct Progress<'a> {
    pub line: &'a str,
    pub done: usize,
    pub total: usize,
}
```

Change `build`'s parameter to `progress: &mut dyn FnMut(Progress<'_>)` and its doc comment's last
sentence to say `progress` gets each plain line the app shows, with the steps done and in all.

- The steps are: one per label part (`parts.len()`), one per distil group, one per test batch
  (`batches(&heldout.questions).len()` when `tested`, else 0).
- The distil count is known only after labelling. Until the last label part is done, count
  `inputs.book.groups.len()` (an upper bound: `distil_in` makes one group per settings group, and
  empty ones are dropped). From the first distil line on, count the non-empty `groups.len()`. So the
  total never grows and `done` never falls.
- `reading(...)` goes out with `done: 0`. Each `step_line(...)` (label, distil, test) goes out after
  `done += 1`. `too_few(held)` goes out with the current `done` and `total`.
- After the last step line, `done == total`.
- Compute the batch count once, before the reading line, and reuse it in the test loop (don't call
  `batches` twice).

Callers to update, each printing or keeping only the line, so their output is unchanged:
`builder/cli/src/main.rs` (`&mut |p| println!("{}", p.line)`), `builder/cli/tests/build.rs` and
`builder/build/tests/build.rs` (`run` keeps `p.line`).

### 2. A part that stopped the build before says so (`build.rs`, `text.rs`)

**The count.** In `Asker::ask_inner`, when both tries didn't fit, count how many times this part has
stopped a build: the files in the `build/` folder whose name is
`rejected-{stem}-{digits}-2.json`, where `{stem}` is `name.trim_end_matches(".json")` (the step and a
40-hex sha1, so one part's prefix never matches another's) and `{digits}` is one or more ASCII digits.
The second try's file, just written, is one of them, so the count is at least 1. A `-1.json` file, a
temporary (`.rejected-…tmp`), another stem or a non-digit middle doesn't count. If the folder can't be
listed, use 1 (never lose the misfit's reason to an I/O error). Add it as a private `Asker` method,
with a one-line doc comment.

**A free name.** `write_private_at` replaces a file of the same name, so two runs that fail the same
part in the same second would keep one file and count one. When choosing the rejected file's name,
start at the current seconds and, while a file of that name exists in the folder
(`symlink_metadata` is `Ok`), add one second. The name's shape doesn't change.

**The sentence.** The misfit function gains the count: `misfit: fn(&str, &str, usize) -> String`
(field, `with_misfit`, and the call `(self.misfit)(what, &last.shown, times)`). In `text.rs`:

```rust
pub fn misfit(what: &str, problem: &str, times: usize) -> String
```

- `times <= 1`: exactly today's sentence (`{what}{MISFIT_PROBLEM}{problem}{MISFIT_END}`).
- `times >= 2`:
  `{what}: Claude's answer didn't fit twice, so the build stopped (the last problem: {problem}). This part has now stopped the build {times} times, so running the build again is unlikely to help. What it already finished is kept, and Claude's answers that didn't fit are saved privately on this computer.`
  Build it from `MISFIT_PROBLEM` and two new private consts (the text before the number,
  `"). This part has now stopped the build "`, and after it,
  `" times, so running the build again is unlikely to help. What it already finished is kept, and Claude's answers that didn't fit are saved privately on this computer."`).
- `is_misfit` accepts both endings: as today, or the "again" ending whose number is one or more ASCII
  digits and nothing else. Everything else it checks today stays (no control characters, one of the
  three openings, `MISFIT_PROBLEM` present).

`builder/grow/src/text.rs`: `refresh_misfit` and `apply_misfit` take a third `_times: usize` and
ignore it; their text is unchanged.

### 3. The app gets the steps (`builder/api`)

- `BuildProgress` (`builder/api/src/dto.rs`) gains, after `spend`:
  ```rust
  /// The build's steps done and in all; `None` for a refresh's test, which counts no steps.
  pub steps: Option<ReadProgress>,
  ```
  (reuse `ReadProgress { done, total }`; no new type). `cargo test` regenerates
  `builder/ui/src/types/generated/BuildProgress.ts` as `steps: ReadProgress | null`; commit-ready, so
  leave the regenerated file in the tree.
- `build_assistant_in` (`building.rs`): the `send` closure takes a `build::Progress` and sends
  `steps: Some(ReadProgress { done: count(p.done), total: count(p.total) })`. `send(text::CHECKING)`
  after the build becomes a send of that line with the last steps seen (keep the last `ReadProgress`
  in the closure's state, as `last_spend` is kept), so the bar stays full while checking.
- `review.rs`'s refresh progress sends `steps: None`.

### 4. The Build screen (`builder/ui/src/screens/build/BuildRun.vue`)

- Keep `const steps = ref<ReadProgress | null>(null)` (type from
  `../../types/generated/ReadProgress`). Reset it to `null` where `lines` is reset at the start of
  `build`. In the progress callback, `if (progress.steps) steps.value = progress.steps`.
- Replace `<ReadProgress v-if="running" :progress="null" label="Building" />` with
  `<ReadProgress v-if="running || (steps && !done)" :progress="steps" label="Building" />`: the
  indeterminate bar until the first counted line, then a real one; after a stop or a failure the bar
  stays where it got to; after a finished build it goes.
- Right under it, `<p v-if="steps && !done" class="text-secondary" aria-live="polite">{{ stepsLine }}</p>`,
  with a `computed` `stepsLine`:
  - `left = total - done`; `noun = total === 1 ? 'step' : 'steps'`.
  - `left === 0`: `All ${total} ${noun} done.` (running or not).
  - running: `${done} of ${total} ${noun} done, ${left} left.`
  - not running: `Stopped after ${done} of ${total} ${noun}, ${left} left.`
- No new colours or classes: Tabler's `.progress` and `.text-secondary` already follow light and dark.
- `builder/ui/src/screens/review/Refresh.test.ts` and `BuildRun.test.ts`: every `progress({...})`
  fixture gains `steps: null` (or real steps where a test needs them) so `pnpm typecheck` passes.

## Tests, red first

Write each test and see it fail on the current code (for a compile error, write the call with the new
shape and see it fail to build) before the change.

`builder/build/tests/build.rs` (stub `claude`, as the other build tests):

1. `the_build_counts_its_steps`: a build with label parts, both settings groups non-empty, and enough
   held-out questions to test (pick a corpus the existing tests already build green, e.g.
   `corpus(12, 32)`, and compute the expected numbers from `build::parts`, `the_book().groups.len()`
   and `build::batches`). Keep every `Progress` as `(line, done, total)`. Assert: the first is the
   reading line with `done == 0` and `total == parts + groups + batches`; each step line raises `done`
   by exactly 1; `total` never grows; the last step line has `done == total`. Then a rerun (all
   reused, `already done.`) gives the same `(done, total)` sequence.
2. `a_part_that_stops_the_build_again_says_running_again_is_unlikely_to_help`: plan every call `bad`
   for three runs in a row (the stub's plan is keyed by call number across runs, so plan calls `0` to
   `5`). Run 1's error is `text::misfit(what, problem, 1)` (today's sentence); run 2's is
   `text::misfit(what, problem, 2)` and contains `stopped the build 2 times`; run 3's says 3. The
   `build/` folder then holds 6 `rejected-` files (the runs are in the same second: the free name keeps
   all of them).
3. `only_this_parts_second_tries_count`: before one failing run, plant in `.claude/state/ada/build/`
   (0700 folder, 0600 files, as the build makes them) one `rejected-{stem}-5-2.json` for the part's own
   stem (from `build::cache_name` on the first prompt, as `partial_retry_builds` computes it), plus
   decoys that must not count: the same stem with `-5-1.json`, the same stem with `-x-2.json`, and
   another stem's `-5-2.json` (`label-` and 40 other hex digits). The run's error says 2 times.

`builder/build/src/text.rs` tests: `is_misfit` accepts `misfit(what, "1 label didn't fit the codebook",
n)` for n = 1, 2 and 12 and each of the three openings; refuses the "again" sentence with its number
swapped for `two`, for an empty number, and with a newline in the problem.

`builder/api/tests/building.rs`:

4. Extend `a_misfit_build_shows_its_reason` (or add a sibling): a second build in the same home, with
   the plan failing again, shows `build_text::misfit(…, 2)`.
5. `build_progress_carries_the_steps`: a build that finishes (as the existing green build tests do):
   every `BuildProgress` it sends has `steps: Some(_)`; the first has `done == 0`; the last (the
   checking line) has `done == total`. Every existing `misfit(` call in `building.rs` tests gains `, 1`.

`builder/ui/src/screens/build/BuildRun.test.ts` (vitest, parallel as CI runs it):

6. While running, progress with `steps: { done: 3, total: 25 }`: the bar's `aria-valuenow` is `12` and
   the text reads `3 of 25 steps done, 22 left.`
7. Progress with steps, then a rejected build: the danger alert shows the failure; the bar stays with
   the same value; the text reads `Stopped after 3 of 25 steps, 22 left.`
8. A finished build: no progress bar and no steps text.
9. Progress with `steps: null` only: the bar is indeterminate (`.progress-bar-indeterminate`) and no
   steps text. A second run resets the steps (no old text before its first counted line).
10. `total: 1, done: 0` reads `0 of 1 step done, 1 left.`; `done == total` reads `All 25 steps done.`

## Done means

- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked -p voice-build -p voice-api -p voice-grow -p voice-cli`, and
  `pnpm -C builder/ui typecheck`, `pnpm -C builder/ui test`, `pnpm -C builder/ui build` all pass.
- `builder/ui/src/types/generated/BuildProgress.ts` is regenerated and in the tree.
- No change to any other line the CLI prints, nor to grow's refresh sentences.
- Report what you ran and what failed in your sandbox (loopback tests may not run there; say which).
