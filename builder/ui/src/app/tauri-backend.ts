import { Channel, invoke } from '@tauri-apps/api/core'
import type { Backend } from '../backend'
import type { AppInfo } from '../types/generated/AppInfo'
import type { Assistant } from '../types/generated/Assistant'
import type { ConsentText } from '../types/generated/ConsentText'
import type { Consent } from '../types/generated/Consent'
import type { ClaudeStatus } from '../types/generated/ClaudeStatus'
import type { SaveOptions } from '../types/generated/SaveOptions'
import type { SignInCode } from '../types/generated/SignInCode'
import type { Said } from '../types/generated/Said'
import type { ClaudeCodeFolders } from '../types/generated/ClaudeCodeFolders'
import type { ReadProgress } from '../types/generated/ReadProgress'
import type { ReadCounts } from '../types/generated/ReadCounts'
import type { ExportStatus } from '../types/generated/ExportStatus'
import type { ChatList } from '../types/generated/ChatList'
import type { ClaudeaiCounts } from '../types/generated/ClaudeaiCounts'
import type { MergeCounts } from '../types/generated/MergeCounts'

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
  claudeCodeFolders: request => invoke<ClaudeCodeFolders>('claude_code_folders', { request }),
  readClaudeCode: (request, onProgress) => {
    const channel = new Channel<ReadProgress>()
    channel.onmessage = onProgress
    return invoke<ReadCounts>('read_claude_code', { request, onProgress: channel })
  },
  chooseClaudeaiExport: request => invoke<ExportStatus | null>('choose_claudeai_export', { request }),
  claudeaiExportStatus: request => invoke<ExportStatus | null>('claudeai_export_status', { request }),
  listClaudeaiChats: request => invoke<ChatList>('list_claudeai_chats', { request }),
  markClaudeaiChats: request => invoke<ChatList>('mark_claudeai_chats', { request }),
  readClaudeai: (request, onProgress) => {
    const channel = new Channel<ReadProgress>()
    channel.onmessage = onProgress
    return invoke<ClaudeaiCounts>('read_claudeai', { request, onProgress: channel })
  },
  mergeHistories: request => invoke<MergeCounts>('merge_histories', { request }),
}
