# PB7.7e brief: alert banners reach readable contrast

Written by Claude (lane V2), 2026-10-27. Spec: lane V's `PB7.7e` entry (found by the attended
light-theme walk, 2026-10-26). Read the repo's `AGENTS.md` first, then `builder/ui/src/theme.css`,
`builder/ui/src/theme.test.ts`, `builder/ui/src/test-support.ts` (`source`), and in Tabler 1.6.1's
`builder/ui/node_modules/@tabler/core/dist/css/tabler.css` the first `:root {` block (the
`--tblr-success`, `--tblr-info`, `--tblr-warning`, `--tblr-danger` values) and the `.alert` rules
(search `.alert {`, `.alert-warning {`, `.alert-important {`).

Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); the look:
`~/.codex/skills/design-guide/SKILL.md` and `~/.codex/skills/frontend/SKILL.md`. Do not commit.

## The problem

Tabler 1.6.1 colours an alert's text with the variant colour itself and its background with a tint
of the same colour:

- `.alert { --tblr-alert-color: var(--tblr-alert-variant-color); --tblr-alert-bg: color-mix(in oklab, var(--tblr-alert-variant-color) 16%, var(--tblr-bg-surface)); … }`
- `.alert { color: var(--tblr-alert-color); background-color: color-mix(in oklab, var(--tblr-alert-bg), var(--tblr-bg-surface)); … }`
- `.alert-warning { --tblr-alert-variant-color: var(--tblr-warning); }` (and the same for success, info, danger).

On the app's light surface (`#FFFFFF`) that gives, computed with the maths below (the walk measured
the warning background as `#FFF8EF`; this computes `#FFF8F0`): warning 2.02:1, success 2.56:1,
info 2.83:1, danger 4.21:1. On the dark surface (`#172238`) danger is 3.15:1; success 5.17, info
4.69 and warning 6.52 pass. WCAG AA for body text is 4.5:1. These banners carry the app's error
messages, so they must be readable.

## Decisions (made, do not reopen)

1. **Only the text colour changes**; the tinted backgrounds stay Tabler's. In `theme.css`, after the
   `[data-bs-theme='dark']` block, add one rule per pair below, of exactly this shape (a literal hex,
   never `light-dark()`, per the file's header comment):

   ```css
   [data-bs-theme='light'] .alert-warning:not(.alert-important) {
     --tblr-alert-color: #7A4D00;
   }
   ```

   | Theme | Variant | Text colour | Computed ratio |
   |---|---|---|---|
   | light | warning | `#7A4D00` | 6.89 |
   | light | success | `#1D6B2C` | 6.13 |
   | light | info | `#1F5C99` | 6.38 |
   | light | danger | `#A82525` | 6.42 |
   | dark | danger | `#F28B82` | 6.15 |

   `:not(.alert-important)` keeps Tabler's white-on-solid `.alert-important` as it is (the app uses
   none today). Add one short comment above the rules: Tabler colours alert text with the variant
   colour, too faint on its own tint, so these set readable text (WCAG AA 4.5:1), checked by
   `theme.test.ts`. Change nothing else in `theme.css`.

2. **A test that computes the real contrast**, added to `theme.test.ts` as
   `it('gives every alert readable text in both themes', …)`. It must fail on today's `theme.css`
   (warning at about 2.0 in light) and pass after. It does this:
   - Read Tabler's CSS with `createRequire(import.meta.url).resolve('@tabler/core/dist/css/tabler.css')`
     (the package has no `exports`, so the path resolves). Assert it still contains the three formulas
     above, as these exact strings: `--tblr-alert-color: var(--tblr-alert-variant-color);`,
     `--tblr-alert-bg: color-mix(in oklab, var(--tblr-alert-variant-color) 16%, var(--tblr-bg-surface));`,
     `background-color: color-mix(in oklab, var(--tblr-alert-bg), var(--tblr-bg-surface));`, and
     for each variant `.alert-<v> {\n  --tblr-alert-variant-color: var(--tblr-<v>);` (so a Tabler
     upgrade that changes how alerts are coloured fails here instead of passing silently).
   - Take each variant's colour from the first `--tblr-<v>: oklch(<L>% <C> <H>deg);` in that file.
   - For each theme (`light`, `dark`), read `--tblr-bg-surface` from that theme's block in
     `theme.css` (comments stripped, as the first test does). For each of the four variants, the text
     colour is the hex in the matching `[data-bs-theme='<theme>'] .alert-<v>:not(.alert-important)`
     rule when there is one, else the Tabler variant colour.
   - Background: in OKLab, `tint = 0.16·variant + 0.84·surface`, then `bg = 0.5·tint + 0.5·surface`.
   - Expect the WCAG contrast ratio of text on `bg` to be at least 4.5, with a message naming the
     theme, the variant and the ratio (`light warning 2.02`).
   - Colour maths, as small functions in the test file: hex → sRGB; sRGB ↔ linear (the WCAG/CSS
     piecewise curve, 0.04045 and 0.0031308); linear sRGB ↔ OKLab with Björn Ottosson's published
     matrices (https://bottosson.github.io/posts/oklab/); oklch → oklab (`a = C·cos h`,
     `b = C·sin h`, L as a fraction); OKLab → sRGB clamped to 0–1 per channel; relative luminance
     `0.2126 R + 0.7152 G + 0.0722 B` on linear values; ratio `(L1 + 0.05) / (L2 + 0.05)`.
   - Also assert the maths reproduces the walk's measurement: the light warning background (with
     Tabler's own colour) rounds to `#FFF8F0`, within 1 per channel of `#FFF8EF`.

## Verify

- Red first: the new test fails on the current `theme.css` naming `light warning`.
- Green: `pnpm -C builder/ui test`, `pnpm -C builder/ui typecheck`, `pnpm -C builder/ui build`.
- Mutation check: change `#7A4D00` to `#F59F00` and the test fails naming light warning; drop the
  dark danger rule and it fails naming dark danger; restore both (keep a `cp` backup, compare with
  `cmp`).
- Report each command's result and the ratios the test computed.
