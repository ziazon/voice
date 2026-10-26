import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../../test-support'
import type { MergeCounts } from '../../types/generated/MergeCounts'
import PutTogether from './PutTogether.vue'

it('puts both histories together and shows the resulting counts', async () => {
  const pending = deferred<MergeCounts>()
  const backend = fakeBackend({ mergeHistories: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(PutTogether, backend, { slug: 'example' })
  await button(wrapper, 'Put it together').trigger('click')
  expect(backend.mergeHistories).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(button(wrapper, 'Putting it together…').attributes('disabled')).toBeDefined()
  pending.resolve({ questions: 1200, heldOut: 180, messages: 3400, duplicates: 12, privateLeftOut: 9 })
  await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe(`Done. Your assistant will learn from ${(1200).toLocaleString()} answered questions and ${(3400).toLocaleString()} messages you typed. 180 answered questions are kept back to test it. 12 repeats were left out.`)
})
it('shows the NOTHING failure in the card', async () => {
  const message = "There's nothing to put together yet. Read your Claude Code history or your claude.ai chats first."
  const wrapper = screen(PutTogether, fakeBackend({ mergeHistories: vi.fn().mockRejectedValue({ message }) }), { slug: 'example' })
  await button(wrapper, 'Put it together').trigger('click')
  await flushPromises()
  expect(wrapper.get('.alert').text()).toBe(message)
})
