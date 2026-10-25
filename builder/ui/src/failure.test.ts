import { expect, it } from 'vitest'
import { failureMessage } from './failure'

it('returns only a plain Failure message', () => {
  expect(failureMessage({ message: 'Safe sentence.' }, 'Fallback.')).toBe('Safe sentence.')
  expect(failureMessage(Object.assign(Object.create(null), { message: 'Safe sentence.' }), 'Fallback.')).toBe('Safe sentence.')
})
it.each(['private detail', new Error('private detail'), null, { message: 7 }, ['private detail'], new (class { message = 'private detail' })()])('uses the fallback for a non-Failure rejection: %s', error => {
  expect(failureMessage(error, 'Fallback.')).toBe('Fallback.')
})
