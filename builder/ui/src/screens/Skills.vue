<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { Skills } from '../types/generated/Skills'
import type { SkillRow } from '../types/generated/SkillRow'
import { words, waitingWords, stateWords, actionLabel, numberWords, withdrawnWords, refreshWords } from './skills/words'

const props = defineProps<{ assistant: Assistant }>()
const backend = useBackend()
const skills = ref<Skills>({ skills: [], notes: [] })
const loading = ref(true)
const changing = ref(false)
const refreshing = ref(false)
const busy = computed(() => loading.value || changing.value || refreshing.value)
const waiting = computed(() => skills.value.skills.filter(skill => skill.state === 'waiting').length)
const loadError = ref('')
const message = ref('')
const error = ref('')
const lines = ref<string[]>([])

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    skills.value = await backend.listSkills({ slug: props.assistant.slug })
  } catch (failure) {
    loadError.value = failureMessage(failure, words.loadFailure)
  } finally { loading.value = false }
}
onMounted(load)

async function choose(skill: SkillRow) {
  if (busy.value) return
  changing.value = true
  message.value = ''
  error.value = ''
  lines.value = []
  const request = { slug: props.assistant.slug, ids: [skill.id] }
  try {
    if (skill.state === 'agreed') {
      const result = await backend.withdrawSkills(request)
      skills.value = result.skills
      if (result.mirror === 'notRemoved') error.value = withdrawnWords(result.mirror)
      else message.value = withdrawnWords(result.mirror)
    } else {
      skills.value = await backend.agreeSkills(request)
      message.value = words.agreed
    }
  } catch (failure) {
    error.value = failureMessage(failure, words.saveFailure)
  } finally { changing.value = false }
}

async function refresh() {
  if (busy.value) return
  refreshing.value = true
  message.value = ''
  error.value = ''
  lines.value = []
  try {
    const report = await backend.refreshSkills({ slug: props.assistant.slug })
    if (report.copied === report.agreed) message.value = refreshWords(report)
    else error.value = refreshWords(report)
    lines.value = report.lines
    await load()
  } catch (failure) {
    error.value = failureMessage(failure, words.refreshFailure)
  } finally { refreshing.value = false }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">{{ words.title }}</h2>
      <p>{{ words.introduction }}</p>
      <div v-if="loading && !refreshing" class="spinner-border" role="status"><span class="visually-hidden">{{ words.loading }}</span></div>
      <template v-else-if="loadError">
        <div class="alert alert-warning" role="alert">{{ loadError }}</div>
        <button type="button" class="btn btn-primary" :disabled="busy" @click="load">{{ words.retry }}</button>
      </template>
      <template v-else>
        <p v-if="waiting">{{ waitingWords(waiting) }}</p>
        <div v-if="skills.skills.length" class="overflow-auto">
          <table class="table table-sm table-vcenter card-table">
            <tbody>
              <tr v-for="skill in skills.skills" :key="skill.id">
                <td>
                  <strong>{{ skill.name }}</strong> <small class="text-secondary">{{ numberWords(skill.number) }}</small>
                  <div class="text-secondary small">{{ skill.place }}</div>
                </td>
                <td>{{ stateWords(skill) }}</td>
                <td>
                  <button v-if="skill.state !== 'neverCopied'" type="button" class="btn btn-outline-primary" :aria-label="actionLabel(skill)" :disabled="busy" @click="choose(skill)">{{ skill.state === 'agreed' ? words.stop : words.yes }}</button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else>{{ words.empty }}</p>
        <p v-for="(note, index) in skills.notes" :key="index" class="text-secondary small">{{ note }}</p>
        <button type="button" class="btn btn-primary" :disabled="busy" @click="refresh">{{ words.refresh }}</button>
      </template>
      <div v-if="refreshing" class="spinner-border ms-3" role="status"><span class="visually-hidden">{{ words.refreshing }}</span></div>
      <div v-if="message" class="alert alert-success mt-3" role="alert">{{ message }}</div>
      <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
      <pre v-if="lines.length" class="mt-3 mb-0">{{ lines.join('\n') }}</pre>
    </div>
  </section>
</template>
