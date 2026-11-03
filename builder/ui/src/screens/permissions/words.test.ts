import { expect, it } from 'vitest'
import { capitalFirst, unreadableWords } from './words'

it.each([
  [1, "1 entry couldn't be read."],
  [2, "2 entries couldn't be read."],
  [0, "0 entries couldn't be read."],
])('unreadableWords(%i) gives the exact sentence', (count, sentence) => {
  expect(unreadableWords(count)).toBe(sentence)
})
it.each([
  ['read your calendar', 'Read your calendar'],
  ['Read your calendar', 'Read your calendar'],
  ['', ''],
])('capitalFirst(%s) only capitalizes its first letter', (text, sentence) => {
  expect(capitalFirst(text)).toBe(sentence)
})
