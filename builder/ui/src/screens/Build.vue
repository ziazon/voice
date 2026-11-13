<script setup lang="ts">
import { ref } from 'vue'
import type { Assistant } from '../types/generated/Assistant'
import BuildRun from './build/BuildRun.vue'
import Interview from './build/Interview.vue'

defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ running: [boolean], report: [] }>()
const run = ref<InstanceType<typeof BuildRun> | null>(null)
const interviewKey = ref(0)
const subscription = ref(false)
</script>

<template>
  <section>
    <h2 class="mb-3">Build {{ assistant.name }}</h2>
    <p>Voice sends your history to Claude through your own Claude Code, a part at a time, and learns how you decide. <template v-if="subscription">It uses your own Claude plan, and stops for the day when Claude Code says your plan is nearly used up, unless you turn on Keep going below.</template><template v-else>It uses your own Claude plan, up to the daily limit you set on Connect Claude, unless you turn on Keep going below.</template> You can leave it running, or stop and carry on later from where it got to.</p>
    <div class="d-flex flex-column gap-4">
      <BuildRun ref="run" :slug="assistant.slug" :name="assistant.name" @subscription="subscription = $event" @running="emit('running', $event)" @finished="interviewKey++" @report="emit('report')" />
      <Interview :key="interviewKey" :slug="assistant.slug" @rebuild="run?.rebuild()" />
    </div>
  </section>
</template>
