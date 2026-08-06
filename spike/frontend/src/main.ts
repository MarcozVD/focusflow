import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'

const app = mount(App, {
  target: document.getElementById('app')!,
})

try {
  const label = getCurrentWindow().label
  void invoke('log_line', { line: `app_mounted label=${label}` })
} catch {
  // navegador
}

export default app
