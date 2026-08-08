# 15 — Notificaciones Contextuales (Fase 11)

**Estado:** implementado en `spike`
**Fecha:** 2026-08-08
**Depende de:** spec/13 (asistente, acción "Plan"), spec/14 (widget), spec/04 (design system)

## Objetivo

Transformar los recordatorios tontos ("Study calculus") en sugerencias
útiles con contexto, acciones y respeto absoluto por la atención del
usuario.

## Tipos de notificación

| Tipo | Regla de disparo | Copia (ejemplo) |
|------|------------------|-----------------|
| `deadline` | pendiente, fin en < 24 h, con preparación restante | "«Estudiar cálculo» termina mañana a las 18:00. Quedan 2h de preparación." |
| `missed` | pendiente terminó hace < 24 h (una sola vez) | "«Entregar informe» terminó hace 3h y sigue pendiente." |
| `conflict` | dos tareas se solapan ≥ 15 min | "«Reunión» se solapa con «Clase» (15:00–16:00)." |
| `free_time` | hueco libre ≥ umbral (120 min) + tarea preparable | "Tienes 2h libres esta tarde. ¿Usarlas para «Preparar entrevista»?" |
| `important` | compromiso de hoy: prioridad alta o todo el día | "Hoy: «Defensa de TFG». a las 18:00." |
| `reschedule` | empieza en < 1 h mientras estás en otra (en-curso) | "«Reunión» empieza a las 15:30 mientras estás en «Proyecto». ¿Moverlo a más tarde?" |

## Urgencia (una por tarea por tick, gana la de mayor score)

1. `deadline` ya iniciada (100)
2. `reschedule` (95)
3. `important` inminente ≤ 3 h (90)
4. `free_time` (75) — gana sobre deadlines lejanos: la oferta "ahora" vale más
5. `conflict` con tarea en curso (70)
6. `missed` (60) · `deadline` no iniciada (60) · `conflict` (40) · `important` (40)

## Anti-spam

- `notif.enabled` — kill-switch global (afecta también a los recordatorios
  manuales por tarea).
- `notif.quiet_start` / `notif.quiet_end` (default 22:00–08:00) — ventana de
  silencio; la hora se interpreta en la zona horaria local (chrono::Local,
  misma convención que el resto de la app).
- `notif.daily_cap` (default 5) — tope diario de disparos; el presupuesto
  restante se consume cada tick (60 s) en orden de urgencia.
- `notif.cooldown_hours` (default 24) — cadencia por (tipo, tarea).
- `notif.free_minutes` (default 120) — umbral para sugerir tiempo libre.
- **Dismiss** — bloquea para siempre ese (tipo, tarea) en `notification_log`.
- **Missed es one-shot**: no reaparece día tras día.
- Deduplicación por tick: una sola notificación por tarea (la más urgente).

## Acciones

El plugin de Tauri no soporta botones nativos en Windows (solo Android), así
que la interacción vive en la app:

- Notificación nativa de Windows: contexto completo (título + cuerpo).
  El clic enfoca la ventana principal.
- Al disparar también se emite `notif:contextual` → la ventana principal
  muestra una tarjeta contextual (ContextualToast) con:
  - **[Plan]** — abre el Asistente con el prompt prefabricado
    "Crea un plan para: «tarea»" (auto-envía); si el asistente no está
    configurado, abre el detalle de la tarea.
  - **[Más tarde]** — marca `later` (respeta la cadencia).
  - **[Descartar]** — marca `dismissed` (no volver a insistir en ese
    tipo/tarea).
- Las decisiones se registran en `notification_log` (migración 0009) y
  alimentan la deduplicación futura.

## Configuración

Nueva sección "Notificaciones contextuales" en Ajustes: activar/desactivar,
horario de silencio (time inputs), tope diario y umbral de tiempo libre.
Comandos: `notif_prefs_get`, `notif_prefs_set` (valida formato HH:MM),
`notif_respond(id, status)`.

## Cambios

- **`store.rs`**: migración 0009 `notification_log` (+índices); `log_notification`,
  `set_notif_status`, `notif_dismissed`, `notif_fired_recently`,
  `notif_fired_today`; `open_memory_clean_pub` (tests sin demo data).
- **`notify.rs`** (nuevo): motor puro y testeable — `prefs`, `in_quiet_hours`,
  `collect` (6 reglas + dedup + urgencia), `tick` (gates + presupuesto),
  `fire` (toast nativo + `notif:contextual`).
- **`reminders.rs`**: los recordatorios manuales respetan ahora enabled +
  horario de silencio.
- **`lib.rs`**: defaults `notif.*`; comandos `notif_prefs_get/set`,
  `notif_respond`; registro en invoke_handler.
- **Frontend**: `ContextualToast.svelte` (tarjeta con acciones), listeners
  `notif:contextual`, prefs + responder en `data.svelte.ts`, auto-envío de
  draft en `Assistant.svelte`, sección en `Settings.svelte`.

## TEST

- [x] sin notificaciones duplicadas: dedup por (tipo, tarea) con cadencia
  (test `no_spam_dedup_and_cap`), una por tarea por tick, dismiss permanente
- [x] timing correcto: deadlines < 24 h, missed < 24 h, reschedule < 1 h,
  free_time con hueco ≥ umbral (`first_free_block_merges_busy`)
- [x] zona horaria correcta: todo con `chrono::Local` (labels "hoy"/"mañana",
  ventana de silencio por hora local; `quiet_hours_window`)
- [x] contexto correcto de la tarea: título, hora, preparación restante
  (`deadline_candidate_with_remaining`, `conflict_pair_detected`,
  `important_commitment_today`, `reschedule_suggestion_while_en_curso`)
- [x] horario de silencio: `in_quiet_hours` (ventana nocturna y diurna,
  vacía = desactivada) + gate en tick y en recordatorios manuales
- [x] notificaciones desactivadas: `notif.enabled=0` → tick no dispara nada
  (gate en `tick`), recordatorios manuales tampoco
- [x] 144 tests lib + 6 integración verdes; svelte-check 0 errores

## Pendiente

- Verificación manual del toast real de Windows (posición, click → foco) y
  de la tarjeta contextual con las tres acciones.
- El presupuesto diario se consume por tick (máx 1 por minuto); con cap alto
  y muchas candidatas, la cola puede alargarse hasta 5 min — aceptable.
