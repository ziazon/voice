<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { SaveOptions } from '../types/generated/SaveOptions'
import type { Said } from '../types/generated/Said'
import GithubSignIn from './GithubSignIn.vue'

const props = defineProps<{ assistant: Assistant }>()
const backend = useBackend()
const options = ref<SaveOptions>()
const loading = ref(true)
const loadError = ref('')
const linkError = ref('')
const error = ref('')
const message = ref('')
const info = ref('')
const choice = ref(0)
const name = ref('')
const keeping = ref(false)
const saving = ref(false)
const signingOut = ref(false)
const kind = computed(() => options.value?.choices[choice.value]?.kind)
const needsSignIn = computed(() => kind.value !== 'local' && !options.value?.signedIn)
const needsName = computed(() => kind.value === 'github' && !name.value.trim())
async function load() {
  loading.value = true
  loadError.value = ''
  try {
    const firstLoad = !options.value
    options.value = await backend.saveOptions({ slug: props.assistant.slug })
    if (firstLoad || !options.value.choices[choice.value]) choice.value = options.value.selected
    if (firstLoad) name.value = options.value.suggestedName
  } catch (failure) {
    loadError.value = failureMessage(failure, "Persona Builder couldn't check where your assistant can be kept. Close it and open it again.")
  } finally {
    loading.value = false
  }
}
onMounted(load)
async function openSignup() {
  linkError.value = ''
  try {
    await backend.openLink({ link: 'githubSignup' })
  } catch (failure) {
    linkError.value = failureMessage(failure, "Persona Builder couldn't open your web browser. Go to https://github.com/signup yourself.")
  }
}
async function keep() {
  if (keeping.value || saving.value || signingOut.value || needsSignIn.value || needsName.value) return
  keeping.value = true
  error.value = ''
  message.value = ''
  info.value = ''
  try {
    const answer = await backend.saveChoose({ slug: props.assistant.slug, choice: choice.value, name: kind.value === 'github' ? name.value.trim() : null })
    message.value = answer.message
    await load()
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't keep your choice. Close it and open it again, then try again.")
  } finally {
    keeping.value = false
  }
}
async function save() {
  if (saving.value || keeping.value || signingOut.value) return
  saving.value = true
  error.value = ''
  message.value = ''
  info.value = ''
  try {
    message.value = (await backend.savePush({ slug: props.assistant.slug })).message
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't save your assistant. Try again, or keep it on this computer only.")
  } finally {
    saving.value = false
  }
}
async function signOut() {
  if (signingOut.value || keeping.value || saving.value) return
  signingOut.value = true
  error.value = ''
  message.value = ''
  info.value = ''
  try {
    info.value = (await backend.githubSignOut()).message
    if (options.value) options.value.signedIn = false
    await load()
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't sign you out of GitHub. Try again.")
  } finally {
    signingOut.value = false
  }
}
async function signedIn(answer: Said) {
  message.value = answer.message
  if (options.value) options.value.signedIn = true
  await load()
}
</script>

<template>
  <section class="card">
    <div class="card-body p-5">
      <h2 class="mb-3">Keep a copy</h2>
      <p>Persona Builder can keep a copy of {{ assistant.name }} somewhere safe, so you don't lose it if something happens to this computer.</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Looking for places to keep {{ assistant.name }}…</span></div>
      <template v-else-if="loadError">
        <div class="alert alert-warning" role="alert">{{ loadError }}</div>
        <button type="button" class="btn btn-primary" @click="load">Try again</button>
      </template>
      <template v-else-if="options">
        <p v-if="options.chosen" class="text-secondary">Right now: {{ options.chosen }}</p>
        <div class="mb-4">
          <label v-for="(option, index) in options.choices" :key="index" class="form-check">
            <input v-model="choice" type="radio" class="form-check-input" name="save-choice" :value="index" :disabled="keeping || saving || signingOut">
            <span class="form-check-label">{{ option.label }}<span v-if="index === options.selected" class="text-secondary"> (recommended)</span></span>
          </label>
        </div>
        <template v-if="options.note">
          <p>{{ options.note }}</p>
          <button type="button" class="btn btn-outline-secondary mb-3" @click="openSignup">Make a free GitHub account</button>
          <div v-if="linkError" class="alert alert-warning small" role="alert">{{ linkError }}</div>
        </template>
        <div v-if="kind === 'github'" class="mb-4">
          <label for="save-repo-name" class="form-label">Name for the new private repo</label>
          <input id="save-repo-name" v-model="name" type="text" class="form-control" :disabled="keeping || saving || signingOut">
        </div>
        <GithubSignIn v-if="needsSignIn" @signed-in="signedIn" />
        <div class="btn-list mb-4">
          <button type="button" class="btn btn-primary" :disabled="keeping || saving || signingOut || needsSignIn || needsName" @click="keep">{{ keeping ? 'Keeping it…' : 'Keep this choice' }}</button>
          <button v-if="options.chosen" type="button" class="btn btn-outline-primary" :disabled="saving || keeping || signingOut" @click="save">{{ saving ? 'Saving…' : 'Save now' }}</button>
        </div>
        <template v-if="options.signedIn">
          <p class="text-secondary">You're signed in to GitHub on this computer.</p>
          <button type="button" class="btn btn-link p-0" :disabled="signingOut || keeping || saving" @click="signOut">Sign out of GitHub</button>
        </template>
      </template>
      <div v-if="message" class="alert alert-success mt-3" role="alert">{{ message }}</div>
      <div v-if="info" class="alert alert-info mt-3" role="alert">{{ info }}</div>
      <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
    </div>
  </section>
</template>
