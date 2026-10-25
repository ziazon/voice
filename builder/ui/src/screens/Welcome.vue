<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import type { AppInfo } from '../types/generated/AppInfo'

const backend = useBackend()
const info = ref<AppInfo>()
const failed = ref(false)

onMounted(async () => {
  try {
    info.value = await backend.appInfo()
  } catch {
    failed.value = true
  }
})
</script>

<template>
  <div v-if="failed" class="alert alert-danger" role="alert">
    Persona Builder couldn't start properly. Close it and open it again.
  </div>
  <section v-else-if="info" class="card">
    <div class="card-body p-5">
      <h1 class="mb-3">Persona Builder</h1>
      <p class="fs-3">Make an AI assistant that decides the way you do, from your own Claude history.</p>
      <p>Your history stays on this computer. Nothing is saved anywhere else unless you choose it.</p>
      <p class="text-secondary mb-0">Version {{ info.version }}</p>
    </div>
  </section>
  <div v-else class="spinner-border" role="status">
    <span class="visually-hidden">Starting…</span>
  </div>
</template>
