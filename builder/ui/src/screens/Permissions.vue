<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import type { Assistant } from '../types/generated/Assistant'
import type { Permission } from '../types/generated/Permission'
import type { Permissions } from '../types/generated/Permissions'
import AuditLog from './AuditLog.vue'
import { words, capitalFirst, windowWords, calendarWords, calendarChosenWords, actionLabel } from './permissions/words'

const props = defineProps<{ assistant: Assistant }>()
const backend = useBackend()
const permissions = ref<Permissions>({ permissions: [], calendar: null, calendarProblem: null })
const loading = ref(true)
const changing = ref(false)
const auditRunning = ref(false)
const busy = computed(() => loading.value || changing.value || auditRunning.value)
const loadError = ref('')
const message = ref('')
const error = ref('')
const auditChanges = ref(0)

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    permissions.value = await backend.listPermissions({ slug: props.assistant.slug })
  } catch (failure) {
    loadError.value = failureMessage(failure, words.loadFailure)
  } finally { loading.value = false }
}
onMounted(load)
function retry() {
  if (busy.value) return
  return load()
}

async function change(permission: Permission, turnOff = false) {
  if (busy.value) return
  changing.value = true
  message.value = ''
  error.value = ''
  try {
    const request = { slug: props.assistant.slug, id: permission.id }
    const result = turnOff
      ? await backend.turnOffPermission(request)
      : await backend.givePermission({ ...request, pastDays: Math.trunc(Number(permission.pastDays)), nextDays: Math.trunc(Number(permission.nextDays)) })
    permissions.value = result.permissions
    message.value = result.said
  } catch (failure) {
    error.value = failureMessage(failure, words.saveFailure)
    await load()
  } finally {
    changing.value = false
    auditChanges.value++
  }
}

async function choose(folder: boolean) {
  if (busy.value) return
  changing.value = true
  message.value = ''
  error.value = ''
  try {
    const calendar = await backend.chooseCalendarExport({ slug: props.assistant.slug, folder })
    if (calendar) {
      permissions.value.calendar = calendar
      permissions.value.calendarProblem = null
      message.value = calendarChosenWords(calendar)
    }
  } catch (failure) {
    error.value = failureMessage(failure, words.calendarFailure)
  } finally {
    changing.value = false
    auditChanges.value++
  }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">{{ words.title }}</h2>
      <p>{{ words.introduction }}</p>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">{{ words.loading }}</span></div>
      <template v-else-if="loadError">
        <div class="alert alert-warning" role="alert">{{ loadError }}</div>
        <button type="button" class="btn btn-primary" :disabled="busy" @click="retry">{{ words.retry }}</button>
      </template>
      <template v-else>
        <div v-for="(permission, index) in permissions.permissions" :key="permission.id" :class="{ 'border-top mt-4 pt-4': index > 0 }">
          <h3>{{ capitalFirst(permission.words) }} <span class="badge" :class="permission.on ? 'bg-green-lt' : 'bg-secondary-lt'">{{ permission.on ? words.on : words.off }}</span></h3>
          <p v-if="permission.on">{{ windowWords(permission) }}</p>
          <div class="row g-3 mb-3">
            <div class="col-sm-6">
              <label class="form-label" :for="`past-${assistant.slug}-${permission.id}`">{{ words.pastDays }}</label>
              <input :id="`past-${assistant.slug}-${permission.id}`" v-model.number="permission.pastDays" class="form-control" type="number" min="0" max="31" :disabled="busy">
              <small class="text-secondary">{{ words.pastHint }}</small>
            </div>
            <div class="col-sm-6">
              <label class="form-label" :for="`next-${assistant.slug}-${permission.id}`">{{ words.nextDays }}</label>
              <input :id="`next-${assistant.slug}-${permission.id}`" v-model.number="permission.nextDays" class="form-control" type="number" min="1" max="92" :disabled="busy">
              <small class="text-secondary">{{ words.nextHint }}</small>
            </div>
          </div>
          <div class="d-flex flex-wrap gap-2">
            <button type="button" class="btn" :class="permission.on ? 'btn-outline-primary' : 'btn-primary'" :aria-label="actionLabel(permission)" :disabled="busy" @click="change(permission)">{{ permission.on ? words.change : words.give }}</button>
            <button v-if="permission.on" type="button" class="btn btn-outline-danger" :aria-label="actionLabel(permission, true)" :disabled="busy" @click="change(permission, true)">{{ words.turnOff }}</button>
          </div>
          <div v-if="permission.id === 'calendar.read'" class="mt-3">
            <p v-if="permissions.calendarProblem" class="text-warning">{{ permissions.calendarProblem }}</p>
            <p v-else>{{ calendarWords(permissions.calendar) }}</p>
            <div class="d-flex flex-wrap gap-2">
              <button type="button" class="btn btn-outline-secondary btn-sm" :disabled="busy" @click="choose(false)">{{ words.chooseFile }}</button>
              <button type="button" class="btn btn-outline-secondary btn-sm" :disabled="busy" @click="choose(true)">{{ words.chooseFolder }}</button>
            </div>
          </div>
        </div>
      </template>
      <div v-if="message" class="alert alert-success mt-3" role="alert">{{ message }}</div>
      <div v-if="error" class="alert alert-warning mt-3" role="alert">{{ error }}</div>
      <hr>
      <fieldset class="border-0 p-0 m-0" :disabled="busy">
        <AuditLog :key="auditChanges" :assistant="assistant" @running="auditRunning = $event" />
      </fieldset>
    </div>
  </section>
</template>
