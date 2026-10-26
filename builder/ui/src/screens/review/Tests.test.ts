import { expect, it } from 'vitest'
import { fakeBackend, screen } from '../../test-support'
import type { ReviewStatus } from '../../types/generated/ReviewStatus'
import Tests from './Tests.vue'
const status: ReviewStatus = { built: true, areas: ['Work'], tests: { waiting: 0, inUse: 0, retired: 0 }, byArea: [], change: null }
it('explains when there are no answers', () => {
  const wrapper = screen(Tests, fakeBackend(), { status })
  expect(wrapper.text()).toContain("You haven't given your assistant any answers to learn from yet.")
})
it('shows totals and every area count', () => {
  const wrapper = screen(Tests, fakeBackend(), { status: { ...status, tests: { waiting: 1000, inUse: 4, retired: 2 }, byArea: [{ area: 'Work', tests: { waiting: 999, inUse: 3, retired: 2 } }, { area: 'Craft', tests: { waiting: 1, inUse: 1, retired: 0 } }] } })
  expect(wrapper.get('p').text()).toBe('1,000 waiting for a refresh, 4 in use, 2 retired.')
  expect(wrapper.findAll('li').map(item => item.text())).toEqual(['Work: 999 waiting for a refresh, 3 in use, 2 retired.', 'Craft: 1 waiting for a refresh, 1 in use, 0 retired.'])
})

it('titles the answers collected so far', () => {
  const wrapper = screen(Tests, fakeBackend(), { status })
  expect(wrapper.get('h3').text()).toBe('Your answers so far')
})
