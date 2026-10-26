<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { ClaudeCodeFolders } from '../../types/generated/ClaudeCodeFolders'
import type { ReadCounts } from '../../types/generated/ReadCounts'
import type { ReadProgress as Progress } from '../../types/generated/ReadProgress'
import ReadProgress from './ReadProgress.vue'

const props = defineProps<{ slug: string }>()
const backend = useBackend()
const loading = ref(true)
const folders = ref<ClaudeCodeFolders | null>(null)
const error = ref('')
const running = ref<'Counting' | 'Reading' | null>(null)
const progress = ref<Progress | null>(null)
const counts = ref<ReadCounts | null>(null)
const saved = ref(false)
const summary = computed(() => {
  const rows = folders.value?.folders ?? []
  const included = rows.filter(row => row.read)
  const excluded = rows.filter(row => !row.read)
  const conversations = (items: typeof rows) => items.reduce((sum, row) => sum + row.conversations, 0).toLocaleString()
  return `It will read ${included.length.toLocaleString()} folders (${conversations(included)} conversations) and leave out ${excluded.length.toLocaleString()} (${conversations(excluded)} conversations).`
})
const countsLine = computed(() => {
  const answer = counts.value
  if (!answer) return ''
  const dates = answer.first ? `, from ${answer.first} to ${answer.last}` : ''
  const unreadable = answer.unreadable > 0
    ? ` ${answer.unreadable.toLocaleString()} ${answer.unreadable === 1 ? "entry it couldn't read was" : "entries it couldn't read were"} left out.`
    : ''
  return `${saved.value ? 'Read' : 'Counted'} ${answer.questions.toLocaleString()} answered questions and ${answer.messages.toLocaleString()} messages you typed${dates}. ${answer.privateLeftOut.toLocaleString()} that looked private were left out.${unreadable} ${answer.heldOut.toLocaleString()} answered questions are kept back to test your assistant.`
})
onMounted(async () => {
  try {
    folders.value = await backend.claudeCodeFolders({ slug: props.slug })
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't look through your Claude Code history. Close it and open it again.")
  } finally {
    loading.value = false
  }
})
async function read(dryRun: boolean) {
  running.value = dryRun ? 'Counting' : 'Reading'
  progress.value = null
  error.value = ''
  try {
    counts.value = await backend.readClaudeCode({ slug: props.slug, dryRun }, value => { progress.value = value })
    saved.value = !dryRun
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't read your Claude Code history. Close it and open it again.")
  } finally {
    running.value = null
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">Claude Code history</h3>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Looking at your Claude Code history…</span></div>
      <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
      <template v-if="folders">
        <p v-if="!folders.found || !folders.folders.length" class="mb-0">Persona Builder found no Claude Code history on this computer.</p>
        <template v-else>
          <p>{{ summary }}</p>
          <div class="overflow-auto" style="max-height: 16rem">
            <table class="table table-sm table-vcenter">
              <thead><tr><th>Project</th><th>Conversations</th><th>Read?</th></tr></thead>
              <tbody><tr v-for="folder in folders.folders" :key="folder.project"><td>{{ folder.project }}</td><td>{{ folder.conversations.toLocaleString() }}</td><td>{{ folder.read ? 'Yes' : 'Left out' }}</td></tr></tbody>
            </table>
          </div>
          <p class="text-secondary">Your assistant's settings decide which folders are left out.</p>
          <div class="d-flex gap-2">
            <button type="button" class="btn btn-secondary" :disabled="running !== null" @click="read(true)">Count first</button>
            <button v-if="counts" type="button" class="btn btn-primary" :disabled="running !== null" @click="read(false)">Read my Claude Code history</button>
          </div>
          <ReadProgress v-if="running" :progress="progress" :label="running" />
          <p v-if="counts" class="mt-3">{{ countsLine }}</p>
          <div v-if="saved" class="alert alert-success mt-3" role="alert">Done. Your Claude Code history is saved on this computer.</div>
        </template>
      </template>
    </div>
  </section>
</template>
