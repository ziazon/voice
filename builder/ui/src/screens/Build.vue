<script setup lang="ts">
import { ref } from 'vue'
import type { Assistant } from '../types/generated/Assistant'
import BuildRun from './build/BuildRun.vue'
import Interview from './build/Interview.vue'

defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ running: [boolean], report: [] }>()
const run = ref<InstanceType<typeof BuildRun> | null>(null)
const interviewKey = ref(0)
</script>

<template>
  <section>
    <h2 class="mb-3">Build your assistant</h2>
    <p>Persona Builder sends your history to Claude through your own Claude Code, a part at a time, and writes how you decide into your assistant. It uses your own Claude plan and stops for the day at the limit below.</p>
    <div class="d-flex flex-column gap-4">
      <BuildRun ref="run" :slug="assistant.slug" @running="emit('running', $event)" @finished="interviewKey++" @report="emit('report')" />
      <Interview :key="interviewKey" :slug="assistant.slug" @rebuild="run?.rebuild()" />
    </div>
  </section>
</template>
