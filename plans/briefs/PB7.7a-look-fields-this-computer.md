# PB7.7a brief: the look, readable form fields, and "this computer"

Written by Claude (lane V2), 2026-10-26. Spec: lane V's `PB7.7a` entry (split from PB7.7). Read the
repo's `AGENTS.md` first, then `builder/ui/README.md`, `builder/ui/package.json`,
`builder/ui/src/app/main.ts`, `builder/ui/vite.config.ts`, `builder/ui/src/imports.test.ts`,
`builder/agent/src/text.rs`, `builder/save/tests/text.rs` (the pattern for the new wording test),
`builder/ingest/src/private.rs` (`fn only_mine`) and `builder/ingest/tests/private_at.rs`. Work
test-first (`~/.codex/skills/tdd-skill/SKILL.md`); review bar: `~/.codex/skills/code-quality/SKILL.md`;
screens: `~/.codex/skills/frontend/SKILL.md`; colours: `~/.codex/skills/design-guide/SKILL.md`; every
sentence a person reads: `~/.codex/skills/plain-language/SKILL.md`. Run cargo with
`CARGO_NET_OFFLINE=true` and `-j 2`, and vitest as `pnpm -C builder/ui test -- --maxWorkers=2`
(`builder/ui/node_modules` is already installed; install nothing). Do not commit.

## The problem

1. The app uses Tabler's stock colours, not the palette the reviewed mocks use (Navy & Amber).
2. Form fields can be unreadable. Tabler 1.6.1 sets `--tblr-bg-forms` with `light-dark()`, which
   follows `color-scheme`, and its `[data-theme=dark]` rule sets `color-scheme: dark`. A host page
   that puts `data-theme="dark"` on `<html>` while the app's `data-bs-theme` is `light` gets a
   near-black field under light-theme navy text.
3. Some sentences say "this Mac" or "Finder". The app will run on Windows and Linux too, so the
   words say "this computer" and "Show the folder".

## Decisions (made, do not reopen)

1. **New file `builder/ui/src/theme.css`**, exactly this content:

   ```css
   /* Persona Builder's look: the Navy & Amber palette, as awna.ai sets it, over Tabler. Load it after
      Tabler's CSS; Tabler reads data-bs-theme on <html>.
      Form fields get a literal colour per theme, never Tabler's light-dark(): its rules also match a
      host page's data-theme, so a dark host would otherwise put light-theme text on a dark field. */
   :root {
     --awna-navy: #1F2A4D;
     --awna-amber: #F59F00;
     --awna-paper: #FAF8F3;
   }

   [data-bs-theme='light'] {
     color-scheme: light;
     --tblr-primary: #1F2A4D;
     --tblr-primary-rgb: 31, 42, 77;
     --tblr-primary-fg: #FFFFFF;
     --tblr-body-bg: #FAF8F3;
     --tblr-body-bg-rgb: 250, 248, 243;
     --tblr-body-color: #1F2A4D;
     --tblr-body-color-rgb: 31, 42, 77;
     --tblr-heading-color: #1F2A4D;
     --tblr-secondary: #5B6478;
     --tblr-bg-surface: #FFFFFF;
     --tblr-bg-surface-secondary: #F2EFE7;
     --tblr-bg-surface-tertiary: #FAF8F3;
     --tblr-border-color: #E4E0D6;
     --tblr-bg-forms: #FFFFFF;
     --tblr-bg-forms-disabled: #F2EFE7;
   }

   [data-bs-theme='dark'] {
     color-scheme: dark;
     --tblr-primary: #F5B53F;
     --tblr-primary-rgb: 245, 181, 63;
     --tblr-primary-fg: #1F2A4D;
     --tblr-body-bg: #0E1726;
     --tblr-body-bg-rgb: 14, 23, 38;
     --tblr-body-color: #E8ECF3;
     --tblr-body-color-rgb: 232, 236, 243;
     --tblr-heading-color: #E8ECF3;
     --tblr-secondary: #A3ADC2;
     --tblr-bg-surface: #172238;
     --tblr-bg-surface-secondary: #111B2E;
     --tblr-bg-surface-tertiary: #131E33;
     --tblr-border-color: #26324A;
     --tblr-bg-forms: #111B2E;
     --tblr-bg-forms-disabled: #172238;
   }
   ```

2. **The app loads it after Tabler.** In `builder/ui/src/app/main.ts`, add
   `import '../theme.css'` on the line right after `import '@tabler/core/dist/css/tabler.min.css'`.
3. **A host app can load it.** In `builder/ui/package.json`, `exports` becomes
   `{ ".": "./src/index.ts", "./theme.css": "./src/theme.css" }`. In `builder/ui/README.md`, replace the
   sentence "It loads Tabler's CSS and chooses its theme." with: "It loads Tabler's CSS, then
   `@persona-builder/screens/theme.css` (the app's colours and readable form fields), and chooses its
   theme by setting `data-bs-theme` to `light` or `dark` on `<html>`."
4. **The words.** Change only these strings:
   - `builder/agent/src/text.rs`: in `NOT_INSTALLED`, `DOES_NOT_RUN` and `API_KEY_SET`, "this Mac"
     becomes "this computer". Nothing else in those lines changes.
   - `builder/ingest/src/private.rs` `only_mine`: "by others on this Mac" becomes "by others on this
     computer".
   - `builder/ui/src/screens/sources/Claudeai.vue`: "If your Mac unzipped the download, choose the
     folder it made." becomes "If your computer unzipped the download, choose the folder it made."
   - `builder/ui/src/screens/Assistants.vue` and `builder/ui/src/screens/ClaudeAiFiles.vue`: the
     button text "Show in Finder" becomes "Show the folder".
   - `.ai/SECURITY.md`: the table row's "*Show in Finder* runs" becomes "*Show the folder* runs".
   - Leave everything under `builder/install/` alone (the installer is Mac-only), leave Rust comments,
     `cfg(target_os)` code and test function names alone, and leave the root `README.md` alone.

## Tests (red first, then green)

1. **New `builder/agent/tests/text.rs`**, shaped like `builder/save/tests/text.rs`: a test
   `the_wording_says_this_computer_never_this_mac` that asserts `include_str!("../src/text.rs")`
   contains neither "this Mac" nor "your Mac", and that each of `text::NOT_INSTALLED`,
   `text::DOES_NOT_RUN` and `text::API_KEY_SET` contains "this computer".
2. **`builder/ingest/tests/private_at.rs`** `a_folder_others_can_open_is_refused`: also assert the
   error contains "by others on this computer".
3. **New `builder/ui/src/wording.test.ts`** (move the `files()` walk and its `source` folder from
   `imports.test.ts` into `src/test-support.ts` as exports, and import them in both tests; no copy):
   every non-test `.vue` and `.ts` file under `src` (skip `src/types/generated`) matches neither
   `/\bMac\b/` nor `/Finder/`.
4. **New `builder/ui/src/theme.test.ts`** (it reads files with `node:fs`, as `imports.test.ts` does):
   - `theme.css` holds no `light-dark(`;
   - it splits `theme.css` into its `[data-bs-theme='light']` and `[data-bs-theme='dark']` blocks and
     checks each declares `color-scheme` (`light`, `dark`), `--tblr-bg-forms` and
     `--tblr-bg-forms-disabled` as `#RRGGBB` hex literals, and the exact `--tblr-primary`,
     `--tblr-body-bg`, `--tblr-body-color` and `--tblr-bg-forms` values from decision 1;
   - in `app/main.ts`, the Tabler CSS import comes before `'../theme.css'`, and both are present;
   - `package.json`'s `exports['./theme.css']` is `'./src/theme.css'` and that file exists.
5. **Update the existing tests** whose strings change: `ClaudeAiFiles.test.ts` ("Show in Finder" →
   "Show the folder", including the test titles' word "Finder" → "Show the folder"),
   `Assistants.test.ts` if it names the button, `sources/Claudeai.test.ts` (the unzip line), and any
   Rust test that compares one of the changed strings by value.

Run a mutation check on each new test: put one "this Mac" back, one `light-dark(` into `theme.css`,
swap the two CSS imports, and confirm the matching test goes red; restore each with a `cp` backup and
`cmp`, never `git checkout`.

## Done means

- `cargo test -j 2 -p persona-builder-agent -p persona-builder-ingest -p persona-builder-api` passes,
  and `cargo clippy -j 2 -p persona-builder-agent -p persona-builder-ingest --all-targets -- -D warnings`
  and `cargo fmt --check` are clean;
- `pnpm -C builder/ui typecheck`, `pnpm -C builder/ui test -- --maxWorkers=2` and
  `pnpm -C builder/ui build` pass;
- `git grep -n -e 'this Mac' -e 'your Mac' -e 'Show in Finder' -- builder ':!builder/install'`
  prints only Rust comments or test names;
- report each file you changed and each mutation check's result.
