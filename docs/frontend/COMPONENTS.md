# COMPONENTS.md — Componentes de FocusFlow

**Fuente:** código real de `spike/frontend/src/lib/`. Cada componente documenta: propósito, anatomía, variantes, estados, comportamiento, responsive, accesibilidad y relación con el neumorfismo.

> **Convención:** los componentes marcados **[PROPUESTO]** NO existen aún como abstracción global (hay lógica duplicada entre componentes) y son candidatos a extracción. No son inventos: son consolidaciones de código que ya existe repetido.

---

## AppShell — `App.svelte`

- **Propósito:** orquesta toda la app: título, sidebar, topbar, vistas (calendario/agenda/sugerencias/asistente/ajustes), drawer, toast contextual, widget y manejo de errores fatales.
- **Anatomía:** `TitleBar` → `div.body` → (`Sidebar` + `main.content` → `TopBar` + vista). Overlays: `TaskDrawer`, `ContextualToast`, `fatal error`.
- **Estados:** `bootReady` (splash mínimo), `authUser()` (→ Login), `onboardingPending` (→ Onboarding), `taskDetail()` (drawer), `fatalError` (banner role=alert).
- **Comportamiento:** enrutado por hash (`#/widget` → WidgetPage); atajos de teclado (Escape cierra drawer/widget); escucha eventos Tauri (`task:open`, `nav:agenda`, `nav:assistant`, `ui:prefs`).
- **Responsive:** `flex column` 100vh; `cal-wrap` con scroll propio; min-width 0 para no desbordar.
- **Accesibilidad:** landmark `main`; error fatal con `role="alert"`.
- **Neumorfismo:** el shell es base (`--bg`); solo los contenedores de contenido tienen raised.

---

## Sidebar — `Sidebar.svelte`

- **Propósito:** navegación principal + resumen del día + categorías + tema.
- **Anatomía:** logo (F en cuadrado `--primary`), nav (Semana/Mes/Día/Agenda/Asistente/Sugerencias/Ajustes con iconos SVG), sección Categorías (punto de color + count), caja "hoy" (día + número + pendientes), botón Tema.
- **Variantes:** item activo (`--primary-soft` + `--primary` + weight 600), badge de sugerencias pendientes.
- **Estados:** hover `--surface-2`; activo `--primary-soft`.
- **Comportamiento:** llama `setView`/`navigate`; el nav no usa `<a>` (SPA por estado).
- **Responsive:** 232 px fijos; colapsable conceptualmente (64 px en spec), no implementado en v0.1.2.
- **Accesibilidad:** botones reales; iconos `aria-hidden` por defecto (no lo tienen explícito — deuda). El badge de pendientes no anuncia cambios (mejora propuesta).
- **Neumorfismo:** sidebar flota sobre base (sin tarjeta); solo la caja "hoy" lleva `--shadow-raised`. *Buena aplicación del "no saturar de relieves".*

---

## TopBar — `TopBar.svelte`

- **Propósito:** título de la vista + navegación temporal (anterior/siguiente/hoy) + QuickAdd.
- **Anatomía:** título (22 px/700) + botones arrow (34 px raised e1) + "Hoy" + `<QuickAdd/>`.
- **Estados:** hover: translateY(-1px) + e2; active: `--shadow-inset-sm` (se hunde — *press táctil correcto*).
- **Accesibilidad:** `aria-label` en arrows; el título es texto plano (semántica de h1 real propuesta).
- **Neumorfismo:** botones raised individuales sobre base plana — jerarquía clara.

---

## Calendar — `Calendar.svelte` (el corazón del producto)

- **Propósito:** renderiza mes, semana y día como **espacio visual del tiempo**.
- **Anatomía:**
  - **Mes:** `month-head` (7 columnas) + grid 6×7 celdas (80 px alto) con chips de tareas + popup de día (`day-popup`).
  - **Semana/Día:** `week-head` (días clicables) + `week-body` con gutter de horas + columnas por día; cada columna = `allday-row` (chips todo-el-día) + `time-area` (slots, now-line, EventBlocks).
- **Estados:** celda today (anillo `--primary-soft-2`), día fuera de mes (opacity 0.4), hover de celda (translateY(-1px) + e1), drag (`week-body.dragging` atenúa eventos y resalta allday-row).
- **Comportamiento (lo más sofisticado del frontend):**
  - Grid horario dinámico: 6:00–22:00 por defecto, se expande si hay tareas fuera.
  - `pxH` dinámico vía ResizeObserver (el área siempre llena la ventana).
  - **Layout de columnas:** clústeres de eventos encadenados → se reparten en columnas proporcionales (algoritmo tipo "calendario clásico").
  - **Drag & drop:** mover (con grabY en px para precisión), resize inicio/fin, drop a "Todo el día"; snap a 5 min SOLO al soltar; ghost que sigue al cursor; auto-scroll en bordes.
  - Multi-día: stubs de 2 h "Inicio ·"/"Fin ·" + chips continuos intermedios.
  - Mes: chips con `chipTextFor` (Inicio/Fin/continuo), "+N más", popup con "Ver día completo".
- **Responsive:** semana limita a 8 eventos + "+N más"; el mes mantiene 6 filas fijas con scroll propio.
- **Accesibilidad:** celdas de mes `role="button"` + Enter/Espacio; EventBlocks `role="button"` + Enter; resizes con `role="separator"` + `aria-label`. **Deuda:** drag & drop no tiene alternativa de teclado (mejora propuesta).
- **Neumorfismo:** contenedor raised (`--r-lg` + `--shadow-raised`); slots del time-area con línea `--border` (inset sutil); EventBlocks inset-sm con borde izquierdo de categoría — *el calendario es la pieza que mejor expresa el sistema*.

---

## DayView / WeekView / MonthView — vistas dentro de Calendar

- **Propósito:** modos del mismo componente (no archivos separados — misma lógica de segmentos y layout).
- **Comportamiento diferencial:** DayView muestra todos los eventos (sin límite 8); WeekView limita a 8 + "+N más" y agrupa por días; MonthView usa celdas 80 px + chips + popup.
- **Relación con neumorfismo:** todas heredan el contenedor raised y los chips inset.

---

## TimeGrid — lógica en `Calendar.svelte` + `taskDayLogic.ts`

- **Propósito:** posiciona eventos en el tiempo (top/height en px) y decide qué tarea "está en" un día.
- **Lógica pura** (testeada en Vitest): `coversDay`, `segmentFor`, `layoutMetrics`, `topChipsOn`, `monthChipsOn`, `chipTextFor`, `groupAgenda`.
- **Detalle clave:** los minutos se calculan **relativos al día de inicio del segmento** — un fin a medianoche = 1440 min (bug histórico resuelto).
- **Neumorfismo:** los slots son la "zona hundida" lista para recibir drag.

---

## Task (modelo) — `Task` en `data.svelte.ts`

- **Propósito:** entidad central.
- **Campos:** id, title, categoryId, priority (alta/media/baja), status (pendiente/completada/en-curso/vencida), start/end, allDay, tags, progress, description, notes, links, reminderMinutes.
- **Derivación de estado:** `status` se deriva en el frontend: `completed_at` → completada; pasada → vencida; else pendiente.
- **Semántica visual:** vencida = dashed border + danger-bg; completada = tachado + 50 % opacidad; prioridad alta = punto/barra danger.

---

## TaskCard — `TaskCard.svelte` (agenda)

- **Propósito:** fila de tarea en Agenda.
- **Anatomía:** checkbox circular (22 px, `--r-full`) + body (título + meta: hora, chip categoría, badge prioridad, tag "Vencida").
- **Variantes:** `done` (tachado, opacity 0.55), `overdue` (danger-bg + borde izquierdo danger), `pop` (scale 0.98 al completar).
- **Estados:** hover translateY(-1px) + e1; checkbox hover = anillo success suave.
- **Accesibilidad:** `role="button"` + Enter/Espacio; checkbox real con `aria-label="Completar tarea"`; stopPropagation correcto.
- **Neumorfismo:** card raised con borde izquierdo 3 px del color de categoría.

---

## EventBlock — `EventBlock.svelte` (bloque en calendario)

- **Propósito:** bloque visual dentro del time-area de día/semana.
- **Anatomía:** tiempo (10 px/700 tabular) + título (11.5 px/600, clamp 2) + descripción (si alto ≥ 62 px) + handle de resize top/bottom.
- **Variantes:** `compact` (< 36 px: solo hora + título inline), `tall` (≥ 62: muestra descripción), `inicio`/`fin` (stubs multi-día), `overdue` (dashed), `done` (tachado), `ghost` (drag).
- **Estados:** hover translateY(-1px) scale(1.01) + e1 + z-index 3; resize handles aparecen en hover.
- **Accesibilidad:** `role="button"` + Enter/Espacio; tooltip rico (`title`) con descripción/prioridad/estado; handles `role="separator"` con `aria-label`.
- **Neumorfismo:** fondo = color de categoría al 13 % sobre surface + `--shadow-inset-sm` + borde izquierdo 3 px sólido del color. *Inset = "es parte del tiempo", no flota sobre él.*

---

## AllDayTask — chips de "Todo el día" (en Calendar)

- **Propósito:** tareas sin hora fija (all-day) y multi-día intermedias.
- **Anatomía:** fila `allday-row` con label "Todo el día" + chips (`allday-chip`) con color de categoría; ghost de drop; "+N más".
- **Variantes:** `cont` (multi-día continuo: borde dashed), `ghost` (durante drag).
- **Comportamiento:** arrastrar un evento a esta fila lo convierte en all-day.
- **Accesibilidad:** botones con `title`; texto de rango en el tooltip.

---

## QuickAdd — `QuickAdd.svelte`

- **Propósito:** captura por lenguaje natural — la entrada principal del producto.
- **Anatomía:** input inset (44 px, `--surface-3`, `--shadow-inset`, radio 15) con icono rayo `--primary`, placeholder con ejemplo, `kbd` "Ctrl⇧Espacio"; preview flotante con chips de entidades detectadas + botón Planificar; banner "IA lenta" con fallback local; toast de confirmación.
- **Estados:** `:focus-within` = borde `--primary` + anillo inset; `expanded` cuando hay preview; `disabled` durante procesado (anti doble-envío).
- **Comportamiento:**
  - Detección local de entidades (mañana/el N/próximo lunes/horario/urgente/recordatorio/categoría) → chips de color.
  - Enter → `planFromText` (IA) → propuesta; evento único se **auto-acepta**; plan multi-item requiere revisión.
  - Si IA tarda > 8 s → "Usar interpretación rápida" (parser local).
  - Enter repetido bloqueado (guardia anti-duplicado).
- **Accesibilidad:** input real con placeholder; kbd como hint visual; chips de preview decorativos (deuda: sin `aria-live` — el estado "detectado" no se anuncia).
- **Neumorfismo:** **inset puro** — es el elemento hundido más representativo de la app. El preview flota con e3.

---

## AIRecommendation — [PROPUESTO]

No existe como componente único. El concepto se implementa en: `PlanProposal.svelte` (propuesta de plan), `Suggestions.svelte` (eventos del correo) y las respuestas `Answer`/`Action` del `Assistant.svelte`. Propuesta de consolidación documentada para la landing y futuros refactors, no como componente actual.

---

## SuggestionReview — `Suggestions.svelte`

- **Propósito:** bandeja de eventos detectados del correo para revisar antes de aceptar.
- **Anatomía:** card por sugerencia: kind badge (Evento/Vencimiento/Disponibilidad/Tarea) + status + remitente + confianza; título h3; descripción + razón (itálica); meta (chip categoría, fecha/hora, prep, prioridad); aviso de duplicado; acciones (Aceptar/Editar/Fusionar/Rechazar/Borrar).
- **Estados:** `pending` (botones de acción), `settled` (aceptada/rechazada/fusionada/auto-aprobada → revertir/editar/borrar, con cuenta atrás de 1 h), edición inline (card con formulario), fusión (select de tarea).
- **Comportamiento:** botón "Comprobar correo ahora"; estados con semántica de color (pending=primary, accepted=success, rejected=danger, merged=primary).
- **Responsive:** cards apiladas, max-width 760 px.
- **Neumorfismo:** cards raised; formularios de edición inset.
- **Deuda:** no usa `<dialog>`; los formularios son `<div class="card edit">`.

---

## EmailInsight — [PROPUESTO]

La lógica de email vive en el backend Rust (detección) y se presenta en `Suggestions.svelte`. No hay componente separado. La landing debe ilustrar este flujo con una composición propia, no con un componente inexistente.

---

## Widget — `Widget.svelte`

- **Propósito:** extensión de un vistazo (open → glance → act).
- **Anatomía:** header (logo F + "FocusFlow" + status "hoy · N hechas" con pulso verde) → body con secciones Ahora/Por hacer/Siguiente/Importante → footer (Abrir ⤢ / Preguntar).
- **Estados:** sección `now` (label `--primary`), task con dot de categoría, remaining badge (primary), due badge (text-2 / danger si importante), empty state ("Todo claro por ahora"), acciones rápidas (✓ ⟳ ▶) aparecen en hover.
- **Comportamiento:** reloj local cada 30 s; prioridad de secciones: current → relevant → next → important; `widgetAction` (complete/postpone/start) vía IPC; abre app/agenda/asistente.
- **Accesibilidad:** botones con `title`; focus-visible en qa-btns; el estado "todo claro" es texto. Deuda: sin `aria-live` para cambios de sección.
- **Neumorfismo:** contenedor `--r-xl` + `--shadow-raised-lg` + borde `--border` — *la pieza flotante por excelencia*.

---

## Button — [PROPUESTO como `Button.svelte`]

**No existe como componente global.** El patrón real está duplicado:
- Primario: `--primary` / `#fff` / radio 12 / padding 8–9px 14–18px / 600 / hover `--primary-hover` / active scale(0.98).
- Secundario: `--surface-2`→hover `--surface-3` (o `--surface-3` base en TaskDrawer).
- Ghost: transparente / hover `--surface-2` / (en TaskDrawer: borde `--border`).
- Danger: `--danger` texto / hover `--danger-bg`.

**Propuesta:** consolidar en `Button.svelte` con variantes `primary | secondary | ghost | danger`, tamaños `sm | md | lg`, y estados estándar (hover translateY(-1px), active inset/scale, focus ring). La landing NO depende de esto (es standalone), pero la doc lo registra como deuda a pagar.

---

## IconButton — patrón real (no componente)

Usado en: TitleBar (controles), TopBar (arrows), Popups (close), TaskDrawer (✕). Anatomía: cuadrado 30–46 px, `--surface-2` hover, radio 10–14, icono SVG 12–16 px. Consistente en comportamiento, inconsistente en tamaño.

---

## Input — patrón inset (no componente)

Real: `input/select/textarea` con `background: var(--surface-3)`, `box-shadow: var(--shadow-inset-sm)`, radio 10–12, focus = anillo `--primary-soft-2`. En TaskDrawer usa además `border: 1px solid var(--border)` y focus con `0 0 0 3px var(--primary-soft)`. **Dos idiomas de foco detectados** (ver DESIGN §9.2).

---

## Modal — `PlanProposal.svelte` (el más completo)

- **Propósito:** revisión y aprobación de propuestas de plan.
- **Anatomía:** overlay (fondo `--bg` 55 % + blur 3px) + modal centered (560 px, `--r-lg`, e3, borde): head (logo rayo + "Plan sugerido" + sub + chip fuente IA/Local + ✕), body ("Entendí" intents + "Plan propuesto" items con sesiones y edición de bloques), footer (total de bloques + Editar/Cancelar/Aceptar plan).
- **Estados:** edición de bloques (fecha/hora por sesión, añadir/quitar), errores inline, busying.
- **Accesibilidad:** `aria-label` en overlay; Escape cancela; botones con texto claro. Deuda: no es un `<dialog>` nativo (sin focus trap/aria-modal).
- **Neumorfismo:** floating e3 sobre overlay blur — jerarquía de profundidad correcta.

---

## Toast — patrón múltiple (ver deuda DESIGN §9.2)

- QuickAdd toast: fixed bottom-center, `--surface`, borde-izq success, e2.
- Drag toast (Calendar): fixed bottom-center, `--danger` sólido, e2.
- ContextualToast: bottom-right, `--surface-2`, `--r-xl`, `--shadow-raised-lg`, animación rise.

---

## Dropdown / Select — patrón nativo (no componente)

Se usan `<select>` nativos (TaskDrawer, Suggestions, Settings, Onboarding) con estilo inset. Sin dropdown personalizado. Consistencia OK.

---

## Toggle — patrón real

Checkbox estilizado para all-day (TaskDrawer) y switches de ajustes (`label.check > input`). No hay `Switch.svelte` global — **propuesto**.

---

## EmptyState / LoadingState / ErrorState

- **EmptyState:** `Suggestions.svelte` ("Sin eventos detectados todavía" + sub), Widget ("Todo claro por ahora"), popup de mes ("Sin tareas este día").
- **LoadingState:** indicador de typing del Asistente ("Analizando tu calendario…"), botones con "Procesando…/Comprobando…/Guardando…", sync progress bar.
- **ErrorState:** `fatalError` banner (role=alert), `assistantError` + Reintentar, errores inline de formularios, `slowAi` banner con fallback local.
- **Propuesta:** consolidar como componentes reutilizables (actualmente son bloques locales).

---

## Plan de consolidación (priorizado)

1. **`Button.svelte`** — elimina 5 duplicaciones (mayor impacto/riesgo bajo).
2. **Unificar foco**: un solo idioma (`:focus-visible` global + ring en inputs).
3. **Unificar toasts** en un `Toast.svelte` con variantes (success/error/info).
4. **`Modal.svelte`** con `<dialog>` nativo (focus trap + aria-modal + Escape).
5. **Switch/Toggle** y **EmptyState/Loading/Error** como componentes.
6. `IconButton` con tamaños fijos.

> Ninguna de estas consolidaciones se hace "porque sí": se documentan como deuda real detectada en el código (ver DESIGN §9.2). La landing se construye de forma standalone y no las bloquea.
