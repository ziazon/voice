# PB8.6 fix-up 2: test that a line without steps keeps the bar where it was

Test-only change on top of `lane-v1/PB8.6`. Commit nothing. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`.

The Verifier found that the guard `if (progress.steps) steps.value = progress.steps` in
`builder/ui/src/screens/build/BuildRun.vue` is untested: a plain `steps.value = progress.steps` passes
every test.

In `builder/ui/src/screens/build/BuildRun.test.ts`, add one test,
`keeps the last counted steps when a later line has none`: start a build whose progress callback is
kept (as `uses the singular for one step…` keeps it), send `{ line: 'Labelling.', spend, steps: { done:
3, total: 25 } }`, then `{ line: 'A line with no count.', spend, steps: null }`. The bar's
`aria-valuenow` is still `12` and `p[aria-live="polite"]` still reads `3 of 25 steps done, 22 left.`
Resolve the build at the end.

Check it catches the fault: with the guard replaced by `steps.value = progress.steps` it must fail
(restore the file exactly after; say whether you could run vitest). No change to `BuildRun.vue`.
