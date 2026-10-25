import { afterEach, expect, it, vi } from 'vitest'
import { followSystemTheme } from './theme'

afterEach(() => { vi.restoreAllMocks(); delete document.documentElement.dataset.bsTheme })
function media(matches: boolean) {
  const query = Object.assign(new EventTarget(), {
    matches, media: '(prefers-color-scheme: dark)', onchange: null,
    addListener: vi.fn(), removeListener: vi.fn(),
  })
  vi.spyOn(window, 'matchMedia').mockReturnValue(query)
  return query
}

it.each([[true, 'dark'], [false, 'light']])('follows an initial preference of %s', (dark, theme) => {
  media(dark)
  const stop = followSystemTheme(window)
  expect(window.matchMedia).toHaveBeenCalledWith('(prefers-color-scheme: dark)')
  expect(document.documentElement.dataset.bsTheme).toBe(theme)
  stop()
})

it('updates on change and stops listening when disposed', () => {
  const query = media(false)
  const stop = followSystemTheme(window)
  query.matches = true
  query.dispatchEvent(new Event('change'))
  expect(document.documentElement.dataset.bsTheme).toBe('dark')
  stop()
  query.matches = false
  query.dispatchEvent(new Event('change'))
  expect(document.documentElement.dataset.bsTheme).toBe('dark')
})
