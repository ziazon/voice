# VF7c: the rest of the daily-limit words follow a subscription sign-in

VF7b (`plans/briefs/VF7b-subscription-limit.md`) dropped the daily limit when Voice is signed in with a
Claude subscription (testers' builds only): Voice then stops for the day only when Claude Code says the
plan is nearly used up. Three sentences on the screens still describe the daily limit as if it always
applies. Under a subscription each one changes; a Console sign-in, a sign-in that isn't ready yet, a
failed check and every public build keep today's words exactly.

The consent text (`builder/core/src/consent.rs`) is **out of scope**: changing it bumps the consent
version and asks everyone again, which waits on a question. Do not touch `builder/core`.

Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`. Public repo:
invented names only, no fixed paths. **Commit nothing; leave every change in the working tree.** Pass
`--maxWorkers=2` to vitest. If your sandbox blocks pnpm, say which checks didn't run; V1 runs them
outside it.

Only `builder/ui/src/` changes. No DTO, no Rust, no generated type changes: the screens already get
everything they need (`ClaudeStatus.subscription` from `claudeCheck()`, `Spend.usage` from
`spendToday()` and build progress, which is a string exactly when Voice is signed in with a
subscription and `null` otherwise).

## 1. Decisions (do not change them)

The sentences are fixed word for word. Copy them exactly.

1. **Connect Claude's first paragraph** (`builder/ui/src/screens/ConnectClaude.vue`, the `<p>` right
   under `<h2>Connect Claude</h2>`).
   - When `status?.ready && status.subscription` (the same condition the limit card already uses):
     `Voice works through your own Claude Code, signed in with your own Claude subscription, so what it does uses your plan.`
   - Otherwise, today's words, unchanged:
     `Voice works through your own Claude Code, signed in with your own Anthropic Console account, which is billed for what it uses. It checks that now, and you set how much it may use in a day.`
   - One `<p>` with `v-if`/`v-else`, or one `<p>` with a computed text; either way exactly one first
     paragraph is in the page at a time.

2. **The Build screen's sentence** (`builder/ui/src/screens/Build.vue`). The paragraph's first and last
   sentences stay. Only the middle sentence changes:
   - Under a subscription:
     `It uses your own Claude plan, and stops for the day when Claude Code says your plan is nearly used up, unless you turn on Keep going below.`
   - Otherwise, today's: `It uses your own Claude plan, up to the daily limit you set on Connect Claude, unless you turn on Keep going below.`
   - How Build.vue knows: `BuildRun.vue` gains an emit `subscription: [boolean]`. BuildRun emits it every
     time its `spend` ref gets a non-null value whose "subscription-ness" differs from the last one it
     emitted, or the first time `spend` gets a value: `spend.usage !== null` is `true` for a
     subscription. Use a `watch` on `spend` (not on each assignment site), so the first `spendToday()`,
     every build progress and the finished build all count. Nothing is emitted while `spend` is still
     `null` (loading, or a failed `spendToday()`). Build.vue keeps `const subscription = ref(false)`
     and sets it from `@subscription`. Until BuildRun says so, Build.vue shows today's words.

3. **The panels note** (`builder/ui/src/screens/panels/NewPanel.vue`, the `<p>` above "Start the
   panel").
   - Under a subscription:
     `Each assistant's turns use your Claude plan, and stop for the day when Claude Code says your plan is nearly used up.`
   - Otherwise, today's: `Each assistant's turns use your Claude plan and count toward that assistant's daily limit.`
   - How NewPanel knows: on mount it also calls `backend.claudeCheck()`, separately from
     `listAssistants()`, so neither waits for the other and a slow or failed check never holds up the
     list, the form or its error. `subscription` is `true` only when the call resolves with
     `ready && subscription`. A rejected call is swallowed (a comment says why: the note then keeps the
     Console words) and shows nothing else.

## 2. Part A: the specs (write only these; they must fail)

Copy the neighbouring tests' style: `screen`, `fakeBackend`, `deferred`, `flushPromises` from
`builder/ui/src/test-support.ts`, literal expected sentences (never built from the component's code).
`fakeBackend()`'s defaults are a ready Console sign-in (`subscription: false`) and
`spendToday` with `usage: null`.

- `ConnectClaude.test.ts`:
  - a ready subscription: the first paragraph after the `h2` is exactly the subscription sentence, and
    the whole first card's text contains neither `Anthropic Console account` nor `in a day`;
  - a ready Console sign-in: exactly today's first paragraph;
  - a subscription that is not ready (`{ ...signedOut, subscription: true }`): today's first paragraph;
  - while the check is pending (`deferred`), today's first paragraph; after it resolves as a ready
    subscription, the subscription sentence.
- `build/BuildRun.test.ts`:
  - `spendToday` resolving with a `usage` string emits `subscription` once with `[true]`; with
    `usage: null`, once with `[false]`;
  - a failed `spendToday` emits nothing;
  - a Console start (`usage: null`) whose build progress then carries a `usage` string emits `[false]`
    then `[true]`, and a further progress with the same `usage` emits nothing more.
- `Build.test.ts`:
  - with `spendToday` resolving with a `usage` string, the screen shows the subscription sentence and not
    `up to the daily limit you set on Connect Claude`; the first and last sentences are still there;
  - the existing test keeps today's sentence for the default backend (leave it as it is).
- `panels/NewPanel.test.ts`:
  - `claudeCheck` resolving `{ ready: true, subscription: true, … }`: the subscription note, and not
    `count toward that assistant's daily limit`;
  - the default backend: today's note;
  - `claudeCheck` rejecting: today's note, the assistants still listed, no error shown;
  - `claudeCheck` resolving `{ ready: false, subscription: true, … }`: today's note;
  - `claudeCheck` pending forever (`deferred`, never resolved): the assistants are listed and the form
    is usable (the existing `valid`/`submit` helpers emit `start`).

Run `pnpm -C builder/ui exec vitest run --maxWorkers=2 <the four files>` and report which tests fail
and why. Each new test must fail on its assertion (a missing sentence or a missing emit), never on an
import or a typo.

## 3. Part B: the implementation

The Part A specs exist and fail. **They are the specification: do not edit them.** If you believe one
is wrong, stop and report. Implement section 1 in the three screens and BuildRun. Keep the change
minimal; match the files' existing style (single quotes, no semicolons, `<script setup lang="ts">`).
Then run, and report the exit codes of:

- `pnpm -C builder/ui exec vitest run --maxWorkers=2`
- `pnpm -C builder/ui run typecheck`
