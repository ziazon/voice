import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { ReviewStatus } from '../types/generated/ReviewStatus'
import Review from './Review.vue'
import TeachIt from './review/TeachIt.vue'
import Tests from './review/Tests.vue'
import Refresh from './review/Refresh.vue'
const status: ReviewStatus = { built: true, areas: ['Work'], tests: { waiting: 0, inUse: 0, retired: 0 }, byArea: [], change: null }
it('loads answers, replaces every child status when one changes, and relays running', async () => {
  const call = deferred<ReviewStatus>()
  const backend = fakeBackend({ reviewStatus: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(Review, backend, { assistant, active: true })
  expect(wrapper.get('.visually-hidden').text()).toBe('Reading your answers…')
  expect(backend.reviewStatus).toHaveBeenCalledWith({ slug: 'example' })
  call.resolve(status); await flushPromises()
  const changed = { ...status, tests: { waiting: 1, inUse: 0, retired: 0 } }
  wrapper.getComponent(TeachIt).vm.$emit('changed', changed); await flushPromises()
  for (const child of [TeachIt, Tests, Refresh]) {
    expect(wrapper.getComponent(child).props('status')).toEqual(changed)
  }
  for (const child of [TeachIt, Refresh]) expect(wrapper.getComponent(child).props('slug')).toBe('example')
  wrapper.getComponent(Refresh).vm.$emit('running', true)
  wrapper.getComponent(Refresh).vm.$emit('running', false)
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
})
it('shows the review settings fallback on failure', async () => {
  const wrapper = screen(Review, fakeBackend({ reviewStatus: vi.fn().mockRejectedValue(new Error('private detail')) }), { assistant, active: true })
  await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe("Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
})

it('reloads status for every child when active again after the first load', async () => {
  const changed = { ...status, tests: { waiting: 2, inUse: 1, retired: 0 } }
  const backend = fakeBackend({ reviewStatus: vi.fn().mockResolvedValueOnce(status).mockResolvedValue(changed) })
  const wrapper = screen(Review, backend, { assistant, active: true })
  await flushPromises()
  expect(backend.reviewStatus).toHaveBeenCalledTimes(1)
  await wrapper.setProps({ active: false })
  expect(backend.reviewStatus).toHaveBeenCalledTimes(1)
  await wrapper.setProps({ active: true }); await flushPromises()
  expect(backend.reviewStatus).toHaveBeenCalledTimes(2)
  expect(backend.reviewStatus).toHaveBeenLastCalledWith({ slug: 'example' })
  for (const child of [TeachIt, Tests, Refresh]) expect(wrapper.getComponent(child).props('status')).toEqual(changed)
})
it('does not reload on activation while a refresh test is running', async () => {
  const backend = fakeBackend({ reviewStatus: vi.fn().mockResolvedValue(status) })
  const wrapper = screen(Review, backend, { assistant, active: true })
  await flushPromises()
  wrapper.getComponent(Refresh).vm.$emit('running', true)
  await wrapper.setProps({ active: false })
  await wrapper.setProps({ active: true }); await flushPromises()
  expect(backend.reviewStatus).toHaveBeenCalledTimes(1)
  wrapper.getComponent(Refresh).vm.$emit('running', false)
  await wrapper.setProps({ active: false })
  await wrapper.setProps({ active: true }); await flushPromises()
  expect(backend.reviewStatus).toHaveBeenCalledTimes(2)
})
it('shows the same fallback on a failed reload and clears it after a successful retry', async () => {
  const backend = fakeBackend({ reviewStatus: vi.fn().mockResolvedValueOnce(status).mockRejectedValueOnce(new Error('private detail')).mockResolvedValue(status) })
  const wrapper = screen(Review, backend, { assistant, active: true })
  await flushPromises()
  await wrapper.setProps({ active: false })
  await wrapper.setProps({ active: true }); await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe("Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
  await wrapper.setProps({ active: false })
  await wrapper.setProps({ active: true }); await flushPromises()
  expect(wrapper.find('.alert-danger').exists()).toBe(false)
})

it('keeps Refresh lines and outcome across a status reload', async () => {
  const approved = { ...status, change: { id: 'change-one', drafted: '2026-10-25', summary: 'Try one part.', points: [], learnedFrom: 1, approved: true } }
  const backend = fakeBackend({
    reviewStatus: vi.fn().mockResolvedValueOnce(approved).mockResolvedValue(status),
    applyRefresh: vi.fn().mockImplementation((_request, progress) => {
      progress({ line: 'Testing the change.', spend: { spentUsd: 1, dailyCapUsd: 50 } })
      return Promise.resolve({ kept: true, reason: 'Every test passed.', spend: { spentUsd: 2, dailyCapUsd: 50 } })
    }),
  })
  const wrapper = screen(Review, backend, { assistant, active: true }); await flushPromises()
  await button(wrapper, 'Test it').trigger('click'); await flushPromises()
  expect(backend.reviewStatus).toHaveBeenCalledTimes(2)
  await wrapper.setProps({ active: false })
  await wrapper.setProps({ active: true }); await flushPromises()
  expect(backend.reviewStatus).toHaveBeenCalledTimes(3)
  expect(wrapper.get('ul[aria-live="polite"]').text()).toBe('Testing the change.')
  expect(wrapper.get('.alert-success').text()).toBe('Kept. Your assistant now works this way. Every test passed.')
})

it('loads once and works when the host omits active', async () => {
  const backend = fakeBackend({ reviewStatus: vi.fn().mockResolvedValue(status) })
  const wrapper = screen(Review, backend, { assistant }); await flushPromises()
  expect(wrapper.props('active')).toBe(true)
  expect(backend.reviewStatus).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(wrapper.getComponent(Refresh).props('status')).toEqual(status)
})
