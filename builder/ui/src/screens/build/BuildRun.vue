<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { Spend } from '../../types/generated/Spend'
import type { BuildDone } from '../../types/generated/BuildDone'
import type { ReadProgress as ReadProgressValue } from '../../types/generated/ReadProgress'
import ReadProgress from '../sources/ReadProgress.vue'
import { PLAN_STOPPED, RETRY_DELAY_MS, PLAN_WAIT_MS, MAX_STOPS } from './keepGoing'

const props = defineProps<{ slug: string, name: string }>()
const emit = defineEmits<{ running: [boolean], finished: [], report: [] }>()
const backend = useBackend()
const spend = ref<Spend | null>(null)
const running = ref(false)
const stopping = ref(false)
const attempted = ref(false)
const lines = ref<string[]>([])
const steps = ref<ReadProgressValue | null>(null)
const done = ref<BuildDone | null>(null)
const error = ref('')
const info = ref(false)
const keepGoing = ref(false)
watch(() => props.slug, slug => {
  try {
    keepGoing.value = localStorage.getItem(`voice.keepGoing.${slug}`) === 'true'
  } catch {
    keepGoing.value = false
  }
}, { immediate: true })
function rememberKeepGoing() {
  try {
    localStorage.setItem(`voice.keepGoing.${props.slug}`, String(keepGoing.value))
  } catch {
    // Keep the choice for this screen even when it cannot be saved.
  }
}
let cancelWait: (() => void) | null = null
let unmounted = false
function wait(delay: number): Promise<boolean> {
  return new Promise(resolve => {
    const timer = setTimeout(() => {
      cancelWait = null
      resolve(true)
    }, delay)
    cancelWait = () => {
      clearTimeout(timer)
      cancelWait = null
      resolve(false)
    }
  })
}
onUnmounted(() => {
  unmounted = true
  cancelWait?.()
})
const stepsLine = computed(() => {
  if (!steps.value) return ''
  const { done, total } = steps.value
  const left = total - done
  const noun = total === 1 ? 'step' : 'steps'
  if (left === 0) return `All ${total} ${noun} done.`
  return running.value
    ? `${done} of ${total} ${noun} done, ${left} left.`
    : `Stopped after ${done} of ${total} ${noun}, ${left} left.`
})
const STOPPED = 'Stopped. Your assistant is as it was before this build. What was already finished is kept, so running the build again carries on from there.'
onMounted(async () => {
  try {
    const initial = await backend.spendToday({ slug: props.slug })
    if (!attempted.value) spend.value = initial
  } catch (failure) {
    if (!attempted.value) error.value = failureMessage(failure, "Voice's record of what this assistant has spent can't be read, so it won't spend anything more. Use the voice command to see what's wrong.")
  }
})
async function build(mergeFirst = false) {
  if (running.value) return
  running.value = true
  stopping.value = false
  attempted.value = true
  error.value = ''
  info.value = false
  done.value = null
  lines.value = []
  steps.value = null
  emit('running', true)
  try {
    if (mergeFirst) {
      try {
        await backend.mergeHistories({ slug: props.slug })
      } catch (failure) {
        error.value = failureMessage(failure, "Voice couldn't put your history together. Close it and open it again.")
        return
      }
      if (stopping.value) {
        info.value = true
        error.value = STOPPED
        return
      }
    }
    let best = 0
    let stops = 0
    while (!unmounted) {
      lines.value = []
      let reached = 0
      try {
        done.value = await backend.buildAssistant({ slug: props.slug, noLimit: keepGoing.value }, progress => {
          lines.value.push(progress.line)
          spend.value = progress.spend
          if (progress.steps) {
            steps.value = progress.steps
            reached = Math.max(reached, progress.steps.done)
          }
        })
        spend.value = done.value.spend
        error.value = ''
        emit('finished')
        return
      } catch (failure) {
        const message = failureMessage(failure, stopping.value
          ? STOPPED
          : 'The build stopped before it finished, so your assistant is as it was. What it already finished is kept: run the build again to carry on.')
        if (!keepGoing.value || stopping.value) {
          info.value = stopping.value
          error.value = stopping.value ? STOPPED : message
          return
        }
        if (unmounted) return
        let delay = RETRY_DELAY_MS
        if (message === PLAN_STOPPED) {
          delay = PLAN_WAIT_MS
          const at = new Date(Date.now() + delay).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
          error.value = `Your Claude plan's limit is reached, so Voice is waiting. It tries again at ${at}.`
        } else {
          stops = reached > best ? 1 : stops + 1
          if (stops >= MAX_STOPS) {
            info.value = false
            error.value = `The build stopped ${MAX_STOPS} times in a row without getting further, so Voice stopped trying. The last problem: ${message}`
            return
          }
          error.value = `${message} Voice tries again in ${RETRY_DELAY_MS / 1000} seconds.`
        }
        best = Math.max(best, reached)
        info.value = true
        if (!await wait(delay)) {
          error.value = STOPPED
          return
        }
        error.value = ''
      }
    }
  } finally {
    running.value = false
    stopping.value = false
    emit('running', false)
  }
}
async function stop() {
  stopping.value = true
  if (cancelWait) {
    cancelWait()
    return
  }
  try {
    await backend.stopBuild({ slug: props.slug })
  } catch (failure) {
    info.value = true
    error.value = failureMessage(failure, STOPPED)
  }
}
async function rebuild() {
  await build(true)
}
defineExpose({ rebuild })
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">Build</h3>
      <p v-if="spend?.usage">{{ spend.usage }}</p>
      <p v-else-if="spend">Today, about ${{ spend.spentUsd.toFixed(2) }}<template v-if="spend.dailyCapUsd === 0">, with no daily limit</template><template v-else> of the ${{ spend.dailyCapUsd.toFixed(2) }} daily limit</template>, as Claude Code estimates it.</p>
      <div v-if="error" class="alert" :class="info ? 'alert-info' : 'alert-danger'" role="alert">{{ error }}</div>
      <ReadProgress v-if="running || (steps && !done)" :progress="steps" label="Building" />
      <p v-if="steps && !done" class="text-secondary" aria-live="polite">{{ stepsLine }}</p>
      <div v-if="lines.length" class="overflow-auto" style="max-height: 16rem">
        <ul class="list-unstyled" aria-live="polite">
          <li v-for="(line, index) in lines" :key="index">{{ line }}</li>
        </ul>
      </div>
      <div v-if="done" class="alert alert-success" role="alert">
        Done. Your assistant is built from {{ done.answered.toLocaleString() }} answered questions.
        <template v-if="!done.tested"> There weren't enough questions kept back to test it yet, so it decides nothing on its own until it can be tested.</template>
      </div>
      <button v-if="done" type="button" class="btn btn-secondary" @click="emit('report')">See how well {{ name }} knows you</button>
      <label class="form-check form-switch">
        <input v-model="keepGoing" class="form-check-input" type="checkbox" :disabled="running" aria-describedby="keep-going-help" @change="rememberKeepGoing">
        <span class="form-check-label">Keep going until it's done</span>
      </label>
      <p id="keep-going-help" class="text-secondary">Voice runs the build again by itself whenever it stops, with no spending limit, until every step is done. If your Claude plan's limit is reached, it waits and tries again. It gives up if the same step stops 3 times in a row. Keep Voice open while it works.</p>
      <p v-if="running" class="text-secondary">Stopping keeps everything finished so far. Next time it carries on from here.</p>
      <button v-if="running" type="button" class="btn btn-outline-danger" @click="stop">{{ stopping ? 'Stopping…' : 'Stop for now' }}</button>
      <button v-else type="button" class="btn btn-primary" @click="build()">{{ attempted ? 'Run the build again' : 'Start the build' }}</button>
    </div>
  </section>
</template>
