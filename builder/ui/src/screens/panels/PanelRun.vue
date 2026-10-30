<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { ConvenePanelRequest } from '../../types/generated/ConvenePanelRequest'
import type { PanelDetail } from '../../types/generated/PanelDetail'
import type { PanelProgress } from '../../types/generated/PanelProgress'
import type { PanelRan } from '../../types/generated/PanelRan'
import { findingWords, statusWords, TIME_LIMITS } from './words'
import PanelResult from './PanelResult.vue'

const props = defineProps<{ id: string | null, start: ConvenePanelRequest | null }>()
const emit = defineEmits<{ running: [boolean], back: [] }>()
const backend = useBackend()
const id = ref(props.id)
const detail = ref<PanelDetail | null>(null)
const loading = ref(props.id !== null)
const running = ref(false)
const stopping = ref(false)
const stopSent = ref(false)
const lines = ref<string[]>([])
const error = ref('')
const readError = ref('')
const info = ref(false)
const steer = ref('')
async function load() {
  if (!id.value) return
  loading.value = true
  readError.value = ''
  try {
    detail.value = await backend.panel({ id: id.value })
  } catch (failure) {
    readError.value = failureMessage(failure, "This panel couldn't be read. Try again.")
  } finally {
    loading.value = false
  }
}
async function sendStop() {
  if (!id.value || stopSent.value) return
  stopSent.value = true
  try {
    await backend.stopPanel({ id: id.value })
  } catch (failure) {
    stopSent.value = false
    stopping.value = false
    info.value = false
    error.value = failureMessage(failure, "The panel couldn't be asked to stop. Try again.")
  }
}
function progress(value: PanelProgress) {
  if (!id.value) id.value = value.id
  lines.value.push(value.line)
  if (stopping.value) void sendStop()
}
async function run(call: () => Promise<PanelRan>) {
  if (running.value) return
  running.value = true
  stopping.value = false
  stopSent.value = false
  lines.value = []
  error.value = ''
  info.value = false
  emit('running', true)
  try {
    const result = await call()
    id.value = result.id
    if (result.status === 'stopped') {
      info.value = true
      error.value = 'Stopped. You can carry on from here.'
    }
  } catch (failure) {
    info.value = false
    error.value = failureMessage(failure, 'The panel stopped before it finished. You can carry on from where it stopped.')
  } finally {
    await load()
    running.value = false
    stopping.value = false
    emit('running', false)
  }
}
function stop() {
  if (stopping.value) return
  stopping.value = true
  void sendStop()
}
async function stopSaved() {
  if (!id.value || stopping.value) return
  stopping.value = true
  error.value = ''
  info.value = false
  try {
    await backend.stopPanel({ id: id.value })
    await load()
  } catch (failure) {
    error.value = failureMessage(failure, "The panel couldn't be asked to stop. Try again.")
  } finally {
    stopping.value = false
  }
}
function resume(timeBox: string | null = null, reconsider = false) {
  if (!id.value || (reconsider && !steer.value.trim())) return
  const request = { id: id.value, steer: reconsider ? steer.value.trim() : null, timeBox }
  void run(() => backend.resumePanel(request, progress))
}
onMounted(() => {
  const start = props.start
  if (start) void run(() => backend.convenePanel(start, progress))
  else void load()
})
</script>

<template>
  <section class="card">
    <div class="card-body">
      <p v-if="loading" role="status">Loading the panel…</p>
      <p v-if="readError" class="alert alert-danger" role="alert">{{ readError }}</p>
      <div v-if="error" class="alert" :class="info ? 'alert-info' : 'alert-danger'" role="alert">{{ error }}</div>
      <div v-if="lines.length" class="overflow-auto" style="max-height: 16rem">
        <ul class="list-unstyled" aria-live="polite">
          <li v-for="(line, index) in lines" :key="index" class="text-break" style="white-space: pre-wrap">{{ line }}</li>
        </ul>
      </div>
      <template v-if="running">
        <button type="button" class="btn btn-outline-danger" :disabled="stopping" @click="stop">{{ stopping ? 'Stopping…' : 'Stop for now' }}</button>
        <p class="text-secondary">Stopping lets the assistants finish the turns they are on. You can carry on later.</p>
      </template>
      <template v-if="detail">
        <h2 class="text-break" style="white-space: pre-wrap">{{ detail.summary.question }}</h2>
        <p v-if="!running">{{ statusWords(detail.summary.status, detail.summary.round) }}</p>
        <p class="text-break">{{ detail.seats.join(', ') }}</p>
        <div v-for="round in detail.rounds" :key="round.round" class="mb-3">
          <p>Round {{ round.round }}: how far apart they were, {{ round.d.toFixed(2) }} (0 means they all agree).</p>
          <p v-for="(withheld, index) in round.withheld" :key="index" class="text-break">{{ withheld.seat }}'s post was kept from the others: it had {{ withheld.findings.map(findingWords).join(', ') }}.</p>
          <p v-if="round.failed.length" class="text-break">Couldn't answer this round: {{ round.failed.join(', ') }}.</p>
        </div>
        <template v-if="!running">
          <div v-if="detail.summary.status === 'paused' && detail.asked" class="mb-3">
            <h3>How long may they take?</h3>
            <template v-for="limit in TIME_LIMITS" :key="limit.label">
              <button v-if="limit.value !== null" type="button" class="btn btn-primary me-2 mb-2" @click="resume(limit.value)">{{ limit.label }}</button>
            </template>
          </div>
          <button v-if="['stopped', 'failed', 'running'].includes(detail.summary.status) || (detail.summary.status === 'paused' && !detail.asked)" type="button" class="btn btn-primary" :disabled="stopping" @click="resume()">Carry on</button>
          <template v-if="detail.summary.status === 'running'">
            <button type="button" class="btn btn-outline-danger ms-2" :disabled="stopping" @click="stopSaved">{{ stopping ? 'Stopping…' : 'Stop for now' }}</button>
            <p class="text-secondary">If this panel is still running somewhere, stop it first, then carry on.</p>
          </template>
          <template v-if="detail.summary.status === 'closed'">
            <PanelResult v-if="detail.result" :result="detail.result" />
            <h3 class="mt-4">Ask them to reconsider</h3>
            <label for="panel-steer" class="form-label">What should they think about again?</label>
            <textarea id="panel-steer" v-model="steer" class="form-control mb-3" rows="4" />
            <button type="button" class="btn btn-primary" :disabled="!steer.trim()" @click="resume(null, true)">Reconsider</button>
          </template>
        </template>
      </template>
      <button type="button" class="btn btn-link" :disabled="running" @click="emit('back')">Back to your panels</button>
    </div>
  </section>
</template>
