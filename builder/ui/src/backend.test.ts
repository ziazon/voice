import { mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { expect, it, vi } from 'vitest'
import { provideBackend, useBackend } from './backend'

it('throws a plain message without a backend', () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
  try {
    expect(() => useBackend()).toThrow('Persona Builder has no backend')
  } finally { warn.mockRestore() }
})

it('provides a host backend to its screens', () => {
  const backend = { appInfo: async () => ({ version: '1.2.3', agents: [] }) }
  const child = defineComponent({ setup() { return () => h('span', useBackend() === backend ? 'provided' : 'missing') } })
  const host = defineComponent({ setup() { provideBackend(backend); return () => h(child) } })
  const wrapper = mount(host)
  expect(wrapper.text()).toBe('provided')
  wrapper.unmount()
})
