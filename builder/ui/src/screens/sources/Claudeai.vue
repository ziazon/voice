<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { ExportStatus } from '../../types/generated/ExportStatus'
import type { ChatList as Chats } from '../../types/generated/ChatList'
import type { ClaudeaiCounts } from '../../types/generated/ClaudeaiCounts'
import type { ReadProgress as Progress } from '../../types/generated/ReadProgress'
import ChatList from './ChatList.vue'
import ReadProgress from './ReadProgress.vue'

const props = defineProps<{ slug: string }>()
const backend = useBackend()
const loading = ref(true)
const running = ref(false)
const reading = ref(false)
const status = ref<ExportStatus | null>(null)
const list = ref<Chats | null>(null)
const counts = ref<ClaudeaiCounts | null>(null)
const progress = ref<Progress | null>(null)
const error = ref('')
const fallback = "Persona Builder couldn't read this download. Check it's the file claude.ai sent you, then choose it again."
onMounted(async () => {
  try {
    status.value = await backend.claudeaiExportStatus({ slug: props.slug })
  } catch (failure) {
    error.value = failureMessage(failure, fallback)
  } finally {
    loading.value = false
  }
})
async function choose(folder: boolean) {
  running.value = true
  error.value = ''
  try {
    const answer = await backend.chooseClaudeaiExport({ slug: props.slug, folder })
    if (answer) {
      status.value = answer
      list.value = null
      counts.value = null
    }
  } catch (failure) {
    status.value = null
    list.value = null
    counts.value = null
    error.value = failureMessage(failure, fallback)
  } finally {
    running.value = false
  }
}
async function listChats() {
  running.value = true
  error.value = ''
  try {
    list.value = await backend.listClaudeaiChats({ slug: props.slug })
  } catch (failure) {
    error.value = failureMessage(failure, fallback)
  } finally {
    running.value = false
  }
}
async function read() {
  running.value = true
  reading.value = true
  progress.value = null
  error.value = ''
  counts.value = null
  try {
    counts.value = await backend.readClaudeai({ slug: props.slug }, value => { progress.value = value })
  } catch (failure) {
    error.value = failureMessage(failure, fallback)
  } finally {
    running.value = false
    reading.value = false
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">claude.ai chats</h3>
      <p>If you also chat on claude.ai, you can add those chats.</p>
      <p>On claude.ai, open Settings, then Privacy, and choose Export data. claude.ai emails you a link that works for 24 hours. Download the file, then choose it here.</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Checking your download…</span></div>
      <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
      <div class="d-flex gap-2">
        <button type="button" class="btn btn-secondary" :disabled="loading || running" @click="choose(false)">Choose the download</button>
        <button type="button" class="btn btn-secondary" :disabled="loading || running" @click="choose(true)">Choose a folder</button>
      </div>
      <p class="text-secondary mt-2">If your computer unzipped the download, choose the folder it made.</p>
      <template v-if="status">
        <template v-if="status.ready">
          <p>You chose {{ status.name }}. Persona Builder can read it: {{ status.chats.toLocaleString() }} chats.</p>
          <button type="button" class="btn btn-primary" :disabled="running" @click="listChats">List the chats</button>
        </template>
        <div v-else class="alert alert-warning" role="alert">You chose {{ status.name }}, but it's missing parts Persona Builder reads. Download your data from claude.ai again, then choose the new file.</div>
      </template>
      <template v-if="list">
        <ChatList :slug="slug" :list="list" @update="list = $event" />
        <button type="button" class="btn btn-primary mt-3" :disabled="running || list.mine === 0" @click="read">Read the chats marked yours</button>
      </template>
      <ReadProgress v-if="reading" :progress="progress" label="Reading" />
      <div v-if="counts" class="alert alert-success mt-3" role="alert">Done. Read {{ counts.chats.toLocaleString() }} chats: {{ counts.messages.toLocaleString() }} messages. {{ counts.privateLeftOut.toLocaleString() }} that looked private were left out.</div>
    </div>
  </section>
</template>
