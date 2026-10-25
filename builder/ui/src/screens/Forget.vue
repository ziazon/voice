<script setup lang="ts">
import { computed, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { ForgetPreview } from '../types/generated/ForgetPreview'
const props = defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ forgotten: [] }>()
const backend = useBackend()
const preview = ref<ForgetPreview>()
const checking = ref(false)
const deleting = ref(false)
const busy = computed(() => checking.value || deleting.value)
const error = ref('')
const message = ref('')
const FORGET = "Persona Builder couldn't forget anything, and nothing was deleted. Close it and open it again, then try again."
async function check() {
  if (busy.value) return
  checking.value = true
  error.value = ''
  message.value = ''
  try {
    preview.value = await backend.forgetPreview({ slug: props.assistant.slug })
  } catch (failure) {
    error.value = failureMessage(failure, FORGET)
  } finally { checking.value = false }
}
async function forget() {
  if (busy.value || !preview.value?.files) return
  deleting.value = true
  error.value = ''
  message.value = ''
  try {
    message.value = (await backend.forget({ slug: props.assistant.slug, files: preview.value.files })).message
    emit('forgotten')
  } catch (failure) {
    error.value = failureMessage(failure, FORGET)
  } finally {
    preview.value = undefined
    deleting.value = false
  }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">Forget what {{ assistant.name }} learned from</h2>
      <p>Forget deletes the history {{ assistant.name }} learned from and Persona Builder's working files. {{ assistant.name }} itself stays.</p>
      <template v-if="preview && preview.files === 0">
        <p>There's nothing to forget: Persona Builder hasn't kept anything for {{ assistant.name }} yet.</p>
        <button type="button" class="btn btn-link p-0" :disabled="busy" @click="preview = undefined">Close</button>
      </template>
      <template v-else-if="preview">
        <p>This deletes {{ preview.files === 1 ? '1 file' : `${preview.files} files` }} from your Claude folder:</p>
        <ul>
          <li>the history {{ assistant.name }} learned from</li>
          <li>Persona Builder's working files, including the claude.ai files</li>
          <li>your permission to read your history</li>
          <li>your choice of where to keep a copy of {{ assistant.name }}</li>
        </ul>
        <p>These stay:</p>
        <ul>
          <li>{{ assistant.name }}'s own folder, and everything Persona Builder wrote into it</li>
          <li>any copy of {{ assistant.name }} on GitHub</li>
          <li>{{ assistant.name }} in Claude Code, if it's on</li>
        </ul>
        <p>You can't undo this.</p>
        <div class="btn-list">
          <button type="button" class="btn btn-danger" :disabled="busy" @click="forget">{{ deleting ? 'Deleting…' : 'Delete these files' }}</button>
          <button type="button" class="btn btn-link" :disabled="busy" @click="preview = undefined">Cancel</button>
        </div>
      </template>
      <button v-else type="button" class="btn btn-outline-danger" :disabled="busy" @click="check">{{ checking ? 'Checking…' : 'Forget…' }}</button>
      <div v-if="message" class="alert alert-success mt-3" role="alert">{{ message }}</div>
      <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
    </div>
  </section>
</template>
