# PB8.8 fix-up 3: the delta Verifier's findings

Code change on top of this branch, in `builder/ui/src/screens/build/` only. Commit nothing. Same skills and rules as
`plans/briefs/PB8.8-distil-keep-going.md`. Red first for each.

## 1. A stop shows the backend's own message again (`BuildRun.vue`)

Fix-up 2 changed the no-retry branch to `error.value = stopping.value ? STOPPED : message`. That hides any message
the backend sends after a stop and breaks the existing test *stops, keeps the lines and shows stopped failures as
information*, which expects the backend's `'Stopped safely.'`. Revert to `error.value = message`. In the new test
for Stop during a running attempt with the switch on, have the pending call reject with the screen's `STOPPED`
text (what the backend sends after a stop), and keep its assertions: no second call after 15 seconds, the alert is
information.

## 2. Progress before a plan limit resets the count (`BuildRun.vue`)

In the plan-limit branch, when this attempt got further than any before it (`reached > best`), set `stops = 0`
before `best` is raised, so a step that failed once before a plan wait is not given up on early. Test: attempts 1
and 2 fail at the same step (2 stops), attempt 3 reaches further and then ends with `PLAN_STOPPED`, after the
30-minute wait attempt 4 fails at its new step: there is a fifth call (not a give-up).

## Done means

`pnpm -C builder/ui test` and the type check pass, if the sandbox allows it. Report the files changed and the tests.
