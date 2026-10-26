import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { expect, it } from 'vitest'
import { source } from './test-support'

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
