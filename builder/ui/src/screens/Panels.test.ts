import { flushPromises, type VueWrapper } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, panelDetail, screen } from '../test-support'
import type { PanelSummary } from '../types/generated/PanelSummary'
import Panels from './Panels.vue'
import NewPanel from './panels/NewPanel.vue'
import PanelRun from './panels/PanelRun.vue'

const sentence = 'Ask several of your assistants one question. They work it out in rounds and come back with a joint answer, with any disagreement in full.'
function action(wrapper: VueWrapper, text: string) {
  expect(wrapper.findAll('button').some(item => item.text() === text), `button ${text}`).toBe(true)
  return button(wrapper, text)
}

it('loads the list, introduces panels and shows the empty state', async () => {
  const call = deferred<PanelSummary[]>()
  const backend = fakeBackend({ listPanels: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(Panels, backend)
  expect(wrapper.text()).toContain('Loading your panels…')
  expect(wrapper.findAll('h1, h2, h3').some(heading => heading.text() === 'Panels')).toBe(true)
  expect(wrapper.text()).toContain(sentence)
  expect(backend.listPanels).toHaveBeenCalledExactlyOnceWith()
  call.resolve([]); await flushPromises()
  expect(wrapper.text()).toContain('No panels yet.')
  expect(wrapper.text()).not.toContain('Loading your panels…')
  expect(action(wrapper, 'Ask a new panel').exists()).toBe(true)
})

it.each([{ error: { message: 'The panel records are unreadable.' }, text: 'The panel records are unreadable.' }, { error: new Error('private detail'), text: "Your panels couldn't be read. Try again." }])('shows list errors through failureMessage: $text', async ({ error, text }) => {
  const wrapper = screen(Panels, fakeBackend({ listPanels: vi.fn().mockRejectedValue(error) })); await flushPromises()
  expect(wrapper.text()).toContain(text)
  expect(wrapper.text()).not.toContain('private detail')
  expect(wrapper.text()).not.toContain('No panels yet.')
})

it('shows rows with statuses and opens the chosen row with its id', async () => {
  const rows = [{ ...panelDetail.summary, id: 'first', question: 'Which route?', status: 'closed', round: 2 }, { ...panelDetail.summary, id: 'second', question: 'Which tool?', status: 'stopped', round: 3 }]
  const wrapper = screen(Panels, fakeBackend({ listPanels: vi.fn().mockResolvedValue(rows) })); await flushPromises()
  const row = wrapper.findAll('button').find(item => item.text().includes('Which tool?'))
  expect(row).toBeDefined()
  expect(row!.text()).toContain('Stopped after round 3. You can carry on from here.')
  expect(wrapper.text()).toContain('Finished after round 2.')
  await row!.trigger('click')
  expect(wrapper.findComponent(PanelRun).exists()).toBe(true)
  expect(wrapper.getComponent(PanelRun).props()).toMatchObject({ id: 'second', start: null })
})

it('opens a new panel, cancels to the list and starts with its exact request', async () => {
  const wrapper = screen(Panels); await flushPromises()
  await action(wrapper, 'Ask a new panel').trigger('click')
  expect(wrapper.findComponent(NewPanel).exists()).toBe(true)
  wrapper.getComponent(NewPanel).vm.$emit('cancel'); await flushPromises()
  expect(wrapper.findComponent(NewPanel).exists()).toBe(false)
  await action(wrapper, 'Ask a new panel').trigger('click')
  const request = { question: 'Which route?', background: 'Keep it small.', kind: 'list:5', seats: ['pebble', 'quill'], timeBox: '15m' }
  wrapper.getComponent(NewPanel).vm.$emit('start', request); await flushPromises()
  expect(wrapper.findComponent(PanelRun).exists()).toBe(true)
  expect(wrapper.getComponent(PanelRun).props()).toMatchObject({ id: null, start: request })
})

it('forwards running and reloads the list when the run goes back', async () => {
  const backend = fakeBackend({ listPanels: vi.fn().mockResolvedValueOnce([panelDetail.summary]).mockResolvedValue([]) })
  const wrapper = screen(Panels, backend); await flushPromises()
  const row = wrapper.findAll('button').find(item => item.text().includes(panelDetail.summary.question))
  expect(row).toBeDefined()
  await row!.trigger('click')
  expect(wrapper.findComponent(PanelRun).exists()).toBe(true)
  wrapper.getComponent(PanelRun).vm.$emit('running', true)
  wrapper.getComponent(PanelRun).vm.$emit('running', false)
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  wrapper.getComponent(PanelRun).vm.$emit('back'); await flushPromises()
  expect(backend.listPanels).toHaveBeenCalledTimes(2)
  expect(wrapper.findComponent(PanelRun).exists()).toBe(false)
  expect(wrapper.text()).toContain('No panels yet.')
})

it('renders a hostile list question literally without creating markup', async () => {
  const question = '<img src=x onerror="window.hit=1">'
  const wrapper = screen(Panels, fakeBackend({ listPanels: vi.fn().mockResolvedValue([{ ...panelDetail.summary, question }]) })); await flushPromises()
  expect(wrapper.text()).toContain(question)
  expect(wrapper.find('img').exists()).toBe(false)
  expect((window as unknown as { hit?: number }).hit).toBeUndefined()
})
