<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { SpendingLimit } from '../types/generated/SpendingLimit'
import type { ClaudeStatus } from '../types/generated/ClaudeStatus'

const props = defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ next: [] }>()
const backend = useBackend()
const status = ref<ClaudeStatus>()
const loading = ref(true)
async function check() {
  loading.value = true
  status.value = undefined
  try {
    status.value = await backend.claudeCheck()
  } catch (failure) {
    status.value = {
      ready: false,
      message: failureMessage(failure, "Persona Builder couldn't check Claude Code. Close it and open it again."),
      note: null,
    }
  } finally {
    loading.value = false
  }
}
const limit = ref<SpendingLimit>()
const day = ref('')
const step = ref('')
const loadingLimit = ref(true)
const savingLimit = ref(false)
const limitError = ref('')
const savedMessage = ref('')
const limitSaveFailure = "Persona Builder couldn't save the limit. Check there's space on this computer, then try again."
const limitLoadFailure = "Persona Builder couldn't read the limit. Close it and open it again."
const amountProblem = 'Enter an amount like 50.00, or 0 for no limit.'
const dayProblem = computed(() => {
  const amount = dollars(day.value)
  if (amount === undefined) return amountProblem
  return amount > 1000 ? 'The daily limit can be at most $1,000. Enter 0 for no limit.' : ''
})
const stepProblem = computed(() => {
  const amount = dollars(step.value)
  if (amount === undefined) return amountProblem
  if (amount > 1000) return 'One step can use at most $1,000. Enter 0 for no limit.'
  const daily = dollars(day.value)
  return daily !== undefined && daily > 0 && amount > 0 && amount > daily
    ? "One step can't use more than the daily limit." : ''
})

function showLimit(answer: SpendingLimit) {
  limit.value = answer
  day.value = answer.dailyUsd.toFixed(2)
  step.value = answer.callUsd.toFixed(2)
}
async function loadLimit() {
  try {
    showLimit(await backend.spendingLimit({ slug: props.assistant.slug }))
  } catch (failure) {
    limitError.value = failureMessage(failure, limitLoadFailure)
  } finally {
    loadingLimit.value = false
  }
}
function dollars(value: string): number | undefined {
  const amount = value.trim().replace(/^\$/, '')
  const parsed = Number(amount)
  return /^\d+(\.\d{1,2})?$/.test(amount) && Number.isFinite(parsed) ? parsed : undefined
}
async function saveLimit() {
  if (loadingLimit.value || savingLimit.value || !!dayProblem.value || !!stepProblem.value) return
  limitError.value = ''
  savedMessage.value = ''
  const dailyUsd = dollars(day.value)
  const callUsd = dollars(step.value)
  if (dailyUsd === undefined || callUsd === undefined) return
  savingLimit.value = true
  try {
    const answer = await backend.setSpendingLimit({ slug: props.assistant.slug, dailyUsd, callUsd })
    showLimit(answer)
    savedMessage.value = answer.dailyUsd === 0
      ? answer.callUsd === 0
        ? "Saved. There's no limit."
        : `Saved. There's no daily limit, and one step may use at most $${answer.callUsd.toFixed(2)}.`
      : answer.callUsd === 0
        ? `Saved. Persona Builder stops for the day at about $${answer.dailyUsd.toFixed(2)}, with no separate limit for one step.`
        : `Saved. Persona Builder stops for the day at about $${answer.dailyUsd.toFixed(2)}, and one step may use at most $${answer.callUsd.toFixed(2)}.`
  } catch (failure) {
    limitError.value = failureMessage(failure, limitSaveFailure)
  } finally {
    savingLimit.value = false
  }
}
onMounted(check)
onMounted(loadLimit)
</script>

<template>
  <section class="card">
    <div class="card-body p-5">
      <h2 class="mb-3">Connect Claude</h2>
      <p>Persona Builder works through your own Claude Code, signed in to your own Claude account. It checks that now, and you set how much it may use in a day.</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Checking Claude Code…</span></div>
      <template v-else-if="status">
        <div class="alert" :class="status.ready ? 'alert-success' : 'alert-warning'" role="alert">{{ status.message }}</div>
        <p v-if="status.ready && status.note" class="text-secondary">{{ status.note }}</p>
        <button v-if="status.ready" type="button" class="btn btn-primary" @click="emit('next')">Next: your history</button>
        <button v-if="!status.ready" type="button" class="btn btn-primary" @click="check">Check again</button>
      </template>
      <p class="text-secondary mt-3 mb-0">Persona Builder never asks for your Claude password: you sign in to Claude Code yourself. Claude is the only choice for now. Other AI providers come later.</p>
    </div>
  </section>
  <section class="card mt-3">
    <div class="card-body p-5">
      <h2 class="mb-3">How much it may use</h2>
      <p>The amounts are Claude Code's own estimates.</p>
      <div v-if="limitError" class="alert alert-danger" role="alert">{{ limitError }}</div>
      <template v-if="limit">
        <div class="row g-3">
          <div class="col-md-6">
            <label for="limit-day" class="form-label">Stop for the day at about</label>
            <div class="input-group">
              <span class="input-group-text">$</span>
              <input id="limit-day" v-model="day" class="form-control" :class="{ 'is-invalid': !!dayProblem }" :aria-invalid="!!dayProblem" aria-describedby="limit-day-problem" inputmode="decimal" :disabled="loadingLimit || savingLimit" />
              <div id="limit-day-problem" class="invalid-feedback">{{ dayProblem }}</div>
            </div>
          </div>
          <div class="col-md-6">
            <label for="limit-step" class="form-label">Most one step may use</label>
            <div class="input-group">
              <span class="input-group-text">$</span>
              <input id="limit-step" v-model="step" class="form-control" :class="{ 'is-invalid': !!stepProblem }" :aria-invalid="!!stepProblem" aria-describedby="limit-step-problem" inputmode="decimal" :disabled="loadingLimit || savingLimit" />
              <div id="limit-step-problem" class="invalid-feedback">{{ stepProblem }}</div>
            </div>
          </div>
        </div>
        <p class="mt-3">Enter 0 in either box for no limit.</p>
        <p class="mt-3">If the build reaches the limit, it carries on tomorrow. Persona Builder checks the limit before every step, so it stops before it goes over.</p>
        <button type="button" class="btn btn-primary" :disabled="loadingLimit || savingLimit || !!dayProblem || !!stepProblem" @click="saveLimit">Save the limit</button>
        <p v-if="savedMessage" class="mt-3 mb-0" role="status">{{ savedMessage }}</p>
      </template>
    </div>
  </section>
</template>
