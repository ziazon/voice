<script setup lang="ts">
import type { Assistant } from '../types/generated/Assistant'
import ClaudeCode from './sources/ClaudeCode.vue'
import Claudeai from './sources/Claudeai.vue'
import PutTogether from './sources/PutTogether.vue'

defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ next: [] }>()
</script>

<template>
  <section>
    <h2 class="mb-3">What {{ assistant.name }} learns from</h2>
    <p>Your assistant learns from what you've written to Claude. Persona Builder reads it on this computer and keeps it here.</p>
    <div class="d-flex flex-column gap-4">
      <ClaudeCode :slug="assistant.slug" />
      <Claudeai :slug="assistant.slug" />
      <PutTogether :slug="assistant.slug" />
    </div>
    <button type="button" class="btn btn-primary mt-4" @click="emit('next')">Next: build your assistant</button>
  </section>
</template>
