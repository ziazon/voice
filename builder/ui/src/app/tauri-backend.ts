import { invoke } from '@tauri-apps/api/core'
import type { Backend } from '../backend'
import type { AppInfo } from '../types/generated/AppInfo'
import type { Assistant } from '../types/generated/Assistant'
import type { ConsentText } from '../types/generated/ConsentText'
import type { Consent } from '../types/generated/Consent'
import type { ClaudeStatus } from '../types/generated/ClaudeStatus'
import type { SaveOptions } from '../types/generated/SaveOptions'
import type { SignInCode } from '../types/generated/SignInCode'
import type { Said } from '../types/generated/Said'

export const tauriBackend: Backend = {
  appInfo: () => invoke<AppInfo>('app_info'),
  listAssistants: () => invoke<Assistant[]>('list_assistants'),
  newAssistant: request => invoke<Assistant>('new_assistant', { request }),
  showAssistantFolder: request => invoke<void>('show_assistant_folder', { request }),
  consentText: () => invoke<ConsentText>('consent_text'),
  agreeConsent: request => invoke<Consent>('agree_consent', { request }),
  claudeCheck: () => invoke<ClaudeStatus>('claude_check'),
  saveOptions: request => invoke<SaveOptions>('save_options', { request }),
  openLink: request => invoke<void>('open_link', { request }),
  githubSignInStart: () => invoke<SignInCode>('github_sign_in_start'),
  githubSignInFinish: () => invoke<Said>('github_sign_in_finish'),
  githubSignInCancel: () => invoke<void>('github_sign_in_cancel'),
  githubSignOut: () => invoke<Said>('github_sign_out'),
  saveChoose: request => invoke<Said>('save_choose', { request }),
  savePush: request => invoke<Said>('save_push', { request }),
}
