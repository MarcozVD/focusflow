# 18 — UI Polish: tareas completadas visibles + tokens de tipografía y espaciado

**Estado:** Plan aprobado · **Fecha:** 2026-09-30
**Origen:** sesión de grilling (13 decisiones) sobre `spike/frontend`.
**Alcance:** solo frontend. El backend (`list_range`) ya devuelve las completadas (filtra solo `deleted_at`).

---

## 1. Decisiones

| # | Tema | Decisión |
|---|------|----------|
| D1 | Alcance de completadas | Visibles en **todo el calendario**: semana, día, mes y popup del día. Widget (Ahora/Siguiente), contadores del Sidebar, selector de StudyForm, Sugerencias, `studyConflicts` y guard de conflicto de clase **siguen excluyéndolas**. **Excepción multi-día:** una completada de varios días solo se ve en su día de inicio y en su día de fin; en los intermedios no aparece (si no, el calendario se llenaba de repeticiones grises). Feedback del usuario al probar en Tauri. |
| D2 | Look de completada | **Hundida gris**: fondo `--surface-2`, sombra `--shadow-inset` (más profunda que la `--shadow-inset-sm` de las pendientes), borde izquierdo `--text-3`, título tachado en `--text-2` (contenido esencial: `--text-3` es solo para meta, ACCESSIBILITY.md §5; hora y descripción sí en `--text-3`), icono ✓. **Sin `opacity` global** (legibilidad). Sin hover-lift. |
| D3 | Interacción | **Solo click** (abre TaskDrawer → "Reabrir"). Sin arrastrar ni redimensionar. z-index por debajo de las pendientes. |
| D4 | Completar desde calendario | **Check al hover/focus**: círculo neumórfico en bloques no compactos, chips de la fila superior y filas del popup. Click = completar/reabrir sin abrir el drawer. Bloques compactos (<36 px) y minichips del mes: solo vía drawer. En una completada el check queda **siempre visible** (hace de indicador ✓). |
| D5 | Animación | **Hundir + tachar**, solo CSS, `--dur-slow` / `--ease-out`: sombra se hunde, color se apaga a gris, la línea de tachado se dibuja izq→der. Reabrir = inverso. `prefers-reduced-motion`: regla global de `app.css`. |
| D6 | Actualización | **Optimista** (decisión técnica): estado local antes de `invoke`, rollback + toast si falla. Reabrir recalcula con `taskStatus()` (puede quedar `vencida`). |
| D7 | Competencia por espacio | **Pendientes primero**: mes, popup y fila superior ordenan pendientes antes que completadas; el "+N" cuenta todo. Semana/día: completadas entran al algoritmo de columnas normal; en el recorte a 8 visibles se eligen pendientes primero. |
| D8 | Ocultar completadas | **No** hay toggle. Siempre visibles. |
| D9 | "Más limpio" = | **Ruido tipográfico** + **densidad/espaciado**. (Sombras y bordes de controles: fuera.) |
| D10 | Pantallas | **Por fases**. Fase 1: núcleo diario. Fase 2: resto. |
| D11 | Escala tipográfica | **8 pasos, base 13** (ver §2.1). Pesos 400/500/600; 700 solo título de pantalla y hero; 800 eliminado. |
| D12 | Espaciado | **Grid 4 px + aire en contenedores**: todo a `--s-*`; contenedores suben un escalón; bloques y chips del calendario conservan su densidad interna. |
| D13 | Extras | Incluidos: **radios a tokens** y **sombras literales a tokens**. Excluidos: token `--on-primary`, borrar `groupAgenda` (código muerto, se deja como está). |
| D14 | Flujo | **Rama + PR por fase**. `developer` implementa, orquestador revisa, `docs-commits` commitea y documenta. |

---

## 2. Tokens nuevos (`spike/frontend/src/app.css`, `:root`)

### 2.1 Tipografía

```css
--fs-2xs: 10px;  /* overlines, hora mini */
--fs-xs: 11px;   /* meta, horas, chips */
--fs-sm: 12px;   /* secundario, labels */
--fs-base: 13px; /* cuerpo UI */
--fs-md: 14px;   /* inputs, cuerpo destacado */
--fs-lg: 16px;   /* título de card/sección */
--fs-xl: 20px;   /* título de pantalla */
--fs-2xl: 32px;  /* hero de onboarding */
```

Mapeo de literales: `9–10.5 → 2xs` · `11–11.5 → xs` · `12–12.5 → sm` · `13–13.5 → base` · `14–14.5 → md` · `15–18 → lg` · `20–27 → xl` · `38 → 2xl`.
`body { font-size: var(--fs-md); }` (hoy 14 px: sin regresión).

Pesos: `700 → 600` salvo título de pantalla (`--fs-xl`) y hero; `800 → 700`; overlines en 600 (mantienen uppercase + tracking).

### 2.2 Espaciado

Añadir `--s-0_5: 2px;` y `--s-1_5: 6px;` (micro-gaps). Redondear el resto al token más cercano de la escala existente (`--s-1` 4 … `--s-16` 64): `3 → 4`, `5 → 4`, `7 → 8`, `9 → 8`, `10 → 8 ó 12` (según contexto: dentro de componentes denso → 8, contenedor → 12), `11 → 12`, `14 → 12 ó 16`, `18 → 20`, `22 → 24`, `26 → 24`, `30 → 32`.
Hairlines de `1px`/`1.5px` en bordes y outlines quedan literales.
**Contenedores** (cards, paneles, drawer, sidebar, topbar, popup): padding y gap suben **un escalón**. **Contenido denso** (EventBlock, chips, minichips): sin cambios de densidad.

### 2.3 Radios

Añadir `--r-xs: 6px;`. Mapeo: `≤ 9px → --r-xs` · `10–12px → --r-sm` · `14–18px → --r-md` · `22–24px → --r-lg` · `999px → --r-full`. `50%` (círculos) se queda.

### 2.4 Sombras literales

- `TitleBar.svelte:146,150` → `var(--shadow-inset-sm)` (hoy luz blanca fija 0.9: halo claro en dark mode).
- `TaskDrawer.svelte:290,465` → `var(--e3)`.

---

## 3. Fase 1 — rama `feat/ui-completadas-tokens`

Paso 0 (orquestador): capturas **antes** (ver §5).

### T1 · Lógica de presencia: completadas visibles + pendientes primero
**Archivo:** `src/lib/taskDayLogic.ts` (+ test)
- `tasksOnDay` (l.65): quitar `t.status !== "completada"`; actualizar el comentario ("Tareas que cubren el día").
- Nuevo comparador puro y estable `pendingFirst(a, b)`: no-completadas antes que completadas, sin alterar el orden relativo dentro de cada grupo.
- Aplicarlo en `monthChipsOn` y `topChipsOn` (sobre el resultado concatenado).
- `agendaDays` / `groupAgenda`: **no tocar** (D13).
- **Excepción multi-día:** las completadas multi-día solo se ven el día de inicio y el de fin; en los intermedios no. Se resuelve en `tasksOnDay` con un helper puro `isMiddleDay(t, d)` (reutilizado luego en `multiDayChipsOn`, que ya tenía ese cálculo inline): `isMultiDay(t) && !sameDay(t.start, d) && !sameDay(new Date(lastCoveredDayMs(t)), d)`. El `lastCoveredDayMs` es lo que hace que un fin a medianoche cuente como día de fin y no como intermedio. Las pendientes multi-día no cambian.
- Tests (`taskDayLogic.test.ts`): invertir l.128 ("tasksOnDay **incluye** completadas"); nuevos: orden pendientes-primero en `monthChipsOn` y `topChipsOn`, estabilidad del orden cronológico dentro de cada grupo, `isMiddleDay` (inicio/intermedio/tarea de un día/fin a medianoche) y la ausencia de completadas multi-día con horario y all-day en los intermedios, con una pendiente multi-día como control. El test l.192 (agenda) se mantiene.

**Aceptación:** `npm test` verde.

### T2 · Calendar: completadas en la cuadrícula horaria
**Archivo:** `src/lib/Calendar.svelte`
- `grid` (l.163) y `layoutDay` (l.213): quitar la condición `t.status === "completada"` (mantener el salto de `allDay`). Una completada a las 05:00 debe expandir la franja.
- Recorte a 8 visibles en semana: seleccionar **pendientes primero** y luego completadas, **conservando** `top/left/width` ya calculados; `lastShownBottom` usa la misma selección.
- No tocar los filtros de `data.svelte.ts` (`studyConflicts` l.1133, guard l.1580), `Sidebar`, `Widget`, `StudyForm`, `Suggestions`.

**Aceptación:** con datos demo (tarea id 7 completada) la tarea se ve en semana/día/mes/popup.

### T3 · `completeTask` optimista
**Archivo:** `src/lib/data.svelte.ts` (l.1191) (+ test)
- Helper puro exportado `toggledState(t, now)` → `{ status, progress }`: completar = `"completada"`, `100`; reabrir = `taskStatus({ completed_at: null, status: "pendiente" }, t.end.getTime(), now)` y progreso `0` (igual que `set_completed` en el backend, `store.rs`).
- En Tauri: aplicar el nuevo estado **también en la caché** (`putInCache({ ...t, ...nuevo })` + `rebuildTasks()`), no solo mutar el proxy, para que una reconstrucción antes de `tasks:changed` no revierta el cambio. Luego `invoke("task_complete", { id, done })`. Si falla: restaurar el estado previo en caché, `rebuildTasks()` y `setNlToast("No se pudo actualizar la tarea: …", "error")`.
- Modo navegador: misma ruta con el helper.
- Si `store.taskDetail` es esa tarea, apuntarlo al objeto nuevo (y al restaurado en el rollback) para que el botón Completar/Reabrir del drawer cambie al instante.
- `completeTask` devuelve el nuevo `done` (`boolean`); `TaskDrawer.toggleDone` arma el feedback con ese valor (hoy lee `t.status` tras el `await` y en modo navegador sale invertido).
- Tests en `auditFixes.test.ts` (ya importa de `data.svelte`): completar, reabrir futura → `pendiente`, reabrir pasada → `vencida`.

**Aceptación:** `npm test` verde; en Tauri el cambio visual es inmediato.

### T3b · Mostrar `nlToast` (bug previo)
**Archivo:** `src/App.svelte`
`setNlToast` guarda mensajes (p. ej. "No se pudo crear…", "No se pudo actualizar la tarea…") pero **ningún componente los pinta** desde el commit inicial. Sin esto, un fallo del check rápido (T5) solo revierte en silencio.
- Renderizar `nlToast()` en el layout principal: toast flotante abajo al centro, `--surface` + `--e2` + `--r-md`, sin solaparse con `ContextualToast`.
- `source === "error"` → `role="alert"` e indicador `--danger`; resto → `role="status"`, `aria-live="polite"`.
- Entrada/salida con `--dur-base` / `--ease-out`.

### T4 · EventBlock: estado "hundida gris" y bloqueo de drag/resize
**Archivo:** `src/lib/EventBlock.svelte`
- `.evt.done`: `background: var(--surface-2)`, `border-left-color: var(--text-3)`, `box-shadow: var(--shadow-inset)`, `z-index: 0`, sin `transform` en hover, `cursor: pointer`. Eliminar `opacity: 0.5`.
- Textos en done: título en `--text-2`; hora y descripción en `--text-3`; ocultar `prio-dot` / `prio-bar`.
- `.evt.inicio` / `.evt.fin` se declaran **antes** que `.evt.done` (misma especificidad: si no, un stub completado conserva el tinte).
- En compacto (`.evt-inline`) el título va a una línea (`-webkit-line-clamp: 1`): el ✓ resta ancho.
- Tachado animado: envolver el texto del título en `<span class="strike">` inline con
  `background: linear-gradient(currentColor, currentColor) no-repeat 0 55% / 0% 1.5px;`
  `transition: background-size var(--dur-slow) var(--ease-out);` → en done `background-size: 100% 1.5px`. Con `box-decoration-break: slice` (por defecto) la línea recorre las dos líneas del clamp de forma secuencial.
- Transiciones en `.evt`: añadir `background`, `border-color`, `color`, `box-shadow` con `--dur-slow`.
- Drag/resize: si `task.status === "completada"`, `onMove` no llama a `onPointerDown` (el click sigue abriendo el drawer) y no se renderizan los `.resize`.
- Bloques de sesión de estudio: sin cambios.

### T5 · Componente `TaskCheck.svelte` (check rápido)
**Archivos:** nuevo `src/lib/TaskCheck.svelte`; uso en `EventBlock.svelte` y `Calendar.svelte`
- `<button type="button">` circular de 16 px, `--shadow-inset-sm`, borde 1.5 px `--text-3`; hover: borde `--primary`. En done: relleno `--text-3` con ✓ en `--surface`.
- `aria-label`: "Marcar como completada" / "Reabrir tarea"; `aria-pressed={done}`.
- `onpointerdown`, `onclick` **y** `onkeydown` con `stopPropagation()`. El `onkeydown` es obligatorio: el padre de EventBlock abre el drawer con Enter/Espacio. `onclick` → `completeTask(task.id)`.
- Visibilidad: `opacity: 0` → `1` con `:hover` / `:focus-within` del contenedor; **siempre visible si done**.
- Ubicación:
  - EventBlock **no compacto** y no estudio: en la fila de la hora.
  - Chips de la fila superior (`Calendar.svelte` ~l.693) y filas del popup (~l.655): hoy son `<button>`, así que **no se puede anidar** otro botón. Envolver cada uno en `<div class="chip-wrap">` con dos botones hermanos (check + chip).
  - No en minichips del mes ni en bloques compactos.

### T6 · Chips y popup completados
**Archivo:** `src/lib/Calendar.svelte`
- Añadir la clase `done` a `allday-chip` (hoy no la tiene).
- `.minichip.done`, `.allday-chip.done`, `.pop-title.done`: mismo lenguaje que T4 (fondo `--surface-2`, inset, texto `--text-3` tachado, sin opacity).

### T7 · Tokens en `app.css`
Añadir los tokens del §2 (`--fs-*`, `--s-0_5`, `--s-1_5`, `--r-xs`) y `body { font-size: var(--fs-md) }`. Sin migrar componentes todavía.

### T8 · Migración del núcleo diario a tokens
Tres sub-tareas, un diff revisable cada una:
- **T8a** `Calendar.svelte`, `EventBlock.svelte`, `TaskCheck.svelte`
- **T8b** `TopBar.svelte`, `Sidebar.svelte`, `TitleBar.svelte`, `App.svelte`
- **T8c** `TaskDrawer.svelte`, `QuickAdd.svelte`, `Widget.svelte`

En cada una: `font-size` → `--fs-*`, pesos según §2.1, `padding/gap/margin` → `--s-*` con aire en contenedores (§2.2), `border-radius` → `--r-*` (§2.3), sombras literales (§2.4).

**Aceptación T8:** en esos archivos, `grep -E "font-size:\s*[0-9]"` devuelve 0; ningún espaciado fuera de escala salvo hairlines; capturas sin regresiones de layout en light y dark.

### T9 · Documentación (agente `docs-commits`)
- `docs/frontend/DESIGN.md`: §3.4 fila "Completada" → hundida gris; §4 escala con tokens `--fs-*` y regla de pesos; §5 espaciado (`--s-0_5`, `--s-1_5`, regla de aire en contenedores); radios (`--r-xs`, mapeo).
- `docs/frontend/COMPONENTS.md`: `TaskCheck`; estados de EventBlock; orden pendientes-primero en Calendar.
- `docs/frontend/UX.md`: l.73 ("Completado") y l.112 ("se limpian del flujo" → siguen visibles en el calendario y se excluyen de widget y contadores).

**Cierre de fase 1:** PR `feat/ui-completadas-tokens` → `master`.

---

## 4. Fase 2 — rama `feat/ui-polish-resto`

Mismas reglas que T8, aplicadas al resto de pantallas, en grupos de 2–3 componentes por tarea. Las capturas "antes" de la fase 2 (2026-09-30) destaparon defectos visuales previos que se corrigen en la tarea del componente afectado:

- **T10** `Settings.svelte`: solo tokens.
- **T11** `Onboarding.svelte` (hero 38 → `--fs-2xl`, 800 → 700), `Login.svelte`. Los enlaces del Login usan el azul por defecto del navegador (contraste pobre en dark) → `var(--primary)`.
- **T12** `Assistant.svelte`, `PlanProposal.svelte`. Quitar el encabezado de página duplicado "Asistente" (el título vive en la TopBar, ver T12b); se conservan subtítulo y chips.
- **T12b** `TopBar.svelte`: la vista `asistente` no está contemplada (muestra "Semana" y las flechas) → título "Asistente" sin navegación. `text-transform: capitalize` pone en mayúscula cada palabra ("Sesiones De Estudio", "30 De Septiembre") → quitarlo y poner mayúscula solo a la inicial en JS.
- **T13** `Suggestions.svelte`, `ContextualToast.svelte`, `ClassConflictDialog.svelte`. Quitar el encabezado duplicado "Eventos detectados" (se conservan subtítulo y botón "Comprobar correo ahora"). Modal de conflicto: `--shadow-raised-lg` → `--e3` (la luz blanca del relieve deja un halo sobre el overlay).
- **T14** `StudySessions.svelte`, `StudyForm.svelte`. Bloques de sesión con el texto recortado arriba/abajo; primera etiqueta del gutter ("6 a") cortada; en el formulario las horas se truncan ("10:01 |") y la etiqueta en dos líneas desalinea la fila; modal → `--e3`.
- **T15** `Schedule.svelte`, `ClassForm.svelte`, `WidgetPage.svelte`. Primera etiqueta del gutter cortada; horas truncadas en el formulario ("10:00 :"); modal → `--e3`.
- **T16** Docs: marcar la migración como completa en DESIGN.md y documentar los arreglos.

**Aceptación de fase:** `grep -rE "font-size:\s*[0-9]" src/lib src/App.svelte` → 0. PR → `master`.

---

## 5. Verificación (cada tarea)

En `spike/frontend`:
1. `npm test` (vitest)
2. `npx svelte-check`
3. `npm run build`
4. Capturas antes/después (orquestador, playwright-cli sobre `npm run dev` en modo navegador con datos demo): semana, día, mes + popup, drawer abierto; light y dark.
5. Antes de cada PR: smoke en Tauri (`npm run tauri dev`): completar desde el check, reabrir, reiniciar la app y confirmar que persiste; comprobar que un fallo de `invoke` revierte.

---

## 6. Flujo de agentes

| Rol | Agente | Hace | No hace |
|-----|--------|------|---------|
| Orquestador | Claude | Reparte T#, revisa el diff, corre §5, aprueba | — |
| Developer | `developer` (opencode, `b/deepseek-v4.1-flash`, build) | Implementa una T# a la vez | Commits, docs |
| Docs + commits | `docs-commits` (opencode, `big-pickle`, build) | Un commit por T# aprobada (conventional commits en español, p. ej. `feat(calendario): mantener visibles las tareas completadas`), T9/T16, PRs | Código de features |

---

## 7. Fuera de alcance

Toggle para ocultar completadas · token `--on-primary` · borrar `groupAgenda`/`agendaDays` · listar completadas en el widget · check en minichips del mes y en bloques compactos · cambios en backend · bump de versión.
