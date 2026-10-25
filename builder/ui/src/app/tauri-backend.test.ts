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

it.each([
  ['listAssistants', 'list_assistants'],
  ['consentText', 'consent_text'],
  ['claudeCheck', 'claude_check'],
  ['githubSignInStart', 'github_sign_in_start'],
  ['githubSignInFinish', 'github_sign_in_finish'],
  ['githubSignInCancel', 'github_sign_in_cancel'],
  ['githubSignOut', 'github_sign_out'],
] as const)('maps %s to its command', async (method, command) => {
  vi.mocked(invoke).mockClear().mockResolvedValue('answer')
  expect(tauriBackend[method]).toBeTypeOf('function')
  expect(await tauriBackend[method]()).toBe('answer')
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command)
})
it.each([
  ['newAssistant', 'new_assistant', { name: 'Example', pronouns: 'they/them', principal: 'Sample', principalPronouns: 'they/them' }],
  ['showAssistantFolder', 'show_assistant_folder', { slug: 'example' }],
  ['agreeConsent', 'agree_consent', { slug: 'example' }],
  ['saveOptions', 'save_options', { slug: 'example' }],
  ['openLink', 'open_link', { link: 'githubSignup' }],
  ['saveChoose', 'save_choose', { slug: 'example', choice: 0, name: 'example-assistant' }],
  ['savePush', 'save_push', { slug: 'example' }],
] as const)('maps %s with a request argument', async (method, command, request) => {
  vi.mocked(invoke).mockClear().mockResolvedValue('answer')
  expect(tauriBackend[method]).toBeTypeOf('function')
  expect(await (tauriBackend[method] as (value: typeof request) => Promise<unknown>)(request)).toBe('answer')
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { request })
})
