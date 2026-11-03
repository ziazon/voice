import type { CalendarExport } from '../../types/generated/CalendarExport'
import type { Permission } from '../../types/generated/Permission'

export const words = {
  title: 'What your assistant may read',
  introduction: 'Voice reads a source only when you give it permission here, only for the days you allow, and writes each read in the log below. You can turn a permission off at any time.',
  loading: "Looking at this assistant's permissions…",
  loadFailure: "Voice couldn't look at this assistant's permissions. Try again.",
  retry: 'Try again',
  on: 'On',
  off: 'Off',
  pastDays: 'Days back',
  nextDays: 'Days ahead',
  pastHint: '0 to 31',
  nextHint: '1 to 92',
  give: 'Give permission',
  change: 'Change the window',
  turnOff: 'Turn off',
  turnOffPermission: 'Turn off permission',
  saveFailure: "Voice couldn't save this. Try again.",
  noCalendar: 'No calendar file chosen yet. Export your calendar as an .ics file, then choose it here.',
  chooseFile: 'Choose a file',
  chooseFolder: 'Choose a folder',
  calendarFailure: "Voice couldn't use that calendar. Try again.",
  auditTitle: 'What Voice has logged',
  auditIntroduction: 'Each time permission is given or taken back, and each time a source is read or a read is refused. What was read is never logged, only how many items.',
  auditFailure: "Voice couldn't read this assistant's log. Try again.",
  older: "Older entries aren't shown.",
  more: 'Show more',
}
export function unreadableWords(count: number): string {
  return count === 1 ? "1 entry couldn't be read." : `${count} entries couldn't be read.`
}
export function capitalFirst(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}
export function windowWords(permission: Permission): string {
  return `From ${permission.givenOn}: ${permission.window}.`
}
export function calendarWords(calendar: CalendarExport | null): string {
  return calendar ? `Your calendar ${calendar.folder ? 'folder' : 'file'}: ${calendar.name}` : words.noCalendar
}
export function calendarChosenWords(calendar: CalendarExport): string {
  return `Voice will read your calendar from ${calendar.name}.`
}
export function emptyLogWords(name: string): string {
  return `Nothing has been logged for ${name} yet.`
}
export function actionLabel(permission: Permission, turnOff = false): string {
  const action = turnOff ? words.turnOffPermission : permission.on ? words.change : words.give
  return `${action} to ${permission.words}`
}
