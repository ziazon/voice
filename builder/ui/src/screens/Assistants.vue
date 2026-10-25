<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'

const emit = defineEmits<{ open: [assistant: Assistant]; make: [] }>()
const backend = useBackend()
const assistants = ref<Assistant[]>([])
const loading = ref(true)
const error = ref('')
const folderErrors = ref<Record<string, string>>({})

onMounted(async () => {
  try {
    assistants.value = await backend.listAssistants()
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't look for your assistants. Close it and open it again.")
  } finally {
    loading.value = false
  }
})
async function showFolder(assistant: Assistant) {
  delete folderErrors.value[assistant.slug]
  try {
    await backend.showAssistantFolder({ slug: assistant.slug })
  } catch (failure) {
    folderErrors.value[assistant.slug] = failureMessage(failure, "Persona Builder couldn't open the folder.")
  }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-4">Your assistants</h2>
      <div v-if="loading" class="spinner-border" role="status">
        <span class="visually-hidden">Looking for your assistants…</span>
      </div>
      <div v-else-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
      <template v-else>
        <p v-if="assistants.length === 0">You haven't made an assistant yet.</p>
        <div v-else class="list-group mb-4">
          <div v-for="assistant in assistants" :key="assistant.slug" class="list-group-item">
            <strong>{{ assistant.name }}</strong>
            <p class="mb-1">Learns from {{ assistant.principal }}</p>
            <p class="text-secondary">{{ assistant.consent.agreed ? 'Permission given' : 'Needs your permission' }}</p>
            <div class="btn-list">
              <button type="button" class="btn btn-outline-primary" @click="emit('open', assistant)">Open</button>
              <button type="button" class="btn btn-outline-secondary" @click="showFolder(assistant)">Show in Finder</button>
            </div>
            <div v-if="folderErrors[assistant.slug]" class="alert alert-danger small mt-3 mb-0" role="alert">{{ folderErrors[assistant.slug] }}</div>
          </div>
        </div>
      </template>
      <button type="button" class="btn btn-primary mt-3" @click="emit('make')">Make a new one</button>
    </div>
  </section>
</template>
