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
