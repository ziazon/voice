# PB8.5: a label answer that doesn't fit stops the build, and nobody sees why

Code change. Rust only, in `builder/build` and `builder/api`, plus the Python test stub
`builder/build/tests/fixtures/claude_stub.py`. Commit nothing: the reviewer commits.
Skills: `~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md` (model output is untrusted; private files),
`~/.codex/skills/plain-language/SKILL.md` (every string a person reads). Read the repo's `AGENTS.md`
first: invented fixtures only, no real person anywhere.

## What happened

A real build (11 label parts of up to 300 records) kept part 1 only on its 8th call; part 2 failed both
tries of the next run. The app said only "The build stopped before it finished…". Three faults:

1. **The reason is dropped.** `Asker::ask` (`builder/build/src/build.rs`) ends with
   `Err(Error::Invalid((self.misfit)(what, &last.shown)))`, a message made by `text::misfit`
   (`builder/build/src/text.rs`). In the app, `build_assistant_in` (`builder/api/src/building.rs`)
   passes it to `call_failure`, whose `claude_message` lets through only Claude Code's own texts, so it
   becomes `BUILD_FAILED`. (The CLI already prints the reason.)
2. **A retry can't fix a part.** The retry prompt says "Answer again in full", but retries came back at
   842 and 1,999 output tokens against 20,000 to 27,000 for first tries: the model answers with only the
   corrections. The retry is then checked alone, so every record it left out is "missing". And the
   missing records are never named: `label_problems` (`builder/steps/src/labels.rs`) reports only
   `ids differ: missing N, extra M`, and the told list stops at 40 problems.
3. **Nothing rejected is kept**, so no one can see what failed.

## The changes

### 1. Merge a retry by id (labels and backtest only)

In `build.rs`, add to `Asker`:

```rust
/// As `ask`, for a step whose reply is a list of entries keyed by `id` (`list`): a retry is merged
/// over the last reply that didn't fit, so the model may answer with only the entries it was told
/// about. Only ids in `asked` are kept from the last reply.
pub fn ask_by_id<T>(&mut self, step: &str, system: &str, schema: &Value, prompt: &str, what: &str,
    list: &str, asked: &HashSet<&str>, check: impl Fn(&Value) -> std::result::Result<T, Misfit>)
    -> Result<(T, bool)>
```

Move `ask`'s body into one private fn taking `merge: Option<(&str, &HashSet<&str>)>`; `ask` passes
`None` (its behaviour and its retry text stay exactly as today, for distil and for `builder/grow`).
With `Some((list, asked))`:

- The first try is checked as today. On a misfit, keep its reply as `last_reply`.
- The second try's reply is merged before it is checked: `merge_by_id(&last_reply, &reply, list, asked)`,
  a private fn:
  - If either value has no array at `list`, answer `reply` unchanged (the check reports it).
  - Else answer a copy of `reply` whose `list` array is: the last reply's entries whose `id` is a string,
    is in `asked`, and is not the `id` of any entry in `reply`, in their order; then every entry of
    `reply`, in its order. Entries of the last reply with no string id, or an id not in `asked`, are
    dropped.
- What passes the check is what is cached (the merged value), so a rerun reuses it as today.
- The retry prompt for this path is
  `"{prompt}\nYour last answer didn't fit: {told}. Answer again, following every rule above. You may give only the entries for the ids named above: every entry of your last answer that you leave out is kept as it was.\n"`.
- Use it in `build()` for the label step (`list` `"labels"`, `asked` = the part's record ids) and the
  backtest step (`list` `"predictions"`, `asked` = the batch's question ids). Distil keeps `ask`.

### 2. Name what is missing; never echo the model's own ids

`check_label_reply`:

- Compute the part's record ids missing from `labels`, and the count of label ids that are not record
  ids ("extra"). Drop `label_problems`' own `ids differ: …` entry from the field problems (it is the
  only entry starting `ids differ: `).
- `told` joins, with `"; "`, only the parts that apply, in this order:
  `these records have no label: <every missing id, ", "-joined>`;
  `leave out the <n> labels whose id is not a record here`;
  `these labels are wrong (the record id, then the field): <every field problem, "; "-joined>`.
  No cap: a part holds at most 300 records. Never write an extra id: it is the model's text.
- The repeated-id refusal names the repeated ids that are record ids, and counts the rest:
  told `these records are labelled more than once: <ids>` (plus `; and <n> labels for ids that are not
  records here are repeated` when some aren't), shown as below.
- `shown` (it now reaches the app) is counts only, joined with `", "`, each with its plural right
  (`1 record`, `2 records`, `1 label`, `2 labels`):
  `<n> records had no label`, `<n> labels were for records that aren't there`,
  `<n> labels didn't fit the codebook`; repeated: `<n> records were labelled more than once`.
- `Misfit::plain(error.to_string())` after `label_problems` becomes
  `Misfit::new(error.to_string(), "Voice couldn't check its labels")`.

`check_backtest_reply`, the same shape: told
`these questions have no prediction: <ids>`; `leave out the <n> predictions whose id is not a question here`;
`these predictions are wrong (the question id, then the field): <problems>` (no cap; `predicted twice`
problems stay field problems). shown `<n> questions had no prediction`,
`<n> predictions were for questions that weren't asked`, `<n> predictions didn't fit`.

`check_distil_reply`: only `Misfit::plain(error.to_string())` after `overlaps` changes, to
`Misfit::new(error.to_string(), "Voice couldn't check it for your own words")`. Nothing else there.

After this, every `shown` is the code's own words plus counts, or (distil) a codebook category name.

### 3. Keep each rejected reply, privately

In the shared ask loop, on every misfit, write
`{"problem": misfit.told, "reply": <the reply as received, before any merge>}` with
`write_private_at(home, slug, Some(CACHE), <name>, …)` (0600 in a 0700 folder, as the cache), where
`<name>` is `rejected-<cache name without .json>-<unix seconds>-<try 1 or 2>.json`. A failed write fails
the call as the cache write does. `kept()` reads only exact cache names, so these are never reused. This
applies to `ask` too (distil, and `builder/grow`'s callers), since they share the loop.

### 4. The app shows the reason

In `text.rs`, split `misfit`'s fixed text into two private consts (the text after `{what}` up to
`{problem}`, and the text after `{problem}`) so the message is byte-for-byte as today, and add
`pub fn is_misfit(message: &str) -> bool`: true only when the message has no control character, ends with
the second const, and splits on the first const into a `what` that starts with `labelling`'s,
`distilling`'s or `testing`'s fixed opening (`"Sorting your decisions by kind, part "`,
`"Learning how you decide, group "`, `"Testing your assistant on "`; make those openings consts used by
those fns too).

In `building.rs` `call_failure`, in the last arm, an `Error::Invalid(message)` for which
`text::is_misfit(&message)` holds becomes `failure(&message)`; everything else goes through
`claude_message` as today. The daily-limit and ledger arms stay first.

## Tests (red first, then green; report each)

In `builder/build/tests/fixtures/claude_stub.py`, add two plan modes to `label` and `backtest`:
`missing` (drop the first entry) and `first` (answer with only the first entry, correct). Document them
in the stub's docstring.

`builder/build/tests/build.rs`:

1. Update `label_replies_that_do_not_fit_are_refused_without_the_models_words`: the missing case's told
   names `message:m1` and its shown is `1 record had no label`; the wrong case's shown is
   `1 label didn't fit the codebook`; the twice case is the new told and shown; an extra id equal to
   `SENTINEL` is counted in told and never written in told or shown.
2. Update `backtest_replies_need_one_fitting_pick_per_question` the same way (the missing-1/extra-1 case:
   told names `question:h001`, never `SENTINEL`).
3. New: **a reply missing records, then a partial retry, builds.** Plan the first label call `missing`
   and its retry `first`; the build succeeds with one more call than an all-good build, the retry's
   prompt names the dropped record's id and ends with the new sentence, and the cached label reply
   holds every record of the part.
4. New: **a wrong label, then a retry with only the fix, builds** (`bad` then `first`).
5. New: **a backtest batch missing a prediction, then a partial retry, builds** (`missing` then `first`
   on the backtest calls; use `corpus(12, 32)` and find the backtest call's index from `stub.steps()`).
6. Update `a_reply_that_does_not_fit_is_asked_for_once_more` for the label path's new retry ending;
   `two_replies_that_do_not_fit_stop_the_build` (distil) keeps its exact message.
7. New: **two misfits keep both rejected replies.** After `bad`/`bad` on the first label call, `build/`
   holds exactly two `rejected-label-…-1.json` / `-2.json` files, each mode 0600, each a JSON object with
   `problem` and `reply` (the reply's `labels` as the stub sent them, not merged).
8. New unit test for `is_misfit`: true for `text::misfit` made with each of the three openings; false for
   the interview's and grow's misfit texts, `BUILD_FAILED`, a misfit text with a `\n` in the problem, and
   one whose `what` starts with anything else.

`builder/api/tests/building.rs`: new **a misfit build shows its reason**: plan `{"0":"bad","1":"bad"}`;
the failure's message is exactly
`text::misfit("Sorting your decisions by kind, part 1 of <n>", "1 label didn't fit the codebook")`
(work out `<n>` from the fixture), and the assistant's files are unchanged (as the failed-build test
checks). The existing `failed_build_explains_how_to_run_again_without_changing_the_assistant` (an
`error` run) still shows `BUILD_FAILED`.

Fix any other test whose expected text this changes, and say which.

## Proof (report each)

1. Red: the new tests 3, 4, 5, 7, 8 and the API test fail on their assertions before the change.
2. Green: `cargo test --locked -p voice-build` and `cargo test --locked -p voice-api` pass (use the crate
   names in each `Cargo.toml`), and `cargo test --locked -p voice-grow` still passes.
3. Mutations, each with a `cp` backup, the test run, `cp` back and `cmp` (never `git checkout`):
   (a) `merge_by_id` returns `reply` unchanged: 3, 4 and 5 fail;
   (b) `is_misfit` always false: the API test fails;
   (c) the rejected-reply write removed: 7 fails.
4. `cargo fmt --all --check` and `cargo clippy --all-targets --locked -- -D warnings` clean.
5. `git diff --stat`: only the files named here (and the tests you had to fix).

Never run the whole `scripts/ci-local.sh`; the reviewer does.
