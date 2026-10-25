<script setup lang="ts">
import { computed, ref } from 'vue'
import { useBackend } from '../../backend'
import { failureMessage } from '../../failure'
import type { ChatList } from '../../types/generated/ChatList'

const props = defineProps<{ slug: string, list: ChatList }>()
const emit = defineEmits<{ update: [list: ChatList] }>()
const backend = useBackend()
const search = ref('')
const running = ref(false)
const error = ref('')
const chats = computed(() => props.list.chats.filter(chat => chat.title.toLowerCase().includes(search.value.toLowerCase())))
async function mark(mine: boolean, uuids: string[], unmarked = false) {
  running.value = true
  error.value = ''
  try {
    const answer = await backend.markClaudeaiChats({ slug: props.slug, mine, uuids, unmarked })
    emit('update', answer)
  } catch (failure) {
    error.value = failureMessage(failure, "Persona Builder couldn't read this download. Check it's the file claude.ai sent you, then choose it again.")
  } finally {
    running.value = false
  }
}
</script>

<template>
  <div class="mt-3">
    <p>Mark the chats that are yours. Leave out any you wrote for someone else, or that aren't about how you work.</p>
    <p>{{ list.mine.toLocaleString() }} yours, {{ list.notMine.toLocaleString() }} not yours, {{ list.unmarked.toLocaleString() }} not marked yet.</p>
    <div v-if="error" class="alert alert-danger" role="alert">{{ error }}</div>
    <div class="d-flex gap-2 mb-3">
      <button type="button" class="btn btn-secondary btn-sm" :disabled="running" @click="mark(true, list.chats.map(chat => chat.uuid))">Mark all as yours</button>
      <button type="button" class="btn btn-secondary btn-sm" :disabled="running" @click="mark(false, [], true)">Mark the rest as not yours</button>
    </div>
    <label class="form-label">Find a chat<input v-model="search" type="search" class="form-control" placeholder="Find a chat"></label>
    <div class="overflow-auto" style="max-height: 24rem">
      <div class="list-group">
        <div v-for="chat in chats" :key="chat.uuid" class="list-group-item">
          <div class="d-flex justify-content-between align-items-center gap-3">
            <div>
              <div>{{ chat.title === '[filtered]' ? 'Title hidden: it looked private' : chat.title || 'No title' }}</div>
              <div class="text-secondary">{{ chat.created }} · {{ chat.messages.toLocaleString() }} messages</div>
            </div>
            <div class="btn-group" role="group" aria-label="Whose chat">
              <button type="button" class="btn btn-secondary btn-sm" :class="{ active: chat.mark === 'mine' }" :aria-pressed="chat.mark === 'mine'" :disabled="running" @click="mark(true, [chat.uuid])">Yours</button>
              <button type="button" class="btn btn-secondary btn-sm" :class="{ active: chat.mark === 'notMine' }" :aria-pressed="chat.mark === 'notMine'" :disabled="running" @click="mark(false, [chat.uuid])">Not yours</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
