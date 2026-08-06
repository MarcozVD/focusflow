# FocusFlow — Repositorio del proyecto

Aplicación de productividad personal para Windows (calendario + tareas + agenda + recordatorios + widget de escritorio, local-first, Soft UI 2.0).

## Estructura

```
focusflow/
├── README.md          ← este archivo
├── spec/              Especificación completa del producto (7 documentos)
│   ├── 01-PRD.md      Visión, requisitos, flujos, AC del MVP, MoSCoW/RICE
│   ├── 02-Arquitectura.md  Stack (Tauri 2 + Svelte 5 + SQLite), decisiones justificadas
│   ├── 03-Modelo-Datos.md  ER + tablas + índices
│   ├── 04-Design-System.md Tokens Soft UI 2.0 (claro/oscuro, sombras, motion)
│   ├── 05-Roadmap.md  Fases MVP → V5 con esfuerzos y exit criteria
│   ├── 06-Auditoria.md     Crítica competitiva + propuestas disruptivas
│   ├── 07-Spike-Tecnico.md Resultados de la validación técnica
│   └── refimg.jpeg     Referencia visual del diseño (pendiente verificación)
├── spike/              App Tauri 2 (frontend Svelte + backend Rust con SQLite)
│   ├── frontend/       UI real (Svelte 5 + Vite) — migrado del prototipo
│   ├── src-tauri/      Shell Rust: store SQLite, hotkey, tray, widget
│   │   └── target/release/focusflow-spike.exe   ← binario compilado
└── proto/              Prototipo Svelte 5 del diseño (validación de UX)
    └── shots/          Screenshots: semana claro/oscuro, widget, quick add
```

## Estado (2026-08-04)

| Componente | Estado | Cómo ejecutarlo |
|------------|--------|-----------------|
| Spec (7 docs) | ✅ Completa | leer `spec/README.md` |
| Spike Tauri | ✅ Validado | `spike\src-tauri\target\release\focusflow-spike.exe` |
| Prototipo Svelte | ✅ Funcional | `cd proto; npm run dev` (o `npm run build && npx vite preview`) |
| **Integración Tauri+Svelte** | ✅ **Migrada y funcional** | `spike\src-tauri\target\release\focusflow-spike.exe` |

## Integración Tauri + Svelte (app real)

La UI del prototipo vive ahora dentro del shell Tauri. Persistencia real en SQLite
(`%APPDATA%\com.focusflow.spike\focusflow.db`), hotkey global, tray, widget transparente.

```powershell
# Construir la app (frontend + backend embebido)
cd spike\frontend
npm install
npm run build                # → dist/ (assets que Tauri embebe)
cd ..\src-tauri
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo build --release        # → target\release\focusflow-spike.exe

# Ejecutar (opcional: widget al arrancar y toast)
$env:FF_WIDGET = "1"         # auto-crea widget de escritorio
$env:FF_NOTIFY  = "1"        # dispara notificación de prueba
.\target\release\focusflow-spike.exe
```

Verificado al integrar: seed de 5 tareas en primer arranque, sin reseed en arranques
siguientes (persistencia), `single-instance` enfoca la ventana existente, hotkey
Ctrl+Shift+Space registrado y widget SPA funcionando con transparencia.
Logs de diagnóstico: `%TEMP%\focusflow-spike\spike.log`.

## Quick start del prototipo

```powershell
cd proto
npm run dev        # http://localhost:5173  (semana/mes/día/agenda, dark, widget)
npm run build      # build de producción + node shots.mjs genera screenshots
```

Atajos del prototipo: `#/dark` tema oscuro · `#/widget` página widget · Enter en el campo de texto → preview de entidades NL → Enter de nuevo → crea tarea.

## Stack decidido

Tauri 2 (Rust) + Svelte 5 (TypeScript) + SQLite (rusqlite) + Temporal + tauri-plugin-notification + tauri-plugin-global-shortcut + tauri-plugin-single-instance.

Ver justificación completa en `spec/02-Arquitectura.md` y resultados medidos en `spec/07-Spike-Tecnico.md`.
