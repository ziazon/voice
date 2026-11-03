import { expect, it } from 'vitest'
import { whereWords } from './words'
it('says exactly where a question without capabilities may go', () => {
  expect(whereWords('Example', [])).toBe("Voice will send Example's decisions and your question, nothing else. They go to your own Claude Code, unless you've chosen to keep them on your own machines.")
})
it('says exactly where a question with calendar may go', () => {
  expect(whereWords('Other', ['calendar.read'])).toBe("Voice will send Other's decisions, your question and your calendar for the days you allowed, nothing else. Your calendar is kept on your own machines unless you've let it go to Anthropic. Until you do, only a model server on your own machines can answer, and if none is set up, nothing is sent and Voice tells you why.")
})
