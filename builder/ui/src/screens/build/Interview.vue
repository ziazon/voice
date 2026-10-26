<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { InterviewStatus } from '../../types/generated/InterviewStatus'
import type { InterviewAnswer } from '../../types/generated/InterviewAnswer'
import type { InterviewKept } from '../../types/generated/InterviewKept'
import ReadProgress from '../sources/ReadProgress.vue'

const props = defineProps<{ slug: string }>()
const emit = defineEmits<{ rebuild: [] }>()
const backend = useBackend()
const status = ref<InterviewStatus | null>(null)
const answers = ref<Record<number, InterviewAnswer>>({})
const loading = ref(true)
const busy = ref(false)
const drafting = ref(false)
const error = ref('')
const page = ref(0)
const replacing = ref(false)
const kept = ref<InterviewKept | null>(null)
const questions = computed(() => status.value?.questions.slice(page.value * 4, page.value * 4 + 4) ?? [])
const lastPage = computed(() => page.value >= Math.ceil((status.value?.questions.length ?? 0) / 4) - 1)
function adopt(value: InterviewStatus) {
  status.value = value
  answers.value = {}
  page.value = 0
  kept.value = null
  for (const question of value.questions) {
    const saved = value.answers.find(answer => answer.n === question.n)
    answers.value[question.n] = saved ? { ...saved, picks: [...saved.picks] } : { n: question.n, picks: [], ownWords: '' }
  }
}
onMounted(async () => {
  try {
    adopt(await backend.interviewStatus({ slug: props.slug }))
  } catch (failure) {
    error.value = failureMessage(failure, "Something in this assistant's settings stops the build. Use the persona-builder command to see what.")
  } finally {
    loading.value = false
  }
})
async function draft(replace: boolean) {
  busy.value = true
  drafting.value = true
  error.value = ''
  try {
    adopt(await backend.draftInterview({ slug: props.slug, replace }))
    replacing.value = false
  } catch (failure) {
    error.value = failureMessage(failure, "Claude drafted questions that didn't fit, twice, so none were saved. Try again.")
  } finally {
    busy.value = false
    drafting.value = false
  }
}
function orderedAnswers() {
  return (status.value?.questions ?? []).map(question => ({
    n: question.n,
    picks: question.options.filter(option => answers.value[question.n]!.picks.includes(option.letter)).map(option => option.letter),
    ownWords: answers.value[question.n]!.ownWords,
  }))
}
async function move(direction: number, keep = false) {
  kept.value = null
  busy.value = true
  error.value = ''
  try {
    await backend.saveInterviewAnswers({ slug: props.slug, answers: orderedAnswers() })
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't save your answers. Check there's space on this computer, then try again.")
    busy.value = false
    return
  }
  try {
    if (keep) kept.value = await backend.keepInterviewAnswers({ slug: props.slug })
    else page.value += direction
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't keep that answer. Close it and open it again.")
  } finally {
    busy.value = false
  }
}
function pick(n: number, letter: string, multi: boolean, checked: boolean) {
  kept.value = null
  const answer = answers.value[n]!
  answer.picks = multi ? (checked ? [...answer.picks, letter] : answer.picks.filter(pick => pick !== letter)) : [letter]
}
</script>

<template>
  <section class="card">
    <div class="card-body">
      <h3 class="card-title">The interview</h3>
      <div v-if="loading" class="spinner-border" role="status">
        <span class="visually-hidden">Checking your interview…</span>
      </div>
      <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
      <template v-if="status">
        <p v-if="status.answered >= status.thinBelow && !status.questions.length">Your history holds {{ status.answered.toLocaleString() }} answers to Claude's questions, enough to build from.</p>
        <template v-else>
          <p>Your history holds {{ status.answered.toLocaleString() }} answers to Claude's questions. About {{ status.thinBelow.toLocaleString() }} helps your assistant know you well. Answering twenty questions fills it out.</p>
          <ReadProgress v-if="drafting" :progress="null" label="Drafting" />
          <template v-if="!status.questions.length">
            <button type="button" class="btn btn-primary" :disabled="busy" @click="draft(false)">{{ drafting ? 'Drafting the questions…' : 'Draft the questions' }}</button>
            <p class="text-secondary mt-2">Claude drafts them from your areas of work, through your own Claude Code. It uses a little of your Claude plan.</p>
          </template>
          <template v-else>
            <p>Questions {{ (page * 4 + 1).toLocaleString() }}–{{ Math.min(page * 4 + 4, status.questions.length).toLocaleString() }} of {{ status.questions.length.toLocaleString() }}</p>
            <fieldset v-for="question in questions" :key="question.n" class="mb-4" :disabled="busy">
              <legend class="h4">
                <small class="d-block text-secondary">{{ question.kind }}</small>
                {{ question.question }}
              </legend>
              <label v-for="option in question.options" :key="option.letter" class="form-check">
                <input class="form-check-input" :type="question.multiSelect ? 'checkbox' : 'radio'" :name="`${slug}-question-${question.n}`" :checked="answers[question.n]!.picks.includes(option.letter)" @change="pick(question.n, option.letter, question.multiSelect, ($event.target as HTMLInputElement).checked)" />
                <span class="form-check-label">
                  {{ option.label }}
                  <span v-if="question.recommended === option.letter" class="badge bg-secondary-lt">Suggested</span>
                </span>
                <span class="form-check-description text-secondary">{{ option.description }}</span>
              </label>
              <label :for="`${slug}-words-${question.n}`" class="form-label mt-2">Or in your own words (optional)</label>
              <textarea :id="`${slug}-words-${question.n}`" v-model="answers[question.n]!.ownWords" @input="kept = null" class="form-control" maxlength="2000" rows="2" />
            </fieldset>
            <p class="text-secondary">Leave out any that don't fit your work.</p>
            <div class="d-flex gap-2 mb-3">
              <button v-if="page > 0" type="button" class="btn btn-secondary" :disabled="busy" @click="move(-1)">Back</button>
              <button v-if="!lastPage" type="button" class="btn btn-primary" :disabled="busy" @click="move(1)">Next</button>
              <button v-else type="button" class="btn btn-primary" :disabled="busy" @click="move(0, true)">Keep my answers</button>
            </div>
            <template v-if="kept">
              <div class="alert alert-success" role="alert">Kept {{ kept.kept.toLocaleString() }} answers. {{ kept.privateLeftOut.toLocaleString() }} that looked private were left out.</div>
              <button type="button" class="btn btn-primary mb-3" :disabled="busy" @click="emit('rebuild')">Add them to your history and build again</button>
            </template>
            <div v-if="replacing" class="alert alert-warning" role="alert">
              <p>This replaces these questions and your answers to them.</p>
              <div class="d-flex gap-2">
                <button type="button" class="btn btn-secondary" :disabled="busy" @click="draft(true)">Draft different questions</button>
                <button type="button" class="btn btn-secondary" :disabled="busy" @click="replacing = false">Keep these</button>
              </div>
            </div>
            <button v-else type="button" class="btn btn-secondary btn-sm" :disabled="busy" @click="replacing = true">Draft different questions</button>
          </template>
        </template>
      </template>
    </div>
  </section>
</template>
