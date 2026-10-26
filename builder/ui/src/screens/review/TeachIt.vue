<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { ReviewStatus } from '../../types/generated/ReviewStatus'

const props = defineProps<{ slug: string, status: ReviewStatus }>()
const emit = defineEmits<{ changed: [ReviewStatus] }>()
const backend = useBackend()
const area = ref(props.status.areas[0] ?? '')
const situation = ref('')
const wanted = ref('')
const assistantSaid = ref('')
const busy = ref(false)
const error = ref('')
const kept = ref(false)
const ready = computed(() => situation.value.trim() !== '' && wanted.value.trim() !== '')
watch(() => props.status.areas, areas => {
  if (!areas.includes(area.value)) area.value = areas[0] ?? ''
})
async function keep() {
  if (busy.value || !ready.value) return
  busy.value = true
  error.value = ''
  kept.value = false
  try {
    const status = await backend.keepAnswer({ slug: props.slug, area: area.value, situation: situation.value, wanted: wanted.value, assistantSaid: assistantSaid.value })
    situation.value = ''
    wanted.value = ''
    assistantSaid.value = ''
    kept.value = true
    emit('changed', status)
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't keep that answer. Check it and try again.")
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">Teach it something</h3>
      <p v-if="!status.areas.length">Your assistant's settings name no kinds of decision yet.</p>
      <template v-else>
        <p>Tell it how you'd decide something, or where it got one wrong. Each answer becomes a test it has to pass.</p>
        <div v-if="kept" class="alert alert-success" role="alert">Kept. It's now one of the tests your assistant is checked on at its next refresh.</div>
        <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
        <form @submit.prevent="keep">
          <fieldset :disabled="busy">
            <div class="mb-3">
              <label :for="`${slug}-review-area`" class="form-label">Kind of decision</label>
              <select :id="`${slug}-review-area`" v-model="area" class="form-select">
                <option v-for="choice in status.areas" :key="choice" :value="choice">{{ choice }}</option>
              </select>
            </div>
            <div class="mb-3">
              <label :for="`${slug}-review-situation`" class="form-label">What was being decided</label>
              <textarea :id="`${slug}-review-situation`" v-model="situation" class="form-control" rows="2" maxlength="2000" />
            </div>
            <div class="mb-3">
              <label :for="`${slug}-review-wanted`" class="form-label">What you'd have done</label>
              <textarea :id="`${slug}-review-wanted`" v-model="wanted" class="form-control" rows="2" maxlength="2000" />
            </div>
            <div class="mb-3">
              <label :for="`${slug}-review-said`" class="form-label">What your assistant said, if it got this wrong (optional)</label>
              <textarea :id="`${slug}-review-said`" v-model="assistantSaid" class="form-control" rows="2" maxlength="2000" />
            </div>
            <p class="form-hint">Leave out anything private, such as someone's name. It's checked before it's kept.</p>
            <button type="submit" class="btn btn-primary" :disabled="busy || !ready">Keep my answer</button>
          </fieldset>
        </form>
      </template>
    </div>
  </section>
</template>
