import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { backendKey, type Backend } from '../backend'
import Welcome from './Welcome.vue'
import { fakeBackend } from '../test-support'

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()) })
function welcome(backend: Backend) {
  const wrapper = mount(Welcome, { global: { provide: { [backendKey]: backend } } })
  wrappers.push(wrapper)
  return wrapper
}

describe('Welcome', () => {
  it('shows the app version and explains the build and privacy', async () => {
    const wrapper = welcome(fakeBackend())
    await flushPromises()
    expect(wrapper.get('h1').text()).toBe('Persona Builder')
    expect(wrapper.text()).toContain('Version 1.2.3')
    expect(wrapper.text()).toContain('Make an AI assistant that decides the way you do, from your own Claude history.')
    expect(wrapper.text()).toContain('Your history stays on this computer. Nothing is saved anywhere else unless you choose it.')
  })
  it('announces starting while the backend is pending', () => {
    const wrapper = welcome(fakeBackend({ appInfo: () => new Promise(() => {}) }))
    expect(wrapper.get('.spinner-border .visually-hidden').text()).toBe('Starting…')
    expect(wrapper.find('h1').exists()).toBe(false)
  })
  it('shows a safe error without the rejection text', async () => {
    const wrapper = welcome(fakeBackend({ appInfo: async () => { throw new Error('private backend detail') } }))
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe("Persona Builder couldn't start properly. Close it and open it again.")
    expect(wrapper.text()).not.toContain('private backend detail')
    expect(wrapper.find('.spinner-border').exists()).toBe(false)
  })
})
