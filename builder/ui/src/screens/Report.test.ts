import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { BuildReport } from '../types/generated/BuildReport'
import type { ReportView } from '../types/generated/ReportView'
import Report from './Report.vue'

const report: BuildReport = { built: '2026-10-25', overall: 'thin', answered: 120, thinBelow: 150, heldOut: 12, test: null, areas: [] }
function shown(value: Partial<ReportView> = {}) {
  const backend = fakeBackend({ showReport: vi.fn().mockResolvedValue({ report, unreadable: false, growth: null, ...value }) })
  return { backend, wrapper: screen(Report, backend, { assistant }) }
}
it('reads on mount, shows loading, and offers Build when there is no report', async () => {
  const call = deferred<ReportView>()
  const backend = fakeBackend({ showReport: vi.fn().mockReturnValue(call.promise) })
  const wrapper = screen(Report, backend, { assistant })
  expect(wrapper.get('[role="status"] .visually-hidden').text()).toBe('Reading your report…')
  expect(backend.showReport).toHaveBeenCalledWith({ slug: 'example' })
  call.resolve({ report: null, unreadable: false, growth: null }); await flushPromises()
  expect(wrapper.find('[role="status"]').exists()).toBe(false)
  expect(wrapper.get('h2').text()).toBe('How well it knows you')
  expect(wrapper.text()).toContain("There's no report yet. Build your assistant first.")
  await button(wrapper, 'Go to Build').trigger('click')
  expect(wrapper.emitted('build')).toEqual([[]])
})
it('shows the unreadable warning and a Build button', async () => {
  const { wrapper } = shown({ report: null, unreadable: true }); await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe("Your last report couldn't be read, so it isn't shown. Build your assistant again to make a new one.")
  await button(wrapper, 'Go to Build').trigger('click')
  expect(wrapper.emitted('build')).toEqual([[]])
  expect(wrapper.text()).not.toContain("There's no report yet")
})
it('shows Thin history, the honest untested line, and the interview button', async () => {
  const { wrapper } = shown(); await flushPromises()
  expect(wrapper.findAll('.card')).toHaveLength(1)
  expect(wrapper.text()).toContain('Overall: Thin')
  expect(wrapper.text()).toContain('From the build on 2026-10-25.')
  expect(wrapper.text()).toContain('Your history holds 120 answered questions. It takes about 150 to know you well.')
  expect(wrapper.text()).toContain("Only 12 questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own.")
  expect(wrapper.text()).toContain('The interview fills out a thin history.')
  expect(wrapper.text()).toContain('No kinds of decision yet.')
  await button(wrapper, 'Go to the interview').trigger('click')
  expect(wrapper.emitted('build')).toEqual([[]])
})
it('shows the tested result, counts Strong areas and renders all badge, example and help lines', async () => {
  const { wrapper } = shown({ report: { ...report, overall: 'gettingThere', answered: 1500, heldOut: 1000, test: { right: 900, of: 1000 }, areas: [
    { area: 'Planning', level: 'strong', examples: 30, help: [] },
    { area: 'Writing', level: 'gettingThere', examples: 15, help: ['time'] },
    { area: 'Choosing', level: 'thin', examples: 1, help: ['interview', 'claudeai'] },
    { area: '<script>private</script>', level: 'thin', examples: 3, help: ['time'] },
  ] } }); await flushPromises()
  expect(wrapper.text()).toContain('It passed the test on 1 of 4 kinds of decision, and got 900 of 1,000 test questions right.')
  expect(wrapper.text()).not.toContain('Only 1,000 questions')
  expect(wrapper.findAll('thead th').map(cell => cell.text())).toEqual(['Kind of decision', 'How well', 'What would help'])
  const rows = wrapper.findAll('tbody tr')
  expect(rows[0]!.get('.badge').classes()).toContain('bg-green-lt')
  expect(rows[0]!.text()).toContain('Strong')
  expect(rows[0]!.text()).toContain('Nothing needed')
  expect(rows[1]!.get('.badge').classes()).toContain('bg-yellow-lt')
  expect(rows[1]!.text()).toContain('Getting there')
  expect(rows[1]!.text()).toContain('Keep using Claude: it learns from every answer')
  expect(rows[2]!.get('.badge').classes()).toContain('bg-secondary-lt')
  expect(rows[2]!.text()).toContain('Thin')
  expect(rows[2]!.text()).toContain('1 example so far')
  expect(rows[2]!.findAll('li').map(line => line.text())).toEqual(['Answer a few questions about it', 'Add your claude.ai chats'])
  expect(rows[3]!.text()).toContain('3 examples so far')
  expect(rows[3]!.text()).toContain('<script>private</script>')
  expect(wrapper.find('script').exists()).toBe(false)
  expect(wrapper.find('a').exists()).toBe(false)
  await button(wrapper, 'Next: review what it learns').trigger('click')
  expect(wrapper.emitted('next')).toEqual([[]])
})
it.each([{ answered: 149, label: 'Answer the interview' }, { answered: 150, label: 'Answer a few questions about it' }])('offers the interview help for $answered answers', async ({ answered, label }) => {
  const { wrapper } = shown({ report: { ...report, answered, areas: [{ area: 'Scope', level: 'thin', examples: 3, help: ['interview'] }] } }); await flushPromises()
  expect(wrapper.get('tbody li').text()).toBe(label)
  if (answered < report.thinBelow) expect(wrapper.text()).not.toContain('Answer a few questions')
})
it('opens Build from the short interview button when a long history has a thin kind', async () => {
  const { wrapper } = shown({ report: { ...report, answered: 150, areas: [{ area: 'Scope', level: 'thin', examples: 3, help: ['interview'] }] } }); await flushPromises()
  const shortInterview = button(wrapper, 'Answer a few questions')
  expect(shortInterview.classes()).toEqual(expect.arrayContaining(['btn', 'btn-secondary']))
  await shortInterview.trigger('click')
  expect(wrapper.emitted('build')).toEqual([[]])
})
it('explains an untested report with enough history without offering the interview', async () => {
  const { wrapper } = shown({ report: { ...report, overall: 'strong', answered: 1500 } }); await flushPromises()
  expect(wrapper.text()).toContain("There's enough history to learn from, but too little to test yet.")
  expect(wrapper.text()).not.toContain('Go to the interview')
  expect(wrapper.text()).not.toContain('Answer a few questions')
})
it.each([{ kept: 1, word: 'change' }, { kept: 2000, word: 'changes' }])('shows growth with $word', async ({ kept, word }) => {
  const { wrapper } = shown({ growth: { kept, undone: 3, passing: 1000, of: 1200 } }); await flushPromises()
  expect(wrapper.text()).toContain(`Since the build: ${kept === 1 ? '1' : '2,000'} ${word} kept and 3 undone. Your assistant passes 1,000 of 1,200 tests from your answers.`)
})
it.each([{ message: 'Could not read it.' }, new Error('private')])('shows safe failures', async failure => {
  const backend = fakeBackend({ showReport: vi.fn().mockRejectedValue(failure) })
  const wrapper = screen(Report, backend, { assistant }); await flushPromises()
  expect(wrapper.get('.alert-danger').text()).toBe(failure instanceof Error ? "Something in this assistant's settings stops the review. Use the persona-builder command to see what." : failure.message)
  expect(wrapper.text()).not.toContain("There's no report yet")
})
