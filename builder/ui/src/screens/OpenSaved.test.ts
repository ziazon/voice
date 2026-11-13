import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { SavedOnGithub } from '../types/generated/SavedOnGithub'
import type { OpenedSaved } from '../types/generated/OpenedSaved'
import type { Said } from '../types/generated/Said'
import OpenSaved from './OpenSaved.vue'

const wren = { owner: 'example-owner', repo: 'example-assistant', slug: 'wren', name: 'Wren', here: false }
const ari = { ...wren, slug: 'ari', name: 'Ari', here: true }
const saved = { signedIn: true, items: [wren, ari] }
const opened = { assistant: { ...assistant, slug: 'wren', name: 'Wren' }, backup: '~/a-backup/', message: 'Opened Wren from example-owner/example-assistant. Give permission again on this computer. A backup is in ~/a-backup/.' }

it('fetches only on request, announces looking and labels the next look', async () => {
  const pending = deferred<SavedOnGithub>()
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  expect(backend.savedOnGithub).not.toHaveBeenCalled()
  expect(backend.openSaved).not.toHaveBeenCalled()
  await button(wrapper, 'Look on GitHub').trigger('click')
  expect(backend.savedOnGithub).toHaveBeenCalledExactlyOnceWith()
  expect(wrapper.get('.spinner-border[role="status"]').text()).toBe('Looking in your private GitHub repos…')
  expect(button(wrapper, 'Looking…').attributes('disabled')).toBeDefined()
  await button(wrapper, 'Looking…').trigger('click')
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(1)
  pending.resolve(saved)
  await flushPromises()
  expect(wrapper.find('.spinner-border').exists()).toBe(false)
  expect(button(wrapper, 'Look again').attributes('disabled')).toBeUndefined()
})

it('re-lists immediately after sign-in finishes', async () => {
  const finish = deferred<Said>()
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockResolvedValueOnce({ signedIn: false, items: [] }).mockResolvedValue(saved), githubSignInFinish: vi.fn().mockReturnValue(finish.promise) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain('To open an assistant you saved, sign in to your GitHub account first.')
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(1)
  finish.resolve({ message: 'Signed in.' })
  await flushPromises()
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(2)
  expect(button(wrapper, 'Open Wren').exists()).toBe(true)
})

it('shows the empty sentence or saved names, repo and already-here guard', async () => {
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockResolvedValueOnce({ signedIn: true, items: [] }).mockResolvedValue(saved) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  expect(wrapper.get('p.text-secondary').text()).toBe('Voice found no saved assistants in your private GitHub repos.')
  await button(wrapper, 'Look again').trigger('click')
  await flushPromises()
  const items = wrapper.findAll('ul.list-unstyled li')
  expect(items.map(item => item.get('strong').text())).toEqual(['Wren', 'Ari'])
  expect(items[0]!.get('span').text()).toBe('example-owner/example-assistant')
  expect(button(wrapper, 'Open Wren').exists()).toBe(true)
  expect(items[1]!.find('button').exists()).toBe(false)
  expect(items[1]!.text()).toContain('Already on this computer')
  expect(wrapper.find('p.text-secondary').exists()).toBe(false)
})

it('opens exactly the chosen repo and slug, locks the card and waits for permission navigation', async () => {
  const pending = deferred<OpenedSaved>()
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockResolvedValue({ signedIn: true, items: [wren, { ...ari, here: false }] }), openSaved: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  await button(wrapper, 'Open Wren').trigger('click')
  expect(backend.openSaved).toHaveBeenCalledExactlyOnceWith({ owner: wren.owner, repo: wren.repo, slug: wren.slug })
  expect(button(wrapper, 'Opening…').exists()).toBe(true)
  expect(button(wrapper, 'Open Ari').exists()).toBe(true)
  for (const item of wrapper.findAll('button')) expect(item.attributes('disabled')).toBeDefined()
  await button(wrapper, 'Open Ari').trigger('click')
  await button(wrapper, 'Look again').trigger('click')
  expect(backend.openSaved).toHaveBeenCalledTimes(1)
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(1)
  expect(wrapper.emitted('running')).toEqual([[true]])
  pending.resolve(opened)
  await flushPromises()
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  expect(wrapper.find('ul').exists()).toBe(false)
  const success = wrapper.get('.alert-success[role="status"]')
  expect(success.get('h3.alert-title').text()).toBe('Wren is ready')
  expect(success.get('p').text()).toBe(opened.message)
  expect(wrapper.text().split(opened.backup)).toHaveLength(2)
  expect(wrapper.emitted('made')).toBeUndefined()
  await button(wrapper, 'Next: give permission').trigger('click')
  expect(wrapper.emitted('made')).toEqual([[opened.assistant]])
})

it.each([
  [{ message: 'Please try opening again.' }, 'Please try opening again.'],
  [new Error('private detail'), "Voice couldn't open that saved assistant. Try again."],
])('keeps the list on a safe open failure and allows retry', async (failure, message) => {
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockResolvedValue(saved), openSaved: vi.fn().mockRejectedValueOnce(failure).mockResolvedValue(opened) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  await button(wrapper, 'Open Wren').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning[role="alert"]').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  expect(button(wrapper, 'Open Wren').attributes('disabled')).toBeUndefined()
  await button(wrapper, 'Open Wren').trigger('click')
  await flushPromises()
  expect(wrapper.find('.alert-warning').exists()).toBe(false)
  expect(wrapper.get('.alert-success p').text()).toBe(opened.message)
})

it.each([
  [{ message: 'Please look again.' }, 'Please look again.'],
  [new Error('private detail'), "Voice couldn't look in your GitHub repos. Try again."],
])('shows a safe list failure and lets the person retry', async (failure, message) => {
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockRejectedValueOnce(failure).mockResolvedValue(saved) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning[role="alert"]').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  expect(wrapper.find('.alert-warning').exists()).toBe(false)
  expect(button(wrapper, 'Open Wren').exists()).toBe(true)
})

it('disables looking and every open button while the form is busy', async () => {
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockResolvedValue(saved) })
  const wrapper = screen(OpenSaved, backend, { disabled: true })
  expect(button(wrapper, 'Look on GitHub').attributes('disabled')).toBeDefined()
  await button(wrapper, 'Look on GitHub').trigger('click')
  expect(backend.savedOnGithub).not.toHaveBeenCalled()
  await wrapper.setProps({ disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  await wrapper.setProps({ disabled: true })
  for (const item of wrapper.findAll('button')) expect(item.attributes('disabled')).toBeDefined()
  await button(wrapper, 'Open Wren').trigger('click')
  expect(backend.openSaved).not.toHaveBeenCalled()
})

it('guards direct opens for already-here, disabled, looking and another open, and guards looking during open', async () => {
  const listing = deferred<SavedOnGithub>()
  const opening = deferred<OpenedSaved>()
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockReturnValue(listing.promise), openSaved: vi.fn().mockReturnValue(opening.promise) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  const actions = wrapper.vm as unknown as { open: (item: typeof wren) => Promise<void>; look: () => Promise<void> }
  void actions.open(ari)
  expect(backend.openSaved).not.toHaveBeenCalled()
  await wrapper.setProps({ disabled: true })
  void actions.open(wren)
  void actions.look()
  expect(backend.openSaved).not.toHaveBeenCalled()
  expect(backend.savedOnGithub).not.toHaveBeenCalled()
  await wrapper.setProps({ disabled: false })
  const look = actions.look()
  void actions.look()
  void actions.open(wren)
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(1)
  expect(backend.openSaved).not.toHaveBeenCalled()
  listing.resolve(saved)
  await look
  const open = actions.open(wren)
  void actions.open({ ...ari, here: false })
  void actions.look()
  expect(backend.openSaved).toHaveBeenCalledTimes(1)
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(1)
  opening.resolve(opened)
  await open
})

it('also disables sign-in when the form becomes busy', async () => {
  const backend = fakeBackend()
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click')
  await flushPromises()
  await wrapper.setProps({ disabled: true })
  expect(wrapper.get('fieldset[disabled]').get('button').text()).toBe('Sign in to GitHub')
  await wrapper.setProps({ disabled: false })
  expect(wrapper.find('fieldset[disabled]').exists()).toBe(false)
})

it.each([false, true])('finishes sign-in even if disabled becomes %s and never repeats sign-in on failure', async disabled => {
  const finish = deferred<Said>()
  const backend = fakeBackend({ savedOnGithub: vi.fn().mockResolvedValueOnce({ signedIn: false, items: [] }).mockRejectedValue({ message: 'Please look again.' }), githubSignInFinish: vi.fn().mockReturnValue(finish.promise) })
  const wrapper = screen(OpenSaved, backend, { disabled: false })
  await button(wrapper, 'Look on GitHub').trigger('click'); await flushPromises()
  await button(wrapper, 'Sign in to GitHub').trigger('click'); await flushPromises()
  await wrapper.setProps({ disabled })
  finish.resolve({ message: 'Signed in.' }); await flushPromises()
  expect(backend.savedOnGithub).toHaveBeenCalledTimes(2)
  expect(wrapper.get('.alert-warning').text()).toBe('Please look again.')
  expect(button(wrapper, 'Look on GitHub').exists()).toBe(true)
  expect(wrapper.findAll('button').some(item => item.text() === 'Sign in to GitHub')).toBe(false)
})
