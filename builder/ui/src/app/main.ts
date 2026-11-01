import '@tabler/core/dist/css/tabler.min.css'
import '../theme.css'
import { createApp } from 'vue'
import VoiceApp from '../VoiceApp.vue'
import { backendKey } from '../backend'
import { tauriBackend } from './tauri-backend'
import { followSystemTheme } from './theme'

followSystemTheme()
const app = createApp(VoiceApp)
app.provide(backendKey, tauriBackend)
app.mount('#app')
