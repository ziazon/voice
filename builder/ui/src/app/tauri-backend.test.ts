import { expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { tauriBackend } from './tauri-backend'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

it('asks for app_info and returns its answer', async () => {
  const answer = { version: '1.2.3', agents: [{ id: 'example', name: 'Example' }] }
  vi.mocked(invoke).mockResolvedValue(answer)
  expect(await tauriBackend.appInfo()).toEqual(answer)
  expect(invoke).toHaveBeenCalledExactlyOnceWith('app_info')
})
