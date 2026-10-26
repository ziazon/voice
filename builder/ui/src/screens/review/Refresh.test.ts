import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { button, deferred, fakeBackend, screen } from '../../test-support'
import type { ReviewStatus } from '../../types/generated/ReviewStatus'
import type { BuildProgress } from '../../types/generated/BuildProgress'
import type { RefreshApplied } from '../../types/generated/RefreshApplied'
import type { RefreshDrafted } from '../../types/generated/RefreshDrafted'
import Refresh from './Refresh.vue'

const change = { id: 'prop-one', drafted: '2026-10-25', summary: 'Offer a smaller first step.', learnedFrom: 1234, approved: false, points: [
  { why: 'Start small.', before: ['Wait for everything.'], after: ['Try one part.'] },
  { why: 'Add a rule.', before: [], after: ['Ask first.'] },
  { why: 'Remove a rule.', before: ['Always wait.'], after: [] },
] }
const status: ReviewStatus = { built: true, areas: ['Planning'], tests: { waiting: 1, inUse: 2, retired: 0 }, byArea: [], change: null }
const spend = { spentUsd: 3, dailyCapUsd: 50 }
const fallback = 'The test stopped before it finished, so your assistant is as it was. What was already finished is kept: test again to carry on.'
it.each([
  [{ ...status, built: false }, 'Build your assistant first.'],
  [{ ...status, tests: { waiting: 0, inUse: 2, retired: 0 } }, "Keep an answer first: there's nothing new to learn from."],
])('disables drafting with its reason', (value, reason) => {
  const wrapper = screen(Refresh, fakeBackend(), { slug: 'example', status: value })
  expect(button(wrapper, 'Draft a change').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).toContain(reason)
})
it.each([false, true])('drafts with progress and spend, change=%s', async drafted => {
  const call = deferred<RefreshDrafted>()
  const next = { ...status, change }
  const backend = fakeBackend({ draftRefresh: vi.fn().mockReturnValue(call.promise), reviewStatus: vi.fn().mockResolvedValue(next) })
  const wrapper = screen(Refresh, backend, { slug: 'example', status })
  await button(wrapper, 'Draft a change').trigger('click')
  expect(wrapper.text()).toContain('Drafting')
  expect(button(wrapper, 'Drafting a change…').attributes('disabled')).toBeDefined()
  call.resolve({ change: drafted ? change : null, spend }); await flushPromises()
  expect(backend.draftRefresh).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(wrapper.text()).toContain('Today, about $3.00 of the $50.00 daily limit, as Claude Code estimates it.')
  if (drafted) {
    expect(backend.reviewStatus).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
    expect(wrapper.emitted('changed')).toEqual([[next]])
  } else expect(wrapper.get('.alert-info').text()).toBe("There's nothing new to learn from yet. Keep answering your assistant, then try again.")
})
it('shows readable points with escaped text and empty sides', () => {
  const wrapper = screen(Refresh, fakeBackend(), { slug: 'example', status: { ...status, change: { ...change, summary: '<b>Summary</b>' } } })
  expect(wrapper.text()).toContain('Drafted 2026-10-25.')
  expect(wrapper.get('strong').text()).toBe('<b>Summary</b>')
  expect(wrapper.text()).toContain(`Learned from ${(1234).toLocaleString()} of your answers.`)
  expect(wrapper.findAll('h4').map(h => h.text())).toEqual(['1. Start small.', '2. Add a rule.', '3. Remove a rule.'])
  const first = wrapper.findAll('.row')[0]!
  expect(first.findAll('small').map(label => label.text())).toEqual(['Before', 'After'])
  expect(first.findAll('li').map(line => line.text())).toEqual(['Wait for everything.', 'Try one part.'])
  expect(wrapper.text()).toContain('Nothing: this is new.')
  expect(wrapper.text()).toContain('Taken out.')
  expect(wrapper.find('a').exists()).toBe(false)
})
it.each([true, false])('answers approve=%s and replaces the status', async approve => {
  const next = { ...status, change: approve ? { ...change, approved: true } : null }
  const call = deferred<ReviewStatus>()
  const backend = fakeBackend({ answerRefresh: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: { ...status, change } })
  await button(wrapper, approve ? 'Approve' : 'Reject').trigger('click')
  expect(wrapper.findAll('button').every(b => b.attributes('disabled') !== undefined)).toBe(true)
  call.resolve(next); await flushPromises()
  expect(backend.answerRefresh).toHaveBeenCalledExactlyOnceWith({ slug: 'example', id: 'prop-one', approve })
  expect(wrapper.emitted('changed')).toEqual([[next]])
  if (!approve) expect(wrapper.get('.alert-info').text()).toBe('Rejected. Nothing changed. Your answers stay saved for the next refresh.')
})
it.each([true, false])('tests an approved change with live lines and kept=%s', async kept => {
  const call = deferred<RefreshApplied>()
  let progress!: (value: BuildProgress) => void
  const backend = fakeBackend({ applyRefresh: vi.fn().mockImplementation((_request, callback) => { progress = callback; return call.promise }), reviewStatus: vi.fn().mockResolvedValue(status) })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: { ...status, change: { ...change, approved: true } } })
  expect(wrapper.text()).toContain('You approved this change. Next, Persona Builder tests it on all your answers: it keeps the change only if every new test passes and none of the others gets worse. It uses a little of your Claude plan.')
  expect(wrapper.findAll('button').map(b => b.text())).toEqual(['Test it'])
  await button(wrapper, 'Test it').trigger('click')
  expect(wrapper.emitted('running')).toEqual([[true]])
  expect(button(wrapper, 'Testing the change…').attributes('disabled')).toBeDefined()
  progress({ line: 'Testing your assistant as it is now on 3 tests.', spend: { spentUsd: 1, dailyCapUsd: 50 } }); await flushPromises()
  expect(wrapper.get('ul[aria-live="polite"]').text()).toContain('as it is now')
  progress({ line: 'Testing it with the change on 3 tests.', spend }); await flushPromises()
  expect(wrapper.get('ul[aria-live="polite"]').text()).toContain('with the change')
  expect(wrapper.text()).toContain('Today, about $3.00')
  call.resolve({ kept, reason: 'All tests checked.', spend: { spentUsd: 4, dailyCapUsd: 50 } }); await flushPromises()
  expect(backend.applyRefresh).toHaveBeenCalledExactlyOnceWith({ slug: 'example', id: 'prop-one' }, expect.any(Function))
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  expect(wrapper.get(kept ? '.alert-success' : '.alert-info').text()).toBe(kept ? 'Kept. Your assistant now works this way. All tests checked.' : 'Undone. Your assistant is as it was, and your answers stay saved for the next refresh. All tests checked.')
  expect(wrapper.text()).toContain('Today, about $4.00')
  expect(wrapper.emitted('changed')).toEqual([[status]])
  await wrapper.setProps({ status })
  expect(button(wrapper, 'Draft a change').exists()).toBe(true)
  expect(wrapper.find(kept ? '.alert-success' : '.alert-info').exists()).toBe(true)
})
it('keeps progress on failure, uses fallback and always releases running', async () => {
  const backend = fakeBackend({ applyRefresh: vi.fn().mockImplementation((_request, progress) => { progress({ line: 'Checking the written answers against what you wanted.', spend }); return Promise.reject(new Error('private path')) }) })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: { ...status, change: { ...change, approved: true } } })
  await button(wrapper, 'Test it').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe(fallback)
  expect(wrapper.get('ul[aria-live="polite"]').text()).toBe('Checking the written answers against what you wanted.')
  expect(wrapper.emitted('running')).toEqual([[true], [false]])
  expect(wrapper.text()).not.toContain('private path')
})
it.each(['draft', 'answer'] as const)('shows the safe %s fallback', async action => {
  const backend = fakeBackend({ draftRefresh: vi.fn().mockRejectedValue(new Error('private path')), answerRefresh: vi.fn().mockRejectedValue(new Error('private path')) })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: action === 'draft' ? status : { ...status, change } })
  await button(wrapper, action === 'draft' ? 'Draft a change' : 'Approve').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe(action === 'draft' ? "Claude's draft didn't fit, twice, so nothing was saved. Try again." : "Persona Builder couldn't save your answer. Check there's space on this computer, then try again.")
})
it('shows nothing new as information after a previous kept change', async () => {
  const backend = fakeBackend({
    applyRefresh: vi.fn().mockResolvedValue({ kept: true, reason: 'Every test passed.', spend }),
    reviewStatus: vi.fn().mockResolvedValue(status),
    draftRefresh: vi.fn().mockResolvedValue({ change: null, spend }),
  })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: { ...status, change: { ...change, approved: true } } })
  await button(wrapper, 'Test it').trigger('click'); await flushPromises()
  await wrapper.setProps({ status })
  await button(wrapper, 'Draft a change').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-info').text()).toBe("There's nothing new to learn from yet. Keep answering your assistant, then try again.")
  expect(wrapper.find('.alert-success').exists()).toBe(false)
})
it.each(['draft', 'test'] as const)('keeps a successful %s distinct from a failed status reload', async action => {
  const backend = fakeBackend({
    draftRefresh: vi.fn().mockResolvedValue({ change, spend }),
    applyRefresh: vi.fn().mockResolvedValue({ kept: true, reason: 'Every test passed.', spend }),
    reviewStatus: vi.fn().mockRejectedValue(new Error('private path')),
  })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: action === 'draft' ? status : { ...status, change: { ...change, approved: true } } })
  await button(wrapper, action === 'draft' ? 'Draft a change' : 'Test it').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe("Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
  expect(wrapper.text()).not.toContain('nothing was saved')
  expect(wrapper.text()).not.toContain('The test stopped before it finished')
  if (action === 'test') {
    expect(wrapper.get('.alert-success').text()).toBe('Kept. Your assistant now works this way. Every test passed.')
    expect(wrapper.emitted('running')).toEqual([[true], [false]])
  }
})

it('clears previous test progress as soon as a new draft starts', async () => {
  const backend = fakeBackend({ applyRefresh: vi.fn().mockImplementation((_request, progress) => { progress({ line: 'Old test progress.', spend }); return Promise.resolve({ kept: true, reason: 'Passed.', spend }) }), draftRefresh: vi.fn().mockReturnValue(deferred<RefreshDrafted>().promise) })
  const wrapper = screen(Refresh, backend, { slug: 'example', status: { ...status, change: { ...change, approved: true } } })
  await button(wrapper, 'Test it').trigger('click'); await flushPromises()
  expect(wrapper.get('ul[aria-live="polite"]').text()).toBe('Old test progress.')
  await wrapper.setProps({ status }); await button(wrapper, 'Draft a change').trigger('click')
  expect(wrapper.find('ul[aria-live="polite"]').exists()).toBe(false)
})

it('explains approval and rejection only while awaiting a decision', async () => {
  const wrapper = screen(Refresh, fakeBackend(), { slug: 'example', status: { ...status, change } })
  const help = wrapper.get('p.text-secondary.mt-2.mb-0')
  expect(help.text()).toContain('Approve: your assistant changes only after the change is tested on all your answers and nothing gets worse.')
  expect(help.text()).toContain('Reject: nothing changes, and your answers stay saved for the next refresh.')
  expect(help.element.previousElementSibling).toBe(button(wrapper, 'Approve').element.parentElement)
  await wrapper.setProps({ status: { ...status, change: { ...change, approved: true } } })
  expect(wrapper.find('p.text-secondary.mt-2.mb-0').exists()).toBe(false)
  await wrapper.setProps({ status })
  expect(wrapper.find('p.text-secondary.mt-2.mb-0').exists()).toBe(false)
})
