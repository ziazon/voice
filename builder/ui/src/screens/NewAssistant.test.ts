import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import NewAssistant from './NewAssistant.vue'

it('requires both names and both custom pronouns, trims the request and announces saving', async () => {
  const pending = deferred<typeof assistant>()
  const backend = fakeBackend({ newAssistant: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(NewAssistant, backend)
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeDefined()
  await wrapper.get('#assistant-name').setValue(' Example ')
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeDefined()
  await wrapper.get('#principal-name').setValue(' Sample ')
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeUndefined()
  await wrapper.get('#assistant-pronouns').setValue('other')
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeDefined()
  await wrapper.get('#assistant-pronouns-other').setValue(' xe/xem ')
  await wrapper.get('#principal-pronouns').setValue('other')
  await wrapper.get('#principal-pronouns-other').setValue(' ze/zir ')
  await wrapper.get('form').trigger('submit')
  expect(backend.newAssistant).toHaveBeenCalledExactlyOnceWith({ name: 'Example', pronouns: 'xe/xem', principal: 'Sample', principalPronouns: 'ze/zir' })
  expect(button(wrapper, 'Making it…').attributes('disabled')).toBeDefined()
  pending.resolve(assistant)
  await flushPromises()
  expect(wrapper.emitted('made')).toEqual([[assistant]])
})
it.each([
  [{ message: 'Pick another name.' }, 'Pick another name.'],
  [new Error('private detail'), "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again."],
])('shows a safe failure and keeps typed values', async (error, message) => {
  const backend = fakeBackend({ newAssistant: vi.fn().mockRejectedValue(error) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Example')
  await wrapper.get('#principal-name').setValue('Sample')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe(message)
  expect((wrapper.get('#assistant-name').element as HTMLInputElement).value).toBe('Example')
  expect((wrapper.get('#principal-name').element as HTMLInputElement).value).toBe('Sample')
  expect(backend.newAssistant).toHaveBeenCalledWith({ name: 'Example', pronouns: 'they/them', principal: 'Sample', principalPronouns: 'they/them' })
  await button(wrapper, 'Back').trigger('click')
  expect(wrapper.emitted('cancel')).toHaveLength(1)
})
