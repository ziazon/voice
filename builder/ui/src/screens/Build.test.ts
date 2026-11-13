import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, fakeBackend, screen } from '../test-support'
import Build from './Build.vue'
import BuildRun from './build/BuildRun.vue'
import Interview from './build/Interview.vue'
it('rebuilds from the interview and reloads it after a finished build', async () => {
  const backend = fakeBackend()
  const wrapper = screen(Build, backend, { assistant }); await flushPromises()
  wrapper.getComponent(Interview).vm.$emit('rebuild'); await flushPromises()
  expect(backend.mergeHistories).toHaveBeenCalledWith({ slug: 'example' })
  expect(backend.buildAssistant).toHaveBeenCalledWith({ slug: 'example', noLimit: false }, expect.any(Function))
  expect(backend.interviewStatus).toHaveBeenCalledTimes(2)
  wrapper.getComponent(BuildRun).vm.$emit('running', true)
  expect(wrapper.emitted('running')).toContainEqual([true])
})

it('names the assistant and explains the build, daily limit and resuming', () => {
  const wrapper = screen(Build, fakeBackend(), { assistant })
  expect(wrapper.get('h2').text()).toBe(`Build ${assistant.name}`)
  expect(wrapper.text()).toContain('Voice sends your history to Claude through your own Claude Code, a part at a time, and learns how you decide.')
  expect(wrapper.text()).toContain('It uses your own Claude plan, up to the daily limit you set on Connect Claude, unless you turn on Keep going below.')
  expect(wrapper.text()).toContain('You can leave it running, or stop and carry on later from where it got to.')
  expect(wrapper.getComponent(BuildRun).props('name')).toBe(assistant.name)
})

it('explains the subscription stop after initial spend while keeping the first and last sentences', async () => {
  const backend = fakeBackend({ spendToday: vi.fn().mockResolvedValue({ spentUsd: 3, dailyCapUsd: 50, usage: 'Today, 2 calls to Claude and 850 tokens, on your Claude subscription.' }) })
  const wrapper = screen(Build, backend, { assistant }); await flushPromises()
  expect(wrapper.text()).toContain('It uses your own Claude plan, and stops for the day when Claude Code says your plan is nearly used up, unless you turn on Keep going below.')
  expect(wrapper.text()).not.toContain('up to the daily limit you set on Connect Claude')
  expect(wrapper.text()).toContain('Voice sends your history to Claude through your own Claude Code, a part at a time, and learns how you decide.')
  expect(wrapper.text()).toContain('You can leave it running, or stop and carry on later from where it got to.')
})
