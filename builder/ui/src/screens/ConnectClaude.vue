<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { ClaudeStatus } from '../types/generated/ClaudeStatus'

const backend = useBackend()
const status = ref<ClaudeStatus>()
const loading = ref(true)
async function check() {
  loading.value = true
  status.value = undefined
  try {
    status.value = await backend.claudeCheck()
  } catch (failure) {
    status.value = {
      ready: false,
      message: failureMessage(failure, "Persona Builder couldn't check Claude Code. Close it and open it again."),
      note: null,
    }
  } finally {
    loading.value = false
  }
}
onMounted(check)
</script>

<template>
  <section class="card">
    <div class="card-body p-5">
      <h2 class="mb-3">Connect Claude</h2>
      <p>Persona Builder works through your own Claude Code, signed in to your own Claude account. It never asks for your Claude password.</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Checking Claude Code…</span></div>
      <template v-else-if="status">
        <div class="alert" :class="status.ready ? 'alert-success' : 'alert-warning'" role="alert">{{ status.message }}</div>
        <p v-if="status.ready && status.note" class="text-secondary">{{ status.note }}</p>
        <button v-if="!status.ready" type="button" class="btn btn-primary" @click="check">Check again</button>
      </template>
    </div>
  </section>
</template>
