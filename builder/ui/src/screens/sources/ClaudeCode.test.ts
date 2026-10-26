import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../../test-support'
import type { ReadCounts } from '../../types/generated/ReadCounts'
import type { ReadProgress } from '../../types/generated/ReadProgress'
import ClaudeCode from './ClaudeCode.vue'

const folders = { found: true, folders: [
  { project: 'project-one', conversations: 1200, read: true },
  { project: 'project-two', conversations: 3, read: false },
] }
const counts: ReadCounts = { questions: 1200, heldOut: 180, messages: 3500, privateLeftOut: 7, unreadable: 0, first: '2026-01-01', last: '2026-10-01' }
const counted = `Counted ${(1200).toLocaleString()} answered questions and ${(3500).toLocaleString()} messages you typed, from 2026-01-01 to 2026-10-01. 7 that looked private were left out. 180 answered questions are kept back to test your assistant.`

it('announces loading before the folders arrive', () => {
  const backend = fakeBackend({ claudeCodeFolders: vi.fn().mockReturnValue(new Promise(() => {})) })
  const wrapper = screen(ClaudeCode, backend, { slug: 'example' })
  expect(wrapper.text()).toContain('Looking at your Claude Code history…')
  expect(wrapper.find('.spinner-border').exists()).toBe(true)
  expect(backend.claudeCodeFolders).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
})
it.each([{ found: false, folders: [] }, { found: true, folders: [] }])('shows only the empty state for %o', async answer => {
  const wrapper = screen(ClaudeCode, fakeBackend({ claudeCodeFolders: vi.fn().mockResolvedValue(answer) }), { slug: 'example' })
  await flushPromises()
  expect(wrapper.text()).toContain('Persona Builder found no Claude Code history on this computer.')
  expect(wrapper.find('table').exists()).toBe(false)
  expect(wrapper.find('button').exists()).toBe(false)
})
it('shows the folder table and sums included and excluded conversations', async () => {
  const wrapper = screen(ClaudeCode, fakeBackend({ claudeCodeFolders: vi.fn().mockResolvedValue(folders) }), { slug: 'example' })
  await flushPromises()
  expect(wrapper.text()).toContain(`It will read 1 folders (${(1200).toLocaleString()} conversations) and leave out 1 (3 conversations).`)
  expect(wrapper.findAll('th').map(item => item.text())).toEqual(['Project', 'Conversations', 'Read?'])
  expect(wrapper.findAll('tbody tr').map(item => item.findAll('td').map(cell => cell.text()))).toEqual([
    ['project-one', (1200).toLocaleString(), 'Yes'], ['project-two', '3', 'Left out'],
  ])
  expect(wrapper.get('.overflow-auto').attributes('style')).toContain('max-height: 16rem')
  expect(wrapper.text()).toContain("Your assistant's settings decide which folders are left out.")
  expect(wrapper.find('a').exists()).toBe(false)
})
it('counts first with progress, then reads and saves the history', async () => {
  const counting = deferred<ReadCounts>()
  const reading = deferred<ReadCounts>()
  let progress: ((value: ReadProgress) => void) | undefined
  const readClaudeCode = vi.fn().mockImplementation((_request, callback) => { progress = callback; return counting.promise })
  const backend = fakeBackend({ claudeCodeFolders: vi.fn().mockResolvedValue(folders), readClaudeCode })
  const wrapper = screen(ClaudeCode, backend, { slug: 'example' })
  await flushPromises()
  expect(wrapper.findAll('button').map(item => item.text())).not.toContain('Read my Claude Code history')
  await button(wrapper, 'Count first').trigger('click')
  expect(readClaudeCode).toHaveBeenLastCalledWith({ slug: 'example', dryRun: true }, expect.any(Function))
  expect(button(wrapper, 'Count first').attributes('disabled')).toBeDefined()
  progress!({ done: 40, total: 80 })
  await flushPromises()
  expect(wrapper.get('[role="progressbar"]').attributes('aria-label')).toBe('Counting')
  expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('50')
  counting.resolve(counts)
  await flushPromises()
  expect(wrapper.text()).toContain(counted)
  readClaudeCode.mockImplementation((_request, callback) => { progress = callback; return reading.promise })
  await button(wrapper, 'Read my Claude Code history').trigger('click')
  expect(readClaudeCode).toHaveBeenLastCalledWith({ slug: 'example', dryRun: false }, expect.any(Function))
  expect(wrapper.findAll('button').every(item => item.attributes('disabled') !== undefined)).toBe(true)
  progress!({ done: 1, total: 2 })
  await flushPromises()
  expect(wrapper.get('[role="progressbar"]').attributes('aria-label')).toBe('Reading')
  reading.resolve(counts)
  await flushPromises()
  expect(wrapper.text()).toContain(counted.replace('Counted', 'Read'))
  expect(wrapper.get('.alert-success').text()).toBe('Done. Your Claude Code history is saved on this computer.')
})
it('omits the date range when no first date is available', async () => {
  const wrapper = screen(ClaudeCode, fakeBackend({ claudeCodeFolders: vi.fn().mockResolvedValue(folders), readClaudeCode: vi.fn().mockResolvedValue({ ...counts, first: null, last: null }) }), { slug: 'example' })
  await flushPromises()
  await button(wrapper, 'Count first').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain(`${(3500).toLocaleString()} messages you typed. 7 that looked private`)
  expect(wrapper.text()).not.toContain('from null')
})
it.each([
  [{ message: 'Try again.' }, 'Try again.'],
  [new Error('private detail'), "Persona Builder couldn't read your Claude Code history. Close it and open it again."],
])('shows a safe read rejection', async (error, message) => {
  const wrapper = screen(ClaudeCode, fakeBackend({ claudeCodeFolders: vi.fn().mockResolvedValue(folders), readClaudeCode: vi.fn().mockRejectedValue(error) }), { slug: 'example' })
  await flushPromises()
  await button(wrapper, 'Count first').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
})
it('shows the folder fallback on an unexpected rejection', async () => {
  const wrapper = screen(ClaudeCode, fakeBackend({ claudeCodeFolders: vi.fn().mockRejectedValue(new Error('private detail')) }), { slug: 'example' })
  await flushPromises()
  expect(wrapper.get('.alert').text()).toBe("Persona Builder couldn't look through your Claude Code history. Close it and open it again.")
})

it('names the history card clearly', () => {
  const wrapper = screen(ClaudeCode, fakeBackend(), { slug: 'example' })
  expect(wrapper.get('h3').text()).toBe('Claude Code history')
})

it.each([
  [3, "3 entries it couldn't read were left out."],
  [1, "1 entry it couldn't read was left out."],
  [0, ''],
])("says how many entries it couldn't read (%i)", async (unreadable, sentence) => {
  const wrapper = screen(ClaudeCode, fakeBackend({
    claudeCodeFolders: vi.fn().mockResolvedValue(folders),
    readClaudeCode: vi.fn().mockResolvedValue({ ...counts, unreadable }),
  }), { slug: 'example' })
  await flushPromises()
  await button(wrapper, 'Count first').trigger('click')
  await flushPromises()
  if (unreadable) {
    expect(wrapper.text()).toContain(`7 that looked private were left out. ${sentence} 180 answered questions`)
  } else {
    expect(wrapper.text()).toContain(counted)
    expect(wrapper.text()).not.toContain("it couldn't read")
  }
})
