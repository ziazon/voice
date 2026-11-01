import { expect, it } from 'vitest'
import { waitingWords } from './words'

it.each([
  [0, '0 skills wait for your yes.'],
  [1, '1 skill waits for your yes.'],
  [2, '2 skills wait for your yes.'],
])('waitingWords(%i) gives the exact sentence', (count, words) => {
  expect(waitingWords(count)).toBe(words)
})
