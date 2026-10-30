import { flushPromises, type VueWrapper } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, panelDetail, screen } from '../../test-support'
import type { ConvenePanelRequest } from '../../types/generated/ConvenePanelRequest'
import type { PanelDetail } from '../../types/generated/PanelDetail'
import type { PanelProgress } from '../../types/generated/PanelProgress'
import type { PanelRan } from '../../types/generated/PanelRan'
import PanelResult from './PanelResult.vue'
import PanelRun from './PanelRun.vue'

const id = 'dl-20261030T120000Z-0a1b2c3d'
const start: ConvenePanelRequest = { question: 'Which path should we take?', background: '', kind: 'text', seats: ['pebble', 'quill'], timeBox: null }
const done: PanelRan = { id, status: 'closed', outcome: 'agreed' }
const fallback = 'The panel stopped before it finished. You can carry on from where it stopped.'
function action(wrapper: VueWrapper, text: string) {
  expect(wrapper.findAll('button').some(item => item.text() === text), `button ${text}`).toBe(true)
  return button(wrapper, text)
}
function detail(status: string, extra: Partial<PanelDetail> = {}): PanelDetail {
  return { ...panelDetail, summary: { ...panelDetail.summary, id, status, round: 2 }, ...extra }
}

it('convenes on mount, shows progress live, reloads the known id and emits running true then false', async () => {
  const call = deferred<PanelRan>()
  let progress!: (value: PanelProgress) => void
  const backend = fakeBackend({ convenePanel: vi.fn().mockImplementation((_request, callback) => { progress = callback; return call.promise }) })
  const wrapper = screen(PanelRun, backend, { id: null, start })
  await flushPromises()
  expect(backend.convenePanel).toHaveBeenCalledWith(start, expect.any(Function))
  expect(wrapper.emitted('running')).toEqual([[true]])
  progress({ id, round: 1, line: 'Pebble is considering the question.' }); await flushPromises()
  expect(wrapper.find('ul[aria-live="polite"]').exists()).toBe(true)
  expect(wrapper.get('ul[aria-live="polite"]').text()).toBe('Pebble is considering the question.')
  progress({ id, round: 2, line: 'Quill answered.' }); await flushPromises()
  expect(wrapper.get('ul[aria-live="polite"]').findAll('li').map(line => line.text())).toEqual(['Pebble is considering the question.', 'Quill answered.'])
  expect(wrapper.text()).toContain('Stopping lets the assistants finish the turns they are on. You can carry on later.')
  expect(action(wrapper, 'Back to your panels').attributes('disabled')).toBeDefined()
  await action(wrapper, 'Back to your panels').trigger('click')
  expect(wrapper.emitted('back')).toBeUndefined()
  call.resolve(done); await flushPromises()
  expect(backend.panel).toHaveBeenCalledExactlyOnceWith({ id })
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  expect(action(wrapper, 'Back to your panels').attributes('disabled')).toBeUndefined()
  await action(wrapper, 'Back to your panels').trigger('click')
  expect(wrapper.emitted('back')).toEqual([[]])
})

it.each(['before', 'after'] as const)('stops %s the first progress message and displays Stopping…', async timing => {
  const call = deferred<PanelRan>()
  let progress!: (value: PanelProgress) => void
  const backend = fakeBackend({ convenePanel: vi.fn().mockImplementation((_request, callback) => { progress = callback; return call.promise }) })
  const wrapper = screen(PanelRun, backend, { id: null, start }); await flushPromises()
  expect(backend.convenePanel).toHaveBeenCalledTimes(1)
  if (timing === 'after') { progress({ id, round: 1, line: 'The first round.' }); await flushPromises() }
  await action(wrapper, 'Stop for now').trigger('click')
  expect(action(wrapper, 'Stopping…').exists()).toBe(true)
  if (timing === 'before') {
    expect(backend.stopPanel).not.toHaveBeenCalled()
    progress({ id, round: 1, line: 'The first round.' }); await flushPromises()
  }
  expect(backend.stopPanel).toHaveBeenCalledExactlyOnceWith({ id })
  progress({ id, round: 1, line: 'Finishing the turn.' }); await flushPromises()
  expect(backend.stopPanel).toHaveBeenCalledTimes(1)
  call.reject({ message: 'Stopped by request.' }); await flushPromises()
  expect(wrapper.find('.alert-info').exists()).toBe(true)
  expect(wrapper.get('.alert-info').text()).toBe('Stopped. You can carry on from here.')
  expect(wrapper.find('.alert-danger').exists()).toBe(false)
  expect(backend.panel).toHaveBeenCalledWith({ id })
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})

it('reloads using the resolved PanelRan id when no progress supplied an id', async () => {
  const backend = fakeBackend()
  const wrapper = screen(PanelRun, backend, { id: null, start }); await flushPromises()
  expect(backend.panel).toHaveBeenCalledExactlyOnceWith({ id })
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})

it.each([{ error: { message: 'The daily limit was reached.' }, text: 'The daily limit was reached.' }, { error: new Error('private detail'), text: fallback }])('shows a call rejection through failureMessage: $text', async ({ error, text }) => {
  const backend = fakeBackend({ panel: vi.fn().mockResolvedValue(detail('stopped')), resumePanel: vi.fn().mockRejectedValue(error) })
  const wrapper = screen(PanelRun, backend, { id, start: null }); await flushPromises()
  await action(wrapper, 'Carry on').trigger('click'); await flushPromises()
  expect(wrapper.find('.alert-danger').exists()).toBe(true)
  expect(wrapper.get('.alert-danger').text()).toBe(text)
  expect(wrapper.text()).not.toContain('private detail')
  expect(backend.panel).toHaveBeenCalledTimes(2)
  expect(backend.panel).toHaveBeenLastCalledWith({ id })
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})

it('shows panel loading until its detail arrives', async () => {
  const call = deferred<PanelDetail>()
  const backend = fakeBackend({ panel: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(PanelRun, backend, { id, start: null })
  expect(wrapper.text()).toContain('Loading the panel…')
  expect(backend.panel).toHaveBeenCalledWith({ id })
  call.resolve(detail('closed')); await flushPromises()
  expect(wrapper.text()).not.toContain('Loading the panel…')
})

it.each([{ error: { message: 'The panel is missing.' }, text: 'The panel is missing.' }, { error: new Error('private detail'), text: "This panel couldn't be read. Try again." }])('shows a detail read failure through failureMessage: $text', async ({ error, text }) => {
  const wrapper = screen(PanelRun, fakeBackend({ panel: vi.fn().mockRejectedValue(error) }), { id, start: null }); await flushPromises()
  expect(wrapper.text()).toContain(text)
  expect(wrapper.text()).not.toContain('private detail')
})

it('shows the question, seats, status, round distance, withheld findings and failed seats', async () => {
  const loaded = detail('stopped', { seats: ['Pebble', 'Quill'], rounds: [{ round: 2, d: 0.423, withheld: [{ seat: 'Pebble', findings: ['email', 'term'] }], failed: ['Quill', 'Moss'] }] })
  const wrapper = screen(PanelRun, fakeBackend({ panel: vi.fn().mockResolvedValue(loaded) }), { id, start: null }); await flushPromises()
  expect(wrapper.findAll('h1, h2, h3, h4').some(heading => heading.text() === loaded.summary.question)).toBe(true)
  for (const text of ['Stopped after round 2. You can carry on from here.', 'Pebble, Quill', 'Round 2: how far apart they were, 0.42 (0 means they all agree).', "Pebble's post was kept from the others: it had an email address, a private word.", "Couldn't answer this round: Quill, Moss."]) expect(wrapper.text()).toContain(text)
})

it.each([{ value: '15m', label: '15 minutes' }, { value: '1h', label: 'An hour' }, { value: 'deadlock', label: 'No limit: until they agree or get stuck' }])('answers the paused time question with $value', async ({ value, label }) => {
  const call = deferred<PanelRan>()
  const backend = fakeBackend({ panel: vi.fn().mockResolvedValue(detail('paused', { asked: true })), resumePanel: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(PanelRun, backend, { id, start: null }); await flushPromises()
  expect(wrapper.text()).toContain('How long may they take?')
  for (const choice of ['15 minutes', 'An hour', 'No limit: until they agree or get stuck']) expect(action(wrapper, choice).exists()).toBe(true)
  expect(wrapper.findAll('button').some(item => item.text() === 'Ask me after five minutes')).toBe(false)
  await action(wrapper, label).trigger('click')
  expect(backend.resumePanel).toHaveBeenCalledWith({ id, steer: null, timeBox: value }, expect.any(Function))
  expect(wrapper.emitted('running')).toEqual([[true]])
  call.resolve(done); await flushPromises()
  expect(backend.panel).toHaveBeenCalledTimes(2)
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})

it('does not offer the time choices when paused without an asked time question', async () => {
  const backend = fakeBackend({ panel: vi.fn().mockResolvedValue(detail('paused', { asked: false })) })
  const wrapper = screen(PanelRun, backend, { id, start: null }); await flushPromises()
  expect(backend.panel).toHaveBeenCalledWith({ id })
  expect(wrapper.text()).toContain('Waiting for you: how long may they take?')
  expect(wrapper.findAll('button').some(item => item.text() === '15 minutes')).toBe(false)
})

it.each(['stopped', 'failed', 'running'])('carries on a saved %s panel', async status => {
  const backend = fakeBackend({ panel: vi.fn().mockResolvedValue(detail(status)) })
  const wrapper = screen(PanelRun, backend, { id, start: null }); await flushPromises()
  await action(wrapper, 'Carry on').trigger('click'); await flushPromises()
  expect(backend.resumePanel).toHaveBeenCalledWith({ id, steer: null, timeBox: null }, expect.any(Function))
  expect(backend.convenePanel).not.toHaveBeenCalled()
})

it('shows the closed result and reconsiders only a nonempty trimmed steer', async () => {
  const loaded = detail('closed')
  const backend = fakeBackend({ panel: vi.fn().mockResolvedValue(loaded) })
  const wrapper = screen(PanelRun, backend, { id, start: null }); await flushPromises()
  expect(wrapper.findComponent(PanelResult).exists()).toBe(true)
  expect(wrapper.getComponent(PanelResult).props('result')).toEqual(loaded.result)
  expect(wrapper.text()).toContain('Ask them to reconsider')
  expect(wrapper.text()).toContain('What should they think about again?')
  expect(action(wrapper, 'Reconsider').attributes('disabled')).toBeDefined()
  expect(wrapper.find('textarea').exists()).toBe(true)
  await wrapper.get('textarea').setValue('   ')
  expect(action(wrapper, 'Reconsider').attributes('disabled')).toBeDefined()
  await wrapper.get('textarea').setValue('  Consider the smaller route.  ')
  expect(action(wrapper, 'Reconsider').attributes('disabled')).toBeUndefined()
  await action(wrapper, 'Reconsider').trigger('click'); await flushPromises()
  expect(backend.resumePanel).toHaveBeenCalledWith({ id, steer: 'Consider the smaller route.', timeBox: null }, expect.any(Function))
})

it('renders hostile question, progress and withheld seat names literally without making elements', async () => {
  const question = '<img src=x onerror="window.hit=1">'
  const line = '<script>window.hit=1</script>'
  const seat = '<a href="https://example.com">Pebble</a>'
  const call = deferred<PanelRan>()
  const loaded = detail('stopped', { summary: { ...panelDetail.summary, question }, rounds: [{ round: 1, d: 0.42, withheld: [{ seat, findings: ['email'] }], failed: [] }] })
  const backend = fakeBackend({ convenePanel: vi.fn().mockImplementation((_request, progress) => { progress({ id, round: 1, line }); return call.promise }), panel: vi.fn().mockResolvedValue(loaded) })
  const wrapper = screen(PanelRun, backend, { id: null, start }); await flushPromises()
  expect(wrapper.text()).toContain(line)
  expect(wrapper.find('script').exists()).toBe(false)
  call.resolve(done); await flushPromises()
  expect(wrapper.text()).toContain(question)
  expect(wrapper.text()).toContain(`${seat}'s post was kept from the others: it had an email address.`)
  expect(wrapper.find('img, script, a').exists()).toBe(false)
  expect((window as unknown as { hit?: number }).hit).toBeUndefined()
})
