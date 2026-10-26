import { expect, it } from 'vitest'
import { assistant, fakeBackend, screen } from '../test-support'
import Sources from './Sources.vue'

it('names the assistant learning from the history', () => {
  const wrapper = screen(Sources, fakeBackend(), { assistant })
  expect(wrapper.get('h2').text()).toBe(`What ${assistant.name} learns from`)
})
