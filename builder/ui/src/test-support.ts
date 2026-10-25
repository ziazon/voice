import { mount } from '@vue/test-utils'
import { afterEach, vi } from 'vitest'
import { backendKey, type Backend } from './backend'
import type { Assistant } from './types/generated/Assistant'
import type { SaveOptions } from './types/generated/SaveOptions'

export const assistant: Assistant = { slug: 'example', name: 'Example', principal: 'Sample', consent: { agreed: false, agreedOn: null } }
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
  return {
    appInfo: vi.fn().mockResolvedValue({ version: '1.2.3', agents: [] }),
    listAssistants: vi.fn().mockResolvedValue([]),
    newAssistant: vi.fn().mockResolvedValue(assistant),
    showAssistantFolder: vi.fn().mockResolvedValue(undefined),
    consentText: vi.fn().mockResolvedValue(consentText),
    agreeConsent: vi.fn().mockResolvedValue({ agreed: true, agreedOn: '2026-10-25' }),
    claudeCheck: vi.fn().mockResolvedValue({ ready: true, message: 'Claude Code is ready.', note: null }),
    saveOptions: vi.fn().mockResolvedValue(saveOptions),
    openLink: vi.fn().mockResolvedValue(undefined),
    githubSignInStart: vi.fn().mockResolvedValue({ userCode: 'ABCD-EFGH' }),
    githubSignInFinish: vi.fn().mockResolvedValue({ message: 'You are signed in to GitHub.' }),
    githubSignInCancel: vi.fn().mockResolvedValue(undefined),
    githubSignOut: vi.fn().mockResolvedValue({ message: 'You are signed out of GitHub.' }),
    saveChoose: vi.fn().mockResolvedValue({ message: 'Your choice is kept.' }),
    savePush: vi.fn().mockResolvedValue({ message: 'Your copy is saved.' }),
    claudeCodeFolders: vi.fn().mockResolvedValue({ found: false, folders: [] }),
    readClaudeCode: vi.fn().mockResolvedValue({ questions: 0, heldOut: 0, messages: 0, privateLeftOut: 0, first: null, last: null }),
    chooseClaudeaiExport: vi.fn().mockResolvedValue(null),
    claudeaiExportStatus: vi.fn().mockResolvedValue(null),
    listClaudeaiChats: vi.fn().mockResolvedValue({ chats: [], mine: 0, notMine: 0, unmarked: 0 }),
    markClaudeaiChats: vi.fn().mockResolvedValue({ chats: [], mine: 0, notMine: 0, unmarked: 0 }),
    readClaudeai: vi.fn().mockResolvedValue({ chats: 0, messages: 0, privateLeftOut: 0 }),
    mergeHistories: vi.fn().mockResolvedValue({ questions: 0, heldOut: 0, messages: 0, duplicates: 0, privateLeftOut: 0 }),
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
