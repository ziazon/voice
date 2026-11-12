# PB8.8 fix-up 2: no quote may span two points

Code change on top of this branch. Commit nothing. Same skills and rules as
`plans/briefs/PB8.8-distil-keep-going.md`. Red first for each.

## Why

PB8.8 §2 counted a quoted run only when it lies inside one point. The rendered file writes points one after
another (`- {point}\n`), and the tokenizer ignores `-` and the newline, so a 6-word run of the person's own words
split across two neighbouring points would reach the saved file. That weakens the privacy check. **Undo §2**, and
make the repair remove such pairs instead.

## 1. The strict check is as it was on `main` (`check_distil_reply`)

Remove the `point_runs` filter: `overlaps` on all points joined with `\n`, in render order, every run counts.
Remove or invert the tests that asserted a cross-point run passes (`distil_keeps_runs_that_only_cross_point_boundaries`,
`distil_checks_points_individually_and_repairs_without_rewriting`): a cross-point run now fails the strict check.

## 2. One corpus run set (`builder/steps/src/shingle.rs`)

Add `pub fn corpus_runs(corpora: &[(&str, &Records)], n: usize) -> Result<BTreeSet<String>>`: the corpus side of
`overlaps` (the same fields, options and errors), and make `overlaps` use it. Behaviour of `overlaps` unchanged.

## 3. The repair walks points in render order (`repair_distil_reply`)

Order of the repair, per kept section, unchanged until the quotes: per-point rule drops (empty, newline, hidden,
`MAX_POINT`, undated precedent, filter), then `truncate(most)`, then the word-cap trim. **Then, last**, the quote
walk over the whole reply:

- Build the corpus set once with `corpus_runs(&[(corpus_name, records)], SHINGLE)` (an error: return the reply
  unchanged, as now).
- Walk every surviving point in exactly the order `check_distil_reply` joins them: sections in `categories` order
  (not reply order), then `LISTS` order, then list order. Keep `tail`: the last `SHINGLE - 1` tokens (the shingle
  tokenizer's tokens) of all the text kept so far, across points, lists and sections; empty at the start. A short
  kept point makes the tail reach back over several points, so a run across three or more points is caught.
- Drop the point if any run of `runs(&format!("{tail}\n{point}"), SHINGLE)` is in the corpus set. Otherwise keep it
  and update `tail` from the tail's tokens followed by the point's. Expose a small `pub fn tokens(text) -> Vec<String>`
  in `shingle.rs` (the one tokenizer `runs` uses) rather than a second regex.
- Dropping only removes words and points, so the count and word limits still hold after the walk. A section whose
  `lean` empties stays for the strict check to fail and retry, as now.

The strict check on the repaired reply must then pass whenever every section still has a `lean` point: add a test
that asserts exactly that on a reply built to have a quote inside a point, a quote spanning two points of one
list, and a quote spanning the last point of one list and the first of the next.

## 4. The Verifier's other findings

- **The 3 counts the first stop** (`BuildRun.vue`): `stops = reached > best ? 1 : stops + 1`, so a step that keeps
  failing is tried 3 times, as the copy says. Update the give-up test to send progress (finished label steps) on
  every attempt and assert exactly 3 calls.
- **Each attempt starts with an empty line list:** clear `lines` at the start of every attempt in the loop, not only
  at the start of `build()`.
- **Stop during a running attempt with the switch on:** a test where *Stop for now* is clicked while
  `buildAssistant` is pending, the call then rejects: no second call after 15 seconds, `STOPPED` shown as
  information.
- **The pin test compares exactly:** assert `keepGoing.ts` contains
  `` export const PLAN_STOPPED = '<PLAN_STOPPED>' `` with the Rust text inserted, not just the text anywhere.
- **The Build screen's intro** (`builder/ui/src/screens/Build.vue`, the sentence that says the build runs "up to the
  daily limit you set"): add *, unless you turn on Keep going below* (or the plainest equivalent that keeps the
  sentence true). Update its test.
- **`.ai/SECURITY.md`:** the distil row (it says replies are "checked whole before use" and lists "none sharing a
  6-word run with the corpus") gains: a reply that fails is first repaired by dropping points only, the whole-reply
  check then runs on the result, and kept replies in `build/` are stored as Claude sent them (0600, read only by the
  build), like the rejected ones. The spending row (the daily limit set for that assistant; a damaged file stops all
  spending) gains: a build started with *Keep going until it's done* runs with no daily or per-call limit and does
  not read `limit.json`; the saved limit is unchanged. Plain words, the file's own style.

## 5. Tests

- The three quote cases above, plus a run across three points (a middle point of 3 words): the repaired reply
  drops the right points (inside one point: that point; spanning: the later point), passes `check_distil_reply`,
  and repairing it again changes nothing.
- A cross-point run in a reply fails `check_distil_reply` (the restored strict rule).
- Existing PB8.8 tests stay green, adjusted only where §1 changes their expectation.

## Done means

`cargo test -p voice-build -p voice-steps -p voice-api` (2 jobs, `--test-threads=2`) where the sandbox allows it; fmt
and clippy clean. Report the files changed and the tests.

Also `pnpm -C builder/ui test` and the type check, if the sandbox allows it.
