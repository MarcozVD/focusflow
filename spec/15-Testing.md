# 15 — Estrategia y Suite de Pruebas

Objetivo: que cualquier cambio en FocusFlow se pueda validar con una sola
orden, sin red ni cuentas reales. La suite entera es determinista (proveedor
de IA ficticio o heurística local; DB en memoria o en disco temporal).

## 1. Cómo ejecutar

| Suite | Orden |
|---|---|
| Todo (recomendado) | `cargo test` en `src-tauri/` |
| Solo unit (lib) | `cargo test --lib` |
| Flujos de integración | `cargo test --test flows` |
| E2E (DB real en disco) | `cargo test --test e2e` |
| Fase 7 (bucle planner) | `cargo test --test phase7` |
| Benchmarks | `cargo test --release --test perf_bench -- --ignored --nocapture` |
| Perf de app (arranque/memoria) | `pwsh tools/perf.ps1` (requiere la app construida) |
| Frontend | `npx svelte-check` en `frontend/` (0 errores exigido) |

**Total: 174 pruebas (158 unit + 16 integración/E2E) + 1 benchmark + 1
script de perf. Tiempo de ejecución completo: ~6 s.**

## 2. Cobertura unitaria

| Área | Archivo | Pruebas clave |
|---|---|---|
| Parseo de fechas relativas/absolutas | `ai/nl.rs`, `ai/rule_based.rs` | días relativos ("mañana"), horas 24h/12h, ambigüedad sin am/pm, hora sin fecha → no se inventa día |
| Parseo de duraciones | `ai/nl.rs` | "1 hora", "90 min", ausencia → None |
| Validación de intents | `ai/intent_validator.rs` (17) | nulls válidos, end<start, deadline pasado, duración 0/>24 h, recurrencia, reminders, confidence acotada, consistencia tipo↔campos |
| Constraints | `ai/rule_based.rs`, `ai/nl.rs` | detección de restricciones (bloqueos, daily_cap), horarios de estudio |
| Scheduling | `engine/planner.rs` (16), `engine/mod.rs` (33) | planificación de preparación, fragmentación 2–6 sesiones, respeto de ventanas, tiempo insuficiente → parcial + explicación |
| Detección de conflictos | `engine/mod.rs`, `planning.rs` | solapamientos rechazados al aceptar, edición conflictiva, bloques editados solapados |
| Operaciones de DB | `store.rs` (15) | migraciones 0005/0007, recordatorios, dedupe (sugerencias y tareas), borrado de sugerencias, export sin secretos, wipe, overflow |
| Notificaciones | `notify.rs` (10) | cadencia, tope diario, horario de silencio, dedupe por tarea |
| Seguridad (fase 12) | `lib.rs`, `email.rs`, `email_intent.rs`, `intent_parser.rs` | logs saneados, TLS obligatorio, delimitación de datos del correo, tope de intents, caps de longitud |

## 3. Cobertura de integración (`tests/flows.rs`)

Cadenas completas entre módulos reales, sin red:

1. **Quick Add → Intent → Calendario**: texto → intent → tarea → el motor de
   calendario descuenta el bloque del tiempo libre.
2. **Correo → Intent → Sugerencia → Calendario**: `RawEmail` → `parse_email_intent`
   (IA ficticia) → sugerencia `pending` (sin remitente de confianza no se
   auto-aprueba) → `accept_suggestion` crea la tarea en su hueco → `revert`
   la borra y restaura el estado.
3. **Asistente → Propuesta → Aprobación → Calendario**: texto → propuesta
   `pending` → aprobación con edición (mover sesión) → tareas reflejan la
   edición y aparecen en `list_range`.
4. **Conflicto → detección → alternativa**: compromiso nuevo que choca →
   `accept_plan` falla con aviso y la propuesta sigue `pending`; al liberar
   el hueco, el mismo plan se acepta sin solapamientos.
5. Correo sin compromisos → nada; sin IA configurada → `NotConfigured` claro.

## 4. E2E (`tests/e2e.rs`) — viajes reales con DB en disco

Usan `Db::open` real (la misma de producción): crear → cerrar → reabrir
equivale a relanzar la app.

| Escenario | Qué verifica |
|---|---|
| 1. Crear tarea → cerrar → reabrir | la tarea persiste; sin re-siembra de demos |
| 2. Lenguaje natural → plan → aceptar | evento + sesiones persisten; el motor sigue viendo el hueco ocupado; propuesta `accepted` |
| 3. Correo → compromiso → sugerencia → aceptar | la tarea usa la hora del correo; estado `accepted`; dedupe por `message_id` |
| 4. Conflicto → detectar → alternativa | aceptación bloqueada; tras mover el compromiso, el plan persiste sin solapamientos |

## 5. Regresión de bugs críticos corregidos

Cada bug crítico corregido tiene su prueba de regresión:

| Bug | Prueba |
|---|---|
| Filtros de correo AND (excluían remitentes válidos) | `union_semantics_sender_or_domain_or_keyword` |
| Checkpoint avanzaba sobre correos filtrados (irrecuperables) | `rollback_uid` × 3 (lógica extraída a función pura) |
| Panic por overflow en `find_similar_suggestion` (sin fecha) | `find_similar_suggestion_without_date_does_not_overflow` |
| Dedupe contra tareas existentes sin cubrir | `find_similar_task_dedupes_against_existing_tasks` |
| Hora local convertida a UTC (timezone) | `windows_and_dates_use_local_time` |
| Forja de entradas de log vía asunto malicioso | `sanitize_strips_control_chars_and_caps_length` |
| IMAP en claro | `plaintext_imap_rejected_outside_localhost` |
| Prompt injection vía correo | `email_body_is_delimited_as_data_not_instructions` |
| Spam de sugerencias desde la IA | `oversized_batch_rejected`, `llm_text_fields_are_capped` |
| Overflow de recordatorios re-disparados | `reminder_rearms_only_when_changed` |

## 6. Rendimiento (medido 2026-08-08)

### 6.1 Nivel de proceso (build debug, Windows 11)

`tools/perf.ps1`:

| Métrica | Valor |
|---|---|
| Arranque (exe → primera línea de log, incluye sync inicial + WebView2 frío) | **2283 ms** |
| Working set a los 10 s | **44.8 MB** |
| Private bytes a los 10 s | **8.2 MB** |

Nota: build debug; release reduce arranque y memoria. El log de referencia
se genera solo al arrancar (no en bucle), así que el número es estable.

### 6.2 Nivel de datos (release, DB con 2000 tareas)

`cargo test --release --test perf_bench -- --ignored --nocapture`:

| Operación | µs/op |
|---|---|
| `db.create` (insert) | 32.3 |
| `db.list` (2000 filas completas) | 6 658 |
| `db.list_range` (ventana 1 día — el del render del calendario) | 94.6 |
| `db.find_overlap` (detección de conflictos) | 240.1 |
| `db.find_similar_suggestion` (dedupe) | 9.4 |
| `db.insert_suggestion` | 24.7 |
| `db.export_data` (JSON completo) | 16 234 (16 ms) |
| `engine.available_minutes` (día completo, 2000 tareas) | 3.1 |
| Handler de IPC `task_list` (serialización) | 8 384 |

Lectura: el calendario pinta con `list_range` (~95 µs) y el widget con datos
compactos; con 2000 tareas ambos quedan muy por debajo de un frame (16 ms).
`db.list`/serialización completos son ~8 ms: solo se usan en pantallas de
lista, y los `$state` del frontend reaccionan a eventos incrementales, no a
refetch completo.

### 6.3 Widget

Superficie idéntica al resto (misma DB, mismos handlers). El widget pinta
una ventana pequeña con consultas `list_range` de ~95 µs; sin hot path
propio. Único coste extra: el evento `tasks:changed` lo refresca vía los
mismos `$state` que la ventana principal.

## 7. Huecos conocidos

1. **E2E de UI (WebView2)**: no hay driver automatizado sobre la webview.
   Los E2E cubren el comportamiento a nivel de proceso/datos; los clicks y el
   render reales se validan manualmente. Opción futura: msedgedriver +
   Playwright sobre la webview en build de pruebas.
2. **Sync de correo contra IMAP real**: la suite usa `RawEmail` sintéticos y
   proveedor ficticio. La conectividad IMAP real (TLS, login, fetch) quedó
   validada manualmente con la cuenta del usuario; un test de humo opcional
   con `#[ignore]` + credenciales de prueba sería lo siguiente.
3. **Salida del LLM real**: los fixtures cubren los contratos, pero una
   deriva del prompt (formatos nuevos) solo se detecta en manual. El pipeline
   tolerante (parseo relajado) mitiga el riesgo.
4. **Render del frontend con datos grandes**: medido indirectamente vía
   handlers; falta medir el layout real de Svelte con 2000 filas.
5. **Multimonitor/DPI del widget**: sin cobertura automatizada.
6. **Migraciones desde DBs antiguas reales**: las migraciones se prueban en
   memoria; falta una prueba de migración con un fixture de DB de versión
   anterior a la actual.
7. **Concurrencia**: el `Mutex<Db>` serializa accesos; no hay prueba de
   stress con hilos (scheduler + UI + notificaciones).

## 8. Bloqueos de release

Ninguno de los bloques es de calidad de código; la suite completa está
verde. Sí quedan pendientes **de producto** antes de un release real:

1. **UI E2E automatizado** (hueco 1) — sin esto, un cambio de Svelte que
   rompa el render pasa la CI.
2. **IMAP real en CI** (hueco 2) — requiere credenciales de prueba; hoy se
   valida manual.
3. **MSI firmado** — el bundle actual no tiene firma de código; Windows
   SmartScreen bloquea instalaciones sin firmar.
4. **App de producción sin datos demo**: hoy el primer arranque siembra
   5 tareas de ejemplo (intencional para desarrollo, inaceptable para
   producción real) — decidir si se eliminan o se marcan como ejemplo.
5. **Migración del `identifier` de com.focusflow.spike** a un id final antes
   de empaquetar (afecta rutas de datos).
