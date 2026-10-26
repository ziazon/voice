import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant as example, button, deferred, fakeBackend, screen } from '../test-support'
import type { ClaudeCodeStatus } from '../types/generated/ClaudeCodeStatus'
import type { Said } from '../types/generated/Said'
import TurnOn from './TurnOn.vue'
const assistant = { ...example, name: 'Ari', slug: 'ari' }
const off: ClaudeCodeStatus = { state: 'off', onName: null, onSlug: null, outOfDate: false, changed: false }
it('announces loading then shows off and turns on, disabling actions until status reloads', async () => {
  const load = deferred<ClaudeCodeStatus>()
  const on = deferred<Said>()
  const backend = fakeBackend({ claudeCodeStatus: vi.fn().mockReturnValueOnce(load.promise).mockResolvedValue({ ...off, state: 'this', onName: 'Ari', onSlug: 'ari' }), turnOn: vi.fn().mockReturnValue(on.promise) })
  const wrapper = screen(TurnOn, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.find('[role="status"]').text()).toBe('Checking Claude Code…')
  load.resolve(off); await flushPromises()
  expect(wrapper.text()).toContain('Turn Ari on, and Claude Code asks Ari before it asks you a question. One assistant can be on at a time.')
  expect(wrapper.text()).toContain('Ari is off in Claude Code.')
  await button(wrapper, 'Turn Ari on').trigger('click')
  expect(backend.turnOn).toHaveBeenCalledExactlyOnceWith({ slug: 'ari' })
  expect(button(wrapper, 'Turning it on…').attributes('disabled')).toBeDefined()
  on.resolve({ message: 'Ari is on.' }); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Ari is on.')
  expect(backend.claudeCodeStatus).toHaveBeenCalledTimes(2)
  expect(wrapper.text()).toContain('Ari is on in Claude Code.')
})
it('shows not set up without a button', async () => {
  const wrapper = screen(TurnOn, fakeBackend({ claudeCodeStatus: vi.fn().mockResolvedValue({ ...off, state: 'notSetUp' }) }), { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  await flushPromises()
  expect(wrapper.text()).toContain("Claude Code isn't set up on this computer yet. Install it and sign in, then come back.")
  expect(wrapper.findAll('button')).toHaveLength(0)
})
it('shows update and changed notices and updates', async () => {
  const backend = fakeBackend({ claudeCodeStatus: vi.fn().mockResolvedValue({ ...off, state: 'this', onName: 'Ari', onSlug: 'ari', outOfDate: true, changed: true }) })
  const wrapper = screen(TurnOn, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.text()).toContain('Ari has changed since you turned it on.')
  expect(wrapper.findAll('.text-secondary').map(p => p.text())).toContain('Something Persona Builder added for Ari in your Claude folder was changed or removed.')
  await button(wrapper, 'Update Ari in Claude Code').trigger('click'); await flushPromises()
  expect(backend.turnOn).toHaveBeenCalledExactlyOnceWith({ slug: 'ari' })
})
it.each([
  ['this', 'Ari', 'ari', 'Turn Ari off', 'Ari is on in Claude Code.'],
  ['other', 'Mo Lee', 'mo-lee', 'Turn Mo Lee off', 'Mo Lee is on in Claude Code. Only one assistant can be on at a time.'],
  ['other', null, 'mo-lee', 'Turn it off', 'Another assistant is on in Claude Code. Only one assistant can be on at a time.'],
] as const)('turns off the recorded slug in %s', async (state, onName, onSlug, label, text) => {
  const pending = deferred<Said>()
  const backend = fakeBackend({ claudeCodeStatus: vi.fn().mockResolvedValue({ ...off, state, onName, onSlug }), turnOff: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(TurnOn, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.text()).toContain(text)
  await button(wrapper, label).trigger('click')
  expect(backend.turnOff).toHaveBeenCalledExactlyOnceWith({ slug: onSlug })
  expect(wrapper.findAll('button').every(b => b.attributes('disabled') !== undefined)).toBe(true)
  pending.resolve({ message: 'The assistant is off.' }); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('The assistant is off.')
  expect(backend.claudeCodeStatus).toHaveBeenCalledTimes(2)
})
it.each([
  [{ message: 'Look again.' }, 'Look again.'],
  [new Error('private detail'), "Persona Builder couldn't check Claude Code. Close it and open it again."],
])('shows safe status failure and retries', async (error, message) => {
  const backend = fakeBackend({ claudeCodeStatus: vi.fn().mockRejectedValueOnce(error).mockResolvedValue(off) })
  const wrapper = screen(TurnOn, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.find('.alert-warning').text()).toBe(message)
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(wrapper.text()).toContain('Ari is off in Claude Code.')
})
it.each([
  [{ message: 'Turn the other assistant off first.' }, 'Turn the other assistant off first.'],
  [new Error('private detail'), "Persona Builder couldn't turn Ari on, so nothing in your Claude folder was changed."],
])('shows safe on failure and reloads', async (error, message) => {
  const backend = fakeBackend({ turnOn: vi.fn().mockRejectedValue(error) })
  const wrapper = screen(TurnOn, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  await button(wrapper, 'Turn Ari on').trigger('click'); await flushPromises()
  expect(wrapper.find('.alert-warning').text()).toBe(message)
  expect(backend.claudeCodeStatus).toHaveBeenCalledTimes(2)
})
it('uses the safe off fallback and reloads', async () => {
  const backend = fakeBackend({ claudeCodeStatus: vi.fn().mockResolvedValue({ ...off, state: 'this', onSlug: 'ari' }), turnOff: vi.fn().mockRejectedValue(new Error('private')) })
  const wrapper = screen(TurnOn, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  await button(wrapper, 'Turn Ari off').trigger('click'); await flushPromises()
  expect(wrapper.find('.alert-warning').text()).toBe("Persona Builder couldn't turn the assistant off. Close it and open it again, then try again.")
  expect(backend.claudeCodeStatus).toHaveBeenCalledTimes(2)
})

it('explains what turning on adds and turning off removes', () => {
  const wrapper = screen(TurnOn, fakeBackend(), { assistant: example })
  const paragraphs = wrapper.findAll('p')
  expect(paragraphs[1]!.text()).toContain(`Turning ${example.name} on adds a skill and a short block to your Claude Code instructions.`)
  expect(paragraphs[1]!.text()).toContain("Turning it off removes what it added, except a file you've changed since.")
})
