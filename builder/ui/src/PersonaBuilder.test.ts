import { flushPromises, mount } from '@vue/test-utils'
import { expect, it } from 'vitest'
import { backendKey } from './backend'
import PersonaBuilder from './PersonaBuilder.vue'

it('lists all seven future steps in order and shows Welcome', async () => {
  const wrapper = mount(PersonaBuilder, { global: { provide: { [backendKey]: {
    appInfo: async () => ({ version: '1.2.3', agents: [] }),
  } } } })
  await flushPromises()
  const items = wrapper.findAll('ol li')
  expect(items.map(item => item.get('[data-step-title]').text())).toEqual([
    'Your permission', 'Connect Claude', 'Your history', 'Build', 'How well it knows you', 'Review', 'Save',
  ])
  for (const item of items) {
    expect(item.attributes('aria-disabled')).toBe('true')
    expect(item.text()).toContain('Not here yet')
  }
  expect(wrapper.get('main h1').text()).toBe('Persona Builder')
  expect(wrapper.text()).toContain('Version 1.2.3')
  wrapper.unmount()
})
