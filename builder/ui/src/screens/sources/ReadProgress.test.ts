import { expect, it } from 'vitest'
import { screen } from '../../test-support'
import ReadProgress from './ReadProgress.vue'

it('announces 40 of 80 as 50 percent', () => {
  const wrapper = screen(ReadProgress, undefined, { progress: { done: 40, total: 80 }, label: 'Counting' })
  const bar = wrapper.get('[role="progressbar"]')
  expect(bar.attributes('aria-valuenow')).toBe('50')
  expect(bar.attributes('aria-valuemin')).toBe('0')
  expect(bar.attributes('aria-valuemax')).toBe('100')
  expect(bar.attributes('aria-label')).toBe('Counting')
  expect(bar.attributes('style')).toContain('50%')
})
it('shows an indeterminate bar before the first update', () => {
  const wrapper = screen(ReadProgress, undefined, { progress: null, label: 'Reading' })
  expect(wrapper.find('.progress-bar-indeterminate').exists()).toBe(true)
  expect(wrapper.get('[role="progressbar"]').attributes('aria-label')).toBe('Reading')
})
