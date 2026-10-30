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

const boundAttribute = /(?:^|\s)(?:v-bind\s*=|(?::|v-bind:)(?:\[|(?:href|src|srcdoc|style|action|formaction)\b))/m

it('binds no link, source or style to anything on the panel screens', () => {
  const panelScreens = files(source).filter(path => /screens\/(Panels\.vue|panels\/.*\.vue)$/.test(relative(source, path)))
  expect(panelScreens.length).toBeGreaterThan(0)
  const bound = panelScreens.filter(path => boundAttribute.test(readFileSync(path, 'utf8')))
  expect(bound.map(path => relative(source, path))).toEqual([])
})

it.each(['v-bind="{ title: value }"', ':[attribute]="value"', 'v-bind:[attribute]="value"'])('rejects a panel binding that can hide its attribute: %s', binding => {
  expect(boundAttribute.test(`<div ${binding}>Text</div>`)).toBe(true)
})
