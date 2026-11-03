import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { AuditLog as Log } from '../types/generated/AuditLog'
import AuditLog from './AuditLog.vue'

it('loads fifty entries and renders the returned order in a bounded plain list', async () => {
  const load = deferred<Log>()
  const backend = fakeBackend({ auditLog: vi.fn().mockReturnValue(load.promise) })
  const wrapper = screen(AuditLog, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h3').exists()).toBe(true)
  expect(backend.auditLog).toHaveBeenCalledExactlyOnceWith({ slug: 'example', last: 50 })
  load.resolve({ lines: ['Permission taken back.', 'Permission given.'], unreadable: 0, older: false }); await flushPromises()
  expect(wrapper.get('h3').text()).toBe('What Voice has logged')
  expect(wrapper.text()).toContain('Each time permission is given or taken back, and each time a source is read or a read is refused. What was read is never logged, only how many items.')
  expect(wrapper.findAll('li').map(line => line.text())).toEqual(['Permission taken back.', 'Permission given.'])
  expect(wrapper.get('ul').classes()).toEqual(expect.arrayContaining(['list-unstyled', 'mb-0', 'small']))
  expect(wrapper.get('.overflow-auto').attributes('style')).toContain('max-height: 20rem')
})

it.each([[1, "1 entry couldn't be read."], [2, "2 entries couldn't be read."]])('shows unreadable count %i', async (unreadable, sentence) => {
  const wrapper = screen(AuditLog, fakeBackend({ auditLog: vi.fn().mockResolvedValue({ lines: [], unreadable, older: false }) }), { assistant }); await flushPromises()
  expect(wrapper.find('h3').exists()).toBe(true)
  expect(wrapper.text()).toContain(sentence)
  expect(wrapper.text()).not.toContain('Nothing has been logged')
})

it('shows older entries and expands to one thousand once', async () => {
  const backend = fakeBackend({ auditLog: vi.fn().mockResolvedValue({ lines: ['A line.'], unreadable: 0, older: true }) })
  const wrapper = screen(AuditLog, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h3').exists()).toBe(true)
  expect(wrapper.text()).toContain("Older entries aren't shown.")
  await button(wrapper, 'Show more').trigger('click'); await flushPromises()
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
  expect(backend.auditLog).toHaveBeenLastCalledWith({ slug: 'example', last: 1000 })
  expect(wrapper.text()).toContain("Older entries aren't shown.")
  expect(wrapper.findAll('button').map(item => item.text())).not.toContain('Show more')
})

it('shows the empty sentence with the assistant name', async () => {
  const wrapper = screen(AuditLog, fakeBackend(), { assistant }); await flushPromises()
  expect(wrapper.find('h3').exists()).toBe(true)
  expect(wrapper.text()).toContain('Nothing has been logged for Example yet.')
})

it.each([
  [{ message: 'Read again.' }, 'Read again.'],
  [new Error('private detail'), "Voice couldn't read this assistant's log. Try again."],
])('shows safe load failure then retries the same request', async (failure, sentence) => {
  const backend = fakeBackend({ auditLog: vi.fn().mockRejectedValueOnce(failure).mockResolvedValue({ lines: ['A line.'], unreadable: 0, older: false }) })
  const wrapper = screen(AuditLog, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h3').exists()).toBe(true)
  expect(wrapper.get('.alert-warning').text()).toBe(sentence)
  expect(wrapper.get('.alert-warning').attributes('role')).toBe('alert')
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
  expect(backend.auditLog).toHaveBeenLastCalledWith({ slug: 'example', last: 50 })
  expect(wrapper.get('li').text()).toBe('A line.')
})

it('renders a log line as literal text without markup', async () => {
  const line = '<img src=x onerror=alert(1)>'
  const wrapper = screen(AuditLog, fakeBackend({ auditLog: vi.fn().mockResolvedValue({ lines: [line], unreadable: 0, older: false }) }), { assistant }); await flushPromises()
  expect(wrapper.find('h3').exists()).toBe(true)
  expect(wrapper.get('li').text()).toBe(line)
  expect(wrapper.find('img').exists()).toBe(false)
})
