# PB8.8: distil keeps what fits, and the build keeps going until it's done

Code change. Rust in `builder/steps`, `builder/build`, `builder/api` and `builder/app`; Vue and TypeScript in
`builder/ui`. Commit nothing: the reviewer commits. Skills: `~/.codex/skills/tdd-skill/SKILL.md` (red first),
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md` (model output is
untrusted; the privacy checks must still hold), `~/.codex/skills/frontend/SKILL.md` (states, light and dark),
`~/.codex/skills/tabler-ui/SKILL.md` (the switch), `~/.codex/skills/plain-language/SKILL.md` (every string a
person reads). Read the repo's `AGENTS.md` first: invented fixtures only, no real person anywhere (names like
Ari, Robin, Mo Lee). No fixed paths: tests use scratch homes and each crate's existing fixtures
(`builder/build/tests/fixtures/claude_stub.py`, `builder/api/tests/building.rs`'s helpers,
`builder/ui/src/test-support.ts`).

## Why

A tester's build on 0.1.0 never finishes. Its ledger: every call `ok`, about $30 of a $50 daily limit, all
label parts kept, but **no distil group ever kept**: the same group asked twice per run, about 22 runs. Each
distil reply fails `check_distil_reply` (`builder/build/src/build.rs`) on both tries, the build stops, and
*Run the build again* repeats the same failing call. Which rule fails is not recorded on 0.1.0. The likeliest
is the quoted-words rule: a reply of about 3,500 words checked for any 6-word run (`SHINGLE`) shared with a
history of 3,000+ records. But any of the per-point rules can do it, and each one rejects the **whole** reply
for one bad point. PB8.5 made label and backtest retries partial (`Asker::ask_by_id`); distil still dead-ends.

## What to build

### 1. `voice_steps::shingle::runs` (`builder/steps/src/shingle.rs`)

Add `pub fn runs(text: &str, n: usize) -> BTreeSet<String>`: the text's `n`-word runs, with the same lowercase
and `[a-z0-9']+` tokens `overlaps` uses. Make the private `shingles` use it, or the other way round, so there
is one tokenizer. `overlaps` keeps its signature and behaviour.

### 2. The distil check counts a quoted run only inside one point (`check_distil_reply`)

Today the points are joined with `\n` and checked with `overlaps`; the tokenizer ignores the newline, so a
run spanning the end of one point and the start of the next counts as quoted, though no point holds it. Keep
the single `overlaps` call on the joined text, then keep only the runs that are in some point's own
`runs(point, SHINGLE)`. Everything else in the check is unchanged.

### 3. `repair_distil_reply` (new, `builder/build/src/build.rs`)

`pub fn repair_distil_reply(categories: &[(String, usize)], corpus_name: &str, records: &Records, filters:
&Filters, reply: &Value) -> Value`. It builds a new reply that **only drops** points, lists or sections; it never
edits, merges or writes any text, except trimming the whitespace at a point's ends as the check already does.

- `sections` not an array: return the reply unchanged (the check then fails it as now).
- Keep, in order, each section that is an object whose `category` is a string asked for in `categories`, and
  that is the first section for its category. Drop the others (unknown categories, repeats, non-objects).
- In each kept section, for each list in `LISTS` order (`lean`, `flips`, `own_answer`, `rules`, `precedents`):
  if the value is not an array of strings, leave the section as it is (the check fails it). Otherwise drop
  every point that:
  - is empty after trimming, or holds `\n` or `\r`;
  - holds a character `hidden` rejects;
  - is longer than `MAX_POINT` characters;
  - in `precedents`, does not pass `dated`;
  - `filters.kind(point)` flags;
  - holds a quoted run: compute `overlaps` **once** on all the reply's surviving candidate points joined with
    `\n`, then drop each point whose own `runs(point, SHINGLE)` meets that set.
  Then keep only the first `most` points of the list.
- Then, while the section's words (`split_whitespace` over its points, as the check counts them) are more than
  `MAX_WORDS`, drop the **last** point of the first non-empty list in this order: `precedents`, `rules`,
  `own_answer`, `flips`, then `lean`, but never take `lean` below one point.
- Any other field of a section passes through as it was.
- A section left with no `lean` point stays in the reply with an empty `lean`; the check then fails it and the
  retry is asked as now.

### 4. Distil uses the repair (`build()` in `builder/build/src/build.rs`)

The distil `asker.ask(..)` check closure becomes: run `check_distil_reply` on the reply; on a misfit, run it on
`repair_distil_reply(..)` of the same reply and answer that result (its misfit, if it still fails, is the one
told and shown). The Asker still caches the raw reply and still re-runs the closure on a kept reply, so a reuse
repairs the same way. Label and backtest are unchanged.

### 5. A build with no spending limit (`builder/api`, `builder/app`, `builder/ui` types)

- New DTO in `builder/api/src/dto.rs`, exported to TypeScript like the others:
  `BuildRequest { slug: String, #[serde(default)] no_limit: bool }` (TS: `{ slug: string, noLimit: boolean }`).
- `build_assistant` and `build_assistant_in` take `BuildRequest` instead of `SlugRequest`. With `no_limit`,
  the session for **this build only** opens with `Cap::new(0.0, 0.0)` (no daily limit, no `--max-budget-usd`);
  the saved `limit.json` is neither read for this build nor changed. Add a small `open_with` beside `open`
  (`building.rs`) rather than duplicating its error mapping. Without `no_limit`, everything is as now.
- `builder/app/src/commands.rs`, the Tauri bridge in `builder/ui/src/app/tauri-backend.ts`, `backend.ts`,
  `test-support.ts` and any test or lockdown list that names the command's request type follow. The CLI
  already has the same choice (`voice build --daily-cap-usd 0 --call-cap-usd 0`), so the parity table
  (`builder/api/src/parity.rs`) does not change; if a parity test checks request fields, map `no_limit` to
  those two flags there.

### 6. *Keep going until it's done* (`builder/ui/src/screens/build/BuildRun.vue`)

- A Tabler switch above the build button: `<label class="form-check form-switch">` with
  `<input class="form-check-input" type="checkbox">` and the label **Keep going until it's done**. Under it,
  `text-secondary`: *Voice runs the build again by itself whenever it stops, with no spending limit, until every
  step is done. If your Claude plan's limit is reached, it waits and tries again. It gives up if the same step
  stops 3 times in a row. Keep Voice open while it works.* Disabled while a build is running.
- Remember the switch per assistant in `localStorage` under `voice.keepGoing.<slug>`. Every read and write is in
  `try/catch`; when storage throws or is empty the switch is off.
- Every `buildAssistant` call passes `{ slug, noLimit: keepGoing }`.
- With the switch off, behaviour is exactly as now.
- With it on, `build()` loops. Keep `running` true for the whole loop, including waits, so the screen stays
  locked and *Stop for now* shows. Only the first attempt merges first (`rebuild`).
  - On success: as now; the loop ends.
  - On a failure after *Stop for now*: as now (`STOPPED`, information); the loop ends.
  - On any other failure, take its message (`failureMessage`, the same fallback as now):
    - **The plan limit**: the message is exactly the agent's `PLAN_STOPPED`. Wait 30 minutes; this never counts
      toward the 3. Show (alert-info): *Your Claude plan's limit is reached, so Voice is waiting. It tries again
      at {local time, hours and minutes}.*
    - **Anything else**: if this attempt got further than any earlier attempt of this loop (the highest
      `steps.done` it reached is more than the best before it), the count of stops in a row goes back to 0;
      otherwise it goes up by 1. At 3, the loop ends with (alert-danger): *The build stopped 3 times in a row
      without getting further, so Voice stopped trying. The last problem: {message}* Below 3, show
      (alert-info): *{message} Voice tries again in 15 seconds.* and wait 15 seconds.
  - *Stop for now* during a wait cancels the timer at once and ends the loop with `STOPPED` (information).
  - Put `PLAN_STOPPED`'s text, the 15 seconds, the 30 minutes and the 3 in one small module beside the screen,
    `builder/ui/src/screens/build/keepGoing.ts`, as named constants.
- Unmounting the screen clears any pending timer.

### 7. The plan-limit text cannot drift

A Rust test in `builder/api/tests/building.rs` reads `builder/ui/src/screens/build/keepGoing.ts` (path from
`env!("CARGO_MANIFEST_DIR")`) and asserts it contains `voice_agent::text::PLAN_STOPPED` exactly (use the crate's
real path to that constant). Also assert, with the existing claude stub, that a build whose call ends in a
rejected plan reaches the app as a `Failure` whose `message` is that text, if no test already shows it.

### 8. Version 0.1.2

Bump 0.1.1 to 0.1.2 in the same places `release: 0.1.1` (`8fcf8e3`) did: `Cargo.toml`, `Cargo.lock` (by
`cargo`, not by hand), `builder/app/tauri.conf.json`, `builder/ui/package.json`.

## Tests (red first, each seen failing before its fix)

`builder/steps/tests/` (or the shingle module's tests): `runs` matches `overlaps`'s tokens (case, apostrophes,
punctuation).

`builder/build/tests/build.rs`, with the stub's distil reply:
1. One reply holding, among good points: a point over 400 characters, an undated precedent, 7 `lean` points,
   a point that repeats 6 words of an invented record's own words, and a point a test filter flags. The build
   finishes with **one** distil call; the rendered section has none of those points and keeps the good ones in
   order; `lean` keeps its first 6.
2. A section over 900 words: points go from the end of `precedents`, then `rules`, and `lean` keeps its
   points; the result is at most 900 words.
3. A reply whose only `lean` point quotes the history: the build asks again (two calls); when the second reply
   fits, it is kept; when it fails the same way, the build stops with the misfit sentence as now.
4. An extra section for a kind not asked, and a repeated section: dropped, one call. A missing section: asked
   again.
5. Two points whose end and start together make a 6-word run of the history, while neither holds one: kept.
6. A kept (cached) reply that needed repair is reused on the next run with no call.

`builder/api/tests/building.rs`:
7. A saved limit of $0.01 a day with spend already past it: `no_limit: false` fails as the daily limit does now;
   `no_limit: true` builds, and the stub was called with no `--max-budget-usd`. `limit.json` is unchanged after.
8. The pin test in §7.

`builder/ui/src/screens/build/BuildRun.test.ts` (fake timers):
9. Switch off: a failure ends the build after one call, `noLimit: false`, the message as now.
10. Switch on: `noLimit: true`; a failure, then 15 seconds, then a second call that succeeds: done.
11. The plan-limit message: no call for 30 minutes, the waiting line with the time, then a call; plan waits do
    not count toward 3.
12. Three failures with no further progress: three calls, then the give-up message, no fourth call. A failure
    that got further resets the count (four failures with progress in between keep going).
13. *Stop for now* during a wait: no further call, `STOPPED` as information, the button back to *Run the build
    again*.
14. The switch is remembered per slug; a `localStorage` that throws leaves it off and nothing breaks.

## Done means

- `bash scripts/ci-local.sh` passes (fmt, clippy, Rust and Python tests, `pnpm -C builder/ui test`, the
  type check). Generated TS types regenerated by the repo's own way (see how `types/generated/` is made), not
  hand-written.
- No `unwrap` on model output, no new dependency, no change to label or backtest, no change to the saved limit
  or to the Connect Claude screen.
- Report: the files changed, the tests added (red then green), anything you could not do.
