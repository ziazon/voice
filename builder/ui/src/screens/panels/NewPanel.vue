<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { Assistant } from '../../types/generated/Assistant'
import type { ConvenePanelRequest } from '../../types/generated/ConvenePanelRequest'
import { TIME_LIMITS } from './words'

const emit = defineEmits<{ start: [ConvenePanelRequest]; cancel: [] }>()
const backend = useBackend()
const assistants = ref<Assistant[]>([])
const loading = ref(true)
const error = ref('')
const question = ref('')
const background = ref('')
const kind = ref('text')
const items = ref<number | string>(3)
const selected = ref<string[]>([])
const timeBox = ref<string | null>(null)
const questionCount = computed(() => [...question.value.trim()].length)
const backgroundBytes = computed(() => new TextEncoder().encode(background.value).length)
const seats = computed(() => assistants.value.filter(item => selected.value.includes(item.slug)).map(item => item.slug))
const valid = computed(() => !loading.value && !error.value && questionCount.value >= 1 && questionCount.value <= 200
  && backgroundBytes.value <= 4096 && seats.value.length >= 2 && seats.value.length <= 24
  && (kind.value === 'text' || (typeof items.value === 'number' && Number.isInteger(items.value) && items.value >= 1 && items.value <= 12)))
onMounted(async () => {
  try {
    assistants.value = await backend.listAssistants()
  } catch (failure) {
    error.value = failureMessage(failure, "Your assistants couldn't be read. Close Persona Builder and open it again.")
  } finally {
    loading.value = false
  }
})
function start() {
  if (!valid.value) return
  emit('start', { question: question.value.trim(), background: background.value,
    kind: kind.value === 'text' ? 'text' : `list:${items.value}`, seats: seats.value, timeBox: timeBox.value })
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <p v-if="loading" role="status">Loading your assistants…</p>
      <p v-else-if="error" class="alert alert-danger" role="alert">{{ error }}</p>
      <p v-else-if="assistants.length < 2">Make at least two assistants first. A panel needs two or more.</p>
      <form @submit.prevent="start">
        <div class="mb-3">
          <label for="panel-question" class="form-label">The question</label>
          <input id="panel-question" v-model="question" class="form-control" aria-describedby="panel-question-count">
          <p id="panel-question-count" class="form-hint">{{ questionCount }} of 200 characters</p>
        </div>
        <div class="mb-3">
          <label for="panel-background" class="form-label">Anything they should know (optional)</label>
          <textarea id="panel-background" v-model="background" class="form-control" rows="4" aria-describedby="panel-background-count" />
          <p id="panel-background-count" class="form-hint">{{ backgroundBytes }} of 4096 (some letters and symbols count as more than one)</p>
        </div>
        <fieldset class="mb-3">
          <legend class="form-label">The kind of answer</legend>
          <label class="form-check"><input v-model="kind" type="radio" value="text" class="form-check-input"><span class="form-check-label">One written answer</span></label>
          <label class="form-check"><input v-model="kind" type="radio" value="list" class="form-check-input"><span class="form-check-label">A list</span></label>
          <template v-if="kind === 'list'">
            <label for="panel-items" class="form-label">How many items</label>
            <input id="panel-items" v-model.number="items" type="number" min="1" max="12" class="form-control">
          </template>
        </fieldset>
        <fieldset class="mb-3">
          <legend class="form-label">Who sits on the panel</legend>
          <div v-for="assistant in assistants" :key="assistant.slug" class="mb-2">
            <label class="form-check"><input v-model="selected" type="checkbox" :value="assistant.slug" :disabled="!assistant.consent.agreed" class="form-check-input"><span class="form-check-label">{{ assistant.name }}</span></label>
            <p v-if="!assistant.consent.agreed" class="text-secondary">Needs your permission first</p>
          </div>
        </fieldset>
        <fieldset class="mb-3">
          <legend class="form-label">How long they may take</legend>
          <label v-for="limit in TIME_LIMITS" :key="limit.label" class="form-check">
            <input v-model="timeBox" type="radio" :value="limit.value" class="form-check-input"><span class="form-check-label">{{ limit.label }}</span>
          </label>
        </fieldset>
        <p>Each assistant's turns use your Claude plan and count toward that assistant's daily limit.</p>
        <button type="submit" class="btn btn-primary" :disabled="!valid">Start the panel</button>
        <button type="button" class="btn btn-link" @click="emit('cancel')">Cancel</button>
      </form>
    </div>
  </section>
</template>
