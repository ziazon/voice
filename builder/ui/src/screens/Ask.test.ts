import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { Permissions } from '../types/generated/Permissions'
import type { Assistant } from '../types/generated/Assistant'
import type { AskAnswer } from '../types/generated/AskAnswer'
import Ask from './Ask.vue'

const example = { ...assistant, consent: { agreed: true, agreedOn: '2026-11-02' } }
const other = { ...assistant, slug: 'other', name: 'Other' }
const permission = { id: 'calendar.read', words: 'read your calendar', on: true, givenOn: '2026-11-02', pastDays: 0, nextDays: 14, window: 'from today to 14 days ahead' }
const permissions: Permissions = { permissions: [permission], calendar: { name: 'team.ics', folder: false }, calendarProblem: null }
const nothing = "Voice will send Example's decisions and your question, nothing else. They go to your own Claude Code, unless you've chosen to keep them on your own machines."
const calendar = "Voice will send Example's decisions, your question and your calendar for the days you allowed, nothing else. Your calendar is kept on your own machines unless you've let it go to Anthropic. Until you do, only a model server on your own machines can answer, and if none is set up, nothing is sent and Voice tells you why."
function backend(overrides: Parameters<typeof fakeBackend>[0] = {}) {
  return fakeBackend({ listAssistants: vi.fn().mockResolvedValue([example, other]), listPermissions: vi.fn().mockResolvedValue(permissions), ...overrides })
}
async function choose(wrapper: ReturnType<typeof screen>, slug = 'example') {
  await wrapper.get(`input[type="radio"][value="${slug}"]`).setValue(true)
  await flushPromises()
}
async function ready(overrides: Parameters<typeof fakeBackend>[0] = {}) {
  const service = backend(overrides)
  const wrapper = screen(Ask, service)
  await flushPromises(); await choose(wrapper)
  await wrapper.get('textarea').setValue('Which day?')
  return { wrapper, service }
}

it('loads assistants before any permissions, gates consent and starts with no choice', async () => {
  const pending = deferred<Assistant[]>()
  const service = backend({ listAssistants: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(Ask, service)
  expect(wrapper.get('[role="status"]').text()).toBe('Loading your assistants…')
  expect(button(wrapper, 'Ask').attributes('disabled')).toBeDefined()
  pending.resolve([example, other]); await flushPromises()
  expect(service.listAssistants).toHaveBeenCalledExactlyOnceWith()
  expect(wrapper.findAll('input[type="radio"]')).toHaveLength(2)
  expect(wrapper.findAll('input:checked')).toHaveLength(0)
  expect(wrapper.get('input[value="other"]').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).toContain('Needs your permission first')
  expect(service.listPermissions).not.toHaveBeenCalled()
})

it('loads the exact choice with an unticked permission, window and route words', async () => {
  const { wrapper, service } = await ready()
  expect(service.listPermissions).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(wrapper.text()).toContain('Read your calendar')
  expect(wrapper.get('.form-hint').text()).toBe('From today to 14 days ahead.')
  expect((wrapper.get('input[type="checkbox"]').element as HTMLInputElement).checked).toBe(false)
  expect(wrapper.get('[aria-live="polite"]').text()).toBe(nothing)
  await wrapper.get('input[type="checkbox"]').setValue(true)
  expect(wrapper.get('[aria-live="polite"]').text()).toBe(calendar)
})

it.each([false, true])('sends a trimmed question and exactly the ticked permissions (calendar=%s)', async ticked => {
  const { wrapper, service } = await ready()
  await wrapper.get('textarea').setValue('  Which day suits me?  ')
  if (ticked) await wrapper.get('input[type="checkbox"]').setValue(true)
  await wrapper.get('form').trigger('submit'); await flushPromises()
  expect(service.askAssistant).toHaveBeenCalledExactlyOnceWith({ slug: 'example', question: 'Which day suits me?', capabilities: ticked ? ['calendar.read'] : [] })
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('  Which day suits me?  ')
})

it('locks every control while asking and displays the exact answer with line breaks and said', async () => {
  const pending = deferred<AskAnswer>()
  const { wrapper } = await ready({ askAssistant: vi.fn().mockReturnValue(pending.promise) })
  await wrapper.get('form').trigger('submit')
  expect(wrapper.get('[role="status"]').text()).toBe('Asking Example… This can take a minute.')
  expect(wrapper.findAll('input, textarea, button').every(control => control.attributes('disabled') !== undefined)).toBe(true)
  expect(wrapper.emitted('running')).toEqual([[true]])
  pending.resolve({ answer: 'First.\n\n- one', said: 'Claude answered.' }); await flushPromises()
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  expect(wrapper.get('h3').text()).toBe("Example's answer")
  expect(wrapper.get('[data-answer]').element.textContent).toBe('First.\n\n- one')
  expect(wrapper.get('[data-answer]').attributes('style')).toContain('white-space: pre-wrap')
  expect(wrapper.get('[data-said]').text()).toBe('Claude answered.')
})

it.each([
  [{ message: 'Nothing was sent. Your calendar stays on your own machines.' }, 'Nothing was sent. Your calendar stays on your own machines.'],
  [new Error('private detail'), "Voice couldn't ask this assistant. Try again."],
])('shows safe ask failure as a warning and ends running', async (failure, message) => {
  const { wrapper } = await ready({ askAssistant: vi.fn().mockRejectedValue(failure) })
  await wrapper.get('form').trigger('submit'); await flushPromises()
  expect(wrapper.get('.alert-warning[role="alert"]').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})

it('renders answer and said as literal text', async () => {
  const value = '<img src=x onerror=alert(1)>'
  const { wrapper } = await ready({ askAssistant: vi.fn().mockResolvedValue({ answer: value, said: value }) })
  await wrapper.get('form').trigger('submit'); await flushPromises()
  expect(wrapper.get('[data-answer]').text()).toBe(value)
  expect(wrapper.get('[data-said]').text()).toBe(value)
  expect(wrapper.find('img').exists()).toBe(false)
})

it.each([[2000, false, '2,000'], [2001, true, '2,001'], [0, true, '0']])('counts trimmed Unicode characters (%s)', async (count, disabled, label) => {
  const { wrapper } = await ready()
  await wrapper.get('textarea').setValue(`  ${'🌱'.repeat(count)}  `)
  expect(wrapper.get('#ask-question-count').text()).toBe(`${label} of 2,000 characters`)
  expect(button(wrapper, 'Ask').element.matches(':disabled')).toBe(disabled)
})

it('omits off permissions and allows a question without any', async () => {
  const { wrapper, service } = await ready({ listPermissions: vi.fn().mockResolvedValue({ ...permissions, permissions: [{ ...permission, on: false, window: null }] }) })
  expect(wrapper.find('input[type="checkbox"]').exists()).toBe(false)
  expect(wrapper.text()).toContain("Example hasn't been given permission to read anything, so it will use only its decisions and your question. You can give permission on its Save page.")
  await wrapper.get('form').trigger('submit'); await flushPromises()
  expect(service.askAssistant).toHaveBeenCalledExactlyOnceWith({ slug: 'example', question: 'Which day?', capabilities: [] })
})

it.each([
  [null, 'No calendar file chosen yet. Choose one on this assistant\'s Save page.'],
  ["Voice can't find your calendar file any more. Choose it again.", "Voice can't find your calendar file any more. Choose it again."],
])('disables calendar when unavailable (%s)', async (calendarProblem, sentence) => {
  const { wrapper } = await ready({ listPermissions: vi.fn().mockResolvedValue({ ...permissions, calendar: null, calendarProblem }) })
  expect(wrapper.get('input[type="checkbox"]').attributes('disabled')).toBeDefined()
  expect(wrapper.get('.text-warning.small').text()).toBe(sentence)
})

it.each([
  [{ message: 'Look again.' }, 'Look again.'],
  [new Error('private detail'), "Voice couldn't look at this assistant's permissions. Try again."],
])('blocks during permission loading and failure, then retries', async (failure, message) => {
  const pending = deferred<Permissions>()
  const service = backend({ listPermissions: vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(permissions) })
  const wrapper = screen(Ask, service); await flushPromises(); await choose(wrapper)
  await wrapper.get('textarea').setValue('Which day?')
  expect(wrapper.get('[role="status"]').text()).toBe("Looking at this assistant's permissions…")
  expect(button(wrapper, 'Ask').attributes('disabled')).toBeDefined()
  pending.reject(failure); await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
  expect(button(wrapper, 'Ask').attributes('disabled')).toBeDefined()
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(service.listPermissions).toHaveBeenCalledTimes(2)
  expect(service.listPermissions).toHaveBeenLastCalledWith({ slug: 'example' })
  expect(button(wrapper, 'Ask').attributes('disabled')).toBeUndefined()
})

it('clears ticks, answer and said when choosing another agreed assistant', async () => {
  const { wrapper } = await ready({ listAssistants: vi.fn().mockResolvedValue([example, { ...other, consent: example.consent }]), askAssistant: vi.fn().mockResolvedValue({ answer: 'Take the morning slot.', said: 'Claude answered.' }) })
  await wrapper.get('input[type="checkbox"]').setValue(true)
  await wrapper.get('form').trigger('submit'); await flushPromises()
  expect(wrapper.get('[data-answer]').text()).toBe('Take the morning slot.')
  await choose(wrapper, 'other')
  expect(wrapper.find('[data-answer]').exists()).toBe(false)
  expect(wrapper.find('[data-said]').exists()).toBe(false)
  expect((wrapper.get('input[type="checkbox"]').element as HTMLInputElement).checked).toBe(false)
})

it.each([false, true])('ignores stale permission success or failure after a newer choice (failure=%s)', async fails => {
  const pending = deferred<Permissions>()
  const service = backend({ listAssistants: vi.fn().mockResolvedValue([example, { ...other, consent: example.consent }]), listPermissions: vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue({ ...permissions, permissions: [] }) })
  const wrapper = screen(Ask, service); await flushPromises(); await choose(wrapper); await choose(wrapper, 'other')
  if (fails) pending.reject({ message: 'Stale error.' })
  else pending.resolve(permissions)
  await flushPromises()
  expect(wrapper.find('input[type="checkbox"]').exists()).toBe(false)
  expect(wrapper.text()).toContain("Other hasn't been given permission to read anything")
  expect(wrapper.text()).not.toContain('Stale error.')
})

it('ends running when unmounted during a question', async () => {
  const pending = deferred<AskAnswer>()
  const { wrapper } = await ready({ askAssistant: vi.fn().mockReturnValue(pending.promise) })
  await wrapper.get('form').trigger('submit')
  expect(wrapper.emitted('running')).toEqual([[true]])
  wrapper.unmount()
  expect(wrapper.emitted('running')).toEqual([[false]])
})

it.each([
  [{ message: 'Look again.' }, 'Look again.'],
  [new Error('private detail'), "Your assistants couldn't be read. Close Voice and open it again."],
])('shows safe assistant listing failure', async (failure, message) => {
  const wrapper = screen(Ask, backend({ listAssistants: vi.fn().mockRejectedValue(failure) })); await flushPromises()
  expect(wrapper.get('.alert-danger[role="alert"]').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
})
it('shows the empty assistants sentence', async () => {
  const wrapper = screen(Ask, fakeBackend()); await flushPromises()
  expect(wrapper.text()).toContain('Make an assistant first.')
})
