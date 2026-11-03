<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { AuditLog } from '../types/generated/AuditLog'
import { words, unreadableWords, emptyLogWords } from './permissions/words'

const props = defineProps<{ assistant: Assistant }>()
const emit = defineEmits<{ running: [value: boolean] }>()
const backend = useBackend()
const log = ref<AuditLog>({ lines: [], unreadable: 0, older: false })
const loading = ref(true)
const loadError = ref('')
const last = ref(50)
async function load() {
  loading.value = true
  emit('running', true)
  loadError.value = ''
  try {
    log.value = await backend.auditLog({ slug: props.assistant.slug, last: last.value })
  } catch (failure) {
    loadError.value = failureMessage(failure, words.auditFailure)
  } finally {
    loading.value = false
    emit('running', false)
  }
}
onMounted(load)
function more() {
  if (loading.value) return
  last.value = 1000
  return load()
}
</script>

<template>
  <div>
    <h3>{{ words.auditTitle }}</h3>
    <p>{{ words.auditIntroduction }}</p>
    <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">{{ words.auditTitle }}</span></div>
    <template v-else-if="loadError">
      <div class="alert alert-warning" role="alert">{{ loadError }}</div>
      <button type="button" class="btn btn-primary" :disabled="loading" @click="load">{{ words.retry }}</button>
    </template>
    <template v-else>
      <div v-if="log.lines.length" class="overflow-auto" style="max-height: 20rem">
        <ul class="list-unstyled mb-0 small">
          <li v-for="(line, index) in log.lines" :key="index">{{ line }}</li>
        </ul>
      </div>
      <p v-if="log.unreadable" class="text-secondary small">{{ unreadableWords(log.unreadable) }}</p>
      <template v-if="log.older">
        <p class="text-secondary small">{{ words.older }}</p>
        <button v-if="last === 50" type="button" class="btn btn-link p-0" :disabled="loading" @click="more">{{ words.more }}</button>
      </template>
      <p v-if="!log.lines.length && !log.unreadable && !log.older">{{ emptyLogWords(assistant.name) }}</p>
    </template>
  </div>
</template>
