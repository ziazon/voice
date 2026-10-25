import '@tabler/core/dist/css/tabler.min.css'
import { createApp } from 'vue'
import PersonaBuilder from '../PersonaBuilder.vue'
import { backendKey } from '../backend'
import { tauriBackend } from './tauri-backend'
import { followSystemTheme } from './theme'

followSystemTheme()
const app = createApp(PersonaBuilder)
app.provide(backendKey, tauriBackend)
app.mount('#app')
