import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'
import { expect, it } from 'vitest'

const source = dirname(fileURLToPath(import.meta.url))
function files(folder: string): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap(entry => {
    const path = join(folder, entry.name)
    return entry.isDirectory() ? files(path) : [path]
  })
}

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
