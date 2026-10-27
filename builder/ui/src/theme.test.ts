import { existsSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { join } from 'node:path'
import { expect, it } from 'vitest'
import { source } from './test-support'

type Colour = [number, number, number]

function hexToSrgb(hex: string): Colour {
  return [1, 3, 5].map(start => parseInt(hex.slice(start, start + 2), 16) / 255) as Colour
}

function srgbToLinear(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
}

function linearToSrgb(channel: number): number {
  return channel <= 0.0031308 ? 12.92 * channel : 1.055 * channel ** (1 / 2.4) - 0.055
}

// Björn Ottosson's matrices: https://bottosson.github.io/posts/oklab/
function srgbToOklab(rgb: Colour): Colour {
  const [r, g, b] = rgb.map(srgbToLinear) as Colour
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  return [
    0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
  ]
}

function oklabToSrgb([L, a, b]: Colour): Colour {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
  ].map(channel => Math.max(0, Math.min(1, linearToSrgb(channel)))) as Colour
}

function oklchToOklab(L: number, C: number, H: number): Colour {
  const radians = H * Math.PI / 180
  return [L / 100, C * Math.cos(radians), C * Math.sin(radians)]
}

function mixOklab(first: Colour, second: Colour, weight: number): Colour {
  return first.map((channel, index) => weight * channel + (1 - weight) * second[index]!) as Colour
}

function luminance(rgb: Colour): number {
  const [r, g, b] = rgb.map(srgbToLinear) as Colour
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

function contrast(text: Colour, background: Colour): number {
  const first = luminance(text)
  const second = luminance(background)
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05)
}

it('gives every alert readable text in both themes', () => {
  const tabler = readFileSync(createRequire(import.meta.url).resolve('@tabler/core/dist/css/tabler.css'), 'utf8')
  for (const formula of [
    '--tblr-alert-color: var(--tblr-alert-variant-color);',
    '--tblr-alert-bg: color-mix(in oklab, var(--tblr-alert-variant-color) 16%, var(--tblr-bg-surface));',
    'background-color: color-mix(in oklab, var(--tblr-alert-bg), var(--tblr-bg-surface));',
  ]) {
    expect(tabler).toContain(formula)
  }
  const css = readFileSync(join(source, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
  for (const theme of ['light', 'dark']) {
    const block = css.match(new RegExp(`\\[data-bs-theme='${theme}'\\]\\s*\\{([^}]+)\\}`))?.[1]
    const surfaceHex = block?.match(/--tblr-bg-surface: (#[0-9A-Fa-f]{6});/)?.[1]
    expect(surfaceHex, `${theme} surface`).toBeDefined()
    const surface = srgbToOklab(hexToSrgb(surfaceHex!))
    for (const variant of ['warning', 'success', 'info', 'danger']) {
      expect(tabler).toContain(`.alert-${variant} {\n  --tblr-alert-variant-color: var(--tblr-${variant});`)
      const match = tabler.match(new RegExp(`--tblr-${variant}: oklch\\(([\\d.]+)% ([\\d.]+) ([\\d.]+)deg\\);`))
      expect(match, `${variant} OKLCH`).not.toBeNull()
      const colour = oklchToOklab(Number(match![1]), Number(match![2]), Number(match![3]))
      const tint = mixOklab(colour, surface, 0.16)
      const background = oklabToSrgb(mixOklab(tint, surface, 0.5))
      if (theme === 'light' && variant === 'warning') {
        const channels = background.map(channel => Math.round(channel * 255))
        expect(channels).toEqual([255, 248, 240])
        hexToSrgb('#FFF8EF').forEach((channel, index) => {
          expect(Math.abs(channels[index]! - Math.round(channel * 255))).toBeLessThanOrEqual(1)
        })
      }
      const override = css.match(new RegExp(
        `\\[data-bs-theme='${theme}'\\] \\.alert-${variant}:not\\(\\.alert-important\\)\\s*\\{([^}]+)\\}`,
      ))?.[1]?.match(/--tblr-alert-color: (#[0-9A-Fa-f]{6});/)?.[1]
      const text = override ? hexToSrgb(override) : oklabToSrgb(colour)
      const ratio = contrast(text, background)
      const message = `${theme} ${variant} ${ratio.toFixed(2)}`
      expect(ratio, message).toBeGreaterThanOrEqual(4.5)
    }
  }
})

it('gives each theme literal, readable form colours and the app palette', () => {
  const css = readFileSync(join(source, 'theme.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')
  expect(css).not.toContain('light-dark(')
  for (const [theme, primary, background, colour, forms] of [
    ['light', '#1F2A4D', '#FAF8F3', '#1F2A4D', '#FFFFFF'],
    ['dark', '#F5B53F', '#0E1726', '#E8ECF3', '#111B2E'],
  ]) {
    const block = css.match(new RegExp(`\\[data-bs-theme='${theme}'\\]\\s*\\{([^}]+)\\}`))?.[1]
    expect(block, theme).toBeDefined()
    expect(block).toMatch(new RegExp(`color-scheme: ${theme};`))
    for (const token of ['--tblr-bg-forms', '--tblr-bg-forms-disabled']) {
      expect(block).toMatch(new RegExp(`${token}: #[0-9A-Fa-f]{6};`))
    }
    for (const [token, value] of [
      ['--tblr-primary', primary],
      ['--tblr-body-bg', background],
      ['--tblr-body-color', colour],
      ['--tblr-bg-forms', forms],
    ]) {
      expect(block).toContain(`${token}: ${value};`)
    }
  }
})

it('loads the app theme after Tabler', () => {
  const main = readFileSync(join(source, 'app/main.ts'), 'utf8')
  const tabler = main.indexOf("import '@tabler/core/dist/css/tabler.min.css'")
  const theme = main.indexOf("import '../theme.css'")
  expect(tabler).toBeGreaterThanOrEqual(0)
  expect(theme).toBeGreaterThan(tabler)
})

it('exports the theme for host apps', () => {
  const manifest = JSON.parse(readFileSync(join(source, '../package.json'), 'utf8'))
  expect(manifest.exports['./theme.css']).toBe('./src/theme.css')
  expect(existsSync(join(source, '../', manifest.exports['./theme.css']))).toBe(true)
})
