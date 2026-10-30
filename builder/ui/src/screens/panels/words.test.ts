import { expect, it } from 'vitest'
import { findingWords, outcomeWords, statusWords, TIME_LIMITS } from './words'

it.each([
  ['running', 2, 'Running: round 2.'],
  ['paused', 2, 'Waiting for you: how long may they take?'],
  ['stopped', 2, 'Stopped after round 2. You can carry on from here.'],
  ['failed', 2, 'It stopped on a problem after round 2. You can try again from there.'],
  ['closed', 2, 'Finished after round 2.'],
  ['unknown', 2, "This panel's state couldn't be read."],
  ['running', 0, 'Running: before the first round.'],
  ['paused', 0, 'Waiting for you: how long may they take?'],
  ['stopped', 0, 'Stopped before the first round. You can carry on from here.'],
  ['failed', 0, 'It stopped on a problem before the first round. You can try again from there.'],
  ['closed', 0, 'Finished before the first round.'],
] as const)('statusWords(%s, %i) gives the exact sentence', (state, round, words) => {
  expect(statusWords(state, round)).toBe(words)
})
it.each([
  ['agreed', 'They agreed.'],
  ['deadlock', "They couldn't agree."],
  ['open', 'Time ran out before they agreed.'],
  ['unknown', "This result's outcome couldn't be read."],
])('outcomeWords(%s) gives the exact sentence', (outcome, words) => {
  expect(outcomeWords(outcome!)).toBe(words)
})
it.each([
  ['email', 'an email address'],
  ['path', 'a file or folder location'],
  ['term', 'a private word'],
  ['private', 'something personal'],
  ['secret', 'something that looks like a password or key'],
  ['unknown', 'something private'],
])('findingWords(%s) gives plain words', (finding, words) => {
  expect(findingWords(finding!)).toBe(words)
})
it('offers the four time limits in order with their exact labels', () => {
  expect(TIME_LIMITS).toEqual([
    { value: null, label: 'Ask me after five minutes' },
    { value: '15m', label: '15 minutes' },
    { value: '1h', label: 'An hour' },
    { value: 'deadlock', label: 'No limit: until they agree or get stuck' },
  ])
})
