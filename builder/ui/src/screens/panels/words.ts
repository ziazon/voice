export function statusWords(status: string, round: number): string {
  const after = round === 0 ? 'before the first round' : `after round ${round}`
  switch (status) {
    case 'running': return `Running: ${round === 0 ? 'before the first round' : `round ${round}`}.`
    case 'paused': return 'Waiting for you: how long may they take?'
    case 'stopped': return `Stopped ${after}. You can carry on from here.`
    case 'failed': return `It stopped on a problem ${after}. You can try again from there.`
    case 'closed': return `Finished ${after}.`
    default: return "This panel's state couldn't be read."
  }
}
export function outcomeWords(outcome: string): string {
  switch (outcome) {
    case 'agreed': return 'They agreed.'
    case 'deadlock': return "They couldn't agree."
    case 'open': return 'Time ran out before they agreed.'
    default: return "This result's outcome couldn't be read."
  }
}
export function findingWords(finding: string): string {
  switch (finding) {
    case 'email': return 'an email address'
    case 'path': return 'a file or folder location'
    case 'term': return 'a private word'
    case 'private': return 'something personal'
    case 'secret': return 'something that looks like a password or key'
    default: return 'something private'
  }
}
export const TIME_LIMITS: { value: string | null; label: string }[] = [
  { value: null, label: 'Ask me after five minutes' },
  { value: '15m', label: '15 minutes' },
  { value: '1h', label: 'An hour' },
  { value: 'deadlock', label: 'No limit: until they agree or get stuck' },
]
