import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, deferred, fakeBackend, screen } from '../test-support'
import type { ReviewStatus } from '../types/generated/ReviewStatus'
import Review from './Review.vue'
import TeachIt from './review/TeachIt.vue'
import Tests from './review/Tests.vue'
import Refresh from './review/Refresh.vue'
const status: ReviewStatus = { built: true, areas: ['Work'], tests: { waiting: 0, inUse: 0, retired: 0 }, byArea: [], change: null }
it('loads answers, replaces every child status when one changes, and relays running', async () => {
  const call = deferred<ReviewStatus>()
  const backend = fakeBackend({ reviewStatus: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(Review, backend, { assistant })
  expect(wrapper.get('.visually-hidden').text()).toBe('Reading your answers…')
  expect(backend.reviewStatus).toHaveBeenCalledWith({ slug: 'example' })
  call.resolve(status); await flushPromises()
  const changed = { ...status, tests: { waiting: 1, inUse: 0, retired: 0 } }
  wrapper.getComponent(TeachIt).vm.$emit('changed', changed); await flushPromises()
  for (const child of [TeachIt, Tests, Refresh]) {
    expect(wrapper.getComponent(child).props('status')).toEqual(changed)
    expect(wrapper.getComponent(child).props('slug')).toBe('example')
  }
  wrapper.getComponent(Refresh).vm.$emit('running', true)
  wrapper.getComponent(Refresh).vm.$emit('running', false)
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})
it('shows the review settings fallback on failure', async () => {
  const wrapper = screen(Review, fakeBackend({ reviewStatus: vi.fn().mockRejectedValue(new Error('private detail')) }), { assistant })
  await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe("Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
})
