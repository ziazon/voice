import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../test-support'
import type { Said } from '../types/generated/Said'
import type { SignInCode } from '../types/generated/SignInCode'
import GithubSignIn from './GithubSignIn.vue'

it('starts once, shows the code and fixed plain address, then finishes immediately', async () => {
  const start = deferred<SignInCode>()
  const finish = deferred<Said>()
  const backend = fakeBackend({ githubSignInStart: vi.fn().mockReturnValue(start.promise), githubSignInFinish: vi.fn().mockReturnValue(finish.promise) })
  const wrapper = screen(GithubSignIn, backend)
  expect(wrapper.text()).toContain('To save to GitHub, sign in to your GitHub account first.')
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  expect(button(wrapper, 'Sign in to GitHub').attributes('disabled')).toBeDefined()
  expect(backend.githubSignInFinish).not.toHaveBeenCalled()
  start.resolve({ userCode: 'ABCD-EFGH' })
  await flushPromises()
  expect(backend.githubSignInStart).toHaveBeenCalledExactlyOnceWith()
  expect(backend.githubSignInFinish).toHaveBeenCalledExactlyOnceWith()
  expect(wrapper.get('[data-code]').text()).toBe('ABCD-EFGH')
  expect(wrapper.text()).toContain('Go to github.com/login/device and enter this code:')
  expect(wrapper.get('[role="status"]').text()).toBe('Waiting for you to approve Persona Builder on GitHub…')
  expect(wrapper.find('a').exists()).toBe(false)
  await button(wrapper, "Open GitHub's page").trigger('click')
  expect(backend.openLink).toHaveBeenCalledExactlyOnceWith({ link: 'githubDevice' })
  finish.resolve({ message: 'Signed in as example.' })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Signed in as example.')
  expect(wrapper.emitted('signed-in')).toEqual([[{ message: 'Signed in as example.' }]])
  expect(wrapper.find('[data-code]').exists()).toBe(false)
})
it.each(['start', 'finish'] as const)('shows a Failure from %s and offers sign-in again', async stage => {
  const failure = { message: 'Please start signing in again.' }
  const backend = fakeBackend(stage === 'start'
    ? { githubSignInStart: vi.fn().mockRejectedValue(failure) }
    : { githubSignInFinish: vi.fn().mockRejectedValue(failure) })
  const wrapper = screen(GithubSignIn, backend)
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(failure.message)
  expect(button(wrapper, 'Sign in to GitHub').attributes('disabled')).toBeUndefined()
})
it.each(['start', 'finish'] as const)('hides a non-Failure rejection from %s', async stage => {
  const backend = fakeBackend(stage === 'start'
    ? { githubSignInStart: vi.fn().mockRejectedValue(new Error('private detail')) }
    : { githubSignInFinish: vi.fn().mockRejectedValue(new Error('private detail')) })
  const wrapper = screen(GithubSignIn, backend)
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe("GitHub sign-in didn't work. Try again later, or keep your assistant on this computer only.")
  expect(wrapper.text()).not.toContain('private detail')
})
it.each([
  [{ message: 'Open the device page yourself.' }, 'Open the device page yourself.'],
  [new Error('private detail'), "Persona Builder couldn't open your web browser. Go to https://github.com/login/device yourself."],
])('shows a safe device-page failure', async (error, message) => {
  const wrapper = screen(GithubSignIn, fakeBackend({ githubSignInFinish: vi.fn().mockReturnValue(deferred<Said>().promise), openLink: vi.fn().mockRejectedValue(error) }))
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  await button(wrapper, "Open GitHub's page").trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.find('a').exists()).toBe(false)
})
it('cancels, returns to idle and suppresses the old finish rejection even after a new start', async () => {
  const oldFinish = deferred<Said>()
  const newFinish = deferred<Said>()
  const backend = fakeBackend({ githubSignInFinish: vi.fn().mockReturnValueOnce(oldFinish.promise).mockReturnValueOnce(newFinish.promise) })
  const wrapper = screen(GithubSignIn, backend)
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  await button(wrapper, 'Cancel').trigger('click')
  await flushPromises()
  expect(backend.githubSignInCancel).toHaveBeenCalledExactlyOnceWith()
  expect(wrapper.find('[data-code]').exists()).toBe(false)
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  oldFinish.reject({ message: 'The old sign-in was cancelled.' })
  await flushPromises()
  expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  expect(wrapper.find('[data-code]').exists()).toBe(true)
  newFinish.resolve({ message: 'The new sign-in worked.' })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('The new sign-in worked.')
})
it('cancels on unmount while finishing and ignores its late resolution', async () => {
  const finish = deferred<Said>()
  const backend = fakeBackend({ githubSignInFinish: vi.fn().mockReturnValue(finish.promise) })
  const wrapper = screen(GithubSignIn, backend)
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  await flushPromises()
  wrapper.unmount()
  expect(backend.githubSignInCancel).toHaveBeenCalledExactlyOnceWith()
  finish.resolve({ message: 'Late success.' })
  await flushPromises()
  expect(wrapper.emitted('signed-in')).toBeUndefined()
})
it('cancels if unmounted while starting, and never begins finishing', async () => {
  const start = deferred<SignInCode>()
  const backend = fakeBackend({ githubSignInStart: vi.fn().mockReturnValue(start.promise) })
  const wrapper = screen(GithubSignIn, backend)
  await button(wrapper, 'Sign in to GitHub').trigger('click')
  wrapper.unmount()
  expect(backend.githubSignInCancel).toHaveBeenCalledExactlyOnceWith()
  start.resolve({ userCode: 'ABCD-EFGH' })
  await flushPromises()
  expect(backend.githubSignInFinish).not.toHaveBeenCalled()
})
