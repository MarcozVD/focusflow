import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { applySavedTheme } from './lib/data.svelte'

// El tema guardado (localStorage) se aplica antes del primer render, sin IPC:
// una llamada al backend aquí (await loadUiPrefs) puede bloquear la creación
// del widget, que ocurre en el hilo principal. loadUiPrefs corre en onMount.
applySavedTheme()

// Sin menú contextual de navegador (aplica a todas las ventanas).
window.addEventListener('contextmenu', (e) => e.preventDefault())

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
