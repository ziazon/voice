<script setup lang="ts">
import { computed, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'

const emit = defineEmits<{ made: [assistant: Assistant]; cancel: [] }>()
const backend = useBackend()
const name = ref('')
const principal = ref('')
const pronouns = ref('they/them')
const principalPronouns = ref('they/them')
const otherPronouns = ref('')
const otherPrincipalPronouns = ref('')
const saving = ref(false)
const error = ref('')
const request = computed(() => ({
  name: name.value.trim(),
  pronouns: (pronouns.value === 'other' ? otherPronouns.value : pronouns.value).trim(),
  principal: principal.value.trim(),
  principalPronouns: (principalPronouns.value === 'other' ? otherPrincipalPronouns.value : principalPronouns.value).trim(),
}))
const incomplete = computed(() => Object.values(request.value).some(value => !value))
async function make() {
  if (saving.value || incomplete.value) return
  saving.value = true
  error.value = ''
  try {
    emit('made', await backend.newAssistant(request.value))
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't make the assistant's folder. Check there's space on this computer, then try again.")
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body p-5">
      <h2 class="mb-4">Make a new assistant</h2>
      <form @submit.prevent="make">
        <div class="mb-3">
          <label for="assistant-name" class="form-label">Your assistant's name</label>
          <input id="assistant-name" v-model="name" type="text" class="form-control" required aria-describedby="assistant-name-help">
          <div id="assistant-name-help" class="form-hint">Pick any name. You'll call it by this name.</div>
        </div>
        <div class="mb-3">
          <label for="assistant-pronouns" class="form-label">Your assistant's pronouns</label>
          <select id="assistant-pronouns" v-model="pronouns" class="form-select">
            <option>they/them</option><option>she/her</option><option>he/him</option><option value="other">Other…</option>
          </select>
          <template v-if="pronouns === 'other'">
            <label for="assistant-pronouns-other" class="form-label mt-2">Your assistant's pronouns</label>
            <input id="assistant-pronouns-other" v-model="otherPronouns" type="text" class="form-control" required>
          </template>
        </div>
        <div class="mb-3">
          <label for="principal-name" class="form-label">Your name</label>
          <input id="principal-name" v-model="principal" type="text" class="form-control" required autocomplete="name" aria-describedby="principal-name-help">
          <div id="principal-name-help" class="form-hint">Your assistant learns how you decide, from your own history.</div>
        </div>
        <div class="mb-4">
          <label for="principal-pronouns" class="form-label">How your assistant refers to you</label>
          <select id="principal-pronouns" v-model="principalPronouns" class="form-select">
            <option>they/them</option><option>she/her</option><option>he/him</option><option value="other">Other…</option>
          </select>
          <template v-if="principalPronouns === 'other'">
            <label for="principal-pronouns-other" class="form-label mt-2">How your assistant refers to you</label>
            <input id="principal-pronouns-other" v-model="otherPrincipalPronouns" type="text" class="form-control" required>
          </template>
        </div>
        <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
        <div class="btn-list">
          <button type="submit" class="btn btn-primary" :disabled="saving || incomplete">{{ saving ? 'Making it…' : 'Make it' }}</button>
          <button type="button" class="btn btn-outline-secondary" @click="emit('cancel')">Back</button>
        </div>
      </form>
    </div>
  </section>
</template>
