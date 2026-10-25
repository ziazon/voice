import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  // No .env files: a build's VITE_* settings come from its own environment only, so a stray
  // untracked .env.production.local cannot change the app's build settings.
  envDir: false,
  server: { host: '127.0.0.1', port: 5180, strictPort: true },
  // CSS targets Safari 18 so Tabler's light-dark() stays native: lowered, it resolves once at :root and a
  // dark or colored rail (a nested color-scheme) keeps the light colours. WKWebView here is far newer.
  build: { target: 'safari16', cssTarget: 'safari18' },
  test: { environment: 'happy-dom' },
})
