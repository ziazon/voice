import type { SavedOnGithub } from './types/generated/SavedOnGithub'
import type { OpenSavedRequest } from './types/generated/OpenSavedRequest'
import type { OpenedSaved } from './types/generated/OpenedSaved'
import type { TookNewest } from './types/generated/TookNewest'
import type { BuildRequest } from './types/generated/BuildRequest'
import type { AskRequest } from './types/generated/AskRequest'
import type { AskAnswer } from './types/generated/AskAnswer'
import type { Permissions } from './types/generated/Permissions'
import type { PermissionsChanged } from './types/generated/PermissionsChanged'
import type { GivePermissionRequest } from './types/generated/GivePermissionRequest'
import type { PermissionRequest } from './types/generated/PermissionRequest'
import type { ChooseCalendarRequest } from './types/generated/ChooseCalendarRequest'
import type { CalendarExport } from './types/generated/CalendarExport'
import type { AuditRequest } from './types/generated/AuditRequest'
import type { AuditLog } from './types/generated/AuditLog'
import type { Skills } from './types/generated/Skills'
import type { SkillsRequest } from './types/generated/SkillsRequest'
import type { SkillsWithdrawn } from './types/generated/SkillsWithdrawn'
import type { SkillsRefreshed } from './types/generated/SkillsRefreshed'
import type { ConvenePanelRequest } from './types/generated/ConvenePanelRequest'
import type { ResumePanelRequest } from './types/generated/ResumePanelRequest'
import type { PanelIdRequest } from './types/generated/PanelIdRequest'
import type { PanelProgress } from './types/generated/PanelProgress'
import type { PanelRan } from './types/generated/PanelRan'
import type { PanelSummary } from './types/generated/PanelSummary'
import type { PanelDetail } from './types/generated/PanelDetail'
import type { ReportView } from './types/generated/ReportView'
import type { ReviewStatus } from './types/generated/ReviewStatus'
import type { KeepAnswerRequest } from './types/generated/KeepAnswerRequest'
import type { RefreshDrafted } from './types/generated/RefreshDrafted'
import type { RefreshAnswerRequest } from './types/generated/RefreshAnswerRequest'
import type { RefreshApplyRequest } from './types/generated/RefreshApplyRequest'
import type { RefreshApplied } from './types/generated/RefreshApplied'
import type { TurnOnRequest } from './types/generated/TurnOnRequest'
import type { ClaudeCodeStatus } from './types/generated/ClaudeCodeStatus'
import type { TurnOffRequest } from './types/generated/TurnOffRequest'
import type { ClaudeAiFiles } from './types/generated/ClaudeAiFiles'
import type { ForgetPreview } from './types/generated/ForgetPreview'
import type { ForgetRequest } from './types/generated/ForgetRequest'
import type { SpendingLimit } from './types/generated/SpendingLimit'
import type { SpendingLimitRequest } from './types/generated/SpendingLimitRequest'
import type { Spend } from './types/generated/Spend'
import type { BuildProgress } from './types/generated/BuildProgress'
import type { BuildDone } from './types/generated/BuildDone'
import type { InterviewStatus } from './types/generated/InterviewStatus'
import type { DraftInterviewRequest } from './types/generated/DraftInterviewRequest'
import type { InterviewAnswersRequest } from './types/generated/InterviewAnswersRequest'
import type { InterviewKept } from './types/generated/InterviewKept'
import { inject, provide, type InjectionKey } from 'vue'
import type { AppInfo } from './types/generated/AppInfo'
import type { BringInRequest } from './types/generated/BringInRequest'
import type { BroughtIn } from './types/generated/BroughtIn'
import type { Assistant } from './types/generated/Assistant'
import type { FindExistingRequest } from './types/generated/FindExistingRequest'
import type { ExistingAssistant } from './types/generated/ExistingAssistant'
import type { NewAssistantRequest } from './types/generated/NewAssistantRequest'
import type { SlugRequest } from './types/generated/SlugRequest'
import type { ConsentText } from './types/generated/ConsentText'
import type { Consent } from './types/generated/Consent'
import type { ClaudeStatus } from './types/generated/ClaudeStatus'
import type { ClaudeSignInRequest } from './types/generated/ClaudeSignInRequest'
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
  askAssistant(request: AskRequest): Promise<AskAnswer>
  listPermissions(request: SlugRequest): Promise<Permissions>
  givePermission(request: GivePermissionRequest): Promise<PermissionsChanged>
  turnOffPermission(request: PermissionRequest): Promise<PermissionsChanged>
  chooseCalendarExport(request: ChooseCalendarRequest): Promise<CalendarExport | null>
  auditLog(request: AuditRequest): Promise<AuditLog>
  listSkills(request: SlugRequest): Promise<Skills>
  agreeSkills(request: SkillsRequest): Promise<Skills>
  withdrawSkills(request: SkillsRequest): Promise<SkillsWithdrawn>
  refreshSkills(request: SlugRequest): Promise<SkillsRefreshed>
  convenePanel(request: ConvenePanelRequest, onProgress: (progress: PanelProgress) => void): Promise<PanelRan>
  resumePanel(request: ResumePanelRequest, onProgress: (progress: PanelProgress) => void): Promise<PanelRan>
  stopPanel(request: PanelIdRequest): Promise<void>
  listPanels(): Promise<PanelSummary[]>
  panel(request: PanelIdRequest): Promise<PanelDetail>
  showReport(request: SlugRequest): Promise<ReportView>
  reviewStatus(request: SlugRequest): Promise<ReviewStatus>
  keepAnswer(request: KeepAnswerRequest): Promise<ReviewStatus>
  draftRefresh(request: SlugRequest): Promise<RefreshDrafted>
  answerRefresh(request: RefreshAnswerRequest): Promise<ReviewStatus>
  applyRefresh(request: RefreshApplyRequest, onProgress: (progress: BuildProgress) => void): Promise<RefreshApplied>
  buildAssistant(request: BuildRequest, onProgress: (progress: BuildProgress) => void): Promise<BuildDone>
  stopBuild(request: SlugRequest): Promise<void>
  spendToday(request: SlugRequest): Promise<Spend>
  spendingLimit(request: SlugRequest): Promise<SpendingLimit>
  setSpendingLimit(request: SpendingLimitRequest): Promise<SpendingLimit>
  interviewStatus(request: SlugRequest): Promise<InterviewStatus>
  draftInterview(request: DraftInterviewRequest): Promise<InterviewStatus>
  saveInterviewAnswers(request: InterviewAnswersRequest): Promise<InterviewStatus>
  keepInterviewAnswers(request: SlugRequest): Promise<InterviewKept>
  appInfo(): Promise<AppInfo>
  listAssistants(): Promise<Assistant[]>
  newAssistant(request: NewAssistantRequest): Promise<Assistant>
  bringInAssistant(request: BringInRequest): Promise<BroughtIn>
  findExisting(request: FindExistingRequest): Promise<ExistingAssistant>
  showAssistantFolder(request: SlugRequest): Promise<void>
  consentText(): Promise<ConsentText>
  agreeConsent(request: SlugRequest): Promise<Consent>
  claudeCheck(): Promise<ClaudeStatus>
  claudeSignIn(request: ClaudeSignInRequest): Promise<ClaudeStatus>
  claudeSignInCancel(): Promise<void>
  claudeSignOut(): Promise<ClaudeStatus>
  saveOptions(request: SlugRequest): Promise<SaveOptions>
  openLink(request: LinkRequest): Promise<void>
  githubSignInStart(): Promise<SignInCode>
  githubSignInFinish(): Promise<Said>
  githubSignInCancel(): Promise<void>
  githubSignOut(): Promise<Said>
  saveChoose(request: SaveChoiceRequest): Promise<Said>
  savePush(request: SlugRequest): Promise<Said>
  savedOnGithub(): Promise<SavedOnGithub>
  openSaved(request: OpenSavedRequest): Promise<OpenedSaved>
  takeNewest(request: SlugRequest): Promise<TookNewest>
  claudeCodeFolders(request: SlugRequest): Promise<ClaudeCodeFolders>
  readClaudeCode(request: ReadRequest, onProgress: (progress: ReadProgress) => void): Promise<ReadCounts>
  chooseClaudeaiExport(request: ChooseExportRequest): Promise<ExportStatus | null>
  claudeaiExportStatus(request: SlugRequest): Promise<ExportStatus | null>
  listClaudeaiChats(request: SlugRequest): Promise<ChatList>
  markClaudeaiChats(request: MarkChatsRequest): Promise<ChatList>
  readClaudeai(request: SlugRequest, onProgress: (progress: ReadProgress) => void): Promise<ClaudeaiCounts>
  mergeHistories(request: SlugRequest): Promise<MergeCounts>
  claudeCodeStatus(request: SlugRequest): Promise<ClaudeCodeStatus>
  turnOn(request: TurnOnRequest): Promise<Said>
  turnOff(request: TurnOffRequest): Promise<Said>
  claudeAiFiles(request: SlugRequest): Promise<ClaudeAiFiles>
  makeClaudeAiFiles(request: SlugRequest): Promise<ClaudeAiFiles>
  showClaudeAiFiles(request: SlugRequest): Promise<void>
  forgetPreview(request: SlugRequest): Promise<ForgetPreview>
  forget(request: ForgetRequest): Promise<Said>
}

export const backendKey: InjectionKey<Backend> = Symbol('voice-backend')

export function provideBackend(backend: Backend): void {
  provide(backendKey, backend)
}

export function useBackend(): Backend {
  const backend = inject(backendKey)
  if (!backend) throw new Error('Voice has no backend')
  return backend
}
