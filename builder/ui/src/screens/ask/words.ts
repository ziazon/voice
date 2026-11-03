export const words = {
  title: 'Ask one assistant',
  introduction: 'Ask one of your assistants a question and see how it would answer. You choose what it may use, each time.',
  loading: 'Loading your assistants…',
  assistantsFailure: "Your assistants couldn't be read. Close Voice and open it again.",
  empty: 'Make an assistant first.',
  who: 'Who to ask',
  consent: 'Needs your permission first',
  permissionsLoading: "Looking at this assistant's permissions…",
  permissionsFailure: "Voice couldn't look at this assistant's permissions. Try again.",
  retry: 'Try again',
  permissions: 'What it may use this time',
  noCalendar: "No calendar file chosen yet. Choose one on this assistant's Save page.",
  question: 'Your question',
  spending: "Each question counts toward this assistant's daily spending limit. Voice doesn't keep the question or the answer.",
  ask: 'Ask',
  askFailure: "Voice couldn't ask this assistant. Try again.",
}
export function whereWords(name: string, ticked: string[]): string {
  return ticked.includes('calendar.read')
    ? `Voice will send ${name}'s decisions, your question and your calendar for the days you allowed, nothing else. Your calendar is kept on your own machines unless you've let it go to Anthropic. Until you do, only a model server on your own machines can answer, and if none is set up, nothing is sent and Voice tells you why.`
    : `Voice will send ${name}'s decisions and your question, nothing else. They go to your own Claude Code, unless you've chosen to keep them on your own machines.`
}
export function noPermissionsWords(name: string): string {
  return `${name} hasn't been given permission to read anything, so it will use only its decisions and your question. You can give permission on its Save page.`
}
export function askingWords(name: string): string {
  return `Asking ${name}… This can take a minute.`
}
export function answerWords(name: string): string { return `${name}'s answer` }
export function countWords(count: number): string { return `${count.toLocaleString('en-US')} of 2,000 characters` }
