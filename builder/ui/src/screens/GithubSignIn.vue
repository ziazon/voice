<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Said } from '../types/generated/Said'

const emit = defineEmits<{ 'signed-in': [answer: Said] }>()
const backend = useBackend()
const busy = ref(false)
const code = ref('')
const error = ref('')
const linkError = ref('')
const message = ref('')
const SIGN_IN = "GitHub sign-in didn't work. Try again later, or keep your assistant on this computer only."
// Each attempt owns its replies. A cancelled attempt cannot update a later one.
let attempt = 0
async function signIn() {
  if (busy.value) return
  const current = ++attempt
  busy.value = true
  error.value = ''
  linkError.value = ''
  message.value = ''
  try {
    const answer = await backend.githubSignInStart()
    if (current !== attempt) return
    code.value = answer.userCode
    const finished = await backend.githubSignInFinish()
    if (current !== attempt) return
    message.value = finished.message
    emit('signed-in', finished)
  } catch (failure) {
    if (current === attempt) error.value = failureMessage(failure, SIGN_IN)
  } finally {
    if (current === attempt) {
      busy.value = false
      code.value = ''
    }
  }
}
async function openPage() {
  linkError.value = ''
  try {
    await backend.openLink({ link: 'githubDevice' })
  } catch (failure) {
    linkError.value = failureMessage(failure, "Persona Builder couldn't open your web browser. Go to https://github.com/login/device yourself.")
  }
}
async function cancel() {
  ++attempt
  try {
    await backend.githubSignInCancel()
  } catch (failure) {
    error.value = failureMessage(failure, SIGN_IN)
  } finally {
    busy.value = false
    code.value = ''
    linkError.value = ''
  }
}
onBeforeUnmount(() => {
  ++attempt
  if (busy.value) void backend.githubSignInCancel().catch(() => {})
})
</script>

<template>
  <div class="mb-4">
    <template v-if="code">
      <p>Go to github.com/login/device and enter this code:</p>
      <p data-code class="font-monospace fs-1">{{ code }}</p>
      <button type="button" class="btn btn-outline-secondary mb-3" @click="openPage">Open GitHub</button>
      <div v-if="linkError" class="alert alert-warning small" role="alert">{{ linkError }}</div>
      <div class="d-flex align-items-center gap-2 mb-3" role="status">
        <span class="spinner-border spinner-border-sm" aria-hidden="true" />
        <span>Waiting for you to approve Persona Builder on GitHub…</span>
      </div>
      <button type="button" class="btn btn-link p-0" @click="cancel">Cancel</button>
    </template>
    <template v-else-if="!message">
      <p>To save to GitHub, sign in to your GitHub account first.</p>
      <button type="button" class="btn btn-primary" :disabled="busy" @click="signIn">Sign in to GitHub</button>
    </template>
    <div v-if="message" class="alert alert-success mt-3" role="alert">{{ message }}</div>
    <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
  </div>
</template>
