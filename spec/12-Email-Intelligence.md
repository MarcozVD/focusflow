# 12 — Email Intelligence (Fase 8)

**Estado:** implementado en `spike`
**Fecha:** 2026-08-08
**Depende de:** spec/09 (sistema de intenciones), spec/07 (email sync)

## Objetivo

Convertir correos electrónicos en **compromisos accionables** para el calendario:
eventos, vencimientos (deadlines) y ventanas de disponibilidad, con control del
usuario (aprobación/edición/borrado), deduplicación entre correos y mínima
exposición de datos a la IA.

## Flujo

```
IMAP → RawEmail ──► minimize_email() (privacidad) ──► LLM (EMAIL_SYSTEM_PROMPT)
        └──────────────────────────────────────────────────┐
                                                           ▼
                                              IntentBatch (reutiliza parse_batch_json)
                                                           │
        ┌──────────────────────────────────────────────────┤
        ▼                ▼                ▼                ▼
     event            deadline        availability        task
        └──────────────────────────────────────────────────┤
                                                           ▼
                        insert_suggestion(kind, ...) → suggested_events
                                                           │
                                       ┌───────────────────┴─────────┐
                                       ▼                           ▼
                                  pending (revisar)          auto_approved (crear tarea)
```

Reutiliza el pipeline de la fase 3: `IntentBatch`, `parse_batch_json`,
`validate_intent`. No hay heurística local para correo: sin IA configurada el
sync de correo no produce sugerencias (`NotConfigured`).

## Tipos de compromiso (suggested_events.kind)

| kind          | Origen                     | Persistencia                                                            |
|---------------|----------------------------|------------------------------------------------------------------------|
| `event`       | actividad con fecha/hora   | `start_at`..`end_at`                                                   |
| `deadline`    | entrega/vencimiento        | `deadline_at` (+ `start_at` espejo), `prep_min` si el correo lo dice    |
| `availability`| rango "del X al Y"         | `start_at`..`end_at` rango completo                                    |
| `task`        | compromiso sin fecha       | sin fechas; al aceptar → tarea de todo el día (hoy)                    |

### Aceptación de disponibilidad

Una ventana se acepta como **UNA tarea de todo el día** que abarca el rango
completo (`start_at` = inicio, `end_at` = fin, `all_day = true`). **Nunca** una
tarea por día.

## Privacidad (minimización)

Antes de enviar el correo a la IA, `minimize_email()`:

1. Elimina líneas citadas/respondidos (prefijo `>` y `On … wrote:`).
2. Trunca el cuerpo a **~900 caracteres** (con marca `[…]`).
3. Mantiene remitente, asunto y fecha (contexto necesario).

Nunca se envía el cuerpo completo del correo al proveedor de IA. Los títulos y
descripciones generados no deben contener PII (regla 8 del prompt).

## Deduplicación

- **Entre correos:** `find_similar_suggestion()` — misma sugerencia `pending` o
  `auto_approved` de OTRO correo, misma fecha (±2 días) y título similar
  (mismo algoritmo `title_similar` que tareas) → la nueva se crea como
  `pending` con `dedupe_note` "Ya detectado en otro correo: …".
- **Contra tareas:** `find_similar_task()` existente (posible duplicado).
- El correo que ya generó sugerencias (`suggestion_count_for_email`) no se
  re-procesa.

## Auto-aprobación

`status = auto_approved` (y se crea la tarea al instante) SOLO si:

1. remitente en `trusted_senders` (controlado en Ajustes), y
2. sin duplicado detectado, y
3. `confidence >= 0.6` (fecha/hora explícitas).

Confianza baja (fecha implícita, ambigüedad) → siempre `pending`.

## Borrado

Nuevo comando `suggestion_delete` (botón "Borrar" en cada tarjeta): elimina la
sugerencia **definitivamente** y, si ya había creado tarea, la borra también.
Es el control del usuario sobre datos derivados de su correo.

## Cambios

- **migración 0007:** columnas `kind` (CHECK event|deadline|availability|task),
  `deadline_at`, `prep_min`, `source_subject` + índice por `kind`.
- **`ai/email_intent.rs`** (nuevo): `EMAIL_SYSTEM_PROMPT`, `minimize_email`,
  `parse_email_intent`, `suggestion_kind`.
- **`ai/email_parser.rs`**: queda como ruta antigua (relevancia + eventos);
  el sync ahora usa `email_intent`.
- **`sync.rs`**: `process_email` → `parse_email_intent`; `insert_intent_suggestion`
  (mapeo por kind, dedupe, auto-aprobación); `accept_suggestion` maneja
  disponibilidad (rango → todo el día).
- **`store.rs`**: `insert_suggestion` ampliado, `find_similar_suggestion`,
  `delete_suggestion`, `SuggestionRow` ampliado.
- **`lib.rs`**: comando `suggestion_delete`.
- **UI**: badge de tipo (📅 Evento / ⏰ Vencimiento / 🟢 Disponibilidad),
  display de rango (`5 ago → 23 ago`) y vencimiento, prep en minutos, botón
  Borrar.

## Comandos

| Comando            | Args                | Efecto                              |
|--------------------|---------------------|-------------------------------------|
| `suggestions_list` | `only_pending`      | lista (con campos nuevos)           |
| `suggestion_accept`| `id`                | crea tarea (availability → rango)   |
| `suggestion_reject`| `id`                | marca rechazada                     |
| `suggestion_revert`| `id`                | vuelve a pending, borra tarea       |
| `suggestion_edit`  | `id`, campos        | edita sugerencia + tarea si existe  |
| `suggestion_merge` | `id`, `task_id`     | fusiona con tarea existente         |
| `suggestion_delete`| `id`                | borra definitivo (+ tarea creada)   |

## Tests

- `email_intent`: minimización (citas/truncado), deadline+evento, rango de
  disponibilidad, correo irrelevante (`{"intents": []}`), sin IA → `NotConfigured`,
  mapeo de kinds.
- `store`: migración 0007, roundtrip de campos nuevos, dedupe entre correos
  (mismo correo excluido, título distinto y ventana lejana → no duplicado),
  `delete_suggestion` borra sugerencia y tarea creada.

## Pendiente / riesgos

- `prep_min` se persiste pero aún no se usa en el planner de sugerencias.
- `source_subject` se guarda para debugging; no se muestra en UI.
- E2E real (tools/run-e2e-email.ps1) pendiente de ejecutar con cuenta real.
