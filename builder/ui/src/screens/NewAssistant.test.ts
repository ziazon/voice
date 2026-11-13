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
  const backend = fakeBackend({ findExisting: vi.fn().mockResolvedValue({ found: true, lines, canBringIn: false, files: [] }) })
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
  const pending = deferred<{ found: boolean; lines: string[]; canBringIn: boolean; files: [] }>()
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
  pending.resolve({ found: false, lines: [], canBringIn: false, files: [] })
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
  const pending = deferred<{ assistant: typeof assistant; backup: string | null; files: string[] }>()
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: ['An older private folder.'], canBringIn: true, files: [] }),
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
  expect(backend.bringInAssistant).toHaveBeenCalledExactlyOnceWith({ assistant: { name: 'Ari', pronouns: 'they/them', principal: 'Robin', principalPronouns: 'they/them' }, files: [] })
  expect(button(wrapper, 'Backing it up…').attributes('disabled')).toBeDefined()
  expect(wrapper.findAll('button').some(button => button.text() === 'Making it…')).toBe(false)
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeDefined()
  pending.resolve({ assistant: brought, backup, files: [] })
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
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files: [] }),
    bringInAssistant: vi.fn().mockResolvedValue({ assistant: { ...assistant, name: 'Ari' }, backup: null, files: [] }),
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
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files: [] }),
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

it.each([false, true])('shows its own files only when bringing in is allowed (%s)', async canBringIn => {
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn, files: [{ name: 'IDENTITY.md', problem: null }] }),
  })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.find('fieldset').exists()).toBe(canBringIn)
})

it('ticks the files it can bring, explains files it leaves and sends the chosen names in shown order', async () => {
  const problem = "It's a link, so Voice leaves it."
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files: [
      { name: 'IDENTITY.md', problem: null },
      { name: 'autonomy.md', problem: null },
      { name: 'lens.md', problem },
      { name: 'briefs', problem: null },
    ] }),
    bringInAssistant: vi.fn().mockResolvedValue({ assistant: { ...assistant, name: 'Ari' }, backup: 'a backup folder', files: ['IDENTITY.md', 'briefs'] }),
  })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  const fieldset = wrapper.get('fieldset')
  expect(fieldset.get('legend.form-label').text()).toBe('Its own files')
  expect(fieldset.get('.form-hint').text()).toBe('Voice keeps its own SKILL.md and member.json. Untick a file to leave it out.')
  const checkboxes = fieldset.findAll('input[type="checkbox"]')
  expect(checkboxes).toHaveLength(4)
  for (const [index, checkbox] of checkboxes.entries()) {
    expect(checkbox.attributes('id')).toBe(`old-file-${index}`)
    expect(checkbox.classes()).toContain('form-check-input')
    expect((checkbox.element as HTMLInputElement).checked).toBe(index !== 2)
    expect((checkbox.element as HTMLInputElement).disabled).toBe(index === 2)
    const label = fieldset.get(`label.form-check-label[for="old-file-${index}"]`)
    expect(label.get('code').text()).toBe(['IDENTITY.md', 'autonomy.md', 'lens.md', 'briefs'][index])
  }
  expect(fieldset.get('label[for="old-file-2"] .text-secondary').text()).toBe(problem)
  await checkboxes[1]!.setValue(false)
  // Toggling a file back on must keep the original shown order.
  await checkboxes[0]!.setValue(false)
  await checkboxes[0]!.setValue(true)
  await button(wrapper, 'Back it up and bring it in').trigger('click')
  await flushPromises()
  expect(backend.bringInAssistant).toHaveBeenCalledExactlyOnceWith({
    assistant: { name: 'Ari', pronouns: 'they/them', principal: 'Robin', principalPronouns: 'they/them' },
    files: ['IDENTITY.md', 'briefs'],
  })
  expect(wrapper.get('.alert-success p').text()).toBe("Voice brought in the older Ari's history and its own files: IDENTITY.md, briefs. A copy of everything it found is in a backup folder.")
})

it('shows no file choices when the older assistant has none', async () => {
  const backend = fakeBackend({ findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files: [] }) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.find('fieldset').exists()).toBe(false)
})

it.each([null, "It changes more than the privacy settings, so Voice leaves it.", 'absent'])('explains private.json only when offered (%s)', async problem => {
  const files = problem === 'absent' ? [{ name: 'IDENTITY.md', problem: null }] : [{ name: 'private.json', problem }]
  const backend = fakeBackend({ findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files }) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Robin')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  const hints = wrapper.findAll('.form-hint').filter(hint => hint.text() === 'Its privacy settings: what the older assistant hid, and any words it let through.')
  expect(hints).toHaveLength(problem === 'absent' ? 0 : 1)
})

it('places opening below the form, locks make, bring-in and back during open, then forwards made', async () => {
  const pending = deferred<{ assistant: typeof assistant; backup: null; message: string }>()
  const saved = { owner: 'example-owner', repo: 'example-assistant', slug: 'wren', name: 'Wren', here: false }
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files: [] }),
    savedOnGithub: vi.fn().mockResolvedValue({ signedIn: true, items: [saved] }),
    openSaved: vi.fn().mockReturnValue(pending.promise),
  })
  const wrapper = screen(NewAssistant, backend)
  expect(wrapper.findAll('section.card').map(card => card.get('h2').text())).toEqual(['Make a new assistant', 'Open one you saved on GitHub'])
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Rowan Vale')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  await button(wrapper, 'Open Wren').trigger('click')
  for (const label of ['Make it', 'Back', 'Back it up and bring it in']) expect(button(wrapper, label).attributes('disabled')).toBeDefined()
  await wrapper.get('form').trigger('submit')
  expect(backend.findExisting).toHaveBeenCalledTimes(1)
  expect(backend.newAssistant).not.toHaveBeenCalled()
  await button(wrapper, 'Back it up and bring it in').trigger('click')
  expect(backend.bringInAssistant).not.toHaveBeenCalled()
  await button(wrapper, 'Back').trigger('click')
  expect(wrapper.emitted('cancel')).toBeUndefined()
  pending.resolve({ assistant, backup: null, message: 'Opened the saved assistant.' })
  await flushPromises()
  expect(button(wrapper, 'Make it').attributes('disabled')).toBeUndefined()
  expect(button(wrapper, 'Back').attributes('disabled')).toBeUndefined()
  expect(button(wrapper, 'Back it up and bring it in').attributes('disabled')).toBeUndefined()
  await button(wrapper, 'Next: give permission').trigger('click')
  expect(wrapper.emitted('made')).toEqual([[assistant]])
})

it('disables the saved card while making an assistant', async () => {
  const pending = deferred<typeof assistant>()
  const backend = fakeBackend({ newAssistant: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Rowan Vale')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(button(wrapper, 'Look on GitHub').attributes('disabled')).toBeDefined()
  pending.resolve(assistant)
  await flushPromises()
  expect(button(wrapper, 'Look on GitHub').attributes('disabled')).toBeUndefined()
})

it('guards the bring-in handler while a saved open runs', async () => {
  const pending = deferred<{ assistant: typeof assistant; backup: null; message: string }>()
  const backend = fakeBackend({
    findExisting: vi.fn().mockResolvedValue({ found: true, lines: [], canBringIn: true, files: [] }),
    savedOnGithub: vi.fn().mockResolvedValue({ signedIn: true, items: [{ owner: 'example-owner', repo: 'example-assistant', slug: 'wren', name: 'Wren', here: false }] }),
    openSaved: vi.fn().mockReturnValue(pending.promise),
  })
  const wrapper = screen(NewAssistant, backend)
  await wrapper.get('#assistant-name').setValue('Ari')
  await wrapper.get('#principal-name').setValue('Rowan Vale')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  await button(wrapper, 'Open Wren').trigger('click')
  const actions = wrapper.vm as unknown as { bringIn: () => Promise<void> }
  await actions.bringIn()
  expect(backend.bringInAssistant).not.toHaveBeenCalled()
  pending.resolve({ assistant, backup: null, message: 'Opened.' })
  await flushPromises()
})
