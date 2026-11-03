import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, screen } from '../test-support'
import type { Permissions as List } from '../types/generated/Permissions'
import type { PermissionsChanged } from '../types/generated/PermissionsChanged'
import type { CalendarExport } from '../types/generated/CalendarExport'
import Permissions from './Permissions.vue'

const off = { id: 'calendar.read', words: 'read your calendar', on: false, givenOn: null, pastDays: 0, nextDays: 14, window: null }
const on = { ...off, on: true, givenOn: '2026-11-02', window: 'from today to 14 days ahead' }
const list: List = { permissions: [off], calendar: null, calendarProblem: null }
const enabled: List = { ...list, permissions: [on] }

it('loads once, shows its spinner then the off row, exact words and labelled bounded inputs', async () => {
  const load = deferred<List>()
  const backend = fakeBackend({ listPermissions: vi.fn().mockReturnValue(load.promise) })
  const wrapper = screen(Permissions, backend, { assistant })
  await flushPromises()
  expect(wrapper.text()).toContain("Looking at this assistant's permissions…")
  expect(backend.listPermissions).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  load.resolve(list); await flushPromises()
  expect(wrapper.get('h2').text()).toBe('What your assistant may read')
  expect(wrapper.text()).toContain('Voice reads a source only when you give it permission here, only for the days you allow, and writes each read in the log below. You can turn a permission off at any time.')
  expect(wrapper.text()).toContain('Read your calendar')
  expect(wrapper.get('.badge').text()).toBe('Off')
  expect(wrapper.get('.badge').classes()).toContain('bg-secondary-lt')
  expect(button(wrapper, 'Give permission').attributes('aria-label')).toBe('Give permission to read your calendar')
  const inputs = wrapper.findAll('input[type="number"]')
  expect(inputs).toHaveLength(2)
  expect(inputs.map(input => [(input.element as HTMLInputElement).value, input.attributes('min'), input.attributes('max')])).toEqual([['0', '0', '31'], ['14', '1', '92']])
  expect(wrapper.findAll('label').map(label => label.text())).toEqual(['Days back', 'Days ahead'])
  wrapper.findAll('label').forEach((label, i) => expect(label.attributes('for')).toBe(inputs[i]!.attributes('id')))
  expect(wrapper.text()).toContain('0 to 31'); expect(wrapper.text()).toContain('1 to 92')
  expect(wrapper.find('table').exists()).toBe(false)
})

it('shows the on row with dated window and both accessible actions', async () => {
  const wrapper = screen(Permissions, fakeBackend({ listPermissions: vi.fn().mockResolvedValue(enabled) }), { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  expect(wrapper.text()).toContain('From 2026-11-02: from today to 14 days ahead.')
  expect(wrapper.get('.badge').text()).toBe('On')
  expect(wrapper.get('.badge').classes()).toContain('bg-green-lt')
  expect(button(wrapper, 'Change the window').attributes('aria-label')).toBe('Change the window to read your calendar')
  expect(button(wrapper, 'Turn off').attributes('aria-label')).toBe('Turn off permission to read your calendar')
})

it.each([false, true])('gives or changes permission with integer inputs, replaces rows and reloads audit (on=%s)', async isOn => {
  const change = deferred<PermissionsChanged>()
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue(isOn ? enabled : list), givePermission: vi.fn().mockReturnValue(change.promise) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  const inputs = wrapper.findAll('input')
  await inputs[0]!.setValue('3'); await inputs[1]!.setValue('30')
  await button(wrapper, isOn ? 'Change the window' : 'Give permission').trigger('click')
  expect(backend.givePermission).toHaveBeenCalledExactlyOnceWith({ slug: 'example', id: 'calendar.read', pastDays: 3, nextDays: 30 })
  expect(wrapper.findAll('button, input').length).toBeGreaterThan(3)
  expect(wrapper.findAll('button, input').every(control => control.attributes('disabled') !== undefined)).toBe(true)
  change.resolve({ permissions: enabled, said: 'Permission is given.' }); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe('Permission is given.')
  expect(wrapper.get('.badge').text()).toBe('On')
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
  expect(backend.auditLog).toHaveBeenLastCalledWith({ slug: 'example', last: 50 })
})

it.each([false, true].flatMap(isOn => [
  ['', '14'], ['abc', '14'], ['-1', '14'], ['2.5', '14'], ['32', '14'],
  ['0', '0'], ['0', '93'], ['0', '1e9'],
].map(([pastDays, nextDays]) => ({ isOn, pastDays: pastDays!, nextDays: nextDays! }))))('rejects invalid window $pastDays, $nextDays (on=$isOn)', async ({ isOn, pastDays, nextDays }) => {
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue({ ...list, permissions: [{ ...(isOn ? on : off) }] }) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  const inputs = wrapper.findAll('input')
  await inputs[0]!.setValue(pastDays); await inputs[1]!.setValue(nextDays)
  await button(wrapper, isOn ? 'Change the window' : 'Give permission').trigger('click'); await flushPromises()
  expect.soft(backend.givePermission).not.toHaveBeenCalled()
  expect.soft(wrapper.findAll('.alert-warning').map(alert => alert.text())).toEqual(['The calendar window goes from 0 to 31 days back and from 1 to 92 days ahead.'])
})

it('turns off with the exact request and replaces the row', async () => {
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue(enabled), turnOffPermission: vi.fn().mockResolvedValue({ permissions: list, said: 'Permission is off.' }) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  await button(wrapper, 'Turn off').trigger('click'); await flushPromises()
  expect(backend.turnOffPermission).toHaveBeenCalledExactlyOnceWith({ slug: 'example', id: 'calendar.read' })
  expect(wrapper.get('.badge').text()).toBe('Off')
  expect(wrapper.get('.alert-success').text()).toBe('Permission is off.')
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
})

it.each([
  [false, { message: 'Try this again.' }, 'Try this again.'],
  [false, new Error('private detail'), "Voice couldn't save this. Try again."],
  [true, { message: 'Try this again.' }, 'Try this again.'],
])('reloads permissions and audit after failed change (on=%s)', async (isOn, failure, message) => {
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValueOnce(isOn ? enabled : list).mockResolvedValue(enabled), givePermission: vi.fn().mockRejectedValue(failure), turnOffPermission: vi.fn().mockRejectedValue(failure) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  await button(wrapper, isOn ? 'Turn off' : 'Give permission').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.get('.alert-warning').attributes('role')).toBe('alert')
  expect(backend.listPermissions).toHaveBeenCalledTimes(2)
  expect(backend.listPermissions).toHaveBeenLastCalledWith({ slug: 'example' })
  expect(wrapper.get('.badge').text()).toBe('On')
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
  expect(wrapper.text()).not.toContain('private detail')
})

it.each([
  [null, 'No calendar file chosen yet. Export your calendar as an .ics file, then choose it here.'],
  [{ name: 'team.ics', folder: false }, 'Your calendar file: team.ics'],
  [{ name: 'exports', folder: true }, 'Your calendar folder: exports'],
])('shows the calendar choice %j', async (calendar, sentence) => {
  const wrapper = screen(Permissions, fakeBackend({ listPermissions: vi.fn().mockResolvedValue({ ...list, calendar }) }), { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  expect(wrapper.text()).toContain(sentence)
})

it.each([false, true])('chooses a calendar with folder=%s and disables all controls while pending', async folder => {
  const choose = deferred<CalendarExport | null>()
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue(list), chooseCalendarExport: vi.fn().mockReturnValue(choose.promise) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  await button(wrapper, folder ? 'Choose a folder' : 'Choose a file').trigger('click')
  expect(backend.chooseCalendarExport).toHaveBeenCalledExactlyOnceWith({ slug: 'example', folder })
  expect(wrapper.findAll('button, input').every(control => control.attributes('disabled') !== undefined)).toBe(true)
  choose.resolve({ name: folder ? 'exports' : 'team.ics', folder }); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe(`Voice will read your calendar from ${folder ? 'exports' : 'team.ics'}.`)
  expect(wrapper.text()).toContain(`Your calendar ${folder ? 'folder: exports' : 'file: team.ics'}`)
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
})

it('shows a calendar problem instead of the no-file line and keeps both choose buttons', async () => {
  const calendarProblem = "Voice can't find your calendar file any more. Choose it again."
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue({ ...list, calendarProblem }) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('p.text-warning').exists()).toBe(true)
  expect(wrapper.get('p.text-warning').text()).toBe(calendarProblem)
  expect(wrapper.text()).not.toContain('No calendar file chosen yet.')
  expect(button(wrapper, 'Choose a file').element.matches(':disabled')).toBe(false)
  expect(button(wrapper, 'Choose a folder').element.matches(':disabled')).toBe(false)
})

it.each([false, true])('clears a calendar problem after successfully choosing (folder=%s)', async folder => {
  const calendarProblem = "Voice can't find your calendar file any more. Choose it again."
  const backend = fakeBackend({
    listPermissions: vi.fn().mockResolvedValue({ ...list, calendarProblem }),
    chooseCalendarExport: vi.fn().mockResolvedValue({ name: folder ? 'exports' : 'team.ics', folder }),
  })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.text()).toContain(calendarProblem)
  await button(wrapper, folder ? 'Choose a folder' : 'Choose a file').trigger('click'); await flushPromises()
  expect(backend.chooseCalendarExport).toHaveBeenCalledExactlyOnceWith({ slug: 'example', folder })
  expect(wrapper.text()).not.toContain(calendarProblem)
  expect(wrapper.find('p.text-warning').exists()).toBe(false)
  expect(wrapper.text()).toContain(`Your calendar ${folder ? 'folder: exports' : 'file: team.ics'}`)
})

it('keeps the current calendar and shows nothing on cancellation', async () => {
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue({ ...list, calendar: { name: 'team.ics', folder: false } }) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  await button(wrapper, 'Choose a file').trigger('click'); await flushPromises()
  expect(wrapper.text()).toContain('Your calendar file: team.ics')
  expect(wrapper.find('.alert-success').exists()).toBe(false)
  expect(wrapper.find('.alert-warning').exists()).toBe(false)
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
})

it.each([
  [{ message: 'Choose again.' }, 'Choose again.'],
  [new Error('private detail'), "Voice couldn't use that calendar. Try again."],
])('shows a safe calendar failure and reloads audit', async (failure, message) => {
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue(list), chooseCalendarExport: vi.fn().mockRejectedValue(failure) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  await button(wrapper, 'Choose a file').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  expect(wrapper.get('.alert-warning').attributes('role')).toBe('alert')
  expect(backend.auditLog).toHaveBeenCalledTimes(2)
})

it.each([
  [{ message: 'Look again.' }, 'Look again.'],
  [new Error('private detail'), "Voice couldn't look at this assistant's permissions. Try again."],
])('shows safe load error and retries', async (failure, message) => {
  const backend = fakeBackend({ listPermissions: vi.fn().mockRejectedValueOnce(failure).mockResolvedValue(list) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  expect(wrapper.get('.alert-warning').text()).toBe(message)
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(backend.listPermissions).toHaveBeenCalledTimes(2)
  expect(backend.listPermissions).toHaveBeenLastCalledWith({ slug: 'example' })
  expect(wrapper.get('.badge').text()).toBe('Off')
})

it('renders the calendar name and success sentence literally without markup', async () => {
  const name = '<img src=x onerror=alert(1)>'
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue({ ...list, calendar: { name, folder: false } }), givePermission: vi.fn().mockResolvedValue({ permissions: enabled, said: name }) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(wrapper.find('h2').exists()).toBe(true)
  expect(wrapper.text()).toContain(`Your calendar file: ${name}`)
  expect(wrapper.find('img').exists()).toBe(false)
  await button(wrapper, 'Give permission').trigger('click'); await flushPromises()
  expect(wrapper.get('.alert-success').text()).toBe(name)
  expect(wrapper.find('img').exists()).toBe(false)
})

it('disables every permission control while the initial audit request is pending', async () => {
  const audit = deferred<{ lines: string[], unreadable: number, older: boolean }>()
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue(list), auditLog: vi.fn().mockReturnValue(audit.promise) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  expect(backend.listPermissions).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(backend.auditLog).toHaveBeenCalledExactlyOnceWith({ slug: 'example', last: 50 })
  expect(wrapper.findAll('input')).toHaveLength(2)
  expect(wrapper.findAll('button, input').every(control => control.element.matches(':disabled'))).toBe(true)
  audit.resolve({ lines: [], unreadable: 0, older: false }); await flushPromises()
  expect(wrapper.findAll('button, input').every(control => !control.element.matches(':disabled'))).toBe(true)
})

it('disables every permission control while Show more loads the audit log', async () => {
  const audit = deferred<{ lines: string[], unreadable: number, older: boolean }>()
  const backend = fakeBackend({ listPermissions: vi.fn().mockResolvedValue(list), auditLog: vi.fn().mockResolvedValueOnce({ lines: ['A line.'], unreadable: 0, older: true }).mockReturnValue(audit.promise) })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  await button(wrapper, 'Show more').trigger('click'); await flushPromises()
  expect(backend.auditLog).toHaveBeenNthCalledWith(2, { slug: 'example', last: 1000 })
  expect(wrapper.findAll('input')).toHaveLength(2)
  expect(wrapper.findAll('button, input').every(control => control.element.matches(':disabled'))).toBe(true)
  audit.resolve({ lines: ['An older line.'], unreadable: 0, older: false }); await flushPromises()
  expect(wrapper.findAll('button, input').every(control => !control.element.matches(':disabled'))).toBe(true)
})

it.each([
  [false, false], [false, true], [true, false], [true, true],
])('disables the audit action during a parent call (retry=%s, choose=%s)', async (retry, choose) => {
  const pending = deferred<PermissionsChanged>()
  const calendar = deferred<CalendarExport | null>()
  const backend = fakeBackend({
    listPermissions: vi.fn().mockResolvedValue(list),
    auditLog: retry ? vi.fn().mockRejectedValue({ message: 'Read again.' }) : vi.fn().mockResolvedValue({ lines: ['A line.'], unreadable: 0, older: true }),
    givePermission: vi.fn().mockReturnValue(pending.promise),
    chooseCalendarExport: vi.fn().mockReturnValue(calendar.promise),
  })
  const wrapper = screen(Permissions, backend, { assistant }); await flushPromises()
  const action = button(wrapper, retry ? 'Try again' : 'Show more')
  expect(action.element.matches(':disabled')).toBe(false)
  await button(wrapper, choose ? 'Choose a file' : 'Give permission').trigger('click'); await flushPromises()
  // happy-dom does not include a disabled fieldset's descendants in :disabled.
  expect(action.element.closest('fieldset')?.disabled).toBe(true)
  expect(backend.auditLog).toHaveBeenCalledTimes(1)
  if (choose) calendar.resolve(null)
  else pending.resolve({ permissions: enabled, said: 'Permission is given.' })
  await flushPromises()
})
