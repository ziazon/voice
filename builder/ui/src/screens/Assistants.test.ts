import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, fakeBackend, screen } from '../test-support'
import Assistants from './Assistants.vue'

it('announces loading, then shows the empty list and make action', async () => {
  const wrapper = screen(Assistants)
  expect(wrapper.get('[role="status"]').text()).toBe('Looking for your assistants…')
  await flushPromises()
  expect(wrapper.get('h2').text()).toBe('Your assistants')
  expect(wrapper.text()).toContain("You haven't made an assistant yet.")
  await button(wrapper, 'Make a new one').trigger('click')
  expect(wrapper.emitted('make')).toHaveLength(1)
})
it('shows a safe list error without leaking a rejection', async () => {
  const wrapper = screen(Assistants, fakeBackend({ listAssistants: vi.fn().mockRejectedValue(new Error('private detail')) }))
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe("Persona Builder couldn't look for your assistants. Close it and open it again.")
  expect(wrapper.text()).not.toContain('private detail')
})
it('keeps backend order, shows status, opens and reveals each assistant', async () => {
  const second = { ...assistant, slug: 'another', name: 'Another', consent: { agreed: true, agreedOn: '2026-10-25' } }
  const backend = fakeBackend({ listAssistants: vi.fn().mockResolvedValue([assistant, second]), showAssistantFolder: vi.fn().mockRejectedValue({ message: 'Folder could not open.' }) })
  const wrapper = screen(Assistants, backend)
  await flushPromises()
  const items = wrapper.findAll('.list-group-item')
  expect(items.map(item => item.get('strong').text())).toEqual(['Example', 'Another'])
  expect(items[0]!.text()).toContain('Learns from Sample')
  expect(items[0]!.text()).toContain('Needs your permission')
  expect(items[1]!.text()).toContain('Permission given')
  await items[1]!.findAll('button')[0]!.trigger('click')
  expect(wrapper.emitted('open')).toEqual([[second]])
  await items[0]!.findAll('button')[1]!.trigger('click')
  await flushPromises()
  expect(backend.showAssistantFolder).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(items[0]!.get('[role="alert"]').text()).toBe('Folder could not open.')
  expect(items[1]!.find('[role="alert"]').exists()).toBe(false)
})
