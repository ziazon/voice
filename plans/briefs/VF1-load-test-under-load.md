# VF1 brief: the anonymizer's load test holds on a busy computer

`builder/ingest/tests/anonymize_rules.rs` has `two_hundred_three_run_terms_load_within_five_seconds`.
VO3 added it with commit `1425860` ("long terms load again"). It loads 200 terms such as
`zindle7-vorpet-glimnar` and asserts that loading takes under 5 seconds of wall time. On a busy
computer it fails with no change to the code: `bash scripts/ci-local.sh` on `main` `8542963`
took 5.43 s at load about 14 on about 8 cores, and the same test run alone at that load took 12.8,
27.1 and 30.6 s. Wall time measures the computer as much as the code.

What it guards: `1425860` replaced counted separators in `Term::new`
(`builder/ingest/src/anonymize/terms.rs`), namely `[^\p{L}\p{N}\n]{1,8}`, `[^\p{L}\p{N}]{1,8}` and
`[^\p{L}\p{N}]{0,8}`, with `+` and `*`. Counting a Unicode class repeats it in the compiled
program, which made each term's patterns several times bigger and slower to build, and pushed a term
of 4 or more pieces over the `size_limit(1 << 20)` that `Term::new` builds with. Compiled size is
deterministic for a given `regex` version (1.13.1, locked in `Cargo.lock`). So the guard becomes a
check on size, not on time.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `builder/ingest/src/anonymize.rs`,
`builder/ingest/src/anonymize/terms.rs`, `builder/ingest/tests/anonymize_rules.rs`, and
`git show 1425860 -- builder/ingest/src/anonymize/terms.rs`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

This is a public repo: invented words only (the existing `zindle`, `vorpet` and `glimnar` style), no
real person, no fixed paths. **Commit nothing; leave every change in the working tree.**

## 1. Decisions already made (do not change them)

- **A unit test inside `terms.rs`**, because `Term` is `pub(super)`. Add `#[cfg(test)] mod tests { … }`
  at the end of `builder/ingest/src/anonymize/terms.rs`, with one test,
  `term_patterns_stay_small`. Add no new public API and no new dependency.
- **What it checks:** two invented terms, each with its own limit:

  | Term | Limit | Measured with regex 1.13.1 (largest pattern now; with `1425860` reverted) |
  |---|---|---|
  | `zindle7-vorpet-glimnar` (the shape the 200-term test loads) | `1 << 18` (262,144) | 97,128; 764,000 |
  | the first 12 words of `thirteen_word_term_is_refused_by_word_cap`'s term (12 is the cap) | `1 << 20`, the limit `Term::new` builds with | 532,152; about 4,206,264 |

  Each limit is the smallest power of two at least twice the pattern's current size (the 12-word
  term is held to the production limit, about 1.97 times its size), and each is below the reverted
  size. Build each `Term` with `Term::new(text, "term")`. Then rebuild every pattern it holds (`exact`
  and each regex in `rescans`) with
  `RegexBuilder::new(re.as_str()).case_insensitive(true).size_limit(limit).build()`, and assert each
  build is `Ok`. If one fails, the message names the term and the pattern. Put the table's numbers
  in one comment line above the test's term list, for example
  `// regex 1.13.1, largest pattern now / with 1425860 reverted: three-run 97128 / 764000, 12-word 532152 / 4206264.`
  The measuring is done (an earlier pass, its scratch test deleted): don't measure again.
- **The wall-clock test goes.** Remove `two_hundred_three_run_terms_load_within_five_seconds` and
  put in its place, in `anonymize_rules.rs`, `two_hundred_three_run_terms_load`. It loads the same 200
  terms through `Rules::from_value` and asserts `is_ok()`, with no time bound. The size test now
  carries the guard.
- Change nothing else in `terms.rs` or the crate. Behaviour does not change; only tests do.

## 2. Test first, and prove the test catches the regression

1. Write `term_patterns_stay_small` and run
   `cargo test -p voice-ingest --lib anonymize::terms -j 2`. It passes on the current code.
2. **Mutation:** with `cp` backup, revert the three separators again. The test must fail, naming a
   pattern. Restore with `cp`, then `cmp` against the backup.
3. **Second mutation:** with `cp` backup, change the three-run term's limit to `1 << 16`. The test
   must fail. Restore and `cmp`.
4. Run `cargo test -p voice-ingest -j 2`,
   `cargo clippy -p voice-ingest --all-targets --locked -- -D warnings` and `cargo fmt --check`.

## 3. Report back

Both mutation outcomes with the failing message,
the commands you ran with their results, and `git status --short`.
