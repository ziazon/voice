import type { PanelDetail } from './types/generated/PanelDetail'
import { readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { mount } from '@vue/test-utils'
import { afterEach, vi } from 'vitest'
import { backendKey, type Backend } from './backend'
import type { Assistant } from './types/generated/Assistant'
import type { SaveOptions } from './types/generated/SaveOptions'

export const assistant: Assistant = { slug: 'example', name: 'Example', principal: 'Sample', consent: { agreed: false, agreedOn: null } }
export const panelDetail: PanelDetail = {
  summary: {
    id: 'dl-20261030T120000Z-0a1b2c3d', question: 'Which path should we take?',
    status: 'closed', round: 2, outcome: 'agreed', updatedAt: '2026-10-30T12:00:00Z',
  },
  kind: 'text', seats: ['Pebble', 'Quill'], timeBox: null, asked: false, rounds: [],
  result: { outcome: 'agreed', answerText: 'Take the shorter path.', answerItems: [], groups: [], dissent: [], markdown: '**Their answer**\nTake the shorter path.' },
}
export const saveOptions: SaveOptions = {
  choices: [
    { kind: 'github', label: 'A new private GitHub repo' },
    { kind: 'local', label: 'This computer only' },
  ],
  selected: 0,
  note: 'A free GitHub account lets you keep a private copy.',
  chosen: null,
  signedIn: false,
  suggestedName: 'example-assistant',
}
export function fakeBackend(overrides: Partial<Backend> = {}): Backend {
  const reviewStatus = { built: false, areas: [], tests: { waiting: 0, inUse: 0, retired: 0 }, byArea: [], change: null }
  return {
    convenePanel: vi.fn().mockResolvedValue({ id: panelDetail.summary.id, status: 'closed', outcome: 'agreed' }),
    resumePanel: vi.fn().mockResolvedValue({ id: panelDetail.summary.id, status: 'closed', outcome: 'agreed' }),
    stopPanel: vi.fn().mockResolvedValue(undefined),
    listPanels: vi.fn().mockResolvedValue([]),
    panel: vi.fn().mockResolvedValue(panelDetail),
    showReport: vi.fn().mockResolvedValue({ report: null, unreadable: false, growth: null }),
    reviewStatus: vi.fn().mockResolvedValue(reviewStatus),
    keepAnswer: vi.fn().mockResolvedValue(reviewStatus),
    draftRefresh: vi.fn().mockResolvedValue({ change: null, spend: { spentUsd: 0, dailyCapUsd: 50 } }),
    answerRefresh: vi.fn().mockResolvedValue(reviewStatus),
    applyRefresh: vi.fn().mockResolvedValue({ kept: true, reason: 'Every test passed.', spend: { spentUsd: 0, dailyCapUsd: 50 } }),
    appInfo: vi.fn().mockResolvedValue({ version: '1.2.3', agents: [] }),
    listAssistants: vi.fn().mockResolvedValue([]),
    newAssistant: vi.fn().mockResolvedValue(assistant),
    showAssistantFolder: vi.fn().mockResolvedValue(undefined),
    consentText: vi.fn().mockResolvedValue(consentText),
    agreeConsent: vi.fn().mockResolvedValue({ agreed: true, agreedOn: '2026-10-25' }),
    claudeCheck: vi.fn().mockResolvedValue({ ready: true, message: 'Claude Code is ready.', note: null, canSignIn: false }),
    claudeSignIn: vi.fn().mockResolvedValue({ ready: true, message: 'Claude Code is ready.', note: null, canSignIn: false }),
    claudeSignInCancel: vi.fn().mockResolvedValue(undefined),
    saveOptions: vi.fn().mockResolvedValue(saveOptions),
    openLink: vi.fn().mockResolvedValue(undefined),
    githubSignInStart: vi.fn().mockResolvedValue({ userCode: 'ABCD-EFGH' }),
    githubSignInFinish: vi.fn().mockResolvedValue({ message: 'You are signed in to GitHub.' }),
    githubSignInCancel: vi.fn().mockResolvedValue(undefined),
    githubSignOut: vi.fn().mockResolvedValue({ message: 'You are signed out of GitHub.' }),
    saveChoose: vi.fn().mockResolvedValue({ message: 'Your choice is kept.' }),
    savePush: vi.fn().mockResolvedValue({ message: 'Your copy is saved.' }),
    claudeCodeFolders: vi.fn().mockResolvedValue({ found: false, folders: [] }),
    readClaudeCode: vi.fn().mockResolvedValue({ questions: 0, heldOut: 0, messages: 0, privateLeftOut: 0, unreadable: 0, first: null, last: null }),
    chooseClaudeaiExport: vi.fn().mockResolvedValue(null),
    claudeaiExportStatus: vi.fn().mockResolvedValue(null),
    listClaudeaiChats: vi.fn().mockResolvedValue({ chats: [], mine: 0, notMine: 0, unmarked: 0 }),
    markClaudeaiChats: vi.fn().mockResolvedValue({ chats: [], mine: 0, notMine: 0, unmarked: 0 }),
    readClaudeai: vi.fn().mockResolvedValue({ chats: 0, messages: 0, privateLeftOut: 0 }),
    mergeHistories: vi.fn().mockResolvedValue({ questions: 0, heldOut: 0, messages: 0, duplicates: 0, privateLeftOut: 0 }),
    claudeCodeStatus: vi.fn().mockResolvedValue({ state: 'off', onName: null, onSlug: null, outOfDate: false, changed: false }),
    turnOn: vi.fn().mockResolvedValue({ message: 'Ari is on.' }),
    turnOff: vi.fn().mockResolvedValue({ message: 'Ari is off.' }),
    claudeAiFiles: vi.fn().mockResolvedValue({ files: [] }),
    makeClaudeAiFiles: vi.fn().mockResolvedValue({ files: ['instructions.md', 'decisions-scope.md'] }),
    showClaudeAiFiles: vi.fn().mockResolvedValue(undefined),
    forgetPreview: vi.fn().mockResolvedValue({ files: 0 }),
    forget: vi.fn().mockResolvedValue({ message: 'Deleted 7 files.' }),
    buildAssistant: vi.fn().mockResolvedValue({ answered: 2, heldOut: 0, tested: true, spend: { spentUsd: 1, dailyCapUsd: 50 } }),
    stopBuild: vi.fn().mockResolvedValue(undefined),
    spendToday: vi.fn().mockResolvedValue({ spentUsd: 0, dailyCapUsd: 50 }),
    spendingLimit: vi.fn().mockResolvedValue({ dailyUsd: 50, callUsd: 5 }),
    setSpendingLimit: vi.fn().mockImplementation(async request => ({ dailyUsd: request.dailyUsd, callUsd: request.callUsd })),
    interviewStatus: vi.fn().mockResolvedValue({ answered: 0, thinBelow: 150, thinKinds: [], questions: [], answers: [], kept: false }),
    draftInterview: vi.fn().mockResolvedValue({ answered: 0, thinBelow: 150, thinKinds: [], questions: [], answers: [], kept: false }),
    saveInterviewAnswers: vi.fn().mockResolvedValue({ answered: 0, thinBelow: 150, thinKinds: [], questions: [], answers: [], kept: false }),
    keepInterviewAnswers: vi.fn().mockResolvedValue({ kept: 2, unanswered: 18, privateLeftOut: 1 }),
    ...overrides,
  }
}
const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()) })
export function screen<T extends Parameters<typeof mount>[0]>(component: T, backend?: Backend, props?: object): ReturnType<typeof mount<T>>
export function screen(component: Parameters<typeof mount>[0], backend = fakeBackend(), props = {}) {
  const wrapper = mount(component, { props, global: { provide: { [backendKey]: backend } } })
  wrappers.push(wrapper)
  return wrapper
}
export function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
export function button(wrapper: ReturnType<typeof mount>, text: string) {
  const result = wrapper.findAll('button').find(item => item.text() === text)
  if (!result) throw new Error(`Missing button: ${text}`)
  return result
}

export const consentText = {
  "version": 1,
  "title": "Before Persona Builder reads anything",
  "sections": [
    {
      "heading": "What it reads",
      "lines": [
        "The messages you typed to Claude Code on this computer, including your answers to its questions.",
        "Your claude.ai chats, only if you download them and add them yourself."
      ]
    },
    {
      "heading": "What stays on this computer",
      "lines": [
        "Your history, and everything Persona Builder makes from it, stays in your Claude folder on this computer.",
        "It leaves out messages that look private, such as health, family, legal or money matters and account numbers. It can miss some."
      ]
    },
    {
      "heading": "What goes to Claude",
      "lines": [
        "To build your assistant, Persona Builder sends parts of your history to Claude through your own Claude Code, signed in to your own Claude account. The terms of your Claude account apply, as they do to anything else you send through Claude Code.",
        "This uses your own Claude plan's usage, and a build can use a lot of it. Persona Builder stops for the day when it reaches a daily limit, and you can stop it at any time."
      ]
    },
    {
      "heading": "Nothing else",
      "lines": [
        "Persona Builder sends nothing anywhere else. A copy goes to GitHub only if you choose that later.",
        "You can change your mind: Forget deletes the history it read and everything it made from it."
      ]
    }
  ]
}

export const source = dirname(fileURLToPath(import.meta.url))
export function files(folder: string): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap(entry => {
    const path = join(folder, entry.name)
    return entry.isDirectory() ? files(path) : [path]
  })
}
