<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { BroughtIn } from '../types/generated/BroughtIn'
import type { NewAssistantRequest } from '../types/generated/NewAssistantRequest'
import type { OldFile } from '../types/generated/OldFile'
import OpenSaved from './OpenSaved.vue'

const emit = defineEmits<{ made: [assistant: Assistant]; cancel: []; running: [running: boolean] }>()
const backend = useBackend()
const name = ref('')
const principal = ref('')
const pronouns = ref('they/them')
const principalPronouns = ref('they/them')
const otherPronouns = ref('')
const otherPrincipalPronouns = ref('')
const saving = ref(false)
const openingSaved = ref(false)
const checking = ref(false)
const older = ref<{ name: string; lines: string[]; canBringIn: boolean; files: OldFile[]; ticked: Set<string>; request: NewAssistantRequest } | null>(null)
const broughtIn = ref<BroughtIn | null>(null)
const bringingIn = ref(false)
watch(name, () => { older.value = null })
const error = ref('')
const request = computed(() => ({
  name: name.value.trim(),
  pronouns: (pronouns.value === 'other' ? otherPronouns.value : pronouns.value).trim(),
  principal: principal.value.trim(),
  principalPronouns: (principalPronouns.value === 'other' ? otherPrincipalPronouns.value : principalPronouns.value).trim(),
}))
const incomplete = computed(() => Object.values(request.value).some(value => !value))
async function make() {
  if (saving.value || openingSaved.value || incomplete.value) return
  const submitted = request.value
  saving.value = true
  checking.value = true
  error.value = ''
  older.value = null
  try {
    const existing = await backend.findExisting({ name: submitted.name })
    if (existing.found) {
      if (name.value.trim() === submitted.name) older.value = {
        name: submitted.name,
        lines: existing.lines,
        canBringIn: existing.canBringIn,
        files: existing.files,
        ticked: new Set(existing.files.filter(file => file.problem === null).map(file => file.name)),
        request: submitted,
      }
      return
    }
    checking.value = false
    emit('made', await backend.newAssistant(submitted))
  } catch (failure) {
    error.value = failureMessage(failure, checking.value
      ? "Voice couldn't look for an older assistant with that name. Close it and open it again."
      : "Voice couldn't make the assistant's folder. Check there's space on this computer, then try again.")
  } finally {
    checking.value = false
    saving.value = false
  }
}
async function bringIn() {
  if (saving.value || openingSaved.value || !older.value?.canBringIn) return
  const found = older.value
  const submitted = {
    assistant: found.request,
    files: found.files.filter(file => found.ticked.has(file.name)).map(file => file.name),
  }
  saving.value = true
  bringingIn.value = true
  error.value = ''
  try {
    broughtIn.value = await backend.bringInAssistant(submitted)
    older.value = null
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't back up the older assistant, so it changed nothing. Check there's space on this computer, then try again.")
  } finally {
    bringingIn.value = false
    saving.value = false
  }
}
</script>

<template>
  <div>
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
        <div v-if="older" class="alert alert-warning" role="status">
          <div>
            <h3 class="alert-title">Voice found an older {{ older.name }}</h3>
            <ul><li v-for="line in older.lines" :key="line">{{ line }}</li></ul>
            <fieldset v-if="older.canBringIn && older.files.length" class="mb-3">
              <legend class="form-label">Its own files</legend>
              <div class="form-hint mb-2">Voice keeps its own SKILL.md and member.json. Untick a file to leave it out.</div>
              <div v-for="(file, index) in older.files" :key="file.name" class="form-check">
                <input :id="`old-file-${index}`" v-model="older.ticked" type="checkbox" class="form-check-input" :value="file.name" :disabled="!!file.problem">
                <label :for="`old-file-${index}`" class="form-check-label"><code>{{ file.name }}</code><span v-if="file.problem" class="text-secondary"> {{ file.problem }}</span></label>
                <div v-if="file.name === 'private.json'" class="form-hint">Its privacy settings: what the older assistant hid, and any words it let through.</div>
              </div>
            </fieldset>
            <p v-if="older.canBringIn">Voice made nothing and changed nothing. It can copy everything listed here into a backup folder first, then bring this {{ older.name }}'s history in, or you can pick another name.</p>
            <p v-else class="mb-0">Voice made nothing and changed nothing. Voice can't bring this one in, so pick another name.</p>
            <button v-if="older.canBringIn" type="button" class="btn btn-warning" :disabled="saving || openingSaved" @click="bringIn">{{ bringingIn ? 'Backing it up…' : 'Back it up and bring it in' }}</button>
          </div>
        </div>
        <div v-if="broughtIn" class="alert alert-success" role="status">
          <div>
            <h3 class="alert-title">{{ broughtIn.assistant.name }} is ready</h3>
            <p>Voice brought in the older {{ broughtIn.assistant.name }}'s history<template v-if="broughtIn.files.length"> and its own files: {{ broughtIn.files.join(', ') }}</template>.<template v-if="broughtIn.backup"> A copy of everything it found is in {{ broughtIn.backup }}.</template></p>
            <button type="button" class="btn btn-primary" :disabled="openingSaved" @click="emit('made', broughtIn.assistant)">Open {{ broughtIn.assistant.name }}</button>
          </div>
        </div>
        <div v-else class="btn-list">
          <button type="submit" class="btn btn-primary" :disabled="saving || incomplete || openingSaved">{{ checking ? 'Checking…' : saving && !bringingIn ? 'Making it…' : 'Make it' }}</button>
          <button type="button" class="btn btn-outline-secondary" :disabled="openingSaved" @click="emit('cancel')">Back</button>
        </div>
      </form>
    </div>
  </section>
  <OpenSaved :disabled="saving" @made="emit('made', $event)" @running="openingSaved = $event; emit('running', $event)" />
  </div>
</template>
