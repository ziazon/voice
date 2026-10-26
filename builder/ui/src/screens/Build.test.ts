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
  expect(backend.buildAssistant).toHaveBeenCalledWith({ slug: 'example' }, expect.any(Function))
  expect(backend.interviewStatus).toHaveBeenCalledTimes(2)
  wrapper.getComponent(BuildRun).vm.$emit('running', true)
  expect(wrapper.emitted('running')).toContainEqual([true])
})

it('names the assistant and explains the build, daily limit and resuming', () => {
  const wrapper = screen(Build, fakeBackend(), { assistant })
  expect(wrapper.get('h2').text()).toBe(`Build ${assistant.name}`)
  expect(wrapper.text()).toContain('Persona Builder sends your history to Claude through your own Claude Code, a part at a time, and learns how you decide.')
  expect(wrapper.text()).toContain('It uses your own Claude plan, up to the daily limit you set on Connect Claude.')
  expect(wrapper.text()).toContain('You can leave it running, or stop and carry on later from where it got to.')
  expect(wrapper.getComponent(BuildRun).props('name')).toBe(assistant.name)
})
