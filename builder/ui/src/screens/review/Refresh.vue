<script setup lang="ts">
import { ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { ReviewStatus } from '../../types/generated/ReviewStatus'
import type { Spend } from '../../types/generated/Spend'
import ReadProgress from '../sources/ReadProgress.vue'

const props = defineProps<{ slug: string, status: ReviewStatus }>()
const emit = defineEmits<{ changed: [ReviewStatus], running: [boolean] }>()
const backend = useBackend()
const busy = ref(false)
const drafting = ref(false)
const testing = ref(false)
const spend = ref<Spend | null>(null)
const lines = ref<string[]>([])
const error = ref('')
const outcome = ref('')
const kept = ref(false)
async function reload() {
  try {
    emit('changed', await backend.reviewStatus({ slug: props.slug }))
  } catch (failure) {
    error.value = failureMessage(failure, "Something in this assistant's settings stops the review. Use the persona-builder command to see what.")
  }
}
async function draft() {
  if (busy.value || !props.status.built || !props.status.tests.waiting) return
  busy.value = true
  drafting.value = true
  kept.value = false
  error.value = ''
  outcome.value = ''
  lines.value = []
  try {
    const result = await backend.draftRefresh({ slug: props.slug })
    spend.value = result.spend
    if (result.change) await reload()
    else outcome.value = "There's nothing new to learn from yet. Keep answering your assistant, then try again."
  } catch (failure) {
    error.value = failureMessage(failure, "Claude's draft didn't fit, twice, so nothing was saved. Try again.")
  } finally {
    busy.value = false
    drafting.value = false
  }
}
async function answer(approve: boolean) {
  if (busy.value || !props.status.change) return
  busy.value = true
  error.value = ''
  outcome.value = ''
  try {
    const status = await backend.answerRefresh({ slug: props.slug, id: props.status.change.id, approve })
    if (!approve) outcome.value = 'Rejected. Nothing changed. Your answers stay saved for the next refresh.'
    kept.value = false
    emit('changed', status)
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't save your answer. Check there's space on this computer, then try again.")
  } finally {
    busy.value = false
  }
}
async function test() {
  if (busy.value || !props.status.change?.approved) return
  busy.value = true
  testing.value = true
  error.value = ''
  outcome.value = ''
  lines.value = []
  emit('running', true)
  try {
    const result = await backend.applyRefresh({ slug: props.slug, id: props.status.change.id }, progress => {
      lines.value.push(progress.line)
      spend.value = progress.spend
    })
    spend.value = result.spend
    kept.value = result.kept
    outcome.value = result.kept
      ? `Kept. Your assistant now works this way. ${result.reason}`
      : `Undone. Your assistant is as it was, and your answers stay saved for the next refresh. ${result.reason}`
    await reload()
  } catch (failure) {
    error.value = failureMessage(failure, 'The test stopped before it finished, so your assistant is as it was. What was already finished is kept: test again to carry on.')
  } finally {
    busy.value = false
    testing.value = false
    emit('running', false)
  }
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">Refresh</h3>
      <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
      <div v-if="outcome" class="alert" :class="kept ? 'alert-success' : 'alert-info'" role="alert">{{ outcome }}</div>
      <template v-if="!status.change">
        <p>A refresh drafts a change to how your assistant decides, from the answers waiting for it. You read the change and approve or reject it first.</p>
        <ReadProgress v-if="drafting" :progress="null" label="Drafting" />
        <button type="button" class="btn btn-primary" :disabled="busy || !status.built || !status.tests.waiting" @click="draft">{{ drafting ? 'Drafting a change…' : 'Draft a change' }}</button>
        <p v-if="!status.built" class="text-secondary mt-2">Build your assistant first.</p>
        <p v-else-if="!status.tests.waiting" class="text-secondary mt-2">Keep an answer first: there's nothing new to learn from.</p>
      </template>
      <template v-else>
        <p class="text-secondary">Drafted {{ status.change.drafted }}.</p>
        <p><strong>{{ status.change.summary }}</strong></p>
        <p class="text-secondary">Learned from {{ status.change.learnedFrom.toLocaleString() }} of your answers.</p>
        <p v-if="status.change.approved">You approved this change. Next, Persona Builder tests it on all your answers: it keeps the change only if every new test passes and none of the others gets worse. It uses a little of your Claude plan.</p>
        <div v-for="(point, index) in status.change.points" :key="index" class="mb-3">
          <h4>{{ (index + 1).toLocaleString() }}. {{ point.why }}</h4>
          <div class="row">
            <div class="col-6">
              <small class="text-secondary">Before</small>
              <ul v-if="point.before.length" class="list-unstyled"><li v-for="(line, n) in point.before" :key="n">{{ line }}</li></ul>
              <p v-else class="text-secondary">Nothing: this is new.</p>
            </div>
            <div class="col-6">
              <small class="text-secondary">After</small>
              <ul v-if="point.after.length" class="list-unstyled"><li v-for="(line, n) in point.after" :key="n">{{ line }}</li></ul>
              <p v-else class="text-secondary">Taken out.</p>
            </div>
          </div>
        </div>
        <template v-if="status.change.approved">
          <ReadProgress v-if="testing" :progress="null" label="Testing" />
          <button type="button" class="btn btn-primary" :disabled="busy" @click="test">{{ testing ? 'Testing the change…' : 'Test it' }}</button>
        </template>
        <template v-else>
          <div class="d-flex gap-2">
            <button type="button" class="btn btn-primary" :disabled="busy" @click="answer(true)">Approve</button>
            <button type="button" class="btn btn-secondary" :disabled="busy" @click="answer(false)">Reject</button>
          </div>
          <p class="text-secondary mt-2 mb-0">Approve: your assistant changes only after the change is tested on all your answers and nothing gets worse. Reject: nothing changes, and your answers stay saved for the next refresh.</p>
        </template>
      </template>
      <div v-if="lines.length" class="overflow-auto" style="max-height: 16rem">
        <ul class="list-unstyled" aria-live="polite"><li v-for="(line, index) in lines" :key="index">{{ line }}</li></ul>
      </div>
      <p v-if="spend" class="mt-2">Today, about ${{ spend.spentUsd.toFixed(2) }}<template v-if="spend.dailyCapUsd === 0">, with no daily limit</template><template v-else> of the ${{ spend.dailyCapUsd.toFixed(2) }} daily limit</template>, as Claude Code estimates it.</p>
    </div>
  </section>
</template>
