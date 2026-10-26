<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { ReviewStatus } from '../types/generated/ReviewStatus'
import TeachIt from './review/TeachIt.vue'
import Tests from './review/Tests.vue'
import Refresh from './review/Refresh.vue'

const props = defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ running: [boolean] }>()
const backend = useBackend()
const status = ref<ReviewStatus | null>(null)
const loading = ref(true)
const error = ref('')
onMounted(async () => {
  try {
    status.value = await backend.reviewStatus({ slug: props.assistant.slug })
  } catch (failure) {
    error.value = failureMessage(failure, "Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <section>
    <h2 class="mb-3">Review</h2>
    <p>Teach your assistant from your own answers. Persona Builder turns what it learns into a change you can read, and keeps it only if your assistant does at least as well on every test.</p>
    <div v-if="loading" class="spinner-border" role="status">
      <span class="visually-hidden">Reading your answers…</span>
    </div>
    <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
    <div v-if="status" class="d-flex flex-column gap-4">
      <TeachIt :slug="assistant.slug" :status="status" @changed="status = $event" />
      <Tests :slug="assistant.slug" :status="status" @changed="status = $event" />
      <Refresh :slug="assistant.slug" :status="status" @changed="status = $event" @running="emit('running', $event)" />
    </div>
  </section>
</template>
