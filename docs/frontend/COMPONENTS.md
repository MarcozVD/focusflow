# COMPONENTS.md — Componentes de FocusFlow

**Fuente:** código real de `spike/frontend/src/lib/`. Cada componente documenta: propósito, anatomía, variantes, estados, comportamiento, responsive, accesibilidad y relación con el neumorfismo.
**Última actualización:** 2026-10-01 (spec 19: layout responsivo, tokens de relieve en controles, panel del sidebar, modales sobre overlay desenfocado).

> **Convención:** los componentes marcados **[PROPUESTO]** NO existen aún como abstracción global (hay lógica duplicada entre componentes) y son candidatos a extracción. No son inventos: son consolidaciones de código que ya existe repetido.

---

## AppShell — `App.svelte`

- **Propósito:** orquesta toda la app: título, sidebar, topbar, vistas (calendario/agenda/sugerencias/asistente/ajustes), drawer, toast contextual, widget y manejo de errores fatales.
- **Anatomía:** `TitleBar` → `div.body` → (`Sidebar` + `main.content` → `.content-inner` → `TopBar` + vista). Overlays: `TaskDrawer`, `ContextualToast`, `fatal error`.
- **Scroll unificado (spec 19):** `main.content` es **el único scroller** (`overflow-y: auto; overflow-x: hidden`), así que la TopBar sube con el contenido y nada pasa por debajo de una barra. Dentro, `.content-inner` es la columna centrada (`max-width: 1680px`; `class:reading` → `880px` en Ajustes/Sugerencias/Asistente) con `padding: 0 clamp(var(--s-10), 3vw, var(--s-16)) clamp(var(--s-6), 2.5vw, var(--s-12))`. `.cal-wrap` + `.view-fill` propagan la cadena flex para que las vistas de calendario llenen el alto.
- **Reset de scroll:** un `$effect` dependent de `view` y `hmode` pone `contentEl.scrollTop = 0`. Sin él, el scroll unificado arrastraba el desplazamiento de la vista anterior al cambiar de vista o de submodo (horario/sesiones); las flechas de fecha dentro de la misma vista no lo disparan.
- **Estados:** `bootReady` (splash mínimo), `authUser()` (→ Login), `onboardingPending` (→ Onboarding), `taskDetail()` (drawer), `fatalError` (banner role=alert), `nlToast()` (toast flotante).
- **Toast `nlToast`:** `App` pinta el mensaje que guarda `setNlToast` en `data.svelte.ts` — errores de acciones rápidos ("No se pudo crear…", "No se pudo actualizar la tarea…"). Va **abajo al centro**, con `--surface` + `--e2` + `--r-card`. Si `source === "error"` usa `role="alert"` y borde izquierdo `--danger`; el resto, `role="status"` con `aria-live="polite"`.
- **Manejo de errores:** el handler global filtra el aviso benigno `ResizeObserver loop…` del navegador (`console.debug`, no `fatalError`); venía del `ResizeObserver` del calendario al redimensionar y congelaba la app con un error que no lo era.
- **Comportamiento:** enrutado por hash (`#/widget` → WidgetPage); atajos de teclado (Escape cierra drawer/widget); escucha eventos Tauri (`task:open`, `nav:agenda`, `nav:assistant`, `ui:prefs`).
- **Responsive:** `flex column` 100vh; `min-width: 0` en la columna para no desbordar; mínimo de ventana 960×640 (`tauri.conf.json`).
- **Accesibilidad:** landmark `main`; error fatal con `role="alert"`.
- **Neumorfismo:** el shell es la superficie única (`--bg` = `--surface`); el relieve lo llevan el panel del sidebar y las tarjetas de cada vista.

---

## Sidebar — `Sidebar.svelte`

- **Propósito:** navegación principal + resumen del día + categorías + tema.
- **Anatomía:** panel `div.side` → `div.side-scroll` (logo, nav, categorías, caja "hoy", botón Tema). Nav con iconos SVG y etiquetas; sección Categorías (punto de color + count); caja "hoy" (día + número + pendientes); botón Tema y botón de contraer/expandir.
- **Panel (spec 19):** margen `--s-4` a izquierda/alto/bajo, `--r-card`, `--shadow-raised` sobre `--surface`; 232 px de ancho. La zona scrolleable (`.side-scroll`) lleva fundido de 16 px arriba y abajo con `mask-image` y `padding: var(--s-4) 0 var(--s-5)` para que el último elemento no toque el borde redondeado.
- **Modo iconos:** `type SidebarMode = "auto" | "collapsed" | "expanded"`, con la preferencia en `localStorage` (`ff.sidebar`, lectura con try/catch). En `auto` se contrae sola con `matchMedia("(max-width: 1199px)")`; el botón manual guarda la preferencia y **vuelve a `auto`** cuando lo elegido coincide con lo automático. Colapsado = 72 px **con el padding dentro**.
- **Contenido en modo iconos (D12):** se ocultan la marca, las etiquetas del nav, las categorías y la caja de "hoy" (quedan fuera de flujo, no `display: none`, para no perder el texto accesible). Cada item lleva `title` + `aria-label`, el contador de Sugerencias se posa **sobre** su icono, y los botones de añadir horario, añadir sesión, tema y expandir son circulares.
- **Estados:** nav activo hundido (`--primary-soft` + `--shadow-inset-sm` + `--primary` + peso 600); nav hover elevado (`--btn-shadow`); botón de añadir en píldora con relieve.
- **Comportamiento:** llama `setView`/`navigate`; el nav no usa `<a>` (SPA por estado). La transición de ancho usa `--dur-base` y respeta `prefers-reduced-motion` (global, 120 ms).
- **Accesibilidad:** botones reales con `aria-expanded` + `aria-label` en el botón de contraer/expandir; los labels fuera de flujo siguen leyéndose.
- **Neumorfismo:** panel elevado y caja de "hoy" como pozo (`--surface-2` + `--shadow-inset-sm`).

---

## TopBar — `TopBar.svelte`

- **Propósito:** título de la vista + navegación temporal (anterior/siguiente/hoy) + QuickAdd.
- **Anatomía:** título (`--fs-xl`/700, `letter-spacing -0.02em`) + flechas circulares de 34 px + "Hoy" en píldora + `<QuickAdd/>`. En Horario y Sesiones añade el conmutador Semana/Día.
- **Título por vista:** `Semana`, `Mes`, `Día`, `Mi horario`, `Sesiones de estudio`, `Eventos detectados`, `Ajustes`, `Asistente`. La vista `asistente` muestra su título **sin** el bloque de navegación, igual que Sugerencias y Ajustes.
- **Capitalización:** la mayúscula inicial la aplica el helper compartido `capitalizeFirst` (`dateUtils.ts`). **No** hay `text-transform: capitalize`.
- **Estados:** flechas y "Hoy" con `--btn-shadow`, hover `--btn-shadow-hover`, active `--btn-shadow-active` (el press hunde). El switcher es un **riel hundido** (`--shadow-inset-sm`) y la opción activa va elevada con `--grad-accent`.
- **Accesibilidad:** `aria-label` en flechas y en el switcher (`role="group"` en Ajustes); el título es texto plano (semántica de h1 real propuesta).
- **Neumorfismo:** controles elevados sobre la superficie; el switcher invierte la jerarquía (riel hundido + opción elevada).

---

## Calendar — `Calendar.svelte` (el corazón del producto)

- **Propósito:** renderiza mes, semana y día como **espacio visual del tiempo**.
- **Anatomía:**
  - **Mes:** `month-head` (7 columnas) + grid 6×7 celdas con chips de tareas + popup de día (`day-popup`). La grilla usa `grid-template-rows: repeat(6, minmax(80px, 1fr))` + `flex: 1`: **las filas se estiran** para llenar el alto de la tarjeta (antes eran 80 px fijos y a 2560×1440 quedaban ~170 px vacíos).
  - **Semana/Día:** `week-head` (días clicables) + `week-body` con gutter de horas + columnas por día; cada columna = `allday-row` (chips todo-el-día) + `time-area` (pozo con slots, now-line, EventBlocks).
- **Estados:** celda today (anillo de acento `outline: 1.5px solid var(--primary)` con `outline-offset: -1.5px`, sin relleno duro), día fuera de mes (opacity 0.4), hover de celda, drag (`week-body.dragging` atenúa eventos y resalta allday-row).
- **Tareas completadas:** **visibles en todas las vistas** (semana, día, mes y popup), con el estado hundido gris de DESIGN §3.4. Se dibujan en la cuadrícula horaria como cualquier otra tarea y **expanden la franja visible** si caen fuera del rango por defecto. Hay dos ajustes para que no compitan por el espacio:
  - **Excepción multi-día:** una completada de varios días **solo aparece en su día de inicio y en su día de fin**; en los intermedios no se pinta. Lo resuelve `isMiddleDay(t, d)` (`taskDayLogic.ts`), que además usa `multiDayChipsOn` para el chip "cont". Sin esto el calendario se llenaba de la misma tarea repetida en gris en cada día intermedio. Las pendientes multi-día no cambian.
  - **Orden pendientes-primero:** el comparador puro `pendingFirst` (`taskDayLogic.ts`) ordena sin tocar el orden cronológico dentro de cada grupo; se aplica en `monthChipsOn` y `topChipsOn`.
  - **Recorte de la semana a 8 visibles:** `visibleWeekPlaced` selecciona con `pendingFirst` **conservando las posiciones ya calculadas** (`top/left/width`); `lastShownBottom` usa esa misma selección para dibujar el botón "+N más".
- **Estructura de los chips:** los chips de la fila superior y las filas del popup se envuelven en `<div class="chip-wrap">` (clase `done` cuando la tarea está completada) para poder contener el `TaskCheck` como botón hermano sin anidar botones.
- **Comportamiento (lo más sofisticado del frontend):**
  - Grid horario dinámico: 6:00–22:00 por defecto, se expande si hay tareas fuera.
  - `pxH` dinámico vía ResizeObserver (el área siempre llena la ventana). La medida se hace en `requestAnimationFrame` y **solo asigna `timeAreaH` si cambió** (cancelando el frame en el cleanup): la asignación síncrona encadenaba otra medida y disparaba el aviso benigno `ResizeObserver loop`, que `App.svelte` filtra además del banner fatal.
  - **Layout de columnas:** clústeres de eventos encadenados → se reparten en columnas proporcionales (algoritmo tipo "calendario clásico").
  - **Drag & drop:** mover (con grabY en px para precisión), resize inicio/fin, drop a "Todo el día"; snap a 5 min SOLO al soltar; ghost que sigue al cursor; auto-scroll en bordes.
  - Multi-día: stubs de 2 h "Inicio ·"/"Fin ·" + chips continuos intermedios.
  - Mes: chips con `chipTextFor` (Inicio/Fin/continuo), "+N más", popup con "Ver día completo".
- **Responsive:** semana limita a 8 eventos + "+N más"; el mes estira sus filas y, si no cabe, scrollea `.content` junto con la TopBar.
- **Accesibilidad:** celdas de mes `role="button"` + Enter/Espacio; EventBlocks `role="button"` + Enter; resizes con `role="separator"` + `aria-label`. **Deuda:** drag & drop no tiene alternativa de teclado (mejora propuesta).
- **Neumorfismo:** tarjetas del calendario `--r-card` + `--shadow-raised-lg`; `.time-area`, `.allday-row` y cada celda del mes son pozos (`--shadow-inset-sm`); los bloques y chips llevan **relieve mínimo** de 2 px sobre su tinte de categoría y las completadas siguen hundidas; chips y "+N más" en `--r-chip`.

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
- **Semántica visual:** vencida = dashed border + danger-bg; completada = superficie hundida gris + tachado + ✓ (en el calendario); prioridad alta = punto/barra danger.
- **Cambio de estado:** `completeTask(id)` es **optimista** — aplica el estado antes del `invoke` y devuelve el nuevo `done` (o `null` si el backend falla, tras restaurar el estado previo y avisar con `nlToast`). El cálculo puro vive en `toggledState(t, now)`, que replica el `set_completed` del backend: al reabrir, `completed_at` a null, progreso 0 y el estado visible recalculado con `taskStatus` (puede quedar **vencida** si el fin ya pasó).

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
- **Anatomía:** tiempo (`--fs-2xs`/600 tabular) + título (`--fs-xs`/600, clamp 2) + descripción (si alto ≥ 62 px) + handle de resize top/bottom.
- **Variantes:** `compact` (< 36 px: solo hora + título inline), `tall` (≥ 62: muestra descripción), `inicio`/`fin` (stubs multi-día), `overdue` (dashed), `done` (completada), `ghost` (drag).
- **Estado `done` (completada):** superficie **hundida gris** — `background: var(--surface-2)`, `box-shadow: var(--shadow-inset)`, borde izquierdo `--text-3`, `z-index: 1`, **sin `opacity` global** y sin hover-lift. Título tachado con animación izq→der (`--dur-slow`) en `--text-2`; hora y descripción en `--text-3`; `prio-dot`/`prio-bar` ocultos. En bloques compactos añade el ✓ (el `TaskCheck` de `EventBlock` no cabe: el título se limita a una línea para no perder ancho).
  - Las reglas `.inicio`/`.fin` se declaran **antes** que `.done` a propósito: tienen la misma especificidad, y si no, un stub completado conservaría el tinte de categoría.
- **Apilado (`z-index`), para que las sesiones no tapen las tareas:** `.evt.study` = **0** (una sesión de estudio cede siempre), `.evt.done` = **1**, `.evt` pendiente = **2**, `.evt:hover` (incluido el de las completadas y las sesiones) = **4**, y el fantasma del arrastre `.evt.ghost` = **5**. La franja de clase `.class-strip` comparte el 2 con los bloques pendientes. Escala deliberadamente con huecos (no 0-1-2-3) para poder intercalar el hover y el arrastre sin reordenar.
- **Barra de prioridad:** `.prio-bar` solo se pinta cuando el bloque tiene **≥ 50 px** de alto (`height >= 50`). Antes salía en cualquier bloque alto, incluso los que no tenían sitio, y desplazaba el texto.
- **Comportamiento en `done`:** **no se puede arrastrar ni redimensionar** (`onMove` no llama a `onPointerDown` y no se renderizan los handles `.resize`). El click sigue abriendo el drawer, que es la vía para reabrir.
- **Estados:** hover translateY(-1px) scale(1.01) + relieve ampliado (4 px) + z-index 4; resize handles aparecen en hover.
- **Accesibilidad:** `role="button"` + Enter/Espacio; tooltip rico (`title`) con descripción/prioridad/estado; handles `role="separator"` con `aria-label`.
- **Neumorfismo:** fondo = color de categoría al 13 % sobre surface + **relieve mínimo** (`2px 2px 5px` con `neu-dark`/`neu-light`, 4 px en hover) + borde izquierdo 3 px sólido del color. Las **completadas se quedan hundidas** (`--shadow-inset-sm`): el relieve es lo que las distingue.

---

## TaskCheck — `TaskCheck.svelte` (check rápido de completar/reabrir)

- **Propósito:** completar o reabrir una tarea **sin abrir el drawer**, desde el propio calendario.
- **Props:** `task: Task` (obligatoria) y `size = 16` (px; el calendario usa 14 en los chips de la fila superior).
- **Anatomía:** `<button type="button">` circular con un SVG de check `aria-hidden`. Reposo: borde 1.5 px `--text-3` sobre `--surface` + `--shadow-inset-sm`. Hover: borde `--primary` y ✓ al 60 %. `done`: relleno `--text-3` con ✓ en `--surface`. `active`: `scale(0.92)`.
- **Comportamiento:** `onclick` → `completeTask(task.id)`. **Detiene `pointerdown`, `keydown` y `click` con `stopPropagation()`** — es obligatorio: sin eso el gesto arrastraría el bloque o abriría el drawer, y Enter/Espacio lo activarían dos veces.
- **Visibilidad:** el componente no se oculta a sí mismo; la controla el **padre** con el envoltorio `.check-slot` (`opacity: 0` → `1` con `:hover` / `:focus-within` del contenedor). En una completada el check queda **siempre visible** y hace de indicador ✓.
- **Dónde se usa:** bloques no compactos de `EventBlock` (no en bloques compactos < 36 px ni en sesiones de estudio), chips de la fila superior y filas del popup del mes. **No** en los minichips del mes.
- **Restricción de anidamiento:** los chips y las filas del popup ya son `<button>`, así que `Calendar` los envuelve en `<div class="chip-wrap">` con el check y el chip como **botones hermanos** (`<div>` no puede contener un `<button>` dentro de otro).

---

## AllDayTask — chips de "Todo el día" (en Calendar)

- **Propósito:** tareas sin hora fija (all-day) y multi-día intermedias.
- **Anatomía:** fila `allday-row` con label "Todo el día" + chips (`allday-chip`) con color de categoría; ghost de drop; "+N más".
- **Variantes:** `cont` (multi-día continuo: borde dashed), `done` (completada: superficie hundida `--surface-2`, título tachado en `--text-2`, sin opacity), `ghost` (durante drag).
- **Comportamiento:** arrastrar un evento a esta fila lo convierte en all-day.
- **Accesibilidad:** botones con `title`; texto de rango en el tooltip.

---

## QuickAdd — `QuickAdd.svelte`

- **Propósito:** captura por lenguaje natural — la entrada principal del producto.
- **Anatomía:** campo hundido en **píldora** (44 px, `--input-bg` + `--input-shadow` + `--input-border`, `--r-control`) con icono rayo `--primary`, placeholder con ejemplo, `kbd` "Ctrl⇧Espacio"; preview flotante con chips de entidades **elevados** + botón «Planificar» (`--r-control` con `--grad-accent` + `--btn-primary-shadow`); banner "IA lenta" con fallback local; toast de confirmación.
- **Estados:** `:focus-within` = borde `--primary` (el anillo lo aporta el `:focus-visible` global); `expanded` mantiene el radio del control arriba (`--r-control --r-control --r-chip --r-chip`); `disabled` durante procesado (anti doble-envío).
- **Comportamiento:**
  - Detección local de entidades (mañana/el N/próximo lunes/horario/urgente/recordatorio/categoría) → chips de color.
  - Enter → `planFromText` (IA) → propuesta; evento único se **auto-acepta**; plan multi-item requiere revisión.
  - Si IA tarda > 8 s → "Usar interpretación rápida" (parser local).
  - Enter repetido bloqueado (guardia anti-duplicado).
- **Accesibilidad:** input real con placeholder; kbd como hint visual; chips de preview decorativos (deuda: sin `aria-live` — el estado "detectado" no se anuncia).
- **Neumorfismo:** **pozo puro** (es el campo hundido más representativo de la app) y píldora; chips del preview elevados y «Planificar» con degradado + brillo. El preview flota con `--e2`/`--e3`.

---

## AIRecommendation — [PROPUESTO]

No existe como componente único. El concepto se implementa en: `PlanProposal.svelte` (propuesta de plan), `Suggestions.svelte` (eventos del correo) y las respuestas `Answer`/`Action` del `Assistant.svelte`. Propuesta de consolidación documentada para la landing y futuros refactors, no como componente actual.

---

## SuggestionReview — `Suggestions.svelte`

- **Propósito:** bandeja de eventos detectados del correo para revisar antes de aceptar.
- **Encabezado:** **sin h2 propio** — el título «Eventos detectados» lo pone la TopBar. La vista abre con el subtítulo («Eventos extraídos de tus correos por la IA. Revisa antes de añadirlos al calendario.») y el botón «Comprobar correo ahora» en una sola fila centrada (`align-items: center`); el h2 duplicado se retiró en la fase 2.
- **Anatomía:** card por sugerencia: kind badge (Evento/Vencimiento/Disponibilidad/Tarea) + status + remitente + confianza; título h3; descripción + razón (itálica); meta (chip categoría, fecha/hora, prep, prioridad); aviso de duplicado; acciones (Aceptar/Editar/Fusionar/Rechazar/Borrar).
- **Estados:** `pending` (botones de acción), `settled` (aceptada/rechazada/fusionada/auto-aprobada → revertir/editar/borrar, con cuenta atrás de 1 h), edición inline (card con formulario), fusión (select de tarea).
- **Comportamiento:** botón "Comprobar correo ahora"; estados con semántica de color (pending=primary, accepted=success, rejected=danger, merged=primary).
- **Responsive:** cards apiladas, max-width 760 px.
- **Neumorfismo:** cards `--r-card` + `--shadow-raised`; formularios de edición con campos hundidos en `--r-control`; botones de acción con `--r-control` y el principal con degradado; chips de categoría con relieve mínimo.
- **Deuda:** no usa `<dialog>`; los formularios son `<div class="card edit">`.

---

## Asistente — `Assistant.svelte`

- **Propósito:** chat con el asistente IA sobre el calendario (respuestas y acciones propuestas).
- **Encabezado:** **sin h1/h2 propio** — el título «Asistente» lo pone la TopBar (y sin navegación temporal). La vista conserva subtítulo y chips; el encabezado duplicado se retiró en la fase 2.
- **Estados:** mensajes que entran con fade; `task-ref-level` (chip de nivel); errores con "Reintentar"; indicador "Analizando tu calendario…".
- **Neumorfismo:** burbujas de la IA con relieve suave (`--shadow-raised-sm`), las del usuario con `--grad-accent`, chips de nivel y de tarea elevada (`--shadow-raised-sm`) y la tarjeta de acción propuesta como tarjeta elevada (`--r-card` + `--shadow-raised`).

---

## Ajustes — `Settings.svelte`

- **Propósito:** tema, acento, forma, sincronización, notificaciones e información de la cuenta.
- **Selector «Forma» (Apariencia):** `role="group"` + `aria-label="Forma"`, con los dos valores del store: **Rectangular** (`soft`, por defecto) y **Redondeada** (`round`). Mismo patrón de switcher que el tema: riel hundido + opción activa elevada con `--grad-accent`. Al pulsar, `setUiPrefs({ shape })` cambia los radios de **toda** la app en caliente.
- **Preferencia `shape`:** `"soft" | "round"`, expuesta por `uiShape()`. No se guarda en el backend (que solo persiste `theme` y `accent`): vive en la clave **`ff-ui`** de `localStorage` (`{ theme, accent, shape }`).
- **Sincronización entre ventanas:** `initUiPrefsSync()` (llamado desde `init()`) escucha el evento `storage` y, cuando otra ventana escribe `ff-ui`, reaplica todo con `applyUiPrefs()`. Es lo que hace que el **widget** cambie de forma sin abrir la ventana principal. El backend sigue siendo fuente de verdad de tema y acento; `localStorage` es el fast path y la única fuente de la forma.
- **Aplicación:** `applyShape()` valida el valor, pone `data-shape="round"` en `<html>` o **elimina el atributo** en la forma rectangular, y lo refleja en `store.shape`.

Ver DESIGN §6 para la tabla completa de tokens y valores por forma.

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
- **Neumorfismo:** contenedor `--r-card` + `--shadow-raised` + borde `--border` + `margin: var(--s-4)`. Baja de `--raised-lg` porque la ventana es fija y transparente: la sombra larga se recortaba contra el borde de la pantalla, y el margen la deja respirar. Los botones del pie son `--r-control` elevados (el primero, «Abrir», con `--grad-accent` + brillo), las acciones rápidas usan `--r-icon` y los chips llevan relieve mínimo conservando su color semántico.
- **Forma:** el widget lee la misma preferencia `shape` que el resto de la app; no tiene selector propio (es una ventana sin Ajustes).

---

## StudySessions / Schedule — sesiones de estudio y horario

- **Propósito:** `StudySessions.svelte` (bloques de sesión de estudio) y `Schedule.svelte` (bloques de clase) dibujan la cuadrícula semanal/diaria con la misma silueta que `EventBlock`.
- **Bloques:** contenido **alineado arriba** (`justify-content: flex-start`): si no cabe, solo se recorta por abajo, sin comerse el texto. En **compacto (< 36 px)** pasan a una línea (hora + título inline, `-webkit-line-clamp: 1`); el título no compacto es clamp a 2 líneas. En sesiones, la tarea vinculada ("↳ …") solo se muestra a partir de 62 px.
- **Gutter:** la franja superior cede aire (`padding-top` `--s-1_5`) para que la primera etiqueta de hora («6 a») no salga cortada por el `translateY(-6px)`.
- **Modales (`StudyForm` / `ClassForm`):** overlay `--overlay` con `blur(8px)` y panel `--r-card` + `--shadow-raised-lg` (antes `--e3`, para esquivar el halo) con `width: min(480px, 100%)` para que las etiquetas no partan («Día de la semana *»). La fila usa `align-items: end`, de modo que fecha, horas, tarea relacionada y notas quedan alineadas aunque una etiqueta ocupe dos líneas; la columna del horario es de ancho natural y los inputs de hora reservan `min-width: 124px` para mostrar la hora completa («11:01 p. m.») junto al icono de reloj.

---

## Button — patrón real (propuesto como `Button.svelte`)

**No existe como componente global.** El patrón real (spec 19) está duplicado en cada componente, pero ya sale de los mismos tokens:
- Primario: `--grad-accent` + `--btn-primary-shadow` + `#fff`, `--r-control` / hover `filter: brightness(1.05)` / active `--btn-shadow-active`.
- Secundario: `--surface` + `--btn-shadow` en `--r-control` / hover `--btn-shadow-hover` / active hundido.
- Ghost: `--surface` + relieve en `--r-control` (dejó de ser transparente para no romper el lenguaje).
- Danger: texto `--danger` sobre la superficie en relieve (dejó de pintarse en rojo sólido).

**Propuesta:** consolidar en `Button.svelte` con variantes `primary | secondary | ghost | danger`, tamaños `sm | md | lg`, y estados estándar (hover de relieve, active inset, foco global).

---

## IconButton — patrón real (no componente)

Usado en: TitleBar (controles), TopBar (flechas), Popups (close), TaskDrawer (✕), ClassForm/StudyForm (✕), Sidebar (colapsar, tema, añadir). Anatomía: **botón de icono** con `--r-icon` (10 px en forma rectangular, círculo en la redondeada) sobre `--surface` con `--btn-shadow`, icono SVG 12–16 px, hover `--btn-shadow-hover`, active `--btn-shadow-active`. `title` + `aria-label` en todos.

---

## Input — patrón hundido (no componente)

`input/select/textarea` con `--input-bg` (`--surface-3`), `--input-shadow` (`--shadow-inset-sm`), `--input-border` (`1px solid var(--line-input)`, D14) y `--r-control` (los textarea en `--r-card`). El foco **solo cambia el borde** a `--primary`; el anillo lo aporta el `:focus-visible` global de `app.css` — TaskDrawer ya no añade su `box-shadow` propio, así que **hay un solo idioma de foco** (resuelto en el spec 19; ver DESIGN §10.3).

---

## TaskDrawer — `TaskDrawer.svelte`

- **Propósito:** ficha de una tarea con edición completa, propuesta de fecha/hora y confirmación de borrado.
- **Anatomía:** overlay (`--overlay` + `backdrop-filter: blur(--overlay-blur)`) + panel `.drawer` **flotante**: margen `--s-4` en los cuatro lados, `--r-card`, `--shadow-raised-lg`, `overflow: hidden`, `width: min(400px, calc(100vw - 2 * var(--s-4)))`. Antes iba pegado al borde derecho con borde izquierdo sólido.
- **Head:** título + botón de cierre con `--r-icon` (círculo en forma redondeada) y `--btn-shadow`.
- **Cuerpo:** campos hundidos en `--r-control` (`--input-bg`/`--input-shadow`/`--input-border`; los textarea en `--r-card`); el foco **solo cambia el borde** a `--primary` y el anillo lo da el `:focus-visible` global (se retiró su `box-shadow` propio, que era el segundo idioma de foco del repo). La zona scrolleable lleva fundido de 16 px arriba y abajo con `mask-image`.
- **Interruptor «Todo el día»:** sigue siendo un `<input type="checkbox">` nativo (sin `Switch.svelte`) estilizado como interruptor: riel 40×22 **siempre en `--r-full`** (píldora en ambas formas) con knob circular elevado y estado activo con `--grad-accent`.
- **Pie:** botones en `--r-control` con `--btn-shadow` (hover `--btn-shadow-hover`, active hundido); «Guardar» con `--grad-accent` + `--btn-primary-shadow`; los peligrosos en texto `--danger`.
- **Diálogo de borrado:** overlay propio + panel `--r-card` + `--shadow-raised-lg`, con `role="dialog"` + `aria-modal="true"` + `aria-labelledby` (sin focus trap: deuda).
- **Accesibilidad:** overlay con click handler sin role ni teclado (deuda pendiente de `<dialog>` nativo).

---

## Modal — `PlanProposal.svelte` (el más completo)

- **Propósito:** revisión y aprobación de propuestas de plan.
- **Anatomía:** overlay (`--overlay` + `backdrop-filter: blur(8px)`; antes `color-mix(--bg 55%)` + blur 3 px) + modal centrado (`--r-card`, `--shadow-raised-lg`, borde `--border`): head (logo rayo + "Plan sugerido" + sub + chip fuente IA/Local + ✕ con `--r-icon`), body ("Entendí" intents + items de plan con sesiones y edición de bloques, cada propuesta como tarjeta elevada), footer (total de bloques + Editar/Cancelar/Aceptar plan).
- **Estados:** edición de bloques (fecha/hora por sesión, añadir/quitar), errores inline, busying.
- **Accesibilidad:** `aria-label` en overlay; Escape cancela; botones con texto claro. Deuda: no es un `<dialog>` nativo (sin focus trap/aria-modal).
- **Neumorfismo:** panel marcado sobre overlay claro desenfocado — el halo que obligaba antes a bajar a `--e3` desaparece porque el overlay es del propio gris del tema (ver DESIGN §3.5).

---

## Toast — patrón múltiple (ver deuda DESIGN §10.2)

- QuickAdd toast: fixed bottom-center, `--surface`, borde-izq success, e2.
- Drag toast (Calendar): fixed bottom-center, `--danger` sólido, e2.
- ContextualToast: bottom-right, `--surface-2`, `--r-card`, `--e2` (deja `--shadow-raised-lg`, cuyo relieve dejaba un halo sobre el overlay), animación rise.

> Las barras de scroll están **ocultas globalmente** (`* { scrollbar-width: none }` + `::-webkit-scrollbar { display: none }` en `app.css`), pero el `overflow` de cada scroller se conserva: rueda, trackpad y teclado siguen desplazando. Ver DESIGN §9.4.

---

## Dropdown / Select — patrón nativo (no componente)

Se usan `<select>` nativos (TaskDrawer, Suggestions, Settings, Onboarding) con estilo inset. Sin dropdown personalizado. Consistencia OK.

---

## Switch / Toggle — patrón real (propuesto como `Switch.svelte`)

**Siguen siendo `<input type="checkbox">` nativos** con `appearance: none`; no hay componente global — **propuesto**.

- **Anatomía:** riel 40×22 px **siempre en `--r-full`** (píldora en ambas formas) con `--input-border` y `--shadow-inset-sm` sobre `--surface` + knob circular de 16 px elevado con `--btn-shadow`, desplazado 18 px al activarse.
- **Activo:** riel con `--grad-accent` y borde transparente.
- **Dónde:** «Todo el día» del `TaskDrawer` y los interruptores de Ajustes (correo, Google Calendar: al abrir/minimizar/cerrar, conflictos estrictos). Las píldoras de proveedor y el switcher de tema (`role="group"` + `aria-label="Tema"`) son otros patrones: las píldoras activas usan `--grad-accent` + `--btn-primary-shadow`.
- **Accesibilidad:** se conserva el input real, así que teclado, `aria-checked` implícito y label envolvente siguen funcionando; el interruptor es puramente visual.

---

## EmptyState / LoadingState / ErrorState

- **EmptyState:** `Suggestions.svelte` ("Sin eventos detectados todavía" + sub), Widget ("Todo claro por ahora"), popup de mes ("Sin tareas este día").
- **LoadingState:** indicador de typing del Asistente ("Analizando tu calendario…"), botones con "Procesando…/Comprobando…/Guardando…", sync progress bar.
- **ErrorState:** `fatalError` banner (role=alert), `assistantError` + Reintentar, errores inline de formularios, `slowAi` banner con fallback local.
- **Propuesta:** consolidar como componentes reutilizables (actualmente son bloques locales).

---

## Plan de consolidación (priorizado)

1. **`Button.svelte`** — elimina las duplicaciones (mayor impacto/riesgo bajo); los tokens ya unifican el aspecto.
2. ~~**Unificar foco**~~ **hecho (spec 19):** un solo idioma, el `:focus-visible` global; los campos solo cambian el borde.
3. **Unificar toasts** en un `Toast.svelte` con variantes (success/error/info).
4. **`Modal.svelte`** con `<dialog>` nativo (focus trap + aria-modal + Escape).
5. **Switch/Toggle** y **EmptyState/Loading/Error** como componentes.
6. `IconButton` con tamaños fijos.

> Ninguna de estas consolidaciones se hace "porque sí": se documentan como deuda real detectada en el código (ver DESIGN §10.2). La landing se construye de forma standalone y no las bloquea.
