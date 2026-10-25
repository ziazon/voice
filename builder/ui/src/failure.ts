/** Only the bridge's plain Failure shape may supply a sentence to a screen. */
export function failureMessage(error: unknown, fallback: string): string {
  if (error === null || typeof error !== 'object') return fallback
  const prototype = Object.getPrototypeOf(error)
  if (prototype !== Object.prototype && prototype !== null) return fallback
  return 'message' in error && typeof error.message === 'string' ? error.message : fallback
}
