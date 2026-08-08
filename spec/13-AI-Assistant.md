# 13 — AI Assistant (Fase 9)

**Estado:** implementado en `spike`
**Fecha:** 2026-08-08
**Depende de:** spec/09 (intents), spec/10 (constraint engine), spec/12 (email)

## Objetivo

Un asistente de IA para la **gestión del tiempo**, no un chatbot genérico.
Entiende el dominio de FocusFlow (tareas, eventos, vencimientos,
disponibilidad, preferencias) y opera con los pipelines existentes de la app.

## Arquitectura

```
pregunta + historial
      ▼
  decisión (LLM) ──► contexto mínimo (solo lectura)
      ├── answer  ──► segunda llamada ──► texto con datos reales
      ├── plan    ──► intent_parser ─► planning::plan_from_text ─► propuesta pendiente
      └── action  ──► propuesta de acción (asistente_actions, pending)
                          │
                          ▼  aprobación del usuario
                     servicios existentes del store (create/move_to/set_completed)
```

### Regla de oro

**El asistente JAMÁS muta la base de datos directamente.** Toda mutación se
persiste como propuesta `pending` en `assistant_actions` y solo se aplica tras
aprobación explícita, pasando por los servicios existentes del store
(`Db::create`, `Db::move_to`, `Db::set_completed`, `planning::reject_plan`).
No hay ningún `INSERT/UPDATE/DELETE` en el módulo del asistente.

## Contexto mínimo (read-only)

`context_snapshot(db)` construye SOLO lo necesario:

- `today`, `now_local`, `working_hours`
- tareas pendientes compactas (id, título, categoría, prioridad, día, all_day)
  — **sin descripciones, notas ni cuerpos** (verificado por test)
- horas libres por día de los próximos 7 días (ConstraintEngine sobre el
  calendario real)
- totales: pendientes, atrasadas

Límite de 40 tareas en el contexto; el resto se cuenta pero no se detalla.

## Modos

| mode      | Qué hace                                        | Mutación |
|-----------|-------------------------------------------------|----------|
| `answer`  | responde con datos reales del contexto          | nunca    |
| `plan`    | reutiliza intent_parser + planning (fase 7)     | propuesta pending |
| `action`  | complete / reschedule / create_event / cancel_proposal | propuesta pending → aprobación |

### Acciones

| kind               | target                 | Al aprobar (vía store)                        |
|--------------------|------------------------|----------------------------------------------|
| `complete`         | tarea existente        | `set_completed(id, true)`                    |
| `reschedule`       | tarea existente        | `move_to(id, start, end, all_day)` + revalidación de solapamiento |
| `create_event`     | nueva                  | `create(...)`; sin hora → todo el día        |
| `cancel_proposal`  | propuesta de plan      | `planning::reject_plan` (la más reciente)    |

Resolución de tarea por título normalizado (igualdad → contención). Ambiguo →
el asistente **responde conversacionalmente** enumerando candidatos; nunca crea
propuestas basura ni adivina. Fechas relativas ("mañana") resueltas por el LLM
a absolutas respecto a HOY.

## Seguridad

- El `AiProvider` no tiene acceso a la BD (spec/09).
- Sin IA configurada → mensaje informativo, cero escrituras.
- Toda acción pasa por confirmación explícita (botón "Confirmar") — nunca
  auto-aprobación.
- Solapamientos revalidados al aceptar (el calendario pudo cambiar).
- El historial enviado al LLM se limita a los últimos 6 turnos, sin cuerpos
  de descripciones.

## Cambios

- **`src/assistant.rs`** (nuevo): `ASSISTANT_DECISION_SCHEMA`,
  `context_snapshot`, `resolve_task`, `assistant_turn`, `apply_action`,
  `get_action`.
- **`store.rs`**: migración 0008 `assistant_actions` (kind, payload, status) +
  CRUD. Fix de bug latente: `update_task_full` con reminder NULL.
- **`lib.rs`**: comandos `assistant_turn`, `assistant_actions_list`,
  `assistant_action_accept`, `assistant_action_reject`.
- **Frontend**: vista "Asistente" (Sidebar), `Assistant.svelte` con chips
  rápidos, hilo de turnos, propuestas de plan reutilizando `PlanProposal.svelte`,
  tarjetas de acción con Confirmar/Descartar.

## UX (no es un clon de ChatGPT)

- Vista propia del flujo de trabajo, no ventana flotante de chat.
- Chips de preguntas rápidas del dominio ("¿Qué debería hacer hoy?",
  "Organiza mi semana").
- Respuestas = tarjetas con datos (propuestas de plan editables, acciones
  con resumen y confirmación), no burbujas infinitas.
- Pie fijo: "nada se modifica sin tu aprobación".

## Comandos

| Comando                     | Args                     | Efecto                              |
|-----------------------------|--------------------------|-------------------------------------|
| `assistant_turn`            | `text`, `history`        | Answer / Plan / Action / Nothing    |
| `assistant_actions_list`    | `only_pending`           | lista de propuestas                 |
| `assistant_action_accept`   | `id`                     | aplica vía store, marca accepted    |
| `assistant_action_reject`   | `id`                     | marca rejected                      |

## Tests (10 nuevos)

- sin IA → Nothing sin escrituras
- answer (2ª llamada), plan reutiliza planning (proposal pending)
- complete: pendiente hasta aprobar, aplica con `set_completed`
- tarea desconocida → respuesta aclaratoria; título ambiguo → enumera
- reschedule aplica `move_to`; create_event sin hora → todo el día
- cancel_proposal rechaza la propuesta pendiente
- contexto mínimo: título sí, descripción nunca
- store: migración 0008 + roundtrip

## Pendiente

- E2E manual con IA real (OpenAI/Gemini) para calibrar el prompt de decisión.
- `prep_min` de las sugerencias de correo aún no entra en el planner.
- Los chips rápidos no son localizables (hardcoded en UI).
