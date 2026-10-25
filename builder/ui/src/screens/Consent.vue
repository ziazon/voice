<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { Consent } from '../types/generated/Consent'
import type { ConsentText } from '../types/generated/ConsentText'

const props = defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ agreed: [consent: Consent]; next: [] }>()
const backend = useBackend()
const text = ref<ConsentText>()
const loading = ref(true)
const saving = ref(false)
const error = ref('')
onMounted(async () => {
  try {
    text.value = await backend.consentText()
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't show this. Close it and open it again.")
  } finally {
    loading.value = false
  }
})
async function agree() {
  if (saving.value) return
  saving.value = true
  error.value = ''
  try {
    emit('agreed', await backend.agreeConsent({ slug: props.assistant.slug }))
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't keep your answer. Check there's space on this computer, then try again.")
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body p-5">
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Loading…</span></div>
      <template v-else-if="text">
        <h2 class="mb-4">{{ text.title }}</h2>
        <section v-for="section in text.sections" :key="section.heading" class="mb-4">
          <h3>{{ section.heading }}</h3>
          <p v-for="line in section.lines" :key="line">{{ line }}</p>
        </section>
        <template v-if="assistant.consent.agreed">
          <p class="text-secondary">You agreed on {{ assistant.consent.agreedOn }}.</p>
          <button type="button" class="btn btn-primary" @click="emit('next')">Next: connect Claude</button>
        </template>
        <button v-else type="button" class="btn btn-primary" :disabled="saving" @click="agree">{{ saving ? 'Saving…' : 'I agree' }}</button>
      </template>
      <div v-if="error" class="alert alert-danger mt-3" role="alert">{{ error }}</div>
    </div>
  </section>
</template>
