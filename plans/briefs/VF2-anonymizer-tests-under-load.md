# VF2 brief: three anonymizer tests hold on a busy computer

`builder/ingest/tests/anonymize_file.rs` has three tests that check wall time, and all three fail on
a busy computer with no change to the code. On 2026-11-08, with the computer at load 11 to 19 on
about 8 cores, `bash scripts/ci-local.sh` failed on each of them:

| Test | Bound | Seen at load | Commit whose fix it guards |
|---|---|---|---|
| `punctuation_only_term_words_do_not_cause_quadratic_search` | 1 s | 1.10 s | `9f77698` |
| `combining_mark_only_term_words_do_not_cause_quadratic_search` | 5 s | 6.50 s, 6.87 s, 9.03 s | `ffaca9e` |
| `verifier_10_many_matches_have_bounded_line_lookup` | 10 s | timed out | `0dff027` |

This is VF1's sibling (`plans/briefs/VF1-load-test-under-load.md`): replace each wall-clock bound
with a check that does not depend on time, and keep a timing check only where nothing else can see
the fault, comparing a small input with a large one in the same test.

## What each commit fixed

- **`9f77698`** and **`ffaca9e`** (`builder/ingest/src/anonymize/terms.rs`). `Term::new` builds
  *bounded* patterns (`exact`, and each rescan whose flag is `true`) by joining the term's words
  with a separator class. `terms::matches` searches a bounded pattern, and when a match is not
  bounded by non-letters it restarts one character later. A term whose first word has no letter
  or digit (`-`, or the combining mark U+0903) made a pattern that starts with that word, so over a
  long run of it every restart matched again, all the way to the name: quadratic. `9f77698` drops a
  word with no letter or digit from the bounded patterns. `ffaca9e` makes that test
  `has_letter_or_digit` (the regex class `[\p{L}\p{N}]`) instead of `char::is_alphanumeric`,
  because Rust counts U+0903 as alphanumeric (checked: `'\u{0903}'.is_alphanumeric()` is `true`)
  while the separator class does not count it as a letter. So the fix is visible in the patterns
  themselves: a term with a letterless word builds the same bounded patterns as the term without
  it. That check does not depend on time.
- **`0dff027`** (`builder/ingest/src/anonymize/scan.rs`). The line of each finding used to be found
  by counting the newlines in the text before it, once per finding: quadratic in the number of
  findings. Now `line_starts` is built once and each finding does a `partition_point`. Nothing but
  time can see this from outside, so this test keeps a timing check, as a ratio of a large input
  to a small one, measured in the same test.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/ingest/src/anonymize.rs`,
`builder/ingest/src/anonymize/terms.rs`, `builder/ingest/src/anonymize/scan.rs`,
`builder/ingest/tests/anonymize_file.rs`, and `git show 9f77698 ffaca9e -- builder/ingest/src/anonymize/terms.rs`.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

This is a public repo: invented words only (the existing `Rowan` fixtures are fine), no real
person, no fixed paths. **Commit nothing; leave every change in the working tree.**

## 1. Decisions already made (do not change them)

1. **A unit test inside `terms.rs`**, because `Term` is `pub(super)`. Add
   `#[cfg(test)] mod tests { use super::*; … }` at the end of
   `builder/ingest/src/anonymize/terms.rs` with one test,
   `letterless_words_are_left_out_of_bounded_patterns`:
   - A small helper in the test module, `bounded(term: &Term) -> Vec<&str>`, returns
     `term.exact.as_str()` followed by `as_str()` of each rescan whose flag is `true`, in order.
   - For each of `"- Rowan"` (the word `9f77698` drops) and `"\u{0903} Rowan"` (the word `ffaca9e`
     drops), build `Term::new(text, "term").unwrap()` and assert that `bounded` of it equals
     `bounded(&Term::new("Rowan", "term").unwrap())`, with a message naming the term.
   - One comment line above the list: why a letterless word must not lead a bounded pattern (a
     long run of it makes `matches` restart at every character).
   - Do not compare the unbounded rescans (flag `false`): `"\u{0903} Rowan"`'s run pattern keeps
     U+0903 because its split uses `char::is_alphanumeric`, and that is fine, since it is searched
     with `find_iter`, once, in linear time. Do not change that code.
2. **The two term-word tests in `anonymize_file.rs` lose their timing.** Rename
   `punctuation_only_term_words_do_not_cause_quadratic_search` to
   `punctuation_only_term_word_is_found_after_a_long_dash_run`, and
   `combining_mark_only_term_words_do_not_cause_quadratic_search` to
   `combining_mark_only_term_word_is_found_after_a_long_mark_run`. Keep each one's rules, input and
   `Refused` assertion as they are; remove `start`, `elapsed`, the `eprintln!` and the time
   assertion. The unit test above now carries the guard.
3. **`verifier_10_many_matches_have_bounded_line_lookup` keeps its name** and becomes a ratio test:
   - The input shape stays `"~a/ ".repeat(n / 4)` with `Rules::builtin()` (each `~a/` is one
     home-folder finding, all on line 1).
   - **Small:** `n = 1 << 13` (8 KiB, 2,048 findings). One warm-up call, then five timed calls of
     `anonymize::file` on it; take the median duration.
   - **Large:** `n = 1 << 19` (512 KiB, 131,072 findings), 64 times the small input. Run it on a
     spawned thread as the test does now (build `Rules::builtin()` and a `Mapping::default()` inside
     the thread; both cost nothing), and wait with
     `recv_timeout(deadline)`, where `deadline = max(median_small * 256, 1 s)`.
   - A linear scan takes about 64 times the small one; the old quadratic lookup takes far more.
     256 leaves a factor of 4 for load to slow the large run alone, and the relative deadline also
     makes a regression fail fast instead of running for minutes.
   - On timeout, fail with: `512 KiB scan took over 256 times the 8 KiB scan (median {small:?}): the line lookup is no longer linear`.
   - Then assert the large result as now: `Refused` with exactly one finding, on line 1.
   - Print one `eprintln!` line with the small median, the large duration and their ratio.
   - Keep the existing comment's point (a regression must not spend minutes in the old loop) in one
     comment line, rewritten for the relative deadline.
4. Change nothing outside these three tests and the new unit test. Behaviour does not change; only
   tests do. No new dependency, no new public API.

## 2. Test first, and prove each check catches its regression

Use `-j 2` for every cargo command (the computer is busy).

1. Write the unit test and run `cargo test -p voice-ingest --lib anonymize::terms -j 2`: it passes.
2. **M1, `9f77698` reverted:** `cp` backup of `terms.rs`, then delete both
   `.filter(|word| has_letter_or_digit(word))` lines in `Term::new`. The unit test must fail on
   `"- Rowan"`. Restore with `cp`, then `cmp` against the backup.
3. **M2, `ffaca9e` reverted:** `cp` backup, then change those two filters to
   `.filter(|word| word.chars().any(char::is_alphanumeric))`. The unit test must fail on
   `"\u{0903} Rowan"` (and pass on `"- Rowan"`). Restore with `cp`, `cmp`.
4. Rewrite the two integration tests and `verifier_10_…` as above, and run
   `cargo test -p voice-ingest --test anonymize_file -j 2`: all pass. Report the `eprintln!` line.
5. **M3, `0dff027` reverted:** `cp` backup of `scan.rs`, then in `scan`'s `add` closure replace
   `starts.partition_point(|start| *start <= offset) - 1` with
   `text[..offset].bytes().filter(|b| *b == b'\n').count()` (the old lookup). Run
   `cargo test -p voice-ingest --test anonymize_file verifier_10 -j 2`: it must fail with the
   timeout message. Report how long it took. Restore with `cp`, `cmp`.
6. **Ten runs under load:** run `cargo test -p voice-ingest --test anonymize_file -j 2 -- --nocapture`
   ten times in a row, printing `uptime` before each. All ten must pass. Report each run's load and
   the ratio line.
7. Run `cargo test -p voice-ingest -j 2`,
   `cargo clippy -p voice-ingest --all-targets --locked -- -D warnings` and `cargo fmt --check`.

## 3. Report back

M1, M2 and M3 with each failing message (and M3's duration), the ten runs' loads and ratios, the
commands you ran with their results, and `git status --short`.
