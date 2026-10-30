import { flushPromises, type DOMWrapper, type VueWrapper } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, deferred, fakeBackend, screen } from '../../test-support'
import type { Assistant } from '../../types/generated/Assistant'
import NewPanel from './NewPanel.vue'

const assistants: Assistant[] = ['Pebble', 'Quill', 'Moss'].map(name => ({
  ...assistant, slug: name.toLowerCase(), name, principal: 'Rowan Vale',
  consent: { agreed: true, agreedOn: '2026-10-30' },
}))
function control(wrapper: VueWrapper, label: string): DOMWrapper<HTMLInputElement | HTMLTextAreaElement> {
  const labels = wrapper.findAll('label').filter(node => node.text().trim() === label)
  expect(labels, `labelled control: ${label}`).toHaveLength(1)
  const item = labels[0]!
  const id = item.attributes('for')
  const field = id ? wrapper.find<HTMLInputElement | HTMLTextAreaElement>(`[id="${id}"]`) : item.find<HTMLInputElement | HTMLTextAreaElement>('input, textarea')
  expect(field.exists(), `input for ${label}`).toBe(true)
  return field
}
function action(wrapper: VueWrapper, label: string) {
  const buttons = wrapper.findAll('button').filter(item => item.text() === label)
  expect(buttons, `button: ${label}`).toHaveLength(1)
  return buttons[0]!
}
async function ready(list = assistants) {
  const backend = fakeBackend({ listAssistants: vi.fn().mockResolvedValue(list) })
  const wrapper = screen(NewPanel, backend)
  await flushPromises()
  return { wrapper, backend }
}
async function valid(wrapper: VueWrapper) {
  await control(wrapper, 'The question').setValue('  Which path should we try?  ')
  await control(wrapper, 'Moss').setValue(true)
  await control(wrapper, 'Pebble').setValue(true)
}
async function submit(wrapper: VueWrapper) {
  const start = action(wrapper, 'Start the panel')
  expect(start.attributes('disabled')).toBeUndefined()
  await start.trigger('click')
  const form = wrapper.find('form')
  if (!wrapper.emitted('start') && form.exists()) await form.trigger('submit')
}

it('shows loading while it reads the assistants', async () => {
  const pending = deferred<Assistant[]>()
  const backend = fakeBackend({ listAssistants: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(NewPanel, backend)
  expect(wrapper.text()).toContain('Loading your assistants…')
  expect(backend.listAssistants).toHaveBeenCalledExactlyOnceWith()
  pending.resolve(assistants); await flushPromises()
  expect(wrapper.text()).not.toContain('Loading your assistants…')
})
it.each([
  [{ message: 'The assistant list is unavailable.' }, 'The assistant list is unavailable.'],
  [new Error('private detail'), "Your assistants couldn't be read. Close Persona Builder and open it again."],
])('shows the safe assistant error %s', async (error, words) => {
  const wrapper = screen(NewPanel, fakeBackend({ listAssistants: vi.fn().mockRejectedValue(error) }))
  await flushPromises()
  expect(wrapper.text()).toContain(words)
  expect(wrapper.text()).not.toContain('private detail')
})
it.each([0, 1])('explains why %i assistants cannot make a panel', async count => {
  const { wrapper } = await ready(assistants.slice(0, count))
  expect(wrapper.text()).toContain('Make at least two assistants first. A panel needs two or more.')
})
it('disables the assistant without permission and explains why', async () => {
  const { wrapper } = await ready([assistants[0]!, { ...assistants[1]!, consent: { agreed: false, agreedOn: null } }])
  expect(control(wrapper, 'Quill').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).toContain('Needs your permission first')
})
it('labels the fields, counts trimmed Unicode characters and background bytes without maxlength', async () => {
  const { wrapper } = await ready()
  const question = control(wrapper, 'The question')
  expect(question.attributes('maxlength')).toBeUndefined()
  await question.setValue('  🌱🌱  ')
  expect(wrapper.text()).toContain('2 of 200 characters')
  await control(wrapper, 'Anything they should know (optional)').setValue('é🌱')
  expect(wrapper.text()).toMatch(/6\s+of 4096 \(some letters and symbols count as more than one\)/)
  expect(wrapper.text()).toContain('The kind of answer')
  expect(wrapper.text()).toContain('Who sits on the panel')
  expect(wrapper.text()).toContain('How long they may take')
  expect(wrapper.text()).toContain("Each assistant's turns use your Claude plan and count toward that assistant's daily limit.")
})
it.each([
  ['empty question', '   ', '', null, 2],
  ['201 characters', 'q'.repeat(201), '', null, 2],
  ['4097 background bytes', 'Question?', 'é'.repeat(2048) + 'a', null, 2],
  ['zero list items', 'Question?', '', 0, 2],
  ['13 list items', 'Question?', '', 13, 2],
  ['fractional list items', 'Question?', '', 1.5, 2],
  ['one assistant', 'Question?', '', null, 1],
  ['25 assistants', 'Question?', '', null, 25],
] as const)('disables start for %s', async (_case, question, background, count, seats) => {
  const list = Array.from({ length: Math.max(2, seats) }, (_, index) => ({ ...assistants[0]!, slug: `seat-${index}`, name: `Assistant ${index + 1}` }))
  const { wrapper } = await ready(list)
  await control(wrapper, 'The question').setValue(question)
  await control(wrapper, 'Anything they should know (optional)').setValue(background)
  if (count !== null) {
    await control(wrapper, 'A list').setValue(true)
    await control(wrapper, 'How many items').setValue(count)
  }
  for (const item of list.slice(0, seats)) await control(wrapper, item.name).setValue(true)
  expect(action(wrapper, 'Start the panel').attributes('disabled')).toBeDefined()
  expect(wrapper.emitted('start')).toBeUndefined()
})
it('starts disabled before a question or seats are provided', async () => {
  const { wrapper } = await ready()
  expect(action(wrapper, 'Start the panel').attributes('disabled')).toBeDefined()
})
it('emits the exact text request with trimmed question, unchanged background and listing order', async () => {
  const { wrapper } = await ready()
  await valid(wrapper)
  await control(wrapper, 'Anything they should know (optional)').setValue('  Keep this context.\n')
  await submit(wrapper)
  expect(wrapper.emitted('start')).toEqual([[{
    question: 'Which path should we try?', background: '  Keep this context.\n', kind: 'text', seats: ['pebble', 'moss'], timeBox: null,
  }]])
})
it('starts a list at three items and emits list:5 after the number changes', async () => {
  const { wrapper } = await ready()
  await valid(wrapper)
  await control(wrapper, 'A list').setValue(true)
  const count = control(wrapper, 'How many items')
  expect((count.element as HTMLInputElement).value).toBe('3')
  expect(count.attributes('min')).toBe('1')
  expect(count.attributes('max')).toBe('12')
  await count.setValue(5)
  await submit(wrapper)
  expect(wrapper.emitted('start')).toEqual([[{
    question: 'Which path should we try?', background: '', kind: 'list:5', seats: ['pebble', 'moss'], timeBox: null,
  }]])
})
it.each([
  ['Ask me after five minutes', null], ['15 minutes', '15m'], ['An hour', '1h'],
  ['No limit: until they agree or get stuck', 'deadlock'],
])('emits the time limit for %s', async (label, value) => {
  const { wrapper } = await ready()
  await valid(wrapper)
  await control(wrapper, label!).setValue(true)
  await submit(wrapper)
  expect(wrapper.emitted('start')).toEqual([[{
    question: 'Which path should we try?', background: '', kind: 'text', seats: ['pebble', 'moss'], timeBox: value,
  }]])
})
it('accepts the inclusive question, background and seat limits', async () => {
  const list = Array.from({ length: 24 }, (_, index) => ({ ...assistants[0]!, slug: `seat-${index}`, name: `Assistant ${index + 1}` }))
  const { wrapper } = await ready(list)
  await control(wrapper, 'The question').setValue('🌱'.repeat(200))
  await control(wrapper, 'Anything they should know (optional)').setValue('é'.repeat(2048))
  for (const item of list) await control(wrapper, item.name).setValue(true)
  expect(action(wrapper, 'Start the panel').attributes('disabled')).toBeUndefined()
})
it('emits cancel without starting a panel', async () => {
  const { wrapper } = await ready()
  await action(wrapper, 'Cancel').trigger('click')
  expect(wrapper.emitted('cancel')).toEqual([[]])
  expect(wrapper.emitted('start')).toBeUndefined()
})

it('derives selected seats from the selection alone', async () => {
  const { wrapper } = await ready([assistants[0]!, { ...assistants[1]!, consent: { agreed: false, agreedOn: null } }])
  const form = wrapper.vm as unknown as { selected: string[], seats: string[] }
  form.selected = ['quill', 'pebble']
  await flushPromises()
  expect(form.seats).toEqual(['pebble', 'quill'])
  expect(control(wrapper, 'Quill').attributes('disabled')).toBeDefined()
})
