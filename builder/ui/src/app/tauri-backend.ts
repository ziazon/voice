import type { PanelProgress } from '../types/generated/PanelProgress'
import type { PanelRan } from '../types/generated/PanelRan'
import type { PanelSummary } from '../types/generated/PanelSummary'
import type { PanelDetail } from '../types/generated/PanelDetail'
import type { ReportView } from '../types/generated/ReportView'
import type { ReviewStatus } from '../types/generated/ReviewStatus'
import type { RefreshDrafted } from '../types/generated/RefreshDrafted'
import type { RefreshApplied } from '../types/generated/RefreshApplied'
import type { ClaudeCodeStatus } from '../types/generated/ClaudeCodeStatus'
import type { ClaudeAiFiles } from '../types/generated/ClaudeAiFiles'
import type { ForgetPreview } from '../types/generated/ForgetPreview'
import type { SpendingLimit } from '../types/generated/SpendingLimit'
import type { Spend } from '../types/generated/Spend'
import type { BuildProgress } from '../types/generated/BuildProgress'
import type { BuildDone } from '../types/generated/BuildDone'
import type { InterviewStatus } from '../types/generated/InterviewStatus'
import type { InterviewKept } from '../types/generated/InterviewKept'
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
  convenePanel: (request, onProgress) => {
    const channel = new Channel<PanelProgress>()
    channel.onmessage = onProgress
    return invoke<PanelRan>('convene_panel', { request, onProgress: channel })
  },
  resumePanel: (request, onProgress) => {
    const channel = new Channel<PanelProgress>()
    channel.onmessage = onProgress
    return invoke<PanelRan>('resume_panel', { request, onProgress: channel })
  },
  stopPanel: request => invoke<void>('stop_panel', { request }),
  listPanels: () => invoke<PanelSummary[]>('list_panels'),
  panel: request => invoke<PanelDetail>('panel', { request }),
  showReport: request => invoke<ReportView>('show_report', { request }),
  reviewStatus: request => invoke<ReviewStatus>('review_status', { request }),
  keepAnswer: request => invoke<ReviewStatus>('keep_answer', { request }),
  draftRefresh: request => invoke<RefreshDrafted>('draft_refresh', { request }),
  answerRefresh: request => invoke<ReviewStatus>('answer_refresh', { request }),
  applyRefresh: (request, onProgress) => {
    const channel = new Channel<BuildProgress>()
    channel.onmessage = onProgress
    return invoke<RefreshApplied>('apply_refresh', { request, onProgress: channel })
  },
  buildAssistant: (request, onProgress) => {
    const channel = new Channel<BuildProgress>()
    channel.onmessage = onProgress
    return invoke<BuildDone>('build_assistant', { request, onProgress: channel })
  },
  stopBuild: request => invoke<void>('stop_build', { request }),
  spendToday: request => invoke<Spend>('spend_today', { request }),
  spendingLimit: request => invoke<SpendingLimit>('spending_limit', { request }),
  setSpendingLimit: request => invoke<SpendingLimit>('set_spending_limit', { request }),
  interviewStatus: request => invoke<InterviewStatus>('interview_status', { request }),
  draftInterview: request => invoke<InterviewStatus>('draft_interview', { request }),
  saveInterviewAnswers: request => invoke<InterviewStatus>('save_interview_answers', { request }),
  keepInterviewAnswers: request => invoke<InterviewKept>('keep_interview_answers', { request }),
  appInfo: () => invoke<AppInfo>('app_info'),
  listAssistants: () => invoke<Assistant[]>('list_assistants'),
  newAssistant: request => invoke<Assistant>('new_assistant', { request }),
  showAssistantFolder: request => invoke<void>('show_assistant_folder', { request }),
  consentText: () => invoke<ConsentText>('consent_text'),
  agreeConsent: request => invoke<Consent>('agree_consent', { request }),
  claudeCheck: () => invoke<ClaudeStatus>('claude_check'),
  claudeSignIn: () => invoke<ClaudeStatus>('claude_sign_in'),
  claudeSignInCancel: () => invoke<void>('claude_sign_in_cancel'),
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
  claudeCodeStatus: request => invoke<ClaudeCodeStatus>('claude_code_status', { request }),
  turnOn: request => invoke<Said>('turn_on', { request }),
  turnOff: request => invoke<Said>('turn_off', { request }),
  claudeAiFiles: request => invoke<ClaudeAiFiles>('claude_ai_files', { request }),
  makeClaudeAiFiles: request => invoke<ClaudeAiFiles>('make_claude_ai_files', { request }),
  showClaudeAiFiles: request => invoke<void>('show_claude_ai_files', { request }),
  forgetPreview: request => invoke<ForgetPreview>('forget_preview', { request }),
  forget: request => invoke<Said>('forget', { request }),
}
