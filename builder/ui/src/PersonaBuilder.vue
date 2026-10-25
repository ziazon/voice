<script setup lang="ts">
import { ref } from 'vue'
import Welcome from './screens/Welcome.vue'
import Assistants from './screens/Assistants.vue'
import NewAssistant from './screens/NewAssistant.vue'
import Consent from './screens/Consent.vue'
import ConnectClaude from './screens/ConnectClaude.vue'
import Save from './screens/Save.vue'
import type { Assistant } from './types/generated/Assistant'
import type { Consent as ConsentAnswer } from './types/generated/Consent'
import { steps } from './steps'

const view = ref<'home' | 'new' | 'consent' | 'connect' | 'save'>('home')
const assistant = ref<Assistant | null>(null)
function open(chosen: Assistant) {
  assistant.value = chosen
  view.value = chosen.consent.agreed ? 'connect' : 'consent'
}
function made(chosen: Assistant) {
  assistant.value = chosen
  view.value = 'consent'
}
function agreed(consent: ConsentAnswer) {
  if (!assistant.value) return
  assistant.value = { ...assistant.value, consent }
  if (consent.agreed) view.value = 'connect'
}
function goToStep(step: string) {
  if (!assistant.value) return
  if (step === 'consent') view.value = 'consent'
  if (step === 'save') view.value = 'save'
  if (step === 'connect' && assistant.value.consent.agreed) view.value = 'connect'
}
function disabled(step: string) {
  if (step === 'consent' || step === 'save') return !assistant.value
  if (step === 'connect') return !assistant.value?.consent.agreed
  return true
}
function home() {
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
                <button v-if="assistant && (step.id === 'consent' || step.id === 'connect' || step.id === 'save')" type="button"
                  class="btn btn-link p-0 text-start text-reset" :disabled="disabled(step.id)"
                  :aria-disabled="disabled(step.id) ? 'true' : undefined" :aria-current="view === step.id ? 'step' : undefined"
                  @click="goToStep(step.id)">
                  <span data-step-title>{{ step.title }}</span>
                </button>
                <span v-else data-step-title class="d-block">{{ step.title }}</span>
                <span v-if="disabled(step.id)" class="d-block text-secondary small">{{
                  step.id === 'consent' || step.id === 'connect' || step.id === 'save'
                    ? (assistant ? 'Needs your permission first' : 'Choose an assistant first')
                    : 'Not here yet'
                }}</span>
              </li>
            </ol>
          </aside>
          <main class="col-9">
            <button v-if="view !== 'home'" type="button" class="btn btn-link p-0 mb-4" @click="home">Your assistants</button>
            <template v-if="view === 'home'">
              <Welcome />
              <Assistants @open="open" @make="view = 'new'" />
            </template>
            <NewAssistant v-else-if="view === 'new'" @made="made" @cancel="home" />
            <Consent v-else-if="view === 'consent' && assistant" :key="assistant.slug" :assistant="assistant" @agreed="agreed" @next="goToStep('connect')" />
            <ConnectClaude v-else-if="view === 'connect'" />
            <Save v-else-if="view === 'save' && assistant" :key="assistant.slug" :assistant="assistant" />
          </main>
        </div>
      </div>
    </div>
  </div>
</template>
