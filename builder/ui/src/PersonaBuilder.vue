<script setup lang="ts">
import { ref, watch } from 'vue'
import Welcome from './screens/Welcome.vue'
import Assistants from './screens/Assistants.vue'
import NewAssistant from './screens/NewAssistant.vue'
import Consent from './screens/Consent.vue'
import ConnectClaude from './screens/ConnectClaude.vue'
import Save from './screens/Save.vue'
import Sources from './screens/Sources.vue'
import TurnOn from './screens/TurnOn.vue'
import ClaudeAiFiles from './screens/ClaudeAiFiles.vue'
import Forget from './screens/Forget.vue'
import Build from './screens/Build.vue'
import Report from './screens/Report.vue'
import Review from './screens/Review.vue'
import Panels from './screens/Panels.vue'
import type { Assistant } from './types/generated/Assistant'
import type { Consent as ConsentAnswer } from './types/generated/Consent'
import { steps } from './steps'

const view = ref<'home' | 'new' | 'consent' | 'connect' | 'sources' | 'save' | 'build' | 'report' | 'review' | 'panels'>('home')
const buildOpened = ref(false)
const buildRunning = ref(false)
const panelsRunning = ref(false)
const reviewOpened = ref(false)
const reviewRunning = ref(false)
watch(view, value => {
  if (value === 'review') reviewOpened.value = true
})
const resets = ref(0)
function forgotten() {
  if (!assistant.value) return
  assistant.value = { ...assistant.value, consent: { agreed: false, agreedOn: null } }
  buildOpened.value = false
  reviewOpened.value = false
  reviewRunning.value = false
  buildRunning.value = false
  resets.value += 1
}
const assistant = ref<Assistant | null>(null)
function open(chosen: Assistant) {
  if (panelsRunning.value) return
  buildOpened.value = false
  reviewOpened.value = false
  reviewRunning.value = false
  assistant.value = chosen
  view.value = chosen.consent.agreed ? 'connect' : 'consent'
}
function made(chosen: Assistant) {
  buildOpened.value = false
  reviewOpened.value = false
  reviewRunning.value = false
  assistant.value = chosen
  view.value = 'consent'
}
function agreed(consent: ConsentAnswer) {
  if (!assistant.value) return
  assistant.value = { ...assistant.value, consent }
  if (consent.agreed) view.value = 'connect'
}
function goToStep(step: string) {
  if (panelsRunning.value || !assistant.value) return
  if (step === 'consent') view.value = 'consent'
  if (step === 'save') view.value = 'save'
  if ((step === 'connect' || step === 'sources' || step === 'build' || step === 'report' || step === 'review') && assistant.value.consent.agreed) {
    view.value = step
    if (step === 'build') buildOpened.value = true
  }
}
function disabled(step: string) {
  if (step === 'consent' || step === 'save') return !assistant.value
  return !assistant.value?.consent.agreed
}
function home() {
  panelsRunning.value = false
  buildOpened.value = false
  reviewOpened.value = false
  reviewRunning.value = false
  buildRunning.value = false
  assistant.value = null
  view.value = 'home'
}
</script>

<template>
  <div class="page">
    <div class="page-body">
      <div class="container-xl">
        <div class="row g-5">
          <aside class="col-3">
            <h2 v-if="assistant" class="h4 mb-4">Making {{ assistant.name }}</h2>
            <ol class="ps-3 mb-0">
              <li v-for="step in steps" :key="step.id" :aria-disabled="disabled(step.id) ? 'true' : undefined" class="mb-4 ps-1">
                <button v-if="assistant" type="button"
                  class="btn btn-link p-0 text-start text-reset" :disabled="disabled(step.id)"
                  :aria-disabled="disabled(step.id) ? 'true' : undefined" :aria-current="view === step.id ? 'step' : undefined"
                  @click="goToStep(step.id)">
                  <span data-step-title>{{ step.title }}</span>
                </button>
                <span v-else data-step-title class="d-block">{{ step.title }}</span>
                <span v-if="disabled(step.id)" class="d-block text-secondary small">{{
                  assistant ? 'Needs your permission first' : 'Choose an assistant first'
                }}</span>
              </li>
            </ol>
          </aside>
          <main class="col-9">
            <button v-if="view !== 'home'" type="button" class="btn btn-link p-0 mb-4" :disabled="buildRunning || reviewRunning || panelsRunning" @click="home">Your assistants</button>
            <p v-if="panelsRunning" class="text-secondary">Stop the panel first to leave Panels.</p>
            <p v-if="reviewRunning" class="text-secondary">Wait for the test to finish to choose another assistant.</p>
            <p v-if="buildRunning" class="text-secondary">Stop the build first to choose another assistant.</p>
            <template v-if="view === 'home'">
              <Welcome />
              <Assistants @open="open" @make="view = 'new'" />
              <section class="card mt-4">
                <div class="card-body">
                  <h2 class="card-title">Panels</h2>
                  <p>Ask several of your assistants one question. They work it out in rounds and come back with a joint answer, with any disagreement in full.</p>
                  <button type="button" class="btn btn-primary" @click="view = 'panels'">Ask a panel</button>
                </div>
              </section>
            </template>
            <Panels v-else-if="view === 'panels'" @running="panelsRunning = $event" />
            <NewAssistant v-else-if="view === 'new'" @made="made" @cancel="home" />
            <Consent v-else-if="view === 'consent' && assistant" :key="assistant.slug" :assistant="assistant" @agreed="agreed" @next="goToStep('connect')" />
            <ConnectClaude v-else-if="view === 'connect' && assistant" :key="assistant.slug" :assistant="assistant" @next="goToStep('sources')" />
            <Sources v-else-if="view === 'sources' && assistant" :key="assistant.slug" :assistant="assistant" @next="goToStep('build')" />
            <Report v-else-if="view === 'report' && assistant" :assistant="assistant" @build="goToStep('build')" @next="goToStep('review')" />
            <template v-else-if="view === 'save' && assistant">
              <Save :key="`save-${assistant.slug}-${resets}`" :assistant="assistant" />
              <TurnOn :key="`on-${assistant.slug}-${resets}`" :assistant="assistant" />
              <ClaudeAiFiles :key="`files-${assistant.slug}-${resets}`" :assistant="assistant" />
              <Forget :key="`forget-${assistant.slug}`" :assistant="assistant" @forgotten="forgotten" />
            </template>
            <Build v-if="buildOpened && assistant" v-show="view === 'build'" :key="assistant.slug" :assistant="assistant" @running="buildRunning = $event" @report="goToStep('report')" />
            <Review v-if="reviewOpened && assistant" v-show="view === 'review'" :active="view === 'review'" :key="assistant.slug" :assistant="assistant" @running="reviewRunning = $event" />
          </main>
        </div>
      </div>
    </div>
  </div>
</template>
