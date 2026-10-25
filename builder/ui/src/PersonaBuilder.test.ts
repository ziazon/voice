import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, fakeBackend, screen } from './test-support'
import PersonaBuilder from './PersonaBuilder.vue'

it('shows Welcome and disables all steps before choosing an assistant', async () => {
  const wrapper = screen(PersonaBuilder)
  await flushPromises()
  const items = wrapper.findAll('ol li')
  expect(items.map(item => item.get('[data-step-title]').text())).toEqual([
    'Your permission', 'Connect Claude', 'Your history', 'Build', 'How well it knows you', 'Review', 'Save',
  ])
  for (const [index, item] of items.entries()) {
    expect(item.attributes('aria-disabled')).toBe('true')
    expect(item.text()).toContain(index < 2 ? 'Choose an assistant first' : 'Not here yet')
  }
  expect(wrapper.get('main h1').text()).toBe('Persona Builder')
  expect(wrapper.text()).toContain('Version 1.2.3')
})
it('makes an assistant, gates connect on consent, then returns home and reloads', async () => {
  const backend = fakeBackend()
  const wrapper = screen(PersonaBuilder, backend)
  await flushPromises()
  await button(wrapper, 'Make a new one').trigger('click')
  await wrapper.get('#assistant-name').setValue('Example')
  await wrapper.get('#principal-name').setValue('Sample')
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.get('aside h2').text()).toBe('Making Example')
  expect(wrapper.get('main h2').text()).toBe('Before Persona Builder reads anything')
  const connect = wrapper.findAll('ol li')[1]!
  expect(connect.attributes('aria-disabled')).toBe('true')
  expect(connect.text()).toContain('Needs your permission first')
  expect(connect.get('button').attributes('disabled')).toBeDefined()
  await connect.get('button').trigger('click')
  expect(backend.claudeCheck).not.toHaveBeenCalled()
  expect(wrapper.get('[aria-current="step"]').text()).toBe('Your permission')
  await button(wrapper, 'I agree').trigger('click')
  await flushPromises()
  expect(wrapper.get('main h2').text()).toBe('Connect Claude')
  expect(wrapper.get('[aria-current="step"]').text()).toBe('Connect Claude')
  expect(wrapper.findAll('ol li')[1]!.attributes('aria-disabled')).toBeUndefined()
  await button(wrapper, 'Your permission').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain('You agreed on 2026-10-25.')
  await button(wrapper, 'Next: connect Claude').trigger('click')
  await flushPromises()
  expect(wrapper.get('main h2').text()).toBe('Connect Claude')
  await button(wrapper, 'Your assistants').trigger('click')
  await flushPromises()
  expect(wrapper.get('main h1').text()).toBe('Persona Builder')
  expect(backend.listAssistants).toHaveBeenCalledTimes(2)
})
it.each([false, true])('opens an existing assistant with agreed=%s at its next step', async agreed => {
  const wrapper = screen(PersonaBuilder, fakeBackend({ listAssistants: vi.fn().mockResolvedValue([{ ...assistant, consent: { agreed, agreedOn: agreed ? '2026-10-25' : null } }]) }))
  await flushPromises()
  await button(wrapper, 'Open').trigger('click')
  await flushPromises()
  expect(wrapper.get('main h2').text()).toBe(agreed ? 'Connect Claude' : 'Before Persona Builder reads anything')
})
it('can go back from making a new assistant', async () => {
  const wrapper = screen(PersonaBuilder)
  await flushPromises()
  await button(wrapper, 'Make a new one').trigger('click')
  await button(wrapper, 'Back').trigger('click')
  await flushPromises()
  expect(wrapper.get('main h1').text()).toBe('Persona Builder')
})
