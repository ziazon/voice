<script setup lang="ts">
import { computed } from 'vue'
import type { ReadProgress } from '../../types/generated/ReadProgress'

const props = defineProps<{ progress: ReadProgress | null, label: string }>()
const percent = computed(() => props.progress
  ? (props.progress.total === 0 ? 100 : Math.floor(props.progress.done * 100 / props.progress.total))
  : 0)
</script>

<template>
  <div class="progress my-3">
    <div class="progress-bar" :class="{ 'progress-bar-indeterminate': progress === null }"
      :style="progress ? { width: `${percent}%` } : undefined" role="progressbar"
      :aria-valuenow="progress ? percent : undefined" aria-valuemin="0" aria-valuemax="100" :aria-label="label" />
  </div>
</template>
