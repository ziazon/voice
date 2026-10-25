<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { ClaudeCodeStatus } from '../types/generated/ClaudeCodeStatus'
const props = defineProps<{ assistant: Assistant }>()
const backend = useBackend()
const status = ref<ClaudeCodeStatus>()
const loading = ref(true)
const action = ref<'on' | 'off' | ''>('')
const busy = computed(() => loading.value || !!action.value)
const loadError = ref('')
const error = ref('')
const message = ref('')
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    status.value = await backend.claudeCodeStatus({ slug: props.assistant.slug })
  } catch (failure) {
    loadError.value = failureMessage(failure, "Persona Builder couldn't check Claude Code. Close it and open it again.")
  } finally { loading.value = false }
}
onMounted(load)
async function turnOn() {
  if (busy.value) return
  action.value = 'on'
  message.value = ''
  error.value = ''
  try {
    message.value = (await backend.turnOn({ slug: props.assistant.slug })).message
  } catch (failure) {
    error.value = failureMessage(failure, `Persona Builder couldn't turn ${props.assistant.name} on, so nothing in your Claude folder was changed.`)
  }
  await load()
  action.value = ''
}
async function turnOff() {
  const slug = status.value?.onSlug
  if (busy.value || !slug) return
  action.value = 'off'
  message.value = ''
  error.value = ''
  try {
    message.value = (await backend.turnOff({ slug })).message
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't turn the assistant off. Close it and open it again, then try again.")
  }
  await load()
  action.value = ''
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">Use it in Claude Code</h2>
      <p>Turn {{ assistant.name }} on, and Claude Code asks {{ assistant.name }} before it asks you a question. One assistant can be on at a time.</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Checking Claude Code…</span></div>
      <template v-else-if="loadError">
        <div class="alert alert-warning" role="alert">{{ loadError }}</div>
        <button type="button" class="btn btn-primary" :disabled="busy" @click="load">Try again</button>
      </template>
      <template v-else-if="status">
        <p v-if="status.state === 'notSetUp'">Claude Code isn't set up on this computer yet. Install it and sign in, then come back.</p>
        <template v-else-if="status.state === 'off'">
          <p>{{ assistant.name }} is off in Claude Code.</p>
          <button type="button" class="btn btn-primary" :disabled="busy" @click="turnOn">{{ action === 'on' ? 'Turning it on…' : `Turn ${assistant.name} on` }}</button>
        </template>
        <template v-else-if="status.state === 'this'">
          <p>{{ assistant.name }} is on in Claude Code.</p>
          <p v-if="status.outOfDate">{{ assistant.name }} has changed since you turned it on.</p>
          <p v-if="status.changed" class="text-secondary">Something Persona Builder added for {{ assistant.name }} in your Claude folder was changed or removed.</p>
          <div class="btn-list">
            <button v-if="status.outOfDate" type="button" class="btn btn-primary" :disabled="busy" @click="turnOn">{{ action === 'on' ? 'Updating…' : `Update ${assistant.name} in Claude Code` }}</button>
            <button type="button" class="btn btn-outline-secondary" :disabled="busy" @click="turnOff">{{ action === 'off' ? 'Turning it off…' : `Turn ${assistant.name} off` }}</button>
          </div>
        </template>
        <template v-else-if="status.state === 'other'">
          <p>{{ status.onName || 'Another assistant' }} is on in Claude Code. Only one assistant can be on at a time.</p>
          <button type="button" class="btn btn-outline-secondary" :disabled="busy" @click="turnOff">{{ action === 'off' ? 'Turning it off…' : status.onName ? `Turn ${status.onName} off` : 'Turn it off' }}</button>
        </template>
      </template>
      <div v-if="message" class="alert alert-success mt-3" role="alert">{{ message }}</div>
      <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
    </div>
  </section>
</template>
