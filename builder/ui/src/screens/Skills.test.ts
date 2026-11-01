import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { Skills as List } from '../types/generated/Skills'
import type { SkillRow } from '../types/generated/SkillRow'
import type { SkillsRefreshed } from '../types/generated/SkillsRefreshed'
import type { SkillsWithdrawn } from '../types/generated/SkillsWithdrawn'
import Skills from './Skills.vue'

const notes: SkillRow = { id: 'claude:notes', name: 'notes', place: '~/.claude/skills/notes', state: 'waiting', agreedOn: null, number: 2 }
const writing: SkillRow = { id: 'claude:writing', name: 'writing', place: '~/.claude/skills/writing', state: 'agreed', agreedOn: '2026-11-01', number: 5 }
const list: List = { skills: [notes, writing], notes: [] }
const copied: SkillsRefreshed = { lines: ['notes: copied', 'writing: copied'], copied: 2, agreed: 2, waiting: 0, new: 1, updated: 1, unchanged: 0, removed: 0 }
const summary = 'Copied 2 of 2 skills. Since the last refresh: 1 new, 1 updated, 0 unchanged, 0 removed.'

it('loads once, shows the spinner then backend row order, states, numbers and actions', async () => {
  const load = deferred<List>()
  const backend = fakeBackend({ listSkills: vi.fn().mockReturnValue(load.promise) })
  const wrapper = screen(Skills, backend, { assistant })
  await flushPromises()
  expect(wrapper.get('h2').text()).toBe('Your skills')
  expect(wrapper.text()).toContain('Your assistant can keep its own copy of the skills on this computer, with private details taken out. It copies only the skills you say yes to.')
  expect(wrapper.get('[role="status"]').text()).toBe('Looking for the skills on this computer…')
  expect(backend.listSkills).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  load.resolve({ skills: [notes, writing,
    { ...notes, id: 'claude:moved', name: 'moved', state: 'moved', number: 1 },
    { ...notes, id: 'claudeai:offered', name: 'offered', state: 'notOffered', number: 3 },
    { ...notes, id: 'claude:private', name: 'private', state: 'neverCopied', number: 4 },
  ], notes: ['Some skills could not be read.'] }); await flushPromises()
  expect(wrapper.find('[role="status"]').exists()).toBe(false)
  expect(wrapper.text()).toContain('1 skill waits for your yes.')
  const rows = wrapper.findAll('tbody tr')
  expect(rows).toHaveLength(5)
  expect(rows.map(row => row.find('strong').text())).toEqual(['notes', 'writing', 'moved', 'offered', 'private'])
  const words = ['Waiting for your yes', 'You said yes on 2026-11-01', 'Moved since you said yes. Say yes again to copy it.', "A claude.ai skill. It's copied only if you say yes.", "Never copied: it's on your never-copy list."]
  const numbers = [2, 5, 1, 3, 4]
  rows.forEach((row, i) => { expect(row.text()).toContain(words[i]); expect(row.text()).toContain(`No. ${numbers[i]}`) })
  expect(rows[0]!.text()).toContain(notes.place)
  expect(rows[0]!.get('button').attributes('aria-label')).toBe('Say yes to notes')
  expect(rows[1]!.get('button').attributes('aria-label')).toBe('Stop copying writing')
  expect(rows[2]!.get('button').attributes('aria-label')).toBe('Say yes to moved')
  expect(rows[3]!.get('button').attributes('aria-label')).toBe('Say yes to offered')
  expect(rows.slice(0, 4).map(row => row.get('button').text())).toEqual(['Say yes', 'Stop copying', 'Say yes', 'Say yes'])
  expect(rows[4]!.find('button').exists()).toBe(false)
  expect(wrapper.text()).toContain('Some skills could not be read.')
})

it('says yes to one skill, disables every action and replaces rows from the result', async () => {
  const agree = deferred<List>()
  const backend = fakeBackend({ listSkills: vi.fn().mockResolvedValue(list), agreeSkills: vi.fn().mockReturnValue(agree.promise) })
  const wrapper = screen(Skills, backend, { assistant }); await flushPromises()
  await wrapper.get('[aria-label="Say yes to notes"]').trigger('click')
  expect(backend.agreeSkills).toHaveBeenCalledExactlyOnceWith({ slug: 'example', ids: ['claude:notes'] })
  expect(wrapper.findAll('button').every(b => b.attributes('disabled') !== undefined)).toBe(true)
  agree.resolve({ skills: [{ ...notes, state: 'agreed', agreedOn: '2026-11-01' }], notes: [] }); await flushPromises()
  expect(wrapper.findAll('tbody tr')).toHaveLength(1)
  expect(wrapper.get('tbody tr').text()).toContain('You said yes on 2026-11-01')
  expect(wrapper.get('.alert-success').text()).toBe('Your assistant may copy it now. Bring its copy up to date to add it.')
})

it.each([
  ['notThere', "Your assistant won't copy it any more.", '.alert-success'],
  ['removed', "Your assistant won't copy it any more, and its copy of your skills was removed so nothing you stopped stays in it. Bring the copy up to date to make it again.", '.alert-success'],
  ['notRemoved', "Your assistant won't copy it any more, but Voice couldn't remove its copy of your skills, so it may still hold what you stopped. To see why, run voice skills refresh in a terminal.", '.alert-warning'],
] as const)('withdraws one skill with mirror=%s', async (mirror, message, alert) => {
  const withdraw = deferred<SkillsWithdrawn>()
  const backend = fakeBackend({ listSkills: vi.fn().mockResolvedValue(list), withdrawSkills: vi.fn().mockReturnValue(withdraw.promise) })
  const wrapper = screen(Skills, backend, { assistant }); await flushPromises()
  await wrapper.get('[aria-label="Stop copying writing"]').trigger('click')
  expect(backend.withdrawSkills).toHaveBeenCalledExactlyOnceWith({ slug: 'example', ids: ['claude:writing'] })
  expect(wrapper.findAll('button').every(b => b.attributes('disabled') !== undefined)).toBe(true)
  withdraw.resolve({ skills: { skills: [notes], notes: [] }, mirror }); await flushPromises()
  expect(wrapper.findAll('tbody tr')).toHaveLength(1)
  expect(wrapper.get(alert).text()).toBe(message)
  expect(wrapper.get(alert).attributes('role')).toBe('alert')
  expect(wrapper.find(mirror === 'notRemoved' ? '.alert-success' : '.alert-warning').exists()).toBe(false)
})

it.each([false, true])('refreshes, reports plain lines, reloads and warns when partial=%s', async partial => {
  const refresh = deferred<SkillsRefreshed>()
  const backend = fakeBackend({ listSkills: vi.fn().mockResolvedValueOnce(list).mockResolvedValue({ skills: [writing], notes: [] }), refreshSkills: vi.fn().mockReturnValue(refresh.promise) })
  const wrapper = screen(Skills, backend, { assistant }); await flushPromises()
  await button(wrapper, 'Bring the copy up to date').trigger('click')
  expect(backend.refreshSkills).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(wrapper.get('[role="status"]').text()).toBe("Bringing your assistant's copy up to date…")
  expect(wrapper.findAll('button').every(b => b.attributes('disabled') !== undefined)).toBe(true)
  refresh.resolve({ ...copied, copied: partial ? 1 : 2 }); await flushPromises()
  expect(wrapper.get(partial ? '.alert-warning' : '.alert-success').text()).toBe(partial ? summary.replace('Copied 2', 'Copied 1') : summary)
  expect(wrapper.find(partial ? '.alert-success' : '.alert-warning').exists()).toBe(false)
  expect(wrapper.get('pre').text()).toBe('notes: copied\nwriting: copied')
  expect(backend.listSkills).toHaveBeenNthCalledWith(2, { slug: 'example' })
  expect(backend.listSkills).toHaveBeenCalledTimes(2)
  expect(wrapper.findAll('tbody tr')).toHaveLength(1)
})

it('shows the empty state without a table or report', async () => {
  const wrapper = screen(Skills, fakeBackend(), { assistant }); await flushPromises()
  expect(wrapper.text()).toContain('No skills found on this computer.')
  expect(wrapper.find('table').exists()).toBe(false)
  expect(wrapper.find('pre').exists()).toBe(false)
})

it.each([
  [{ message: 'Look again.' }, 'Look again.'],
  [new Error('private detail'), "Voice couldn't look through the skills on this computer. Try again."],
])('shows safe load failure and retries', async (error, message) => {
  const backend = fakeBackend({ listSkills: vi.fn().mockRejectedValueOnce(error).mockResolvedValue(list) })
  const wrapper = screen(Skills, backend, { assistant }); await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(backend.listSkills).toHaveBeenCalledTimes(2)
  expect(backend.listSkills).toHaveBeenLastCalledWith({ slug: 'example' })
  expect(wrapper.findAll('tbody tr')).toHaveLength(2)
  expect(wrapper.find('.alert-warning').exists()).toBe(false)
})

it.each([
  ['agreeSkills', 'Say yes to notes', "Voice couldn't save your choice. Try again."],
  ['withdrawSkills', 'Stop copying writing', "Voice couldn't save your choice. Try again."],
  ['refreshSkills', null, "Voice couldn't bring your assistant's copy up to date. Try again."],
] as const)('shows the safe fallback for %s', async (method, label, message) => {
  const backend = fakeBackend({ listSkills: vi.fn().mockResolvedValue(list), [method]: vi.fn().mockRejectedValue(new Error('hidden backend diagnostic')) })
  const wrapper = screen(Skills, backend, { assistant }); await flushPromises()
  await (label ? wrapper.get(`[aria-label="${label}"]`) : button(wrapper, 'Bring the copy up to date')).trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.text()).not.toContain('hidden backend diagnostic')
})

it('renders a skill name as literal text and never markup', async () => {
  const name = '<img src=x onerror=alert(1)>'
  const wrapper = screen(Skills, fakeBackend({ listSkills: vi.fn().mockResolvedValue({ skills: [{ ...notes, name }], notes: [] }) }), { assistant }); await flushPromises()
  expect(wrapper.get('strong').text()).toBe(name)
  expect(wrapper.get('tbody button').attributes('aria-label')).toBe(`Say yes to ${name}`)
  expect(wrapper.find('img').exists()).toBe(false)
})
