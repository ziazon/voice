<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { Permissions } from '../types/generated/Permissions'
import { capitalFirst } from './permissions/words'
import { words, whereWords, noPermissionsWords, askingWords, answerWords, countWords } from './ask/words'

const emit = defineEmits<{ running: [boolean] }>()
const backend = useBackend()
const assistants = ref<Assistant[]>([])
const loading = ref(true)
const assistantsError = ref('')
const slug = ref('')
const chosen = computed(() => assistants.value.find(assistant => assistant.slug === slug.value))
const permissions = ref<Permissions | null>(null)
const permissionsLoading = ref(false)
const permissionsError = ref('')
const allowed = computed(() => permissions.value?.permissions.filter(permission => permission.on) ?? [])
const ticked = ref<string[]>([])
const question = ref('')
const count = computed(() => [...question.value.trim()].length)
const asking = ref(false)
const answer = ref('')
const said = ref('')
const error = ref('')
let permissionRequest = 0
const valid = computed(() => chosen.value?.consent.agreed && permissions.value !== null
  && !permissionsLoading.value && !permissionsError.value && count.value >= 1 && count.value <= 2000 && !asking.value)

onMounted(async () => {
  try { assistants.value = await backend.listAssistants() }
  catch (failure) { assistantsError.value = failureMessage(failure, words.assistantsFailure) }
  finally { loading.value = false }
})
async function loadPermissions() {
  const request = ++permissionRequest
  permissionsLoading.value = true
  permissionsError.value = ''
  permissions.value = null
  try {
    const result = await backend.listPermissions({ slug: slug.value })
    if (request === permissionRequest) permissions.value = result
  } catch (failure) {
    if (request === permissionRequest) permissionsError.value = failureMessage(failure, words.permissionsFailure)
  } finally {
    if (request === permissionRequest) permissionsLoading.value = false
  }
}
function choose() {
  ticked.value = []
  answer.value = ''
  said.value = ''
  error.value = ''
  return loadPermissions()
}
async function ask() {
  if (!valid.value) return
  asking.value = true
  emit('running', true)
  answer.value = ''
  said.value = ''
  error.value = ''
  try {
    const result = await backend.askAssistant({ slug: slug.value, question: question.value.trim(),
      capabilities: allowed.value.filter(permission => ticked.value.includes(permission.id)).map(permission => permission.id) })
    answer.value = result.answer
    said.value = result.said
  } catch (failure) { error.value = failureMessage(failure, words.askFailure) }
  finally {
    asking.value = false
    emit('running', false)
  }
}
onBeforeUnmount(() => {
  permissionRequest++
  if (asking.value) emit('running', false)
})
</script>

<template>
  <section class="card">
    <div class="card-body p-5">
      <h2 class="card-title">{{ words.title }}</h2>
      <p>{{ words.introduction }}</p>
      <p v-if="loading" role="status">{{ words.loading }}</p>
      <p v-else-if="assistantsError" class="alert alert-danger" role="alert">{{ assistantsError }}</p>
      <p v-else-if="assistants.length === 0">{{ words.empty }}</p>
      <form @submit.prevent="ask">
        <fieldset v-if="assistants.length" class="mb-3">
          <legend class="form-label">{{ words.who }}</legend>
          <div v-for="assistant in assistants" :key="assistant.slug" class="mb-2">
            <label class="form-check">
              <input v-model="slug" class="form-check-input" type="radio" name="ask-assistant" :value="assistant.slug" :disabled="asking || !assistant.consent.agreed" @change="choose">
              <span class="form-check-label">{{ assistant.name }}</span>
            </label>
            <p v-if="!assistant.consent.agreed" class="text-secondary">{{ words.consent }}</p>
          </div>
        </fieldset>
        <template v-if="chosen">
          <p v-if="permissionsLoading" role="status">{{ words.permissionsLoading }}</p>
          <template v-else-if="permissionsError">
            <p class="alert alert-warning" role="alert">{{ permissionsError }}</p>
            <button type="button" class="btn btn-outline-secondary btn-sm" :disabled="asking" @click="loadPermissions">{{ words.retry }}</button>
          </template>
          <template v-else-if="permissions">
            <fieldset class="mb-3">
              <legend class="form-label">{{ words.permissions }}</legend>
              <div v-for="permission in allowed" :key="permission.id" class="mb-2">
                <label class="form-check">
                  <input v-model="ticked" class="form-check-input" type="checkbox" :value="permission.id" :disabled="asking || (permission.id === 'calendar.read' && (!!permissions.calendarProblem || !permissions.calendar))">
                  <span class="form-check-label">{{ capitalFirst(permission.words) }}</span>
                </label>
                <p class="form-hint">{{ `${capitalFirst(permission.window ?? '')}.` }}</p>
                <p v-if="permission.id === 'calendar.read' && (permissions.calendarProblem || !permissions.calendar)" class="text-warning small">{{ permissions.calendarProblem || words.noCalendar }}</p>
              </div>
              <p v-if="!allowed.length" class="text-secondary">{{ noPermissionsWords(chosen.name) }}</p>
            </fieldset>
            <p aria-live="polite">{{ whereWords(chosen.name, ticked) }}</p>
          </template>
        </template>
        <div class="mb-3">
          <label for="ask-question" class="form-label">{{ words.question }}</label>
          <textarea id="ask-question" v-model="question" class="form-control" rows="4" aria-describedby="ask-question-count" :disabled="asking" />
          <p id="ask-question-count" class="form-hint">{{ countWords(count) }}</p>
        </div>
        <p class="text-secondary small">{{ words.spending }}</p>
        <button type="submit" class="btn btn-primary" :disabled="!valid">{{ words.ask }}</button>
        <p v-if="asking && chosen" role="status">{{ askingWords(chosen.name) }}</p>
        <p v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</p>
      </form>
      <template v-if="answer && chosen">
        <h3 class="mt-4">{{ answerWords(chosen.name) }}</h3>
        <div class="mt-4 text-break" style="white-space: pre-wrap" data-answer>{{ answer }}</div>
        <p class="text-secondary small mt-2" data-said>{{ said }}</p>
      </template>
    </div>
  </section>
</template>
