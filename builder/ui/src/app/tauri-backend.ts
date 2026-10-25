import { invoke } from '@tauri-apps/api/core'
import type { Backend } from '../backend'
import type { AppInfo } from '../types/generated/AppInfo'
import type { Assistant } from '../types/generated/Assistant'
import type { ConsentText } from '../types/generated/ConsentText'
import type { Consent } from '../types/generated/Consent'
import type { ClaudeStatus } from '../types/generated/ClaudeStatus'

export const tauriBackend: Backend = {
  appInfo: () => invoke<AppInfo>('app_info'),
  listAssistants: () => invoke<Assistant[]>('list_assistants'),
  newAssistant: request => invoke<Assistant>('new_assistant', { request }),
  showAssistantFolder: request => invoke<void>('show_assistant_folder', { request }),
  consentText: () => invoke<ConsentText>('consent_text'),
  agreeConsent: request => invoke<Consent>('agree_consent', { request }),
  claudeCheck: () => invoke<ClaudeStatus>('claude_check'),
}
