# 08 — Implementation Audit (FASE 1 — completo)

**Producto:** FocusFlow · **Versión:** v0.1 · **Fecha:** 2026-08-08
**Alcance:** auditoría completa código vs PRD (doc 01), frontend + backend + arquitectura + IA. Sin cambios de código en esta fase.
**Estado:** ✅ Motor de recordatorios implementado (cierra la brecha P0 #1 del PRD). ✅ FASE 2 (estabilización) aplicada: ver §10.

---

## 0. FASE 2 — estabilización aplicada (2026-08-08)

| Fix | Qué | Archivos | Test |
|-----|-----|----------|------|
| 🔴 **Bug crítico de zona horaria** | `day_ms - day_ms % DAY` alineaba la medianoche local contra días de epoch: en UTC-5 todas las horas del parser heurístico caían **5 h antes** ("a las 4 PM" → 11 AM). Ahora `start = day_ms + min*60000` sin floor | `ai/nl.rs` | `en_examples_land_on_correct_date_and_time`, `es_keywords_unaffected`, `absolute_date_with_month` |
| 🟡 Hora fantasma por dígitos sueltos | `hour_from_text` aceptaba cualquier dígito ("el 15" → 15:00, "2 horas" → 02:00). Ahora solo con am/pm, prefijo temporal ("a las/at/de las") o `HH:MM` | `ai/nl.rs` | cubierto por los tests anteriores |
| 🟡 NL en inglés (FR-07/12) | Heurística ahora entiende tomorrow/today/next Monday + días EN, meses EN, categorías EN (exam/submit/meeting/pay...) | `ai/nl.rs` | `en_examples_land_on_correct_date_and_time` |
| 🟡 Duración (FR-07) | "durante 3 horas", "for two hours", "1h30m", "media hora" → end = start + duración (si hay hora de inicio); sin hora → Todo el día. Rango explícito manda. Prompt LLM actualizado | `ai/nl.rs`, `ai/task_parser.rs` | `duration_extends_end_time_only_with_start`, `duration_variants` |
| 🟡 Fechas absolutas (FR-07) | "el 15 de agosto" → fecha correcta (año actual/siguiente); título limpio | `ai/nl.rs` | `absolute_date_with_month` |
| 🟡 "Recordarme X" (FR-08) | Heurística crea recordatorio 60 min si el texto pide recordar (antes solo el LLM lo hacía) | `ai/nl.rs` | `recordarme_adds_reminder` |
| 🟡 Drift chip Repetición | Chip prometía repeticiones que el backend no implementa → eliminado; chips ahora detectan "tomorrow"/"at HH"/"urgent" | `QuickAdd.svelte` | svelte-check |
| 🟢 Código muerto | Eliminados command `notify`, `widget_set_height` y `widgetHeight` (sin llamadores) | `lib.rs`, `data.svelte.ts` | cargo test (sin referencias) |

**Verificación e2e (hook FF_NL_TEST, vía IA):** "Tomorrow at 4 PM study calculus" → mañana 16:00–17:00 ✓ · "Exam Friday at 8 AM" → viernes 08:00–09:00, uni, alta ✓ · "Submit project next Monday" → lunes, Todo el día ✓ · "Study calculus for two hours Thursday" → jueves, Todo el día (duración sin hora de inicio = no inventar hora) ✓.

---

## 1. Executive summary

| Capa | Nota (1–10) | Veredicto |
|------|-------------|-----------|
| Calendario (mes/semana/día) | 8.5 | Sólido: DnD con snap, conflictos, multi-día, mes "+N más", hoy destacado, línea de hora |
| Tareas + Agenda | 8 | CRUD completo, soft delete, duplicar, drawer de edición completa, agenda mixta 7 días |
| Quick Add + NL | 7 | Preview editable + chips + heurística ES + LLM con validación. Sin score de confianza ni subrayado |
| Email → sugerencias | 7 | IMAP + checkpoint UID + trusted senders + auto-aprobación + dedupe + revert/merge |
| Widget | 7.5 | Transparente, always-on-bottom, completar desde widget, tema heredado. Fijo 340×500, sin compacto/expandido |
| **Recordatorios** | **8** | **Motor completo**: tick 60 s, disparo al arrancar, rearranque al cambiar, persistencia `reminder_fired_at`. Pendientes P1: múltiples, acciones toast, centro perdidas |
| Bandeja + atajo + autostart | 9 | Estables, con fallbacks |
| Seguridad/privacidad | 8 | Offline-first, claves en keyring (Windows Credential Manager), cero telemetría. Sin backup (riesgo real) |
| Tests | 3 | 8 tests Rust (reminders/store). Cero tests frontend. NFR-14 incumplido |
| **Nota global** | **7.2** | Core de captura + calendario + recordatorios funcional. La promesa P0 del PRD ("recordar en el momento correcto") ya dispara. El gran hueco ahora es **datos (backup/búsqueda/categorías)** y la **IA como Time Manager** (FASE 3) |

---

## 2. Architecture assessment

**Stack:** Tauri 2 (Rust) + Svelte 5 + SQLite (rusqlite bundled) + IMAP + LLM OpenAI-compatible. Local-first 100 %, sin backend, sin telemetría. Repo: `https://github.com/MarcozVD/focusflow` (origin configurado, público).

**Puntos fuertes:**
- Separación limpia backend: `store.rs` (datos) / `lib.rs` (commands + setup) / `sync.rs` (email+scheduler) / `reminders.rs` (motor) / `ai/*` (parser multicapa).
- Parser multicapa conforme a FR-13: `parse_task_text(texto, provider, configured)` → LLM si está configurado, si no heurística ES (`nl.rs`). Nunca confía en JSON del LLM sin validar (`validation.rs`).
- `with_db` (sync.rs:59) evita repetición de `state.lock().unwrap()`.
- Checkpoint de sync por UID con abort sin avanzar ante fallo de red/IA; `sync_history` auditable.
- Frontend: store reactivo único (`data.svelte.ts`), caché de semanas con eviction (64 máx), última-solicitud-vence en NL, listeners idempotentes (doble ventana widget/main).

**Debilidades:**
- `lib.rs` monolítico (1218 líneas, 38 commands). Refactor a `commands/` antes de FASE 3.
- **Categorías con 3 fuentes de verdad**: `data.svelte.ts` (UI) + `seed_if_empty` (store.rs) + prompts IA (task_parser/email_parser). Cualquier cambio de categoría rompe sincronía; por eso CRUD (FR-44) es imposible sin refactor.
- Migraciones SQL inline (`CREATE IF NOT EXISTS` + guards de columnas) sin versionado formal; hay naming 0001–0005 pero no tabla `schema_version`.
- `status` en DB solo pendiente/en-curso/completada; "vencida"/"cancelada" son derivados frontend (divergencia del modelo de datos PRD).
- Sin capa de logging estructurado (log a archivo por `append_log`).
- Widget creado en runtime (no declarado en tauri.conf.json): correcto, pero `widget_info` no expone `always_on_bottom` ni posición.

---

## 3. Feature matrix (PRD vs código)

Estado: ✅ COMPLETE · 🟡 PARTIAL · 🔴 BROKEN · ❌ MISSING · 📄 DOCUMENTED BUT NOT IMPLEMENTED (documentado pero no implementado — README promete algo que no existe)

| Feature | Specification (PRD) | Actual implementation | Status | Bugs / notas | Priority |
|---------|---------------------|-----------------------|--------|--------------|----------|
| Entrada NL con preview editable (FR-01) | Campo global, parser interpreta fecha/horas/duración/categoría/etiquetas/prioridad, preview antes de confirmar | QuickAdd con preview editable + chips de entidades + parse heurístico/LLM (ES+EN, duración, fechas absolutas) | ✅ COMPLETE | Sin subrayado de entidades con color; no muestra tags/ubicación en preview | P0 |
| Formulario manual de tarea (FR-02) | Todos los campos del modelo + validación en vivo | **No existe.** Creación solo por QuickAdd/NL o duplicar | ❌ MISSING | FR P0 incumplido; edición sí cubre los campos (FR-03) | P0 |
| Edición completa (FR-03) | Mismo formulario en modal/panel | TaskDrawer: título, descripción, categoría, prioridad, fechas/horas, all-day, tags, notas, links, recordatorio | ✅ COMPLETE | — | P0 |
| Edición por arrastre (FR-04) | Arrastrar cambia fecha/hora; Ctrl+duplica; multi-selección | DnD con grabY, snap 5 min / resize 30 min, clamps, conflictos (`conflict_strict`) | 🟡 PARTIAL | Ctrl+duplicar ✗; multi-selección ✗ | P1 |
| Duplicar tarea (FR-05) | Duplicar con/sin fechas | `task_duplicate` (copias fechas; sin opción "plantilla") | ✅ COMPLETE | Sin opción sin-fechas | P2 |
| Papelera y restauración (FR-06) | Eliminación reversible 30 días | Soft delete (`deleted_at`) pero sin UI de papelera ni restauración | 🟡 PARTIAL | Datos no se pierden, pero no hay forma de recuperar | P2 |
| Entidades mínimas NL (FR-07) | Fechas relativas/absolutas, horas, rangos, duración | Relativas ✓; absolutas "el 15 de agosto" ✓ (FASE 2); horas ✓ (am/pm, 24h, rangos, "at"); duración "2 horas"/"for two hours" ✓ (end = start + duración) | ✅ COMPLETE | Duración sin hora de inicio → Todo el día (regla: no inventar hora) | P0 |
| Acciones NL (FR-08) | "Recordarme X el [fecha]" → tarea + recordatorio; contexto semántico → categoría | Prefijos limpiados; "recordarme" → recordatorio 60 min en heurística Y LLM (FASE 2); categorías ES+EN por keywords | ✅ COMPLETE | Solo un recordatorio por tarea (FR-27 pendiente) | P0 |
| Categoría/prioridad por keyword (FR-09) | "urgente"→Alta; "examen"→Universidad; "pagar"→Finanzas | Heurística y LLM con mapeo idéntico | ✅ COMPLETE | — | P1 |
| Repetición por texto (FR-10) | "todos los lunes", "cada mes"... | **No implementado** (congelado FASE 9). El chip "Repetición" del QuickAdd **sugiere la feature** | 📄 DOCUMENTED BUT NOT IMPLEMENTED | Drift UX: chip promete función inexistente | P2 |
| Confianza del parseo (FR-11) | Score < umbral → previsualizar y confirmar | Preview editable siempre (mejor garantía que score), sin score explícito | 🟡 PARTIAL | Sin métrica de confianza ni umbral | P1 |
| Soporte idiomas (FR-12) | ES P0, EN P1, parser multicapa | Heurística ES sólida; EN parcial (am/pm, "next week" solo LLM) | 🟡 PARTIAL | — | P2 |
| Parser intercambiable (FR-13) | Contrato `parse(texto, contexto) → Intent` | `AiProvider` trait + `parse_task_text` multicapa + validación estricta | ✅ COMPLETE | Retorna `ParsedTask`, no `Intent` (ver §7 AI readiness) | P0 |
| Vistas día/semana/mes (FR-14) | 3 vistas con transiciones | Mes/semana/día + agenda + sugerencias + ajustes; transiciones parciales | ✅ COMPLETE | — | P0 |
| Agenda mixta (FR-15) | Hoy + próximos 7 días cronológico | Agenda.svelte: grupos por día, hasta 5 por grupo | ✅ COMPLETE | Slice de 5 por día (oculta el resto) | P0 |
| Navegación (FR-16) | Flechas, Hoy, clic en fecha, rueda+shift | Flechas + Hoy ✓; clic en fecha ✗; rueda+shift ✗ | 🟡 PARTIAL | — | P0 |
| Hoy destacado (FR-17) | Acento primario + línea de hora actual | Daynum con acento + línea de hora en día/semana | ✅ COMPLETE | — | P0 |
| Días completos (FR-18) | Cabecera de bloque en mes | Chips all-day en mes + fila allday | ✅ COMPLETE | — | P1 |
| Resaltado prioridad/categoría (FR-19) | Color categoría, borde alta, badge vencida | Colores por categoría + badge "vencida" (derivada) | ✅ COMPLETE | — | P1 |
| Mini-mes lateral (FR-20) | Navegador de meses | — | ❌ MISSING | — | P2 |
| Modelo de tarea completo (FR-21) | Doc 03 | tasks: título, descripción, categoría, prioridad, estado, fechas, all_day, progreso, tags, notas, links, reminder_minutes, soft delete | 🟡 PARTIAL | Sin location en tasks (sí en sugerencias); sin múltiples reminders; sin checklist | P0 |
| Estados (FR-22) | pendiente/en curso/completada/cancelada | pendiente, en-curso, completada ✓; cancelada ✗; vencida derivada | 🟡 PARTIAL | — | P0 |
| Progreso (FR-23) | Slider 0–100, "+25 %" contextual | Campo `progress` + drawer; sin menú "+25 %" | 🟡 PARTIAL | — | P2 |
| Subtareas/checklist (FR-24) | V2 | — | ❌ MISSING | Congelado | P2 |
| Enlaces y notas (FR-25) | Notas multilínea + enlaces clicables | Campos notes/links en drawer | ✅ COMPLETE | — | P1 |
| Adjuntos (FR-26) | P3 | — | ❌ MISSING | Congelado | P3 |
| Múltiples recordatorios (FR-27) | Lista de recordatorios por tarea | Un único `reminder_minutes`; del LLM solo se aplica el primero | ❌ MISSING | **P0 del PRD, pendiente P1** | P0 |
| Predefinidos (FR-28) | 1 día / 3 h / 1 h / 15 min (toggles) | Campo libre de minutos; sin toggles predefinidos | ❌ MISSING | — | P0 |
| Recordatorio personalizado (FR-29) | Absoluto o relativo ("-2d 09:00") | Relativo en minutos ("30", "1d", "3h", "1w" vía NL/IA) | 🟡 PARTIAL | Sin fecha/hora absoluta | P1 |
| Recálculo al mover (FR-30) | Mover propaga a recordatorios, sin duplicar | Disparo derivado de `start_at`; mover no refira; cambiar minutos rearma (`reminder_fired_at`) | ✅ COMPLETE | Test cubre: `move_to_does_not_refire` | P0 |
| Toast nativo Windows (FR-31) | App minimizada, bandeja o **cerrada** | Toast nativo ✓; dispara minimizada/en bandeja; **cerrada no** (bandeja + autostart lo mitigan: proceso sigue vivo) | 🟡 PARTIAL | Límite documentado; cerrada = sin motor (proceso muerto) | P0 |
| Acciones en notificación (FR-32) | Abrir / Completar / Posponer | Solo toast informativo | ❌ MISSING | — | P1 |
| Bandeja del sistema (FR-33) | Cerrar → bandeja; salir → menú | Cerrar X → bandeja + abre widget automáticamente; menú Abrir/Salir | ✅ COMPLETE | UX discutible: X abre el widget de golpe (configurable `close_to_tray_widget`) | P0 |
| Autostart (FR-34) | Iniciar con Windows en segundo plano | `start_with_windows` + `start_minimized` (winreg) | ✅ COMPLETE | — | P1 |
| "Mientras no estabas" (FR-35) | Vencidas agrupadas al abrir | Primer tick al arrancar dispara vencidas (toasts sueltos); sin agrupación en UI | 🟡 PARTIAL | — | P1 |
| Silenciar/posponer todo (FR-36) | Modo no molestar | — | ❌ MISSING | — | P2 |
| Widget flotante transparente (FR-37) | Always-on-top, sin borde, transparencia, esquinas redondeadas | Transparente, sin borde, **always-on-bottom** (no estorba), 340×500 fija, esquinas redondeadas por CSS | ✅ COMPLETE | Elige bottom sobre top (decisión consciente del usuario) | P1 |
| Contenido configurable (FR-38) | Próximas tareas/entregas por categoría, contador regresivo | Hoy + próximas + completadas hoy | 🟡 PARTIAL | Sin configuración por categoría ni contador | P1 |
| Modo compacto/expandido (FR-39) | Alternar con animación | Ventana fija; `widget_set_height` y `widgetHeight` son **código muerto** del intento previo | 📄 DOCUMENTED BUT NOT IMPLEMENTED | Código residual sin llamadores (ver §5) | P1 |
| Tema heredado (FR-40) | Sigue tema de la app | `ui:prefs` difunde a todas las ventanas; localStorage fast-path | ✅ COMPLETE | — | P1 |
| Acciones widget (FR-41) | Completar desde widget; clic abre la app | Checkbox completa ✓; clic abre main + `task:open` ✓ | ✅ COMPLETE | — | P2 |
| Multi-monitor (FR-42) | Recordar posición por monitor | Solo monitor primario (work_area bottom-right) | ❌ MISSING | — | P1 |
| Categorías por defecto (FR-43) | 6 con color e icono | 6 hardcodeadas con color+icono (uni/trab/per/fin/sal/otr) | ✅ COMPLETE | 3 fuentes de verdad (UI, seed, prompts) | P0 |
| CRUD de categorías (FR-44) | Crear/editar/eliminar con reasignación | **No existe** (ni tabla ni commands) | ❌ MISSING | Bloqueado por duplicación de fuentes de verdad | P1 |
| Prioridades (FR-45) | Alta/Media/Baja con color | ✓ con colores semánticos | ✅ COMPLETE | — | P0 |
| Filtros combinables (FR-46) | Categoría, prioridad, estado, etiquetas, rango, vencidas | **No existen** | ❌ MISSING | — | P1 |
| Búsqueda (FR-47) | Incremental fuzzy sobre título/descripción/tags/notas/links | **No existe** | ❌ MISSING | — | P1 |
| Estadísticas (FR-48) | Panel completadas/racha/top categorías | — | ❌ MISSING | Congelado | P2 |
| Export CSV (FR-49) | P3 | — | ❌ MISSING | Congelado | P3 |
| Repetición RRULE (FR-50/51) | Subset RRULE + ocurrencias bajo demanda | — | ❌ MISSING | Congelado FASE 9; chip QuickAdd lo sugiere (drift) | P2 |
| Offline-first (FR-52) | 100 % sin red | Todo local; IA/sync degradan con error claro | ✅ COMPLETE | — | P0 |
| Backup automático (FR-53) | Copia rotativa diaria + restauración | **No existe.** Único archivo SQLite sin copia | ❌ MISSING | **Riesgo real de pérdida de datos** | P1 |
| Export/Import JSON (FR-54) | Interoperabilidad | — | ❌ MISSING | — | P1 |
| Export/Import iCal (FR-55) | Puente Google/Outlook/Apple | — | ❌ MISSING | — | P1 |
| Temas (FR-56) | Claro/oscuro/**sistema** | Claro/oscuro persistente en backend + localStorage; sin seguir el tema del OS | 🟡 PARTIAL | Faltaría "sistema" (matchMedia) | P0 |
| Shortcuts globales (FR-57) | Ctrl+Shift+Espacio captura; Ctrl+K paleta (V2) | Ctrl+Shift+Espacio con 3 fallbacks (Ctrl+Alt+Espacio, Ctrl+Shift+T, Ctrl+Shift+K); Ctrl+K ✗ (V2) | ✅ COMPLETE | — | P1 |
| i18n (FR-58) | ES + EN, tokens | Español hardcodeado en toda la UI | ❌ MISSING | — | P2 |
| Onboarding (FR-59) | Primera ejecución 3 pasos | — | ❌ MISSING | FASE 15 | P0 |
| Cero telemetría (FR-60) | Sin datos fuera del dispositivo | Cumplido: solo IMAP y LLM con lo que el usuario configura | ✅ COMPLETE | Body completo (hasta 8000 chars) al LLM sin sanitizar PII | P0 |

---

## 4. Critical bugs

No hay bugs que rompan el core (crear/ver/editar/completar/recordar funcionan). Defectos y riesgos:

| # | Severidad | Bug / riesgo | Evidencia | Fix propuesto |
|---|-----------|--------------|-----------|---------------|
| 1 | ALTA | **Sin backup**: un archivo SQLite corrupto = pérdida total de datos | store.rs `Db::open` sin copia | FASE 2: copia rotativa diaria + export JSON |
| 2 | MEDIA | **Frontera de `due_reminders`**: tarea a las 23:59 con recordatorio 60 min dispara ~a las 22:59; si se crea con ventana ya abierta, disparo inmediato al arrancar | store.rs:432 query | Documentado; aceptable |
| 3 | MEDIA | **Drift QuickAdd vs backend**: chips "Repetición" y "Recordatorio" prometen features; repetición no existe y el recordatorio solo se aplica si viene del LLM | data.svelte.ts chips + nl.rs reminders vacío | Ocultar chip Repetición hasta FASE 9; heurística → reminder cuando el texto lo pide |
| 4 | MEDIA | **Categorías con 3 fuentes de verdad** — riesgo de divergencia silenciosa (UI vs seed vs prompts LLM) | data.svelte.ts:50, store.rs:265, task_parser.rs:12 | Tabla `categories` + commands (FR-44) |
| 5 | BAJA | `notify`, `widget_set_height`, `widgetHeight` = código muerto expuesto en la API | lib.rs:55, 892; data.svelte.ts:146 | Eliminar |
| 6 | BAJA | Cerrar X → abre el widget de golpe (comportamiento sorprendente, default `close_to_tray_widget=1`) | lib.rs:1190 | Revisar default o animación |
| 7 | BAJA | Agenda oculta >5 tareas por día (slice) sin indicador | Agenda.svelte | Badge "+N" con expansión |
| 8 | BAJA | README del repo describe features que no existen (multi-día "⟳ continúa", widget auto-alto "+N más", "ventana de captura", estado vencida en DB) | README.md root | Actualizar a realidad (📄 → código) |

---

## 5. Technical debt

1. **`lib.rs` monolito** — ~1200 líneas, 36 commands, setup, tray, shortcuts. Refactor a `commands/` al tocar FASE 3.
2. **Sin tests frontend y 8→14 tests Rust** — NFR-14 (≥80 % parser/scheduler) incumplido. QuickAdd/DnD/TaskDrawer sin cobertura.
3. **Migraciones inline** sin tabla `schema_version` (guard por `pragma table_info`). Aceptable en local-first; documentar como límite (doc 03 §4).
4. ~~Código muerto~~ — `notify`, `widget_set_height`/`widgetHeight` **eliminados en FASE 2**.
5. **Tres fuentes de verdad de categorías** (ver bug 4).
6. **Status derivado** en frontend ("vencida") vs DB (pendiente/en-curso/completada) — modelo divergente del PRD.
7. `weekCache` sin invalidación por tiempo; se recarga solo en `tasks:changed`/`refreshRange` (aceptable, eviction 64 semanas).
8. Duplicación de lógica de parseo: chips predictivos del QuickAdd (`findCat`) duplican keywords de `nl.rs`/`validation.rs` (drift potencial).

---

## 6. UX problems

- **X cierra y abre widget** de golpe (bandeja sí, pero sin aviso del cambio de ventana).
- **TopBar semana** muestra solo "Semana" sin rango de fechas visible.
- **Sin empty states educativos** en agenda/sugerencias cuando no hay datos (y sin estados de error globales — solo toasts locales).
- **Sin skeleton loading** en vistas; `loadTasks` en silencio (console.error) si IPC falla → UI vacía engañosa.
- **Agenda slice 5** sin "ver más".
- **Widget**: sin indicación visual de que clic abre la app; fijo sin compacto/expandido.
- Preview del QuickAdd sin **subrayado de entidades** (el PRD y auditoría 06 piden colores por tipo).

---

## 7. AI readiness (FASE 3)

**Lo que hay (base sólida):**
- Contrato intercambiable: `AiProvider` trait + `parse_task_text(texto, provider, configured)` (FR-13 ✅).
- Validación estricta del JSON del LLM: nunca confía en el proveedor (`validation.rs`).
- Heurística local ES como fallback offline (FR-12 parcial).
- Pipeline email → relevancia → confianza → dedupe → auto-aprobación por trusted senders (basado en intención binaria).
- Keyring para credenciales; todo local.

**Lo que falta para ser "AI Personal Time Manager":**
- **`Intent` schema estructurado** (FR-13 dice `→ Intent`, hoy retorna `ParsedTask`): falta `action` (create/update/delete/move/complete/query), `recurrence`, `target_task_id`, `batch`.
- **Planning engine** (núcleo del producto): proposición de bloques de tiempo contra agenda, detección de conflictos, reorganización sugerida — no existe.
- **Principio "IA propone, nunca muta DB"**: hoy `task_from_text` **crea directamente** (el preview editable del QuickAdd es el único control humano). Para FASE 3: `intent_dry_run` (proponer sin mutar) + `intent_apply` (tras aprobación explícita) — la infra de preview ya está en QuickAdd.
- **Constraint extraction**: el prompt ya maneja rangos de fechas (RANGOS DE FECHAS en task_parser.rs) — buen punto de partida para restricciones de planificación.
- Sin logging de decisiones de IA (para auditoría y tests de prompt).

---

## 8. Product gaps (vs visión)

1. **Datos**: backup, export/import JSON/iCal, papelera (riesgo + portabilidad) — P1.
2. **Organización**: búsqueda, filtros, CRUD categorías — P1.
3. **Captura**: formulario manual (FR-02, P0), repetición — P1/P2.
4. **Recordatorios avanzados**: múltiples, predefinidos, acciones toast, centro de perdidas — P1.
5. **IA planificadora** (la promesa central "Time Manager") — FASE 3, nada de esto bloquea FASE 2.

---

## 9. Recommended next sprint (FASE 2)

| Orden | Trabajo | Justificación | Riesgo/notas |
|-------|---------|---------------|--------------|
| 1 | **Backup automático rotativo** + export/import JSON | Protege los datos del usuario (bug crítico #1); barato (copiar archivo SQLite diario) | No toca UX de calendario |
| 2 | **CRUD de categorías** (tabla + commands + Settings) | Desbloquea duplicación de fuentes de verdad; base para filtros | Migración + re-sincronizar prompts |
| 3 | **Búsqueda incremental** (título/descripción/tags/notas) | P1 de mayor valor percibido; SQL LIKE es suficiente | Índice FTS5 opcional |
| 4 | **Formulario manual de tarea** (FR-02 P0) | Única FR P0 sin implementar | Reusar TaskDrawer como modal de creación |
| 5 | **Papelera + restaurar** (UI sobre `deleted_at` existente) | Dato ya existe, falta UI | — |
| 6 | **"Mientras no estabas" agrupado + acciones del toast** (FR-32/35) | Cierra el P0 de recordatorios | API de acciones de Windows toast |
| 7 | Actualizar **README** a la realidad | Evita promesas falsas (bug 8) | — |
| 8 | Refactor `lib.rs` → `commands/` + tests frontend básicos (svelte-check) | Habilita FASE 3 con seguridad | Hacer antes de tocar IA |

Tras FASE 2 → **FASE 3: Intent Schema + planificación** (requiere el refactor del punto 8).
