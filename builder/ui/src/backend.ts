import { inject, provide, type InjectionKey } from 'vue'
import type { AppInfo } from './types/generated/AppInfo'
import type { Assistant } from './types/generated/Assistant'
import type { NewAssistantRequest } from './types/generated/NewAssistantRequest'
import type { SlugRequest } from './types/generated/SlugRequest'
import type { ConsentText } from './types/generated/ConsentText'
import type { Consent } from './types/generated/Consent'
import type { ClaudeStatus } from './types/generated/ClaudeStatus'
import type { SaveOptions } from './types/generated/SaveOptions'
import type { LinkRequest } from './types/generated/LinkRequest'
import type { SignInCode } from './types/generated/SignInCode'
import type { Said } from './types/generated/Said'
import type { SaveChoiceRequest } from './types/generated/SaveChoiceRequest'
import type { ClaudeCodeFolders } from './types/generated/ClaudeCodeFolders'
import type { ReadRequest } from './types/generated/ReadRequest'
import type { ReadProgress } from './types/generated/ReadProgress'
import type { ReadCounts } from './types/generated/ReadCounts'
import type { ChooseExportRequest } from './types/generated/ChooseExportRequest'
import type { ExportStatus } from './types/generated/ExportStatus'
import type { ChatList } from './types/generated/ChatList'
import type { MarkChatsRequest } from './types/generated/MarkChatsRequest'
import type { ClaudeaiCounts } from './types/generated/ClaudeaiCounts'
import type { MergeCounts } from './types/generated/MergeCounts'

/**
 * Everything the screens ask of the app. A host gives its own backend.
 * The webview never names a file or folder. A request carries an assistant's slug or an id
 * the Rust side handed out; Rust finds every path itself and opens every native picker itself.
 * A host app must keep the same rule.
 */
export interface Backend {
  appInfo(): Promise<AppInfo>
  listAssistants(): Promise<Assistant[]>
  newAssistant(request: NewAssistantRequest): Promise<Assistant>
  showAssistantFolder(request: SlugRequest): Promise<void>
  consentText(): Promise<ConsentText>
  agreeConsent(request: SlugRequest): Promise<Consent>
  claudeCheck(): Promise<ClaudeStatus>
  saveOptions(request: SlugRequest): Promise<SaveOptions>
  openLink(request: LinkRequest): Promise<void>
  githubSignInStart(): Promise<SignInCode>
  githubSignInFinish(): Promise<Said>
  githubSignInCancel(): Promise<void>
  githubSignOut(): Promise<Said>
  saveChoose(request: SaveChoiceRequest): Promise<Said>
  savePush(request: SlugRequest): Promise<Said>
  claudeCodeFolders(request: SlugRequest): Promise<ClaudeCodeFolders>
  readClaudeCode(request: ReadRequest, onProgress: (progress: ReadProgress) => void): Promise<ReadCounts>
  chooseClaudeaiExport(request: ChooseExportRequest): Promise<ExportStatus | null>
  claudeaiExportStatus(request: SlugRequest): Promise<ExportStatus | null>
  listClaudeaiChats(request: SlugRequest): Promise<ChatList>
  markClaudeaiChats(request: MarkChatsRequest): Promise<ChatList>
  readClaudeai(request: SlugRequest, onProgress: (progress: ReadProgress) => void): Promise<ClaudeaiCounts>
  mergeHistories(request: SlugRequest): Promise<MergeCounts>
}

export const backendKey: InjectionKey<Backend> = Symbol('persona-builder-backend')

export function provideBackend(backend: Backend): void {
  provide(backendKey, backend)
}

export function useBackend(): Backend {
  const backend = inject(backendKey)
  if (!backend) throw new Error('Persona Builder has no backend')
  return backend
}
