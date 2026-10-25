<script setup lang="ts">
import type { Assistant } from '../types/generated/Assistant'
import ClaudeCode from './sources/ClaudeCode.vue'
import Claudeai from './sources/Claudeai.vue'
import PutTogether from './sources/PutTogether.vue'

defineProps<{ assistant: Assistant }>()
</script>

<template>
  <section>
    <h2 class="mb-3">Your history</h2>
    <p>Your assistant learns from what you've written to Claude. Persona Builder reads it on this computer and keeps it here.</p>
    <div class="d-flex flex-column gap-4">
      <ClaudeCode :slug="assistant.slug" />
      <Claudeai :slug="assistant.slug" />
      <PutTogether :slug="assistant.slug" />
    </div>
  </section>
</template>
