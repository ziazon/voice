# PB8.8 fix-up 1: the review's findings

Code change on top of the uncommitted PB8.8 work in this worktree. Commit nothing. Same skills and rules as
`plans/briefs/PB8.8-distil-keep-going.md`. Red first for each.

## 1. A reply the repair cannot save is told its first problem (`builder/build/src/build.rs`, `build()`)

Today the distil check closure answers the **repaired** reply's misfit when the repaired reply still fails. When
every `lean` point quoted the person, the repair empties `lean` and the retry is told *scope: lean needs 1 to 6
points*, which does not say why: the model sends the same quoting points again. Answer the misfit of the
**original** reply instead (`check_distil_reply` on the reply as it came), so the retry is told *these runs of
words are the person's own…* and the screen shows that problem. The repaired reply is still what is kept when it
passes. This brief replaces PB8.8 §4's last sentence.

Tests: restore `two_replies_that_do_not_fit_stop_the_build`'s assertion to `runs of 6 or more of your own words`;
`distil_retries_when_repair_removes_the_only_lean` asserts the second call's stdin holds the quoted-runs sentence
(`these runs of words are the person's own`), not the lean count.

## 2. A failed save never turns the switch off (`builder/ui/src/screens/build/BuildRun.vue`)

`rememberKeepGoing` sets `keepGoing` to `false` when `localStorage.setItem` throws, so with storage blocked the
switch cannot be turned on. On a failed save, do nothing: the switch keeps the value the person chose for this
screen. A failed read still leaves it off.

Test: in `remembers the switch per assistant and tolerates storage read and write failures`, after
`setValue(true)` on the blocked screen, assert the switch is checked, then start a build and assert
`buildAssistant` was called with `{ slug: 'ari', noLimit: true }`.

## Done means

`cargo test -p voice-build -p voice-api` (2 jobs, `--test-threads=2`) where the sandbox allows it, and
`pnpm -C builder/ui test` and the type check pass. Report the files changed and the tests.
