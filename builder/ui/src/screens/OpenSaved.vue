<script setup lang="ts">
import { ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { SavedOnGithub } from '../types/generated/SavedOnGithub'
import type { SavedAssistant } from '../types/generated/SavedAssistant'
import type { OpenedSaved } from '../types/generated/OpenedSaved'
import type { Assistant } from '../types/generated/Assistant'
import GithubSignIn from './GithubSignIn.vue'

const props = defineProps<{ disabled: boolean }>()
const emit = defineEmits<{ made: [assistant: Assistant]; running: [running: boolean] }>()
const backend = useBackend()
const saved = ref<SavedOnGithub>()
const looking = ref(false)
const error = ref('')
const opening = ref<SavedAssistant | null>(null)
const opened = ref<OpenedSaved | null>(null)
async function look() {
  if (props.disabled || looking.value || opening.value) return
  looking.value = true
  error.value = ''
  try {
    saved.value = await backend.savedOnGithub()
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't look in your GitHub repos. Try again.")
  } finally {
    looking.value = false
  }
}
async function open(item: SavedAssistant) {
  if (props.disabled || looking.value || opening.value || item.here) return
  opening.value = item
  error.value = ''
  emit('running', true)
  try {
    opened.value = await backend.openSaved({ owner: item.owner, repo: item.repo, slug: item.slug })
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't open that saved assistant. Try again.")
  } finally {
    opening.value = null
    emit('running', false)
  }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">Open one you saved on GitHub</h2>
      <p>Already saved an assistant to GitHub from another computer? Open it here, and this computer takes each new version.</p>
      <div v-if="opened" class="alert alert-success" role="status">
        <h3 class="alert-title">{{ opened.assistant.name }} is ready</h3>
        <p>{{ opened.message }}</p>
        <button type="button" class="btn btn-primary" :disabled="disabled || looking || !!opening" @click="emit('made', opened.assistant)">Next: give permission</button>
      </div>
      <template v-else>
        <button type="button" class="btn btn-outline-primary" :disabled="disabled || looking || !!opening" @click="look">{{ looking ? 'Looking…' : saved ? 'Look again' : 'Look on GitHub' }}</button>
        <div v-if="looking" class="spinner-border" role="status"><span class="visually-hidden">Looking in your private GitHub repos…</span></div>
        <template v-else-if="saved">
          <fieldset v-if="!saved.signedIn" :disabled="disabled" class="border-0 p-0 m-0">
            <GithubSignIn lead="To open an assistant you saved, sign in to your GitHub account first." @signed-in="look" />
          </fieldset>
          <p v-else-if="!saved.items.length" class="text-secondary">Voice found no saved assistants in your private GitHub repos.</p>
          <ul v-else class="list-unstyled">
            <li v-for="item in saved.items" :key="`${item.owner}/${item.repo}/${item.slug}`" class="d-flex align-items-center justify-content-between gap-3 mb-3">
              <div><strong>{{ item.name }}</strong><span class="d-block text-secondary small">{{ item.owner }}/{{ item.repo }}</span></div>
              <span v-if="item.here" class="text-secondary small">Already on this computer</span>
              <button v-else type="button" class="btn btn-primary" :disabled="disabled || looking || !!opening" @click="open(item)">{{ opening === item ? 'Opening…' : `Open ${item.name}` }}</button>
            </li>
          </ul>
        </template>
      </template>
      <div v-if="error" class="alert alert-warning" role="alert">{{ error }}</div>
    </div>
  </section>
</template>
