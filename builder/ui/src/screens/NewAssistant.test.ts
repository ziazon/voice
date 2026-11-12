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
  const backend = fakeBackend({ findExisting: vi.fn().mockResolvedValue({ found: true, lines, canBringIn: false }) })
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
  expect(warning.text()).toContain("Voice made nothing and changed nothing. Voice can't bring this one in, so pick another name.")
  expect(wrapper.find('button.btn-warning').exists()).toBe(false)
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

it('backs up the checked request, announces progress and opens the brought-in assistant', async () => {
  const brought = { ...assistant, slug: 'ari', name: 'Ari' }
  const backup = '~/.claude/voice-backups/ari/2026-11-12-093000'
  const pending = deferred<{ assistant: typeof assistant; backup: string | null }>()
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: ['An older private folder.'], canBringIn: true }),
    bringInAssistant: vi.fn().mockReturnValue(pending.promise),
  })
  const wrapper = screen(NewAssistant, backend)
  expect(wrapper.find('button.btn-warning').exists()).toBe(false)
  await wrapper.get('#assistant-name').setValue(' Ari ')
  await wrapper.get('#principal-name').setValue(' Robin ')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toContain("Voice made nothing and changed nothing. It can copy everything listed here into a backup folder first, then bring this Ari's history in, or you can pick another name.")
  // Changes to other fields must not change the request whose name was checked.
  await wrapper.get('#principal-name').setValue('Mo Lee')
  await button(wrapper, 'Back it up and bring it in').trigger('click')
  expect(backend.bringInAssistant).toHaveBeenCalledExactlyOnceWith({ name: 'Ari', pronouns: 'they/them', principal: 'Robin', principalPronouns: 'they/them' })
  expect(button(wrapper, 'Backing it up…').attributes('disabled')).toBeDefined()
  pending.resolve({ assistant: brought, backup })
  await flushPromises()
  const success = wrapper.get('.alert-success[role="status"]')
  expect(success.get('h3').text()).toBe('Ari is ready')
  expect(success.text()).toContain("Voice brought in the older Ari's history. A copy of everything it found is in " + backup + '.')
  expect(wrapper.find('.btn-list').exists()).toBe(false)
  expect(wrapper.emitted('made')).toBeUndefined()
  await button(wrapper, 'Open Ari').trigger('click')
  expect(wrapper.emitted('made')).toEqual([[brought]])
})

it('leaves the backup sentence out when bring-in returns no backup', async () => {
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true }),
    bringInAssistant: vi.fn().mockResolvedValue({ assistant: { ...assistant, name: 'Ari' }, backup: null }),
  })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  await button(wrapper, 'Back it up and bring it in').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toContain("Voice brought in the older Ari's history.")
  expect(wrapper.get('.alert-success').text()).not.toContain('A copy of everything')
})

it.each([
  [new Error('private detail'), "Voice couldn't back up the older assistant, so it changed nothing. Check there's space on this computer, then try again."],
  [{ message: 'Pick another name.' }, 'Pick another name.'],
])('shows a safe bring-in failure and keeps the warning', async (failure, message) => {
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true }),
    bringInAssistant: vi.fn().mockRejectedValue(failure),
  })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  await button(wrapper, 'Back it up and bring it in').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-danger[role="alert"]').text()).toBe(message)
  expect(button(wrapper, 'Back it up and bring it in').attributes('disabled')).toBeUndefined()
  expect(wrapper.emitted('made')).toBeUndefined()
  await wrapper.get('#assistant-name').setValue('Mo Lee')
  expect(wrapper.find('.alert-warning').exists()).toBe(false)
  expect(wrapper.find('button.btn-warning').exists()).toBe(false)
})
