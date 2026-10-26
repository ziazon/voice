# PB7.7b brief: each screen's words checked against its mock

Written by Claude (lane V2), 2026-10-26. Spec: lane V's `PB7.7b` entry (split from PB7.7). Read the
repo's `AGENTS.md` first, then `builder/ui/README.md` and every screen this brief names with its
`.test.ts` beside it. Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); review bar:
`~/.codex/skills/code-quality/SKILL.md`; screens: `~/.codex/skills/frontend/SKILL.md`; every sentence
a person reads: `~/.codex/skills/plain-language/SKILL.md`. Run vitest as
`pnpm -C builder/ui test -- --maxWorkers=2` and the type check as `pnpm -C builder/ui typecheck`
(`builder/ui/node_modules` is already installed; install nothing). Do not commit.

## The problem

The mocks of the app's seven screens were reviewed. The app's first version uses different words for
the same things in places: "answers to Claude's questions" where the mocks say "answered
questions", "Build again" where they say "Run the build again", headings that say "it" where the
mocks use the assistant's name, and a Build lede that still points at "the limit below" although
the limit moved to Connect Claude. This slice changes **words only**: no new controls, no new
backend calls, no layout change, no Rust change.

## Decisions (made, do not reopen)

The rule used to pick them: take the mock's wording where the app shows the same thing and the
mock's sentence is true of what the app does. Left as they are, on purpose: the permission text
(`builder/core/src/consent.rs`, pinned by a SHA-1 because it records what each person agreed to),
every message from Rust (they already match the mocks), the step titles down the left
(`builder/ui/src/steps.ts`), the "Next: …" buttons, and anything a mock shows that the app doesn't
have yet. Never use the assistant's pronouns (people type their own, so the grammar can't be
known): use `assistant.name`, or "your assistant … it", as below.

`{name}` below is the assistant's name, `assistant.name` in each template (`{{ assistant.name }}`, or
`${assistant.name}` inside a template string). `…` in a "from" means the rest of the existing
sentence is unchanged.

1. **`screens/Consent.vue`**: the button `I agree` → `Agree and continue` (`Saving…` stays).
2. **`screens/NewAssistant.vue`**: both labels `Its pronouns` (the select and the *Other…* input)
   → `Your assistant's pronouns`; both labels `Your pronouns` → `How your assistant refers to you`.
3. **`screens/ConnectClaude.vue`**, first card:
   - the lede becomes: `Persona Builder works through your own Claude Code, signed in to your own
     Claude account. It checks that now, and you set how much it may use in a day.`
   - after the `<template v-else-if="status">` block (still inside the first card's body), add
     `<p class="text-secondary mt-3 mb-0">Persona Builder never asks for your Claude password: you sign in to Claude Code yourself. Claude is the only choice for now. Other AI providers come later.</p>`
     shown always (not only once the status is loaded).
   - the second card (*How much it may use*) is not touched.
4. **`screens/Sources.vue`**: the `h2` `Your history` → `What {name} learns from`. The step on the
   left still says *Your history*.
5. **`screens/sources/ClaudeCode.vue`**: the card title `Claude Code` → `Claude Code history`.
6. **`screens/sources/Claudeai.vue`**: the card title `claude.ai (if you use it)` → `claude.ai chats`.
7. **`screens/sources/PutTogether.vue`**, the done line becomes:
   `Done. Your assistant will learn from {questions} answered questions and {messages} messages you
   typed. {heldOut} answered questions are kept back to test it. {duplicates} repeats were left out.`
   (each number with `toLocaleString()` as today).
8. **`screens/build/Interview.vue`**:
   - every `answers to Claude's questions` → `answered questions` (three lines).
   - the full-interview line (`v-else`, under 150) becomes: `Your history holds {answered} answered
     questions. It takes about {thinBelow} to know you well, so Persona Builder asks you twenty
     questions about your work to fill it out.`
   - `Questions {a}–{b} of {n}` → `Questions {a} to {b} of {n}`.
9. **`screens/Build.vue`**:
   - the `h2` `Build your assistant` → `Build {name}`.
   - the lede becomes: `Persona Builder sends your history to Claude through your own Claude Code, a
     part at a time, and learns how you decide. It uses your own Claude plan, up to the daily limit
     you set on Connect Claude. You can leave it running, or stop and carry on later from where it
     got to.`
   - pass the name down: `<BuildRun … :name="assistant.name" …>`.
10. **`screens/build/BuildRun.vue`** (props become `{ slug: string, name: string }`):
    - the start button `Build my assistant` → `Start the build`; `Build again` → `Run the build again`.
    - the stop button `Stop` → `Stop for now` (`Stopping…` stays). While `running`, directly above the
      stop button: `<p class="text-secondary">Stopping keeps everything finished so far. Next time it carries on from here.</p>`
    - the three stopped messages: `…What was already finished is kept, so building again carries on
      from there.` → `…What was already finished is kept, so running the build again carries on from
      there.`; `…What it already finished is kept: build again to carry on.` → `…What it already
      finished is kept: run the build again to carry on.` Keep them as one shared constant if two or
      more stay identical (today the first sentence is written out three times: make it one
      `const STOPPED = '…'` in the script).
    - the done alert: `Done. Your assistant is built from {answered} answered questions.` and, when not
      tested, ` There weren't enough questions kept back to test it yet, so it decides nothing on its
      own until it can be tested.`
    - `See how well it knows you` → `See how well {name} knows you` (here `name` is the prop).
11. **`screens/Report.vue`**:
    - the `h2` `How well it knows you` → `How well {name} knows you`.
    - `It passed the test on …` → `{name} passed the test on …`.
    - the column `What would help` → `To strengthen it`; `Nothing needed` → `Passed the test`.
    - `help.time` → `Keep using Claude, since it learns from every answer`; `help.interview` when the
      history is short → `Answer twenty questions about your work` (the long-history text stays).
    - the button `Go to the interview` → `Answer twenty questions about your work`.
    - the growth line becomes: `Since the build: {kept} change(s) kept and {undone} undone. {passing}
      of the {of} tests made from your answers pass.` (keep today's change/changes choice).
12. **`screens/review/TeachIt.vue`**: the label `What you'd want` → `What you'd have done`; the button
    `Keep this answer` → `Keep my answer`; directly above that button add
    `<p class="form-hint">Leave out anything private, such as someone's name. It's checked before it's kept.</p>`
    (true: `keep_answer_after` in `builder/api/src/review.rs` runs the filters and refuses).
13. **`screens/review/Tests.vue`**: the card title `Tests from your answers` → `Your answers so far`.
14. **`screens/review/Refresh.vue`**: directly after the Approve/Reject button row (the `v-else`
    `div`), add `<p class="text-secondary mt-2 mb-0">Approve: your assistant changes only after the
    change is tested on all your answers and nothing gets worse. Reject: nothing changes, and your
    answers stay saved for the next refresh.</p>`, inside the same `v-else` (wrap both in a
    `<template v-else>`).
15. **`screens/Save.vue`**:
    - the `h2` `Keep a copy` → `Keep {name} safe`.
    - the lede becomes: `Choose where {name} is kept. A private GitHub repo is a backup only you can
      see; or keep it on this computer only.`
    - under the repo-name input add `<div id="save-repo-name-help" class="form-hint">Letters, numbers,
      dots, dashes and underscores, up to 100 characters.</div>` and give the input
      `aria-describedby="save-repo-name-help"` (true: `valid_repo_name` in `builder/save/src/github.rs`).
    - the button `Save now` → `Save {name}` (`Saving…` stays).
16. **`screens/GithubSignIn.vue`**: `Open GitHub's page` → `Open GitHub`.
17. **`screens/TurnOn.vue`**: after the lede paragraph add
    `<p>Turning {name} on adds it as a skill, plus one line in your Claude Code instructions. Turning it off removes exactly what was added.</p>`

## Tests (red first)

- Every test that asserts an old string fails once the screen changes: find them with
  `rg -n "<old string>" builder/ui/src` for each "from" above, and change each assertion to the new
  words. That includes `PersonaBuilder.test.ts`, which checks the `main h2` after a step click
  (`Your history` becomes `What <the fixture's name> learns from`; `Connect Claude` stays).
- Add, before changing each screen, one assertion per new sentence (3, 9, 10's help line, 12, 14,
  15's hint, 17) and one for the name in each heading (4, 9, 10, 11, 15), using the fixture
  assistant's name from `test-support.ts`. Run them red, then make them green.
- `rg -n "answers to Claude's questions|Build again|Build my assistant|Nothing needed|What would help|Open GitHub's page|Save now|Tests from your answers|Keep this answer|I agree" builder/ui/src`
  prints nothing at the end.

## Done when

`pnpm -C builder/ui test -- --maxWorkers=2` and `pnpm -C builder/ui typecheck` both pass, and
`git diff --stat` shows only files under `builder/ui/src/screens/` and `builder/ui/src/PersonaBuilder.test.ts`
(plus `test-support.ts` only if a fixture needs a name). Report each file changed and each test added.

## Follow-up from the reviews (made, do not reopen)

The security review and the Verifier found two sentences that promise more than the code does, and
three Rust messages that now disagree with the renamed build button. Fix all five; red first.

1. **`screens/review/TeachIt.vue`**: the hint becomes `Leave out anything private, such as someone's
   name. Persona Builder catches some private details, but not names.` (The filters match private
   topics and secrets, not personal names.)
2. **`screens/TurnOn.vue`**: the added sentence becomes `Turning {name} on adds a skill and a short
   block to your Claude Code instructions. Turning it off removes what it added, except anything
   you've changed since.` (`install::off` leaves a changed file where it is.)
3. **`builder/api/src/building.rs`**: `STOPPED` → `…What was already finished is kept, so running the
   build again carries on from there.` (the same text as `STOPPED` in `BuildRun.vue`); `BUILD_FAILED`
   → `…What it already finished is kept: run the build again to carry on.`; `PRIVATE` → `…Run the
   build again to try once more.` Update `builder/api/tests/building.rs` to match.
4. **`screens/build/Interview.vue`**: the button `Add them to your history and build again` → `Add
   them to your history and run the build again`, and its tests.

Run `cargo test -p persona-builder-api -j 2` (offline, `CARGO_NET_OFFLINE=true`), `cargo fmt --check`,
the screen tests and the type check. Do not commit.

## Second follow-up, from the delta Verifier (made, do not reopen)

1. **`screens/TurnOn.vue`**: the sentence's end `…except anything you've changed since.` → `…except a
   file you've changed since.` (`install::off` removes the whole block in the instructions even if
   it was edited, and keeps only changed files.) Update `TurnOn.test.ts`.
2. **`builder/api/src/building.rs` `cap_reached`**: `Build again tomorrow to carry on.` → `Run the build
   again tomorrow to carry on.` Update the three assertions (`builder/api/tests/building.rs`, twice,
   and `builder/api/tests/review.rs`).

Same commands as above. Do not commit.
