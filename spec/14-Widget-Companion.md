# 14 — Widget AI Time Companion (Fase 10)

**Estado:** implementado en `spike`
**Fecha:** 2026-08-08
**Depende de:** spec/13 (asistente), spec/04 (design system)

## Objetivo

El widget deja de duplicar el calendario y se convierte en un **compañero de
tiempo contextual**: qué está pasando ahora, qué viene, qué es importante y
cómo actuar — en un vistazo.

## Qué muestra

```
┌───────────────────────────┐
│ F FocusFlow    ● hoy · 2  │
│ AHORA                     │
│   ● Estudiar cálculo      │
│     45 min restantes      │
│   [✓] [⟳] [▶]            │
│ SIGUIENTE                 │
│   ● English assignment    │
│     19:00                 │
│ IMPORTANTE                │
│   ● Examen de cálculo     │
│     mañana                │
│               ⤢ Abrir  ✦ Preguntar │
└───────────────────────────┘
```

| Sección     | Qué es                                                     |
|-------------|------------------------------------------------------------|
| AHORA       | tarea en curso (ventana [start, end] contiene `now`) o la única `en-curso` |
| POR HACER   | fallback cuando no hay actividad: primera pendiente (atrasada primero) |
| SIGUIENTE   | primera actividad que empieza en el futuro                 |
| IMPORTANTE  | vencimiento próximo: prioridad alta / todo el día / fin < 48 h; el más cercano |

## Acciones rápidas

| Acción      | Efecto (vía servicios existentes)               |
|-------------|--------------------------------------------------|
| ✓ completar | `task_complete` → `set_completed`                |
| ⟳ posponer  | `widget_action postpone` → `move_to` +1 hora     |
| ▶ empezar   | `widget_action start` → `set_task_status('en-curso')` (nuevo servicio) |
| ⤢ abrir     | `open_app` (ventana principal)                   |
| ✦ preguntar | `open_assistant` → ventana principal + vista Asistente (`nav:assistant`) |

## Rendimiento

- **Cero polling a la BD.** El contexto se deriva 100% en el frontend del
  store de tareas existente (que ya se refresca con `tasks:changed`).
- La única repetición es el reloj local del frontend (30 s) para el label
  "X min restantes" — sin IPC.
- Nuevo comando `widget_action` solo existe cuando el usuario pulsa un botón.

## Diseño

- Compacto: máx. 4 bloques, labels cortos, botones de acción ocultos hasta
  hover (calm & unobtrusive).
- Desktop-native: la ventana del widget NO se toca — transparencia, posición,
  drag region, tema (`applySavedTheme`/`ui:prefs`) y arranque intactos.
- Sin cambios de layout en `create_widget` (lib.rs) ni en `WidgetPage`.

## Cambios

- **`store.rs`**: `set_task_status` (servicio para 'en-curso').
- **`lib.rs`**: `widget_action` (complete/postpone/start con dispatch a los
  servicios del store), `open_assistant` (nav:assistant).
- **`App.svelte`**: listener `nav:assistant` → vista Asistente.
- **`data.svelte.ts`**: `widgetAction`, `openApp`, `askAssistant`.
- **`Widget.svelte`**: rediseño a secciones contextuales + quick actions.

## Compatibilidad (no romper el widget)

- [x] transparencia: sin cambios de ventana (WidgetPage intacto)
- [x] posicionamiento: drag region conservada en el header
- [x] arranque: auto-start/start_minimized intactos
- [x] cierre: tray/close sin cambios
- [x] tema: `applySavedTheme` + `ui:prefs` intactos
- [x] actualizaciones de tareas: `tasks:changed` → store → re-derivación
- [x] click en tarea → `open_task` (comportamiento previo)

## Tests

- store: `set_task_status` pendiente → en-curso → completada
- frontend: svelte-check 0 errores
- regresión: suite completa (135 tests Rust)

## Pendiente

- Verificación manual multi-monitor (el widget no tiene lógica de monitores;
  se mantiene igual que antes).
- E2E visual del widget real (temas claro/oscuro).
