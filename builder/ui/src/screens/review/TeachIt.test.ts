import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../../test-support'
import type { ReviewStatus } from '../../types/generated/ReviewStatus'
import TeachIt from './TeachIt.vue'
const status: ReviewStatus = { built: true, areas: ['Work', 'Craft'], tests: { waiting: 0, inUse: 0, retired: 0 }, byArea: [], change: null }
it('requires both answers, keeps the exact request, clears text fields and keeps the chosen area', async () => {
  const kept = { ...status, tests: { waiting: 1, inUse: 0, retired: 0 } }
  const call = deferred<ReviewStatus>()
  const backend = fakeBackend({ keepAnswer: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(TeachIt, backend, { slug: 'example', status })
  expect(button(wrapper, 'Keep my answer').attributes('disabled')).toBeDefined()
  await wrapper.get('select').setValue('Craft')
  const fields = wrapper.findAll('textarea')
  expect(fields.every(field => field.attributes('rows') === '2' && field.attributes('maxlength') === '2000')).toBe(true)
  await fields[0]!.setValue('Which approach?')
  expect(button(wrapper, 'Keep my answer').attributes('disabled')).toBeDefined()
  await fields[1]!.setValue('Keep it simple')
  await fields[2]!.setValue('Add complexity')
  expect(button(wrapper, 'Keep my answer').attributes('disabled')).toBeUndefined()
  await wrapper.get('form').trigger('submit')
  expect(backend.keepAnswer).toHaveBeenCalledWith({ slug: 'example', area: 'Craft', situation: 'Which approach?', wanted: 'Keep it simple', assistantSaid: 'Add complexity' })
  expect(button(wrapper, 'Keep my answer').attributes('disabled')).toBeDefined()
  call.resolve(kept); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe("Kept. It's now one of the tests your assistant is checked on at its next refresh.")
  expect(fields.map(field => (field.element as HTMLTextAreaElement).value)).toEqual(['', '', ''])
  expect((wrapper.get('select').element as HTMLSelectElement).value).toBe('Craft')
  expect(wrapper.emitted('changed')).toEqual([[kept]])
})
it('retains fields on failure and uses the fixed fallback', async () => {
  const wrapper = screen(TeachIt, fakeBackend({ keepAnswer: vi.fn().mockRejectedValue(new Error('private detail')) }), { slug: 'example', status })
  await wrapper.findAll('textarea')[0]!.setValue('Situation')
  await wrapper.findAll('textarea')[1]!.setValue('Wanted')
  await wrapper.get('form').trigger('submit'); await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe("Persona Builder couldn't keep that answer. Check it and try again.")
  expect(wrapper.findAll('textarea').map(field => (field.element as HTMLTextAreaElement).value)).toEqual(['Situation', 'Wanted', ''])
  expect(wrapper.emitted('changed')).toBeUndefined()
})
it('has only the settings line when there are no areas', () => {
  const wrapper = screen(TeachIt, fakeBackend(), { slug: 'example', status: { ...status, areas: [] } })
  expect(wrapper.text()).toContain("Your assistant's settings name no kinds of decision yet.")
  expect(wrapper.find('form').exists()).toBe(false)
})

it('labels the answer and explains privacy immediately before keeping it', () => {
  const wrapper = screen(TeachIt, fakeBackend(), { slug: 'example', status })
  expect(wrapper.get('label[for="example-review-wanted"]').text()).toBe("What you'd have done")
  const keep = button(wrapper, 'Keep my answer')
  expect(keep.element.previousElementSibling?.textContent).toBe("Leave out anything private, such as someone's name. It's checked before it's kept.")
  expect(wrapper.get('.form-hint').text()).toContain("Leave out anything private, such as someone's name.")
  expect(wrapper.get('.form-hint').text()).toContain("It's checked before it's kept.")
})
