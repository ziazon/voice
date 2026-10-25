<script setup lang="ts">
import Welcome from './screens/Welcome.vue'
import { steps } from './steps'
</script>

<template>
  <div class="page">
    <div class="page-body">
      <div class="container-xl">
        <div class="row g-5">
          <aside class="col-3">
            <ol class="ps-3 mb-0">
              <li v-for="step in steps" :key="step.id" aria-disabled="true" class="mb-4 ps-1">
                <span data-step-title class="d-block">{{ step.title }}</span>
                <span class="text-secondary small">Not here yet</span>
              </li>
            </ol>
          </aside>
          <main class="col-9">
            <Welcome />
          </main>
        </div>
      </div>
    </div>
  </div>
</template>
