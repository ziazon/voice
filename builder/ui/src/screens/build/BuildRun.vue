<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { Spend } from '../../types/generated/Spend'
import type { BuildDone } from '../../types/generated/BuildDone'
import ReadProgress from '../sources/ReadProgress.vue'

const props = defineProps<{ slug: string }>()
const emit = defineEmits<{ running: [boolean], finished: [] }>()
const backend = useBackend()
const spend = ref<Spend | null>(null)
const running = ref(false)
const stopping = ref(false)
const attempted = ref(false)
const lines = ref<string[]>([])
const done = ref<BuildDone | null>(null)
const error = ref('')
const info = ref(false)
onMounted(async () => {
  try {
    const initial = await backend.spendToday({ slug: props.slug })
    if (!attempted.value) spend.value = initial
  } catch (failure) {
    if (!attempted.value) error.value = failureMessage(failure, "Persona Builder's record of what this assistant has spent can't be read, so it won't spend anything more. Use the persona-builder command to see what's wrong.")
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
  emit('running', true)
  try {
    if (mergeFirst) {
      try {
        await backend.mergeHistories({ slug: props.slug })
      } catch (failure) {
        error.value = failureMessage(failure, "Persona Builder couldn't put your history together. Close it and open it again.")
        return
      }
      if (stopping.value) {
        info.value = true
        error.value = 'Stopped. Your assistant is as it was before this build. What was already finished is kept, so building again carries on from there.'
        return
      }
    }
    done.value = await backend.buildAssistant({ slug: props.slug }, progress => {
      lines.value.push(progress.line)
      spend.value = progress.spend
    })
    spend.value = done.value.spend
    emit('finished')
  } catch (failure) {
    info.value = stopping.value
    error.value = failureMessage(failure, stopping.value
      ? 'Stopped. Your assistant is as it was before this build. What was already finished is kept, so building again carries on from there.'
      : 'The build stopped before it finished, so your assistant is as it was. What it already finished is kept: build again to carry on.')
  } finally {
    running.value = false
    stopping.value = false
    emit('running', false)
  }
}
async function stop() {
  stopping.value = true
  try {
    await backend.stopBuild({ slug: props.slug })
  } catch (failure) {
    info.value = true
    error.value = failureMessage(failure, 'Stopped. Your assistant is as it was before this build. What was already finished is kept, so building again carries on from there.')
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
      <p v-if="spend">Today, about ${{ spend.spentUsd.toFixed(2) }} of the ${{ spend.dailyCapUsd.toFixed(2) }} daily limit, as Claude Code estimates it.</p>
      <div v-if="error" class="alert" :class="info ? 'alert-info' : 'alert-danger'" role="alert">{{ error }}</div>
      <ReadProgress v-if="running" :progress="null" label="Building" />
      <div v-if="lines.length" class="overflow-auto" style="max-height: 16rem">
        <ul class="list-unstyled" aria-live="polite">
          <li v-for="(line, index) in lines" :key="index">{{ line }}</li>
        </ul>
      </div>
      <div v-if="done" class="alert alert-success" role="alert">
        Done. Your assistant is built from {{ done.answered.toLocaleString() }} answers to Claude's questions.
        <template v-if="!done.tested"> There weren't enough answers kept aside to test it yet, so it decides nothing on its own until it can be tested.</template>
      </div>
      <p v-if="done" class="text-secondary">Next, see how well it knows you.</p>
      <button v-if="running" type="button" class="btn btn-outline-danger" :disabled="stopping" @click="stop">{{ stopping ? 'Stopping…' : 'Stop' }}</button>
      <button v-else type="button" class="btn btn-primary" @click="build()">{{ attempted ? 'Build again' : 'Build my assistant' }}</button>
    </div>
  </section>
</template>
