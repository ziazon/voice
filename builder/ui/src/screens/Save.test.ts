import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { SaveOptions } from '../types/generated/SaveOptions'
import type { Said } from '../types/generated/Said'
import Save from './Save.vue'

const options: SaveOptions = {
  choices: [{ kind: 'github', label: 'A new private GitHub repo' }, { kind: 'local', label: 'This computer only' }],
  selected: 0, note: 'A free GitHub account lets you keep a private copy.', chosen: null,
  signedIn: false, suggestedName: 'example-assistant',
}

it('announces loading, then shows choices, recommendation, note and fixed signup action', async () => {
  const pending = deferred<SaveOptions>()
  const backend = fakeBackend({ saveOptions: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(Save, backend, { assistant })
  expect(wrapper.get('[role="status"]').text()).toBe('Looking for places to keep Example…')
  expect(wrapper.text()).toContain("Choose where Example is kept. A private GitHub repo is a backup only you can see; or keep it on this computer only.")
  pending.resolve(options)
  await flushPromises()
  expect(backend.saveOptions).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  const choices = wrapper.findAll('input[type="radio"]')
  expect(choices.map(choice => (choice.element as HTMLInputElement).checked)).toEqual([true, false])
  expect(wrapper.findAll('.form-check-label').map(label => label.text())).toEqual(['A new private GitHub repo (recommended)', 'This computer only'])
  expect(wrapper.text()).toContain(options.note)
  expect(wrapper.find('a').exists()).toBe(false)
  await button(wrapper, 'Make a free GitHub account').trigger('click')
  expect(backend.openLink).toHaveBeenCalledExactlyOnceWith({ link: 'githubSignup' })
})
it.each([
  [{ message: 'Could not check the places.' }, 'Could not check the places.'],
  [new Error('private detail'), "Persona Builder couldn't check where your assistant can be kept. Close it and open it again."],
])('shows a safe options failure and retries', async (error, message) => {
  const backend = fakeBackend({ saveOptions: vi.fn().mockRejectedValueOnce(error).mockResolvedValue(options) })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
  await button(wrapper, 'Try again').trigger('click')
  await flushPromises()
  expect(wrapper.findAll('input[type="radio"]')).toHaveLength(2)
})
it.each([
  [{ message: 'Open the signup page yourself.' }, 'Open the signup page yourself.'],
  [new Error('private detail'), "Persona Builder couldn't open your web browser. Go to https://github.com/signup yourself."],
])('shows a safe signup failure', async (error, message) => {
  const wrapper = screen(Save, fakeBackend({ saveOptions: vi.fn().mockResolvedValue(options), openLink: vi.fn().mockRejectedValue(error) }), { assistant })
  await flushPromises()
  await button(wrapper, 'Make a free GitHub account').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.find('a').exists()).toBe(false)
})
it.each(['github', 'existing'] as const)('requires sign-in for %s, but lets a local choice be kept', async kind => {
  const backend = fakeBackend({ saveOptions: vi.fn().mockResolvedValue({ ...options, choices: [{ kind, label: 'GitHub repo' }, options.choices[1]!] }) })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).toContain('To save to GitHub, sign in to your GitHub account first.')
  expect(wrapper.find('input[type="text"]').exists()).toBe(kind === 'github')
  if (kind === 'github') expect((wrapper.get('input[type="text"]').element as HTMLInputElement).value).toBe('example-assistant')
  await wrapper.findAll('input[type="radio"]')[1]!.setValue()
  expect(wrapper.find('input[type="text"]').exists()).toBe(false)
  expect(wrapper.text()).not.toContain('Sign in to GitHub')
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeUndefined()
  await button(wrapper, 'Keep this choice').trigger('click')
  expect(backend.saveChoose).toHaveBeenCalledExactlyOnceWith({ slug: 'example', choice: 1, kind: 'local', name: null })
})
it('keeps the local choice and typed repo name after reloading options', async () => {
  const backend = fakeBackend({
    saveOptions: vi.fn().mockResolvedValue(options),
  })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  await wrapper.get('input[type="text"]').setValue(' example-copy ')
  await wrapper.findAll('input[type="radio"]')[1]!.setValue()
  await button(wrapper, 'Keep this choice').trigger('click')
  await flushPromises()
  expect(backend.saveOptions).toHaveBeenCalledTimes(2)
  expect(wrapper.findAll('input[type="radio"]').map(choice => (choice.element as HTMLInputElement).checked)).toEqual([false, true])
  await wrapper.findAll('input[type="radio"]')[0]!.setValue()
  expect((wrapper.get('input[type="text"]').element as HTMLInputElement).value).toBe(' example-copy ')
})
it('falls back to the recommended choice when a reload removes the picked index', async () => {
  const backend = fakeBackend({
    saveOptions: vi.fn().mockResolvedValueOnce(options).mockResolvedValue({ ...options, choices: [options.choices[0]!] }),
  })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  await wrapper.get('input[type="text"]').setValue(' example-copy ')
  await wrapper.findAll('input[type="radio"]')[1]!.setValue()
  await button(wrapper, 'Keep this choice').trigger('click')
  await flushPromises()
  expect(wrapper.findAll('input[type="radio"]').map(choice => (choice.element as HTMLInputElement).checked)).toEqual([true])
  expect((wrapper.get('input[type="text"]').element as HTMLInputElement).value).toBe(' example-copy ')
})
it('trims the github name, announces keeping, shows the answer and reloads the saved choice', async () => {
  const pending = deferred<Said>()
  const backend = fakeBackend({
    saveOptions: vi.fn().mockResolvedValueOnce({ ...options, signedIn: true }).mockResolvedValue({ ...options, signedIn: true, chosen: 'A private GitHub repo.' }),
    saveChoose: vi.fn().mockReturnValue(pending.promise),
  })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  expect(wrapper.findAll('button').some(item => item.text() === 'Save Example')).toBe(false)
  await wrapper.get('input[type="text"]').setValue(' example-copy ')
  await button(wrapper, 'Keep this choice').trigger('click')
  expect(backend.saveChoose).toHaveBeenCalledExactlyOnceWith({ slug: 'example', choice: 0, kind: 'github', name: 'example-copy' })
  expect(button(wrapper, 'Keeping it…').attributes('disabled')).toBeDefined()
  pending.resolve({ message: 'Your choice is kept.' })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Your choice is kept.')
  expect(wrapper.text()).toContain('Right now: A private GitHub repo.')
  expect(backend.saveOptions).toHaveBeenCalledTimes(2)
  expect(button(wrapper, 'Save Example').exists()).toBe(true)
})
it.each(['', '   '])('disables keeping a GitHub choice with an empty trimmed name (%j)', async name => {
  const backend = fakeBackend({ saveOptions: vi.fn().mockResolvedValue({ ...options, signedIn: true }) })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeUndefined()
  await wrapper.get('input[type="text"]').setValue(name)
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeDefined()
  await button(wrapper, 'Keep this choice').trigger('click')
  expect(backend.saveChoose).not.toHaveBeenCalled()
  await wrapper.findAll('input[type="radio"]')[1]!.setValue()
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeUndefined()
  await wrapper.findAll('input[type="radio"]')[0]!.setValue()
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeDefined()
  await wrapper.get('input[type="text"]').setValue(' example-copy ')
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeUndefined()
})
it.each([
  [{ message: 'Pick a different repo name.' }, 'Pick a different repo name.'],
  [new Error('private detail'), "Persona Builder couldn't keep your choice. Close it and open it again, then try again."],
])('shows a safe choice failure', async (error, message) => {
  const wrapper = screen(Save, fakeBackend({ saveOptions: vi.fn().mockResolvedValue({ ...options, signedIn: true }), saveChoose: vi.fn().mockRejectedValue(error) }), { assistant })
  await flushPromises()
  await button(wrapper, 'Keep this choice').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeUndefined()
})
it('uses the backend selected index and marks only that recommendation', async () => {
  const wrapper = screen(Save, fakeBackend({ saveOptions: vi.fn().mockResolvedValue({ ...options, selected: 1, note: null }) }), { assistant })
  await flushPromises()
  expect(wrapper.findAll('input[type="radio"]').map(choice => (choice.element as HTMLInputElement).checked)).toEqual([false, true])
  expect(wrapper.findAll('.form-check-label').map(label => label.text())).toEqual(['A new private GitHub repo', 'This computer only (recommended)'])
  expect(wrapper.text()).not.toContain('Make a free GitHub account')
})
it('announces saving and shows the save result', async () => {
  const pending = deferred<Said>()
  const backend = fakeBackend({ saveOptions: vi.fn().mockResolvedValue({ ...options, chosen: 'This computer only.' }), savePush: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  await button(wrapper, 'Save Example').trigger('click')
  expect(backend.savePush).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(button(wrapper, 'Saving…').attributes('disabled')).toBeDefined()
  pending.resolve({ message: 'Your copy is saved.' })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Your copy is saved.')
})
it.each([
  [{ message: 'The privacy check found a private line.' }, 'The privacy check found a private line.'],
  [new Error('private detail'), "Persona Builder couldn't save your assistant. Try again, or keep it on this computer only."],
])('shows the privacy Failure or safe save fallback', async (error, message) => {
  const wrapper = screen(Save, fakeBackend({ saveOptions: vi.fn().mockResolvedValue({ ...options, chosen: 'This computer only.' }), savePush: vi.fn().mockRejectedValue(error) }), { assistant })
  await flushPromises()
  await button(wrapper, 'Save Example').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.text()).not.toContain('private detail')
})
it('shows sign-out message, reloads options and requires sign-in again', async () => {
  const backend = fakeBackend({ saveOptions: vi.fn().mockResolvedValueOnce({ ...options, signedIn: true }).mockResolvedValue(options), githubSignOut: vi.fn().mockResolvedValue({ message: "You're signed out." }) })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  expect(wrapper.text()).toContain("You're signed in to GitHub on this computer.")
  await button(wrapper, 'Sign out of GitHub').trigger('click')
  await flushPromises()
  expect(backend.githubSignOut).toHaveBeenCalledExactlyOnceWith()
  expect(wrapper.get('.alert-info').text()).toBe("You're signed out.")
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).not.toContain("You're signed in to GitHub on this computer.")
})
it('reloads options and shows the sign-in success after approval', async () => {
  const finish = deferred<Said>()
  const backend = fakeBackend({ saveOptions: vi.fn().mockResolvedValueOnce(options).mockResolvedValue({ ...options, signedIn: true }), githubSignInFinish: vi.fn().mockReturnValue(finish.promise) })
  const wrapper = screen(Save, backend, { assistant })
  await flushPromises()
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  finish.resolve({ message: "You're signed in." })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe("You're signed in.")
  expect(wrapper.text()).not.toContain('To save to GitHub')
  expect(button(wrapper, 'Keep this choice').attributes('disabled')).toBeUndefined()
  expect(backend.saveOptions).toHaveBeenCalledTimes(2)
})

it('names the assistant and links the repo name to its rules', async () => {
  const wrapper = screen(Save, fakeBackend({ saveOptions: vi.fn().mockResolvedValue(options) }), { assistant })
  expect(wrapper.get('h2').text()).toBe(`Keep ${assistant.name} safe`)
  expect(wrapper.text()).toContain(`Choose where ${assistant.name} is kept.`)
  expect(wrapper.text()).toContain('A private GitHub repo is a backup only you can see; or keep it on this computer only.')
  await flushPromises()
  expect(wrapper.get('#save-repo-name').attributes('aria-describedby')).toBe('save-repo-name-help')
  expect(wrapper.get('#save-repo-name-help').text()).toBe('Letters, numbers, dots, dashes and underscores, up to 100 characters.')
})
