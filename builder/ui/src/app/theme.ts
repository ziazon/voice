/** Follow this window's system theme until the returned cleanup function is called. */
export function followSystemTheme(win: Window = window): () => void {
  const preference = win.matchMedia('(prefers-color-scheme: dark)')
  const update = () => {
    win.document.documentElement.dataset.bsTheme = preference.matches ? 'dark' : 'light'
  }
  update()
  preference.addEventListener('change', update)
  return () => preference.removeEventListener('change', update)
}
