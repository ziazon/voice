import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, consentText, deferred, fakeBackend, screen } from '../test-support'
import Consent from './Consent.vue'

it('loads and shows the complete backend wording and saves permission', async () => {
  const pending = deferred<{ agreed: boolean; agreedOn: string }>()
  const backend = fakeBackend({ agreeConsent: vi.fn().mockReturnValue(pending.promise) })
  const wrapper = screen(Consent, backend, { assistant })
  expect(wrapper.get('[role="status"]').text()).toBe('Loading…')
  await flushPromises()
  expect(wrapper.get('h2').text()).toBe(consentText.title)
  expect(wrapper.findAll('h3').map(item => item.text())).toEqual(consentText.sections.map(section => section.heading))
  expect(wrapper.findAll('p').map(item => item.text())).toEqual(consentText.sections.flatMap(section => section.lines))
  await button(wrapper, 'I agree').trigger('click')
  expect(backend.agreeConsent).toHaveBeenCalledExactlyOnceWith({ slug: assistant.slug })
  expect(button(wrapper, 'Saving…').attributes('disabled')).toBeDefined()
  const answer = { agreed: true, agreedOn: '2026-10-25' }
  pending.resolve(answer)
  await flushPromises()
  expect(wrapper.emitted('agreed')).toEqual([[answer]])
})
it('shows the agreed date and next action', async () => {
  const wrapper = screen(Consent, fakeBackend(), { assistant: { ...assistant, consent: { agreed: true, agreedOn: '2026-10-25' } } })
  await flushPromises()
  expect(wrapper.text()).toContain('You agreed on 2026-10-25.')
  expect(wrapper.text()).not.toContain('I agree')
  await button(wrapper, 'Next: connect Claude').trigger('click')
  expect(wrapper.emitted('next')).toHaveLength(1)
})
it('shows a Failure from saving', async () => {
  const wrapper = screen(Consent, fakeBackend({ agreeConsent: vi.fn().mockRejectedValue({ message: 'Could not keep your answer.' }) }), { assistant })
  await flushPromises()
  await button(wrapper, 'I agree').trigger('click')
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe('Could not keep your answer.')
})
it('shows a safe wording-load failure', async () => {
  const wrapper = screen(Consent, fakeBackend({ consentText: vi.fn().mockRejectedValue(new Error('private detail')) }), { assistant })
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe("Persona Builder couldn't show this. Close it and open it again.")
  expect(wrapper.find('button').exists()).toBe(false)
})
