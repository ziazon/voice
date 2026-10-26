import { readFileSync } from 'node:fs'
import { join, relative } from 'node:path'
import { expect, it } from 'vitest'
import { files, source } from './test-support'

it('keeps the native API inside the standalone adapter', () => {
  const forbidden = files(source).filter(path => /\.(ts|vue)$/.test(path)
    && !path.endsWith('.test.ts')
    && relative(source, path) !== 'app/tauri-backend.ts'
    && readFileSync(path, 'utf8').includes('@tauri-' + 'apps/'))
  expect(forbidden.map(path => relative(source, path))).toEqual([])
})

it('keeps standalone app code out of the public exports', () => {
  expect(readFileSync(join(source, 'index.ts'), 'utf8')).not.toMatch(/['"]\.\/app\//)
})
