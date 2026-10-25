import { inject, provide, type InjectionKey } from 'vue'
import type { AppInfo } from './types/generated/AppInfo'

/**
 * Everything the screens ask of the app. A host gives its own backend.
 * The webview never names a file or folder. A request carries an assistant's slug or an id
 * the Rust side handed out; Rust finds every path itself and opens every native picker itself.
 * A host app must keep the same rule.
 */
export interface Backend { appInfo(): Promise<AppInfo> }

export const backendKey: InjectionKey<Backend> = Symbol('persona-builder-backend')

export function provideBackend(backend: Backend): void {
  provide(backendKey, backend)
}

export function useBackend(): Backend {
  const backend = inject(backendKey)
  if (!backend) throw new Error('Persona Builder has no backend')
  return backend
}
