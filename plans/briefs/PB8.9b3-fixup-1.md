# PB8.9b3 fix-up 1: the Verifier's two should-fixes and two nits

Lane V, runner V1, 2026-11-13. Card `CJ8B2rLb`. On top of `7df10c9`; the brief is `plans/briefs/PB8.9b3-screens.md`.
Same skills, same Never list, same gate. Codex implements, test-first (each new test red on `7df10c9`), and does
not commit. Keep it small: well under 60 added lines.

## 1. An open can't be left mid-run (should-fix)

Today *Back* is locked while an open runs, but *Your assistants* (`VoiceApp.vue`) and the brought-in alert's
*Open <Name>* (`NewAssistant.vue`) stay live, so a person can leave and miss the "…is ready" alert that names the
backup folder.
- `NewAssistant.vue`: add `running: [running: boolean]` to its emits and emit it whenever `openingSaved` changes
  (`@running="openingSaved = $event; emit('running', $event)"`, or a `watch`). Add `:disabled="openingSaved"` to the
  brought-in alert's *Open {{ broughtIn.assistant.name }}* button.
- `VoiceApp.vue`, following `askRunning` exactly: `const openingRunning = ref(false)`; `<NewAssistant …
  @running="openingRunning = $event" />`; add `|| openingRunning` to *Your assistants*' `:disabled`; under it
  `<p v-if="openingRunning" class="text-secondary">Wait for Voice to finish opening it before you leave.</p>`;
  `home()` resets it to false, as it does `askRunning`.
- Tests: `VoiceApp.test.ts`: while a `deferred` `openSaved` is pending, *Your assistants* is disabled and the
  sentence shows; after it resolves, both clear. `NewAssistant.test.ts`: the brought-in *Open* button is disabled
  while an open runs (if setting up both states in one test is awkward, assert the binding through the emitted
  `running` and the button's `disabled` attribute).

## 2. A finished GitHub sign-in is never asked again (should-fix)

`OpenSaved.vue` wires `@signed-in` straight to `look()`. If the re-list then fails, `saved.signedIn` stays false and
the sign-in shows again to a person who just signed in; and while `disabled` is true, `look()` returns at once, so
no list appears.
- Split `look()`: `look()` keeps the `props.disabled` guard and calls a new `list()`; `list()` holds today's body
  (with the `looking`/`opening` guards, without the `disabled` one).
- `@signed-in="signedIn"` with `async function signedIn() { saved.value = undefined; await list() }`: on a failure
  the card is back at *Look on GitHub* with the warning, never a second sign-in.
- Tests (`OpenSaved.test.ts`): after sign-in a failing re-list shows the warning and *Look on GitHub*, and no
  *Sign in to GitHub* button; with `disabled: true` set after the sign-in started, the re-list still runs.

## 3. Two nits

- `Save.vue`: the intro's `v-if="loading || options?.memberComputer == null"` becomes
  `v-if="!options || options.memberComputer === null"` and its `v-else` stays, so a member computer's card never
  flashes the choose-where sentence on a reload. Test: a member card that reloads after sign-in (a `deferred`
  second `saveOptions`) keeps the member sentence while loading.
- `OpenSaved.vue`: give the Look button `mb-3` (so the list, the empty sentence and the sign-in don't sit flush
  under it), matching its neighbours.

## Gate

`pnpm -C builder/ui test`, `pnpm -C builder/ui run typecheck`, `pnpm -C builder/ui build`. Report each red, the
mutations you checked (`cp`, `cmp`), and the results.
