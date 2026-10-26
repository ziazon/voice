import { readFileSync } from 'node:fs'
import { relative } from 'node:path'
import { expect, it } from 'vitest'
import { files, source } from './test-support'

it('uses words that fit any computer', () => {
  const forbidden = files(source).filter(path => /\.(ts|vue)$/.test(path)
    && !path.endsWith('.test.ts')
    && !relative(source, path).startsWith('types/generated/')
    && /\bMac\b|Finder/.test(readFileSync(path, 'utf8')))
  expect(forbidden.map(path => relative(source, path))).toEqual([])
})
