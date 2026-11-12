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
  await flushPromises()
  expect(backend.newAssistant).toHaveBeenCalledExactlyOnceWith({ name: 'Example', pronouns: 'xe/xem', principal: 'Sample', principalPronouns: 'ze/zir' })
  expect(button(wrapper, 'Making it…').attributes('disabled')).toBeDefined()
  pending.resolve(assistant)
  await flushPromises()
  expect(wrapper.emitted('made')).toEqual([[assistant]])
})
it.each([
  [{ message: 'Pick another name.' }, 'Pick another name.'],
  [new Error('private detail'), "Voice couldn't make the assistant's folder. Check there's space on this computer, then try again."],
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

it('labels both the standard and custom pronouns clearly', async () => {
  const wrapper = screen(NewAssistant)
  expect(wrapper.get('label[for="assistant-pronouns"]').text()).toBe("Your assistant's pronouns")
  expect(wrapper.get('label[for="principal-pronouns"]').text()).toBe('How your assistant refers to you')
  await wrapper.get('#assistant-pronouns').setValue('other')
  await wrapper.get('#principal-pronouns').setValue('other')
  expect(wrapper.get('label[for="assistant-pronouns-other"]').text()).toBe("Your assistant's pronouns")
  expect(wrapper.get('label[for="principal-pronouns-other"]').text()).toBe('How your assistant refers to you')
})

it('warns about every older-assistant finding and makes nothing', async () => {
  const lines = [
    'A private folder from before, ~/.claude/state/ari/, holds 2 files.',
    'One of them is a put-together history that Voice would replace.',
    'One of them is a log of answers you recorded.',
    "Claude Code has a skill called ari that Voice didn't add.",
    "Your CLAUDE.md mentions /ari on 2 lines that Voice didn't add.",
    'That skill points to the folder the assistant was made in, with its own files.',
  ]
  const backend = fakeBackend({ findExisting: vi.fn().mockResolvedValue({ found: true, lines }) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue(' Ari ')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(backend.findExisting).toHaveBeenCalledExactlyOnceWith({ name: 'Ari' })
  const warning = wrapper.get('[role="status"]')
  expect(warning.classes()).toEqual(expect.arrayContaining(['alert', 'alert-warning']))
  expect(warning.get('h3').text()).toBe('Voice found an older Ari')
  expect(warning.findAll('ul li').map(line => line.text())).toEqual(lines)
  expect(warning.text()).toContain("Voice made nothing and changed nothing. It can't bring an older assistant in yet, so pick another name for now.")
  expect(backend.newAssistant).not.toHaveBeenCalled()
  expect(wrapper.emitted('made')).toBeUndefined()
  await wrapper.get('#assistant-name').setValue('Mo Lee')
  expect(wrapper.find('[role="status"]').exists()).toBe(false)
})

it('checks first and then makes the same trimmed request when nothing is found', async () => {
  const pending = deferred<{ found: boolean; lines: string[] }>()
  const backend = fakeBackend({ findExisting: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue(' Ari ')
  await wrapper.get('#principal-name').setValue(' Robin ')
  await wrapper.get('form').trigger('submit')
  expect(button(wrapper, 'Checking…').attributes('disabled')).toBeDefined()
  expect(backend.findExisting).toHaveBeenCalledExactlyOnceWith({ name: 'Ari' })
  expect(backend.newAssistant).not.toHaveBeenCalled()
  await wrapper.get('form').trigger('submit')
  expect(backend.findExisting).toHaveBeenCalledTimes(1)
  pending.resolve({ found: false, lines: [] })
  await flushPromises()
  expect(backend.newAssistant).toHaveBeenCalledExactlyOnceWith({ name: 'Ari', pronouns: 'they/them', principal: 'Robin', principalPronouns: 'they/them' })
  expect(wrapper.emitted('made')).toEqual([[assistant]])
})

it.each([
  [{ message: 'Pick another name.' }, 'Pick another name.'],
  [new Error('private detail'), "Voice couldn't look for an older assistant with that name. Close it and open it again."],
])('shows a safe lookup failure and makes nothing', async (failure, message) => {
  const backend = fakeBackend({ findExisting: vi.fn().mockRejectedValue(failure) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.get('[role="alert"]').classes()).toContain('alert-danger')
  expect(wrapper.get('[role="alert"]').text()).toBe(message)
  expect(backend.newAssistant).not.toHaveBeenCalled()
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeUndefined()
})
