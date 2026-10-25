<script setup lang="ts">
import { ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { MergeCounts } from '../../types/generated/MergeCounts'

const props = defineProps<{ slug: string }>()
const backend = useBackend()
const running = ref(false)
const counts = ref<MergeCounts | null>(null)
const error = ref('')
async function merge() {
  running.value = true
  error.value = ''
  counts.value = null
  try {
    counts.value = await backend.mergeHistories({ slug: props.slug })
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't put your history together. Close it and open it again.")
  } finally {
    running.value = false
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">Put it together</h3>
      <p>When you've read everything you want, put it together into the one history your assistant learns from.</p>
      <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
      <button type="button" class="btn btn-primary" :disabled="running" @click="merge">{{ running ? 'Putting it together…' : 'Put it together' }}</button>
      <div v-if="counts" class="alert alert-success mt-3" role="alert">Done. Your assistant will learn from {{ counts.questions.toLocaleString() }} answers to Claude's questions and {{ counts.messages.toLocaleString() }} other messages. {{ counts.heldOut.toLocaleString() }} answers are kept aside to test it with. {{ counts.duplicates.toLocaleString() }} repeats were left out.</div>
    </div>
  </section>
</template>
