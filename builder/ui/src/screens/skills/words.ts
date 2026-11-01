import type { SkillRow } from '../../types/generated/SkillRow'
import type { SkillsRefreshed } from '../../types/generated/SkillsRefreshed'
import type { MirrorAfter } from '../../types/generated/MirrorAfter'

export const words = {
  title: 'Your skills',
  introduction: 'Your assistant can keep its own copy of the skills on this computer, with private details taken out. It copies only the skills you say yes to.',
  loading: 'Looking for the skills on this computer…',
  loadFailure: "Voice couldn't look through the skills on this computer. Try again.",
  retry: 'Try again',
  empty: 'No skills found on this computer.',
  yes: 'Say yes',
  stop: 'Stop copying',
  agreed: 'Your assistant may copy it now. Bring its copy up to date to add it.',
  saveFailure: "Voice couldn't save your choice. Try again.",
  refresh: 'Bring the copy up to date',
  refreshing: "Bringing your assistant's copy up to date…",
  refreshFailure: "Voice couldn't bring your assistant's copy up to date. Try again.",
}
export function waitingWords(count: number): string {
  return count === 1 ? '1 skill waits for your yes.' : `${count} skills wait for your yes.`
}
export function stateWords(skill: SkillRow): string {
  switch (skill.state) {
    case 'waiting': return 'Waiting for your yes'
    case 'agreed': return `You said yes on ${skill.agreedOn}`
    case 'moved': return 'Moved since you said yes. Say yes again to copy it.'
    case 'notOffered': return "A claude.ai skill. It's copied only if you say yes."
    case 'neverCopied': return "Never copied: it's on your never-copy list."
  }
}
export function actionLabel(skill: SkillRow): string {
  return skill.state === 'agreed' ? `Stop copying ${skill.name}` : `Say yes to ${skill.name}`
}
export function numberWords(number: number): string { return `No. ${number}` }
export function withdrawnWords(mirror: MirrorAfter): string {
  switch (mirror) {
    case 'notThere': return "Your assistant won't copy it any more."
    case 'removed': return "Your assistant won't copy it any more, and its copy of your skills was removed so nothing you stopped stays in it. Bring the copy up to date to make it again."
    case 'notRemoved': return "Your assistant won't copy it any more, but Voice couldn't remove its copy of your skills, so it may still hold what you stopped. To see why, run voice skills refresh in a terminal."
  }
}
export function refreshWords(report: SkillsRefreshed): string {
  return `Copied ${report.copied} of ${report.agreed} skills. Since the last refresh: ${report.new} new, ${report.updated} updated, ${report.unchanged} unchanged, ${report.removed} removed.`
}
