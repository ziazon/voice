import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../../test-support'
import type { ClaudeaiCounts } from '../../types/generated/ClaudeaiCounts'
import type { ChatList } from '../../types/generated/ChatList'
import Claudeai from './Claudeai.vue'

const status = { name: 'download.zip', ready: true, chats: 1200 }
const emptyList: ChatList = { chats: [], mine: 0, notMine: 0, unmarked: 0 }

it('offers a file and folder choice when nothing is chosen', async () => {
  const backend = fakeBackend({ claudeaiExportStatus: vi.fn().mockResolvedValue(null), chooseClaudeaiExport: vi.fn().mockResolvedValue(null) })
  const wrapper = screen(Claudeai, backend, { slug: 'example' })
  await flushPromises()
  expect(backend.claudeaiExportStatus).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  await button(wrapper, 'Choose the download').trigger('click')
  await flushPromises()
  expect(backend.chooseClaudeaiExport).toHaveBeenLastCalledWith({ slug: 'example', folder: false })
  await button(wrapper, 'Choose a folder').trigger('click')
  await flushPromises()
  expect(backend.chooseClaudeaiExport).toHaveBeenLastCalledWith({ slug: 'example', folder: true })
  expect(wrapper.text()).toContain('If your Mac unzipped the download, choose the folder it made.')
  expect(wrapper.find('a').exists()).toBe(false)
})
it('announces checking the chosen download', () => {
  const wrapper = screen(Claudeai, fakeBackend({ claudeaiExportStatus: vi.fn().mockReturnValue(new Promise(() => {})) }), { slug: 'example' })
  expect(wrapper.text()).toContain('Checking your download…')
  expect(wrapper.find('.spinner-border').exists()).toBe(true)
})
it('keeps a ready status when a later choice is cancelled', async () => {
  const wrapper = screen(Claudeai, fakeBackend({ claudeaiExportStatus: vi.fn().mockResolvedValue(status), chooseClaudeaiExport: vi.fn().mockResolvedValue(null) }), { slug: 'example' })
  await flushPromises()
  const sentence = `You chose download.zip. Persona Builder can read it: ${(1200).toLocaleString()} chats.`
  expect(wrapper.text()).toContain(sentence)
  expect(button(wrapper, 'List the chats').exists()).toBe(true)
  await button(wrapper, 'Choose the download').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain(sentence)
})
it('warns when a chosen download is missing required parts', async () => {
  const wrapper = screen(Claudeai, fakeBackend({ claudeaiExportStatus: vi.fn().mockResolvedValue({ ...status, ready: false }) }), { slug: 'example' })
  await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe("You chose download.zip, but it's missing parts Persona Builder reads. Download your data from claude.ai again, then choose the new file.")
  expect(wrapper.findAll('button').map(item => item.text())).not.toContain('List the chats')
})
it('shows a status failure and keeps both choose buttons available', async () => {
  const wrapper = screen(Claudeai, fakeBackend({ claudeaiExportStatus: vi.fn().mockRejectedValue({ message: 'Choose it again.' }) }), { slug: 'example' })
  await flushPromises()
  expect(wrapper.get('.alert').text()).toBe('Choose it again.')
  expect(button(wrapper, 'Choose the download').attributes('disabled')).toBeUndefined()
  expect(button(wrapper, 'Choose a folder').attributes('disabled')).toBeUndefined()
})
it('disables reading when no chats are marked yours', async () => {
  const wrapper = screen(Claudeai, fakeBackend({ claudeaiExportStatus: vi.fn().mockResolvedValue(status), listClaudeaiChats: vi.fn().mockResolvedValue(emptyList) }), { slug: 'example' })
  await flushPromises()
  await button(wrapper, 'List the chats').trigger('click')
  await flushPromises()
  expect(button(wrapper, 'Read the chats marked yours').attributes('disabled')).toBeDefined()
})
it('reads marked chats, shows progress and resulting counts', async () => {
  const pending = deferred<ClaudeaiCounts>()
  const readClaudeai = vi.fn().mockImplementation((_request, progress) => { progress({ done: 1, total: 2 }); return pending.promise })
  const backend = fakeBackend({ claudeaiExportStatus: vi.fn().mockResolvedValue(status), listClaudeaiChats: vi.fn().mockResolvedValue({ ...emptyList, mine: 1 }), readClaudeai })
  const wrapper = screen(Claudeai, backend, { slug: 'example' })
  await flushPromises()
  await button(wrapper, 'List the chats').trigger('click')
  await flushPromises()
  await button(wrapper, 'Read the chats marked yours').trigger('click')
  expect(readClaudeai).toHaveBeenCalledExactlyOnceWith({ slug: 'example' }, expect.any(Function))
  expect(button(wrapper, 'Read the chats marked yours').attributes('disabled')).toBeDefined()
  expect(wrapper.get('[role="progressbar"]').attributes('aria-label')).toBe('Reading')
  expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('50')
  pending.resolve({ chats: 3, messages: 1200, privateLeftOut: 8 })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe(`Done. Read 3 chats: ${(1200).toLocaleString()} messages. 8 that looked private were left out.`)
})
it('clears the previous export and chats when a replacement cannot be read', async () => {
  const backend = fakeBackend({
    claudeaiExportStatus: vi.fn().mockResolvedValue(status),
    listClaudeaiChats: vi.fn().mockResolvedValue({ ...emptyList, mine: 1 }),
    readClaudeai: vi.fn().mockResolvedValue({ chats: 1, messages: 5, privateLeftOut: 0 }),
    chooseClaudeaiExport: vi.fn().mockRejectedValue({ message: 'Choose it again.' }),
  })
  const wrapper = screen(Claudeai, backend, { slug: 'example' })
  await flushPromises()
  await button(wrapper, 'List the chats').trigger('click')
  await flushPromises()
  await button(wrapper, 'Read the chats marked yours').trigger('click')
  await flushPromises()
  await button(wrapper, 'Choose the download').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain('Choose it again.')
  expect(wrapper.text()).not.toContain('You chose download.zip.')
  expect(wrapper.text()).not.toContain('Done. Read')
  expect(wrapper.findAll('button').map(item => item.text())).not.toContain('List the chats')
  expect(wrapper.findAll('button').map(item => item.text())).not.toContain('Read the chats marked yours')
})
