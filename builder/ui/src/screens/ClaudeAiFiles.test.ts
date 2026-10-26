import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant as example, button, deferred, fakeBackend, screen } from '../test-support'
import type { ClaudeAiFiles as Files } from '../types/generated/ClaudeAiFiles'
import ClaudeAiFiles from './ClaudeAiFiles.vue'
const assistant = { ...example, name: 'Ari', slug: 'ari' }
const files = ['instructions.md', 'decisions-scope.md']
it('loads, makes files, shows the list, steps, Show the folder and remake', async () => {
  const load = deferred<Files>(); const make = deferred<Files>()
  const backend = fakeBackend({ claudeAiFiles: vi.fn().mockReturnValue(load.promise), makeClaudeAiFiles: vi.fn().mockReturnValueOnce(make.promise).mockResolvedValue({ files: ['instructions.md', 'decisions-writing-voice.md'] }) })
  const wrapper = screen(ClaudeAiFiles, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.find('[role="status"]').text()).toBe('Looking for the claude.ai files…')
  load.resolve({ files: [] }); await flushPromises()
  expect(backend.claudeAiFiles).toHaveBeenCalledExactlyOnceWith({ slug: 'ari' })
  await button(wrapper, 'Make the claude.ai files').trigger('click')
  expect(button(wrapper, 'Making them…').attributes('disabled')).toBeDefined()
  make.resolve({ files }); await flushPromises()
  expect(backend.makeClaudeAiFiles).toHaveBeenCalledExactlyOnceWith({ slug: 'ari' })
  expect(wrapper.findAll('code').map(item => item.text())).toEqual(files)
  expect(wrapper.findAll('ol li').map(item => item.text())).toEqual([
    'In claude.ai, make a new Project.',
    "Open instructions.md and paste what it says into the Project's instructions.",
    "Add the other files to the Project's knowledge with the + button.",
  ])
  expect(wrapper.text()).toContain('If you change Ari, make the files again and replace them in the Project.')
  await button(wrapper, 'Show the folder').trigger('click'); await flushPromises()
  expect(backend.showClaudeAiFiles).toHaveBeenCalledExactlyOnceWith({ slug: 'ari' })
  await button(wrapper, 'Make them again').trigger('click'); await flushPromises()
  expect(wrapper.findAll('code').map(item => item.text())).toEqual(['instructions.md', 'decisions-writing-voice.md'])
  expect(wrapper.find('a').exists()).toBe(false)
})
it.each([
  [{ message: 'Build Ari first.' }, 'Build Ari first.'],
  [new Error('private detail'), "Persona Builder couldn't make the claude.ai files. Check there's space on this computer, then try again."],
])('shows safe make failure', async (error, message) => {
  const wrapper = screen(ClaudeAiFiles, fakeBackend({ makeClaudeAiFiles: vi.fn().mockRejectedValue(error) }), { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Make the claude.ai files')
  await button(wrapper, 'Make the claude.ai files').trigger('click'); await flushPromises()
  expect(wrapper.find('.alert-warning').text()).toBe(message)
  expect(wrapper.find('a').exists()).toBe(false)
})
it.each([
  [{ message: 'Look again.' }, 'Look again.'],
  [new Error('private detail'), "Persona Builder couldn't look for the claude.ai files. Close it and open it again."],
])('shows safe list failure and retries', async (error, message) => {
  const backend = fakeBackend({ claudeAiFiles: vi.fn().mockRejectedValueOnce(error).mockResolvedValue({ files }) })
  const wrapper = screen(ClaudeAiFiles, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.find('.alert-warning').text()).toBe(message)
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(wrapper.findAll('code').map(item => item.text())).toEqual(files)
})
it.each([
  [{ message: 'The folder could not open.' }, 'The folder could not open.'],
  [new Error('private detail'), "Persona Builder couldn't open the folder."],
])('disables every action during Show the folder and shows a safe small failure', async (error, message) => {
  const pending = deferred<void>()
  const wrapper = screen(ClaudeAiFiles, fakeBackend({ claudeAiFiles: vi.fn().mockResolvedValue({ files }), showClaudeAiFiles: vi.fn().mockReturnValue(pending.promise) }), { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Show the folder')
  await button(wrapper, 'Show the folder').trigger('click')
  expect(wrapper.findAll('button').every(b => b.attributes('disabled') !== undefined)).toBe(true)
  pending.reject(error); await flushPromises()
  expect(wrapper.find('.alert-warning.small').text()).toBe(message)
})
