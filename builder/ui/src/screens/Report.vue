<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { ReportView } from '../types/generated/ReportView'
import type { ReportLevel } from '../types/generated/ReportLevel'
import type { ReportHelp } from '../types/generated/ReportHelp'

const props = defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ build: [], next: [] }>()
const backend = useBackend()
const view = ref<ReportView | null>(null)
const loading = ref(true)
const error = ref('')
const report = computed(() => view.value?.report)
const strong = computed(() => report.value?.areas.filter(area => area.level === 'strong').length ?? 0)
const levels: Record<ReportLevel, { label: string, badge: string }> = {
  strong: { label: 'Strong', badge: 'bg-green-lt' },
  gettingThere: { label: 'Getting there', badge: 'bg-yellow-lt' },
  thin: { label: 'Thin', badge: 'bg-secondary-lt' },
}
const help: Record<ReportHelp, string> = {
  interview: 'Answer the interview',
  claudeai: 'Add your claude.ai chats',
  time: 'Keep using Claude: it learns from every answer',
}
onMounted(async () => {
  try {
    view.value = await backend.showReport({ slug: props.assistant.slug })
  } catch (failure) {
    error.value = failureMessage(failure, "Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <section>
    <h2 class="mb-3">How well it knows you</h2>
    <div v-if="loading" class="spinner-border" role="status">
      <span class="visually-hidden">Reading your report…</span>
    </div>
    <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
    <template v-if="view">
      <template v-if="!report">
        <div v-if="view.unreadable" class="alert alert-warning" role="alert">Your last report couldn't be read, so it isn't shown. Build your assistant again to make a new one.</div>
        <p v-else>There's no report yet. Build your assistant first.</p>
        <button type="button" class="btn btn-primary" @click="emit('build')">Go to Build</button>
      </template>
      <template v-else>
        <div class="card mb-3">
          <div class="card-body">
            <p>Overall: <span class="badge" :class="levels[report.overall].badge">{{ levels[report.overall].label }}</span></p>
            <p class="text-secondary">From the build on {{ report.built }}.</p>
            <p v-if="report.overall === 'thin'">Your history holds {{ report.answered.toLocaleString() }} answered questions. It takes about {{ report.thinBelow.toLocaleString() }} to know you well.</p>
            <p v-else-if="report.test">It passed the test on {{ strong.toLocaleString() }} of {{ report.areas.length.toLocaleString() }} kinds of decision, and got {{ report.test.right.toLocaleString() }} of {{ report.test.of.toLocaleString() }} test questions right.</p>
            <p v-else>There's enough history to learn from, but too little to test yet.</p>
            <template v-if="!report.test">
              <p>Only {{ report.heldOut.toLocaleString() }} questions were kept back to test your assistant, fewer than 30, so there's too little to measure yet. Until it can be tested, it decides nothing on its own.</p>
              <template v-if="report.answered < report.thinBelow">
                <p>The interview fills out a thin history.</p>
                <button type="button" class="btn btn-secondary mb-3" @click="emit('build')">Go to the interview</button>
              </template>
            </template>
            <p v-if="!report.areas.length">No kinds of decision yet.</p>
          </div>
          <div v-if="report.areas.length" class="table-responsive">
            <table class="table table-vcenter card-table">
              <thead>
                <tr><th>Kind of decision</th><th>How well</th><th>What would help</th></tr>
              </thead>
              <tbody>
                <tr v-for="area in report.areas" :key="area.area">
                  <td>{{ area.area }}</td>
                  <td>
                    <span class="badge" :class="levels[area.level].badge">{{ levels[area.level].label }}</span>
                    <small v-if="area.level === 'thin'" class="d-block text-secondary">{{ area.examples.toLocaleString() }} {{ area.examples === 1 ? 'example' : 'examples' }} so far</small>
                  </td>
                  <td>
                    <span v-if="area.level === 'strong'" class="text-secondary">Nothing needed</span>
                    <ul v-else class="list-unstyled mb-0">
                      <li v-for="item in area.help" :key="item">{{ help[item] }}</li>
                    </ul>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-if="view.growth" class="card-body">
            <p class="mb-0">Since the build: {{ view.growth.kept.toLocaleString() }} {{ view.growth.kept === 1 ? 'change' : 'changes' }} kept and {{ view.growth.undone.toLocaleString() }} undone. Your assistant passes {{ view.growth.passing.toLocaleString() }} of {{ view.growth.of.toLocaleString() }} tests from your answers.</p>
          </div>
        </div>
        <button type="button" class="btn btn-primary" @click="emit('next')">Next: review what it learns</button>
      </template>
    </template>
  </section>
</template>
