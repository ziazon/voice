<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { ConvenePanelRequest } from '../types/generated/ConvenePanelRequest'
import type { PanelSummary } from '../types/generated/PanelSummary'
import NewPanel from './panels/NewPanel.vue'
import PanelRun from './panels/PanelRun.vue'
import { statusWords } from './panels/words'

const emit = defineEmits<{ running: [boolean] }>()
const backend = useBackend()
const view = ref<'list' | 'new' | 'run'>('list')
const panels = ref<PanelSummary[]>([])
const loading = ref(true)
const error = ref('')
const id = ref<string | null>(null)
const start = ref<ConvenePanelRequest | null>(null)
async function load() {
  loading.value = true
  error.value = ''
  try {
    panels.value = await backend.listPanels()
  } catch (failure) {
    error.value = failureMessage(failure, "Your panels couldn't be read. Try again.")
  } finally {
    loading.value = false
  }
}
function open(panelId: string) {
  id.value = panelId
  start.value = null
  view.value = 'run'
}
function convene(request: ConvenePanelRequest) {
  id.value = null
  start.value = request
  view.value = 'run'
}
function back() {
  view.value = 'list'
  void load()
}
onMounted(load)
</script>

<template>
  <section v-if="view === 'list'" class="card">
    <div class="card-body">
      <h2 class="card-title">Panels</h2>
      <p>Ask several of your assistants one question. They work it out in rounds and come back with a joint answer, with any disagreement in full.</p>
      <button type="button" class="btn btn-primary mb-3" @click="view = 'new'">Ask a new panel</button>
      <p v-if="loading" role="status">Loading your panels…</p>
      <p v-else-if="error" class="alert alert-danger" role="alert">{{ error }}</p>
      <p v-else-if="!panels.length">No panels yet.</p>
      <ul v-else class="list-unstyled">
        <li v-for="panel in panels" :key="panel.id" class="mb-3">
          <button type="button" class="btn btn-outline-secondary text-start d-block w-100" @click="open(panel.id)">
            <span class="d-block text-break" style="white-space: pre-wrap">{{ panel.question }}</span>
            <span class="d-block">{{ statusWords(panel.status, panel.round) }}</span>
          </button>
        </li>
      </ul>
    </div>
  </section>
  <NewPanel v-else-if="view === 'new'" @start="convene" @cancel="view = 'list'" />
  <PanelRun v-else :id="id" :start="start" @back="back" @running="emit('running', $event)" />
</template>
