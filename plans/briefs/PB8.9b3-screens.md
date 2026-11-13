# PB8.9b3 brief: the screens for *Open one you saved on GitHub* and *Take the newest version*

Lane V, runner V1, 2026-11-13. Card `CJ8B2rLb` (split from PB8.9b). Codex implements; Claude reviews.

Read first: `AGENTS.md` (no real person in any file, fixture or string: invented names only, such as Wren, Ari and
Rowan Vale, and the `example-owner`/`example-assistant` the test support already uses), `.ai/PROJECT.md`,
`.ai/DECISIONS.md`. Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`,
`~/.codex/skills/design-guide/SKILL.md`, `~/.codex/skills/tabler-ui/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence the person sees), `~/.codex/skills/code-quality/SKILL.md`.

## Why

PB8.9b1 (`plans/briefs/PB8.9b1-open-saved.md`) and PB8.9b2 (`plans/briefs/PB8.9b2-take-newest.md`) built the engine,
the app commands and the backend methods for opening a saved assistant on a second computer (a **member computer**)
and for taking each newer version there. No screen uses them yet. This slice adds the two screens. **UI only:** no
Rust change, no new app command, no new DTO, no new dependency. The backend methods already exist in
`builder/ui/src/backend.ts` (`savedOnGithub`, `openSaved`, `takeNewest`) with their fakes in `test-support.ts`, and
`SaveOptions.memberComputer` already carries the refusal sentence (`builder/api/src/save.rs` `save_options_in`).

## What to build

All in `builder/ui/src`. Match the surrounding screens: Vue 3 `<script setup lang="ts">`, Tabler classes as the
neighbours use them (`card`, `alert alert-*` with `role`, `btn-list`, `form-hint`, `text-secondary`), every backend
failure through `failureMessage(failure, <fixed fallback>)`, a busy flag per action, buttons disabled while any
action of the screen runs. No new colours or CSS: Tabler's own classes give light and dark.

### 1. `screens/GithubSignIn.vue`: the lead sentence becomes a prop

Add `const props = withDefaults(defineProps<{ lead?: string }>(), { lead: 'To save to GitHub, sign in to your GitHub
account first.' })` and render `{{ props.lead }}` where the fixed sentence is now. Nothing else changes; Save's
existing use keeps today's sentence byte for byte.

### 2. New `screens/OpenSaved.vue`: *Open one you saved on GitHub*

Props: `disabled: boolean` (true while the *Make it* form is busy). Emits: `made: [assistant: Assistant]` and
`running: [running: boolean]` (true from the moment an open starts until it ends, success or failure).

A `<section class="card mt-4">` with `card-body p-5`:
- Heading `<h2 class="mb-3">Open one you saved on GitHub</h2>`, then
  `<p>Already saved an assistant to GitHub from another computer? Open it here, and this computer takes each new
  version.</p>`
- **Nothing is fetched on mount.** A `btn btn-outline-primary` **Look on GitHub** (label `Looking…` while the list
  call runs) calls `backend.savedOnGithub()`. Fallback: `Voice couldn't look in your GitHub repos. Try again.`
  After the first answer the button is labelled **Look again**.
- While looking: `<div class="spinner-border" role="status"><span class="visually-hidden">Looking in your private
  GitHub repos…</span></div>`.
- `signedIn: false` → `<GithubSignIn lead="To open an assistant you saved, sign in to your GitHub account first."
  @signed-in="…">`; on `signed-in`, call `savedOnGithub()` again at once (no second click).
- Signed in, no items → `<p class="text-secondary">Voice found no saved assistants in your private GitHub repos.</p>`
  (the CLI's sentence).
- Signed in, items → a `<ul class="list-unstyled">`, one `<li class="d-flex align-items-center justify-content-between
  gap-3 mb-3">` per item, keyed by `` `${owner}/${repo}/${slug}` ``:
  - left: `<strong>{{ name }}</strong>` then `<span class="d-block text-secondary small">{{ owner }}/{{ repo }}</span>`;
  - right: when `here` is false, a `btn btn-primary` **Open {{ name }}** (label `Opening…` on the item being
    opened); when `here` is true, no button, `<span class="text-secondary small">Already on this computer</span>`.
- **Open** calls `backend.openSaved({ owner, repo, slug })`. Only one open at a time; every button of this card (and
  the Look button) is disabled while one runs or while `disabled` is true. Emit `running` true before the call and
  false in `finally`. Fallback: `Voice couldn't open that saved assistant. Try again.` (the api's own fallback).
- **Success:** hide the list and show `<div class="alert alert-success" role="status">` with
  `<h3 class="alert-title">{{ opened.assistant.name }} is ready</h3>`, `<p>{{ opened.message }}</p>` (the api's
  message already names the repo, says permission is asked again on this computer, and names the backup folder when
  it backed up an older one: never restate the path), and a `btn btn-primary` **Next: give permission** that emits
  `made` with `opened.assistant`. That takes the person to the permission screen, as *Make it* does.
- **Failure:** `<div class="alert alert-warning" role="alert">` with the message; the list stays, so they can retry.

### 3. `screens/NewAssistant.vue`

- Wrap the template in a root `<div>` and render `<OpenSaved :disabled="saving" @made="emit('made', $event)"
  @running="openingSaved = $event" />` after the existing form card.
- Add `const openingSaved = ref(false)`. While it is true, disable *Make it*, *Back*, and *Back it up and bring it in*
  (add `|| openingSaved` to each existing `:disabled`, and give *Back* a `:disabled="openingSaved"`), and make
  `make()` and `bringIn()` return early.
- The existing form, its texts and its behaviour are otherwise unchanged.

### 4. `screens/Save.vue`: a member computer takes the newest version

When `options.memberComputer` is non-null, the card is the member computer's card instead of the save choices:
- Heading stays `Keep {{ assistant.name }} safe`. The intro paragraph ("Choose where … is kept…") is shown only when
  `memberComputer` is null (and while loading). For a member computer show instead
  `<p>{{ options.memberComputer }}</p>` (the api's sentence: who saves her, and that this computer takes each new
  version).
- Keep the `Right now: {{ options.chosen }}` line.
- Hide the radio choices, the note and signup button, the repo-name field, *Keep this choice* and *Save <Name>*.
- Not signed in → `<GithubSignIn lead="To take the newest version, sign in to your GitHub account first."
  @signed-in="signedIn" />` (the existing handler).
- A `btn btn-primary` **Take the newest version** (label `Taking it…` while it runs), disabled while not signed in or
  while any action of the card runs (`taking`, `signingOut`). It calls `backend.takeNewest({ slug:
  props.assistant.slug })` and shows `answer.message` in the existing success alert (`message`). Fallback:
  `` `Voice couldn't take the newest version of ${props.assistant.name}. Try again.` `` in the existing warning alert
  (`error`). Clear `message`, `error` and `info` at the start, as `save()` does.
- On success also `emit('updated', answer.assistant)`: add `const emit = defineEmits<{ updated: [assistant:
  Assistant] }>()`. In `VoiceApp.vue`, `<Save … @updated="assistant = $event" />` (the slug never changes, so no
  remount).
- *Sign out of GitHub* stays as today; add `taking` to its and to `signOut()`'s busy checks, and add `taking` to
  `keep()`, `save()` and the sign-out guard so no two actions overlap.
- A non-member computer's card is unchanged byte for byte in behaviour (every existing `Save.test.ts` passes untouched).

## Tests (red first: each must fail on today's code for the right reason)

Vitest with `@vue/test-utils`, using `test-support.ts`'s `fakeBackend`, `screen`, `button`, `deferred` and
`assistant` exactly as the neighbouring `*.test.ts` files do. Invented data only.

`screens/GithubSignIn.test.ts`: the default lead is today's sentence; a `lead` prop replaces it.

New `screens/OpenSaved.test.ts`:
- nothing is called on mount; *Look on GitHub* calls `savedOnGithub` once and shows the status spinner with its text
  while pending (use `deferred`);
- not signed in → the sign-in lead for opening shows; after the sign-in finishes `savedOnGithub` is called again
  without a click and the list shows;
- empty list sentence; a list with one `here: false` item (an *Open Wren* button, the `owner/repo` line) and one
  `here: true` item (no button, *Already on this computer*);
- *Open Wren* calls `openSaved` with exactly `{ owner, repo, slug }`, shows `Opening…`, disables every button while
  pending, emits `running` true then false; success shows the title, the api message verbatim, and *Next: give
  permission* emits `made` with the returned assistant;
- a plain-shape failure shows its message, an `Error` shows the fallback and never its text (`private detail`), and
  the list stays; a list failure does the same with its fallback;
- `disabled: true` disables *Look on GitHub* and every *Open* button.

`screens/NewAssistant.test.ts`: the card is there below the form; while an open runs (a `deferred` `openSaved`),
*Make it* and *Back* are disabled; an open's *Next: give permission* emits `made` from NewAssistant.

`screens/Save.test.ts`: with `memberComputer` set and `signedIn: true`: the sentence shows, the choose-where intro,
radios, repo field, *Keep this choice* and *Save Example* are absent, *Take the newest version* calls `takeNewest`
with `{ slug: 'example' }`, shows `Taking it…` while pending, shows the message, and emits `updated` with the
returned assistant; a failure shows the plain message or the fallback; with `signedIn: false` the button is disabled
and the sign-in shows the take lead. `VoiceApp.test.ts`: an `updated` from Save replaces the assistant (the side
panel's `Making <Name>` heading follows a changed name).

Mutation-check each claim (`cp` backup, `cmp`; never `git checkout --`): at least the sign-in re-list, the
`here` guard, the member-card switch, the busy guards and the `updated` emit.

## Never

- No real person's name, handle or text anywhere; no `Mac` or `Finder` in a non-test source (`wording.test.ts`).
- No Rust, app-command, DTO or generated-type change; never hand-edit `types/generated/`.
- No new dependency, no new CSS, no inline colours.
- Never show a token, a raw error's text or a path the api didn't put in its sentence.

## Gate

`pnpm -C builder/ui test`, `pnpm -C builder/ui run typecheck`, `pnpm -C builder/ui build`. Don't commit; Claude
reviews and commits.
