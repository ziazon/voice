<script setup lang="ts">
import type { PanelResultView } from '../../types/generated/PanelResultView'
import { outcomeWords } from './words'
defineProps<{ result: PanelResultView }>()
</script>

<template>
  <section>
    <h3>Their answer</h3>
    <p>A proposal for you, not a decision.</p>
    <p>{{ outcomeWords(result.outcome) }}</p>
    <p v-if="result.answerText !== null" class="text-break" style="white-space: pre-wrap">{{ result.answerText }}</p>
    <ol v-else-if="result.answerItems.length">
      <li v-for="(item, index) in result.answerItems" :key="index" class="text-break" style="white-space: pre-wrap">{{ item }}</li>
    </ol>
    <p v-else>They gave no answer.</p>
    <h3>How they grouped</h3>
    <div v-for="(group, index) in result.groups" :key="index" class="mb-3">
      <p class="text-break">{{ group.members.join(', ') }}</p>
      <p>{{ Math.round(group.weight * 100) }}% of the panel</p>
      <p v-if="group.answerText !== null" class="text-break" style="white-space: pre-wrap">{{ group.answerText }}</p>
      <ol v-else-if="group.answerItems.length">
        <li v-for="(item, itemIndex) in group.answerItems" :key="itemIndex" class="text-break" style="white-space: pre-wrap">{{ item }}</li>
      </ol>
    </div>
    <h3>Who disagreed</h3>
    <p v-if="!result.dissent.length">Nobody disagreed.</p>
    <div v-for="(dissent, index) in result.dissent" :key="index" class="mb-3">
      <h4 class="text-break">{{ dissent.seat }}</h4>
      <p v-if="dissent.holdout">Held out to the end.</p>
      <blockquote class="text-break" style="white-space: pre-wrap">{{ dissent.objections }}</blockquote>
    </div>
    <details>
      <summary>The whole result as text</summary>
      <pre class="text-break" style="white-space: pre-wrap">{{ result.markdown }}</pre>
    </details>
  </section>
</template>
