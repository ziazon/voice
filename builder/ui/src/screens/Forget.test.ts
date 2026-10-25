import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant as example, button, deferred, fakeBackend, screen } from '../test-support'
import type { ForgetPreview } from '../types/generated/ForgetPreview'
import type { Said } from '../types/generated/Said'
import Forget from './Forget.vue'
const assistant = { ...example, name: 'Ari', slug: 'ari' }
it('previews zero and closes without deleting', async () => {
  const pending = deferred<ForgetPreview>()
  const backend = fakeBackend({ forgetPreview: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(Forget, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.text()).toContain('Forget what Ari learned from')
  expect(wrapper.text()).toContain("Forget deletes the history Ari learned from and Persona Builder's working files. Ari itself stays.")
  await button(wrapper, 'Forget…').trigger('click')
  expect(backend.forgetPreview).toHaveBeenCalledExactlyOnceWith({ slug: 'ari' })
  expect(button(wrapper, 'Checking…').attributes('disabled')).toBeDefined()
  pending.resolve({ files: 0 }); await flushPromises()
  expect(wrapper.text()).toContain("There's nothing to forget: Persona Builder hasn't kept anything for Ari yet.")
  expect(wrapper.findAll('button').map(b => b.text())).not.toContain('Delete these files')
  await button(wrapper, 'Close').trigger('click')
  expect(button(wrapper, 'Forget…').exists()).toBe(true)
  expect(backend.forget).not.toHaveBeenCalled()
})
it.each([1, 7])('confirms %i files, deletes the previewed count and emits forgotten', async files => {
  const pending = deferred<Said>()
  const backend = fakeBackend({ forgetPreview: vi.fn().mockResolvedValue({ files }), forget: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(Forget, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Forget…')
  await button(wrapper, 'Forget…').trigger('click'); await flushPromises()
  expect(wrapper.text()).toContain(`This deletes ${files === 1 ? '1 file' : '7 files'} from your Claude folder:`)
  expect(wrapper.findAll('ul').map(list => list.findAll('li').map(item => item.text()))).toEqual([
    ['the history Ari learned from', "Persona Builder's working files, including the claude.ai files", 'your permission to read your history', 'your choice of where to keep a copy of Ari'],
    ["Ari's own folder, and everything Persona Builder wrote into it", 'any copy of Ari on GitHub', "Ari in Claude Code, if it's on"],
  ])
  expect(wrapper.text()).toContain("You can't undo this.")
  expect(button(wrapper, 'Delete these files').classes()).toContain('btn-danger')
  await button(wrapper, 'Delete these files').trigger('click')
  expect(backend.forget).toHaveBeenCalledExactlyOnceWith({ slug: 'ari', files })
  expect(button(wrapper, 'Deleting…').attributes('disabled')).toBeDefined()
  expect(button(wrapper, 'Cancel').attributes('disabled')).toBeDefined()
  pending.resolve({ message: `Deleted ${files}.` }); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe(`Deleted ${files}.`)
  expect(wrapper.emitted('forgotten')).toEqual([[]])
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Forget…')
})
it('cancel sends nothing', async () => {
  const backend = fakeBackend({ forgetPreview: vi.fn().mockResolvedValue({ files: 7 }) })
  const wrapper = screen(Forget, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Forget…')
  await button(wrapper, 'Forget…').trigger('click'); await flushPromises()
  await button(wrapper, 'Cancel').trigger('click')
  expect(backend.forget).not.toHaveBeenCalled()
  expect(wrapper.emitted('forgotten')).toBeUndefined()
  expect(wrapper.findAll('button').map(b => b.text())).toEqual(['Forget…'])
})
it.each(['preview', 'delete'] as const)('shows safe %s failures and closes confirmation', async stage => {
  const backend = fakeBackend({ forgetPreview: stage === 'preview' ? vi.fn().mockRejectedValue({ message: 'Look again, then confirm.' }) : vi.fn().mockResolvedValue({ files: 7 }), forget: vi.fn().mockRejectedValue({ message: 'Look again, then confirm.' }) })
  const wrapper = screen(Forget, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Forget…')
  await button(wrapper, 'Forget…').trigger('click'); await flushPromises()
  if (stage === 'delete') { await button(wrapper, 'Delete these files').trigger('click'); await flushPromises() }
  expect(wrapper.find('.alert-warning').text()).toBe('Look again, then confirm.')
  expect(wrapper.findAll('button').map(b => b.text())).toEqual(['Forget…'])
  expect(wrapper.emitted('forgotten')).toBeUndefined()
})
it.each(['preview', 'delete'] as const)('hides a non-Failure %s rejection', async stage => {
  const backend = fakeBackend({ forgetPreview: stage === 'preview' ? vi.fn().mockRejectedValue(new Error('private detail')) : vi.fn().mockResolvedValue({ files: 7 }), forget: vi.fn().mockRejectedValue(new Error('private detail')) })
  const wrapper = screen(Forget, backend, { assistant })
  expect(wrapper.find('h2').exists()).toBe(true)
  await flushPromises()
  expect(wrapper.findAll('button').map(b => b.text())).toContain('Forget…')
  await button(wrapper, 'Forget…').trigger('click'); await flushPromises()
  if (stage === 'delete') { await button(wrapper, 'Delete these files').trigger('click'); await flushPromises() }
  expect(wrapper.find('.alert-warning').text()).toBe("Persona Builder couldn't forget anything, and nothing was deleted. Close it and open it again, then try again.")
  expect(wrapper.text()).not.toContain('private detail')
})
