# Frontend de FocusFlow — Documentación

Índice de documentación del frontend real de FocusFlow (Svelte 5 + TypeScript + Vite + Tauri 2).

| Documento | Contenido |
|-----------|-----------|
| [PRODUCT.md](PRODUCT.md) | Qué es FocusFlow, problema, usuario, JTBD, propuesta de valor, principios, modelo de IA (propone → explica → aprueba → aplica), qué NO es. |
| [DESIGN.md](DESIGN.md) | Sistema visual real: neumorfismo refinado (base/raised/inset/floating), shadow system, colores light/dark, tipografía, spacing, radius, motion, iconografía, deuda visual detectada. |
| [COMPONENTS.md](COMPONENTS.md) | Inventario de componentes reales (propósito, anatomía, variantes, estados, a11y, neumorfismo) + propuestas de consolidación ([PROPUESTO]). |
| [UX.md](UX.md) | Reglas de experiencia: navegación, QuickAdd, drag & drop, calendario como información espacial, tareas sin hora, urgencia, vencidas, AI scheduling, email, widget, notificaciones, tema, vacíos, loading, errores. |
| [ACCESSIBILITY.md](ACCESSIBILITY.md) | Auditoría: teclado, focus-visible, semántica, contraste, iconos, targets, reduced motion, aria-live, color independence, calendario + plan de acción priorizado. |
| [LANDING-PAGE.md](LANDING-PAGE.md) | Dirección creativa de la landing oficial: narrativa, composiciones, reglas anti-genérico-AI, tokens heredados, prueba de calidad. |

---

## Estado de fidelidad

Esta documentación se escribió contra el **código real** en `spike/frontend/src/` (revisado a fondo el 2026-08-31): componentes, tokens de `app.css`, lógica de `taskDayLogic.ts`, store de `data.svelte.ts`, y warnings de accesibilidad observados al arrancar el dev server. Cualquier desviación entre este documento y el código debe resolverse actualizando el documento (el código manda).

## Cómo se estructura el código

```
spike/frontend/src/
├── App.svelte            # Shell: routing por vista + overlays + errores fatales
├── app.css               # TODOS los design tokens (tema claro/oscuro, sombras, motion)
├── main.ts               # Bootstrap: tema antes de render, detección de widget
└── lib/
    ├── data.svelte.ts    # Store global (Svelte 5 runes), IPC, caché por semanas, tema
    ├── taskDayLogic.ts   # Lógica pura de tiempo (testeable): segmentos, layout, agenda
    ├── Calendar.svelte   # Mes/semana/día + drag & drop + popup de día
    ├── EventBlock.svelte # Bloque de evento dentro del time-area
    ├── TaskCard.svelte   # Card de agenda
    ├── QuickAdd.svelte   # Captura por lenguaje natural
    ├── PlanProposal.svelte # Modal de revisión de planes (IA/local)
    ├── Assistant.svelte  # Hilo conversacional + planes + acciones
    ├── Suggestions.svelte # Bandeja de eventos del correo
    ├── Widget.svelte     # Widget de escritorio (open → glance → act)
    ├── WidgetPage.svelte # Mount del widget (fondo transparente)
    ├── Sidebar / TopBar / TitleBar / Agenda
    ├── TaskDrawer.svelte # Edición de tarea (panel derecho)
    ├── Settings.svelte   # Ajustes completos
    ├── Onboarding.svelte # Primera ejecución (GSAP, reduced-motion aware)
    ├── Login.svelte      # Login Google (OAuth2)
    └── ContextualToast.svelte # Notificaciones contextuales
```

## Verificación

```bash
cd spike/frontend
npm run dev       # demo en navegador (tareas de ejemplo, sin Tauri)
npm test          # vitest (taskDayLogic, assistantError, utils)
npm run build     # vite build
```
