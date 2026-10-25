import { flushPromises, type DOMWrapper } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../../test-support'
import type { ChatList as Chats } from '../../types/generated/ChatList'
import ChatList from './ChatList.vue'

function rowButton(row: DOMWrapper<Element>, text: string) {
  const found = row.findAll('button').find(item => item.text() === text)
  if (!found) throw new Error(`Missing row button: ${text}`)
  return found
}

const list: Chats = { mine: 1, notMine: 1, unmarked: 1, chats: [
  { uuid: 'one', title: 'First project', created: '2026-01-01', messages: 12, mark: 'mine' },
  { uuid: 'two', title: '[filtered]', created: '2026-02-01', messages: 3, mark: 'notMine' },
  { uuid: 'three', title: '', created: '2026-03-01', messages: 4, mark: 'unmarked' },
] }

it('shows counts, safe titles, message counts and the current marks', () => {
  const wrapper = screen(ChatList, undefined, { slug: 'example', list })
  expect(wrapper.text()).toContain('1 yours, 1 not yours, 1 not marked yet.')
  const rows = wrapper.findAll('.list-group-item')
  expect(rows[0]!.text()).toContain('2026-01-01 · 12 messages')
  expect(rows[1]!.text()).toContain('Title hidden: it looked private')
  expect(rows[2]!.text()).toContain('No title')
  expect(rowButton(rows[0]!, 'Yours').attributes('aria-pressed')).toBe('true')
  expect(rowButton(rows[0]!, 'Yours').classes()).toContain('active')
  expect(rowButton(rows[1]!, 'Not yours').attributes('aria-pressed')).toBe('true')
  expect(rowButton(rows[2]!, 'Yours').attributes('aria-pressed')).not.toBe('true')
  expect(rowButton(rows[2]!, 'Not yours').attributes('aria-pressed')).not.toBe('true')
})
it('marks just the clicked uuid as yours and emits the updated list', async () => {
  const pending = deferred<Chats>()
  const backend = fakeBackend({ markClaudeaiChats: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(ChatList, backend, { slug: 'example', list })
  await rowButton(wrapper.findAll('.list-group-item')[2]!, 'Yours').trigger('click')
  expect(backend.markClaudeaiChats).toHaveBeenCalledExactlyOnceWith({ slug: 'example', mine: true, uuids: ['three'], unmarked: false })
  expect(wrapper.findAll('button').every(item => item.attributes('disabled') !== undefined)).toBe(true)
  const answer: Chats = { ...list, mine: 2, unmarked: 0, chats: list.chats.map(chat => chat.uuid === 'three' ? { ...chat, mark: 'mine' } : chat) }
  pending.resolve(answer)
  await flushPromises()
  expect(wrapper.emitted('update')).toEqual([[answer]])
  await wrapper.setProps({ list: answer })
  expect(rowButton(wrapper.findAll('.list-group-item')[2]!, 'Yours').attributes('aria-pressed')).toBe('true')
})
it('marks every chat as yours and the unmarked remainder as not yours', async () => {
  const backend = fakeBackend({ markClaudeaiChats: vi.fn().mockResolvedValue(list) })
  const wrapper = screen(ChatList, backend, { slug: 'example', list })
  await button(wrapper, 'Mark all as yours').trigger('click')
  await flushPromises()
  expect(backend.markClaudeaiChats).toHaveBeenLastCalledWith({ slug: 'example', mine: true, uuids: ['one', 'two', 'three'], unmarked: false })
  await button(wrapper, 'Mark the rest as not yours').trigger('click')
  await flushPromises()
  expect(backend.markClaudeaiChats).toHaveBeenLastCalledWith({ slug: 'example', mine: false, uuids: [], unmarked: true })
})
it('filters chat titles case-insensitively without calling the backend', async () => {
  const backend = fakeBackend()
  const wrapper = screen(ChatList, backend, { slug: 'example', list })
  await wrapper.get('input').setValue('PROJECT')
  expect(wrapper.findAll('.list-group-item')).toHaveLength(1)
  expect(wrapper.get('.list-group-item').text()).toContain('First project')
  expect(backend.markClaudeaiChats).not.toHaveBeenCalled()
})
it('shows a safe failure if marking fails', async () => {
  const wrapper = screen(ChatList, fakeBackend({ markClaudeaiChats: vi.fn().mockRejectedValue({ message: 'Try again.' }) }), { slug: 'example', list })
  await button(wrapper, 'Mark all as yours').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert').text()).toBe('Try again.')
})
