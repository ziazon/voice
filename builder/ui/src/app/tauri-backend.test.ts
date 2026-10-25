import { expect, it, vi } from 'vitest'
import { Channel, invoke } from '@tauri-apps/api/core'
import { tauriBackend } from './tauri-backend'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

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
  ['saveChoose', 'save_choose', { slug: 'example', choice: 0, kind: 'github', name: 'example-assistant' }],
  ['savePush', 'save_push', { slug: 'example' }],
  ['claudeCodeStatus', 'claude_code_status', { slug: 'ari' }],
  ['turnOn', 'turn_on', { slug: 'ari' }],
  ['turnOff', 'turn_off', { slug: 'ari' }],
  ['claudeAiFiles', 'claude_ai_files', { slug: 'ari' }],
  ['makeClaudeAiFiles', 'make_claude_ai_files', { slug: 'ari' }],
  ['showClaudeAiFiles', 'show_claude_ai_files', { slug: 'ari' }],
  ['forgetPreview', 'forget_preview', { slug: 'ari' }],
  ['forget', 'forget', { slug: 'ari', files: 7 }],
] as const)('maps %s with a request argument', async (method, command, request) => {
  vi.mocked(invoke).mockClear().mockResolvedValue('answer')
  expect(tauriBackend[method]).toBeTypeOf('function')
  expect(await (tauriBackend[method] as (value: typeof request) => Promise<unknown>)(request)).toBe('answer')
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { request })
})

it.each([
  ['claudeCodeFolders', 'claude_code_folders', { slug: 'example' }],
  ['chooseClaudeaiExport', 'choose_claudeai_export', { slug: 'example', folder: false }],
  ['claudeaiExportStatus', 'claudeai_export_status', { slug: 'example' }],
  ['listClaudeaiChats', 'list_claudeai_chats', { slug: 'example' }],
  ['markClaudeaiChats', 'mark_claudeai_chats', { slug: 'example', mine: true, uuids: ['one'], unmarked: false }],
  ['mergeHistories', 'merge_histories', { slug: 'example' }],
] as const)('maps history method %s with a request', async (method, command, request) => {
  vi.mocked(invoke).mockClear().mockResolvedValue('answer')
  // The row varies the method's request, so call through the shared invocation shape.
  expect(await (tauriBackend[method] as (value: typeof request) => Promise<unknown>)(request)).toBe('answer')
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { request })
})
it.each([
  ['readClaudeCode', 'read_claude_code', { slug: 'example', dryRun: true }],
  ['readClaudeai', 'read_claudeai', { slug: 'example' }],
] as const)('maps %s with a progress channel whose messages reach the callback', async (method, command, request) => {
  const onProgress = vi.fn()
  vi.mocked(invoke).mockClear().mockResolvedValue('answer')
  expect(await (tauriBackend[method] as (value: typeof request, callback: typeof onProgress) => Promise<unknown>)(request, onProgress)).toBe('answer')
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command, { request, onProgress: expect.any(Channel) })
  const args = vi.mocked(invoke).mock.calls[0]![1]
  if (!args || typeof args !== 'object' || !('onProgress' in args) || !(args.onProgress instanceof Channel)) {
    throw new Error('The read did not send a progress channel')
  }
  args.onProgress.onmessage({ done: 2, total: 5 })
  expect(onProgress).toHaveBeenCalledExactlyOnceWith({ done: 2, total: 5 })
})
