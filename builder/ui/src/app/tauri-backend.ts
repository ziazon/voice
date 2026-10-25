import { invoke } from '@tauri-apps/api/core'
import type { Backend } from '../backend'
import type { AppInfo } from '../types/generated/AppInfo'

export const tauriBackend: Backend = {
  appInfo: () => invoke<AppInfo>('app_info'),
}
