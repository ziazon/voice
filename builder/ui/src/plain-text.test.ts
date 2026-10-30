import { readFileSync } from 'node:fs'
import { relative } from 'node:path'
import { expect, it } from 'vitest'
import { files, source } from './test-support'

it('keeps every screen and TypeScript source free of HTML insertion', () => {
  const forbidden = files(source).filter(path => /\.(ts|vue)$/.test(path)
    && !path.endsWith('.test.ts')
    && !relative(source, path).startsWith('types/generated/')
    && /v-html|innerHTML|outerHTML|insertAdjacentHTML|document\.write/.test(readFileSync(path, 'utf8')))
  expect(forbidden.map(path => relative(source, path))).toEqual([])
})

it('binds no link, source or style to anything on the panel screens', () => {
  const panelScreens = files(source).filter(path => /screens\/(Panels\.vue|panels\/.*\.vue)$/.test(relative(source, path)))
  expect(panelScreens.length).toBeGreaterThan(0)
  const bound = panelScreens.filter(path => /(?:^|\s)(?::|v-bind:)(?:href|src|srcdoc|style|action|formaction)\b/m.test(readFileSync(path, 'utf8')))
  expect(bound.map(path => relative(source, path))).toEqual([])
})
