import { mount } from '@vue/test-utils'
import { afterEach, vi } from 'vitest'
import { backendKey, type Backend } from './backend'
import type { Assistant } from './types/generated/Assistant'

export const assistant: Assistant = { slug: 'example', name: 'Example', principal: 'Sample', consent: { agreed: false, agreedOn: null } }
export function fakeBackend(overrides: Partial<Backend> = {}): Backend {
  return {
    appInfo: vi.fn().mockResolvedValue({ version: '1.2.3', agents: [] }),
    listAssistants: vi.fn().mockResolvedValue([]),
    newAssistant: vi.fn().mockResolvedValue(assistant),
    showAssistantFolder: vi.fn().mockResolvedValue(undefined),
    consentText: vi.fn().mockResolvedValue(consentText),
    agreeConsent: vi.fn().mockResolvedValue({ agreed: true, agreedOn: '2026-10-25' }),
    claudeCheck: vi.fn().mockResolvedValue({ ready: true, message: 'Claude Code is ready.', note: null }),
    ...overrides,
  }
}
const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()) })
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
