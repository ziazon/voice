<script setup lang="ts">
import type { ReviewStatus } from '../../types/generated/ReviewStatus'

defineProps<{ status: ReviewStatus }>()
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">Tests from your answers</h3>
      <p v-if="status.tests.waiting === 0 && status.tests.inUse === 0 && status.tests.retired === 0">You haven't given your assistant any answers to learn from yet.</p>
      <template v-else>
        <p>{{ status.tests.waiting.toLocaleString() }} waiting for a refresh, {{ status.tests.inUse.toLocaleString() }} in use, {{ status.tests.retired.toLocaleString() }} retired.</p>
        <ul class="list-unstyled">
          <li v-for="entry in status.byArea" :key="entry.area">{{ entry.area }}: {{ entry.tests.waiting.toLocaleString() }} waiting for a refresh, {{ entry.tests.inUse.toLocaleString() }} in use, {{ entry.tests.retired.toLocaleString() }} retired.</li>
        </ul>
      </template>
    </div>
  </section>
</template>
