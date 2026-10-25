<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
const props = defineProps<{ assistant: Assistant }>()
const backend = useBackend()
const files = ref<string[]>([])
const loading = ref(true)
const making = ref(false)
const showing = ref(false)
const busy = computed(() => loading.value || making.value || showing.value)
const loadError = ref('')
const error = ref('')
const showError = ref('')
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    files.value = (await backend.claudeAiFiles({ slug: props.assistant.slug })).files
  } catch (failure) {
    loadError.value = failureMessage(failure, "Persona Builder couldn't look for the claude.ai files. Close it and open it again.")
  } finally { loading.value = false }
}
onMounted(load)
async function make() {
  if (busy.value) return
  making.value = true
  error.value = ''
  showError.value = ''
  try {
    files.value = (await backend.makeClaudeAiFiles({ slug: props.assistant.slug })).files
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't make the claude.ai files. Check there's space on this computer, then try again.")
  } finally { making.value = false }
}
async function show() {
  if (busy.value) return
  showing.value = true
  showError.value = ''
  try {
    await backend.showClaudeAiFiles({ slug: props.assistant.slug })
  } catch (failure) {
    showError.value = failureMessage(failure, "Persona Builder couldn't open the folder.")
  } finally { showing.value = false }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">Use it in claude.ai</h2>
      <p>claude.ai can't read your Claude folder, so Persona Builder makes files you add to a claude.ai Project yourself.</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Looking for the claude.ai files…</span></div>
      <template v-else-if="loadError">
        <div class="alert alert-warning" role="alert">{{ loadError }}</div>
        <button type="button" class="btn btn-primary" :disabled="busy" @click="load">Try again</button>
      </template>
      <template v-else-if="files.length">
        <p>The files are ready:</p>
        <ul><li v-for="file in files" :key="file"><code>{{ file }}</code></li></ul>
        <button type="button" class="btn btn-outline-secondary mb-3" :disabled="busy" @click="show">Show in Finder</button>
        <div v-if="showError" class="alert alert-warning small" role="alert">{{ showError }}</div>
        <ol>
          <li>In claude.ai, make a new Project.</li>
          <li>Open instructions.md and paste what it says into the Project's instructions.</li>
          <li>Add the other files to the Project's knowledge with the + button.</li>
        </ol>
        <p class="text-secondary">If you change {{ assistant.name }}, make the files again and replace them in the Project.</p>
        <button type="button" class="btn btn-outline-primary" :disabled="busy" @click="make">{{ making ? 'Making them…' : 'Make them again' }}</button>
      </template>
      <button v-else type="button" class="btn btn-primary" :disabled="busy" @click="make">{{ making ? 'Making them…' : 'Make the claude.ai files' }}</button>
      <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
    </div>
  </section>
</template>
