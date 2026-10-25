import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../test-support'
import ConnectClaude from './ConnectClaude.vue'

it('announces checking and shows ready with its note', async () => {
  const wrapper = screen(ConnectClaude, fakeBackend({ claudeCheck: vi.fn().mockResolvedValue({ ready: true, message: 'Ready.', note: 'API key is set.' }) }))
  expect(wrapper.get('[role="status"]').text()).toBe('Checking Claude Code…')
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Ready.')
  expect(wrapper.get('.text-secondary').text()).toBe('API key is set.')
  expect(wrapper.find('button').exists()).toBe(false)
})
it('shows not-ready text without links and checks again', async () => {
  const pending = deferred<{ ready: boolean; message: string; note: null }>()
  const backend = fakeBackend({ claudeCheck: vi.fn().mockResolvedValueOnce({ ready: false, message: 'Visit https://example.test/setup <a>setup</a>', note: null }).mockReturnValueOnce(pending.promise) })
  const wrapper = screen(ConnectClaude, backend)
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe('Visit https://example.test/setup <a>setup</a>')
  expect(wrapper.find('a').exists()).toBe(false)
  await button(wrapper, 'Check again').trigger('click')
  expect(wrapper.get('[role="status"]').text()).toBe('Checking Claude Code…')
  expect(backend.claudeCheck).toHaveBeenCalledTimes(2)
  pending.resolve({ ready: true, message: 'Ready.', note: null })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Ready.')
})
it('shows the safe bridge rejection and retry', async () => {
  const wrapper = screen(ConnectClaude, fakeBackend({ claudeCheck: vi.fn().mockRejectedValue(new Error('private detail')) }))
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe("Persona Builder couldn't check Claude Code. Close it and open it again.")
  expect(button(wrapper, 'Check again').text()).toBe('Check again')
  expect(wrapper.find('a').exists()).toBe(false)
})
