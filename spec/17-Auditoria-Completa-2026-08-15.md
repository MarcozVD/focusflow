# 17 — Auditoría Completa (2026-08-15)

**Producto:** FocusFlow · **Rama:** master (working tree limpio)
**Alcance:** auditoría completa post-FASE 2: backend Rust, frontend, consistencia spec/README, higiene del repo.
**Verificación real ejecutada:** `cargo test` → **215 passed, 0 failed** (198 unitarios + 17 integración) · `vitest` → **36 passed (3 archivos)** · `svelte-check` → **0 errores, 5 warnings**.

---

## 1. Resumen ejecutivo

| Área | Nota | Estado desde la auditoría 08 (08-ago) |
|------|------|----------------------------------------|
| Backend Rust | 7/10 | Tests pasaron de 8 a 215. Pero `lib.rs` creció de 1218→1801 líneas / 59 commands |
| Frontend | 7/10 | Antes "cero tests", ahora 36 en verde + svelte-check limpio |
| Consistencia spec/README | 4/10 | README congelado: omite asistente, planner y onboarding enteros |
| Higiene del repo | 9/10 | Limpio: solo `test-results/.last-run.json` trackeado indebidamente; sin logs/dist/target en git |

---

## 2. Hallazgos críticos 🔴

1. **`ai_test` bloquea toda la app hasta ~4,5 min** — `spike/src-tauri/src/lib.rs:380-386`: retiene el mutex de la DB durante una llamada HTTP (90 s × 3 con reintentos 429), y es un command **síncrono** en el hilo principal → UI congelada. Contrasta con `verify_connections` (`lib.rs:531`), que sí suelta el guard. Fix: clonar `AiConfig`, soltar el guard, hacer `async` + `spawn_blocking`.
2. **Sin backup del SQLite** — `store.rs:101-113` (`Db::open`) sin copia rotativa: un `focusflow.db` corrupto = pérdida total. `export_data` (JSON manual, `store.rs:406`) no mitiga. Pendiente de la auditoría 08 (bug #1), sigue abierto. Con `panic = "abort"` en release (Cargo.toml:36) el riesgo es real.
3. **`suggestion_accept` permite duplicar tareas** — `sync.rs:86-115`: no verifica `status == 'pending'`; doble-clic o re-aceptar una sugerencia `auto_approved` (que ya tiene tarea, `sync.rs:292-300`) crea una segunda tarea y sobrescribe `result_task_id`, perdiendo el enlace a la primera. El asistente sí hace este check (`lib.rs:932`, `planning.rs:490`).
4. **README promete lo que no existe** — widget "auto-altura" y "+N tareas más" (`README.md:38,62`) vs ventana fija 340×500 (`lib.rs:963`); "Formulario completo" de captura (`README.md:28`) cuando FR-02 sigue sin existir; marcador "⟳ continúa" inexistente (el real es "Inicio/Fin · título", `EventBlock.svelte:39`); "Spec (7 documentos)" (`README.md:172`) cuando hay 16. Y **omite** asistente (`assistant.rs`, 1044 líneas), planner (`engine/planner.rs`, 915) y onboarding.

## 3. Hallazgos medios 🟡

5. **Escrituras multi-paso sin transacción** — solo `wipe_data` usa transacción (`store.rs:462`). `accept_suggestion` (3 escrituras, `sync.rs:108-113`), `update_task_full` (`store.rs:649-667`), `delete_suggestion` (`store.rs:996-1009`) quedan inconsistentes ante crash a mitad (y alimentan el bug #3). `accept_plan` usa rollback compensatorio (`planning.rs:573-578`) — aceptable pero deja filas muertas.
6. **`Mutex<Db>` + `.lock().unwrap()` + `panic="abort"`** (~40 sitios): un pánico envenena el mutex y cada command posterior aborta el proceso. `log_dir()` hace `create_dir_all().unwrap()` (`lib.rs:33`) desde callbacks de tray/shortcuts; `app.default_window_icon().unwrap()` (`lib.rs:1623`) aborta el arranque si falta el icono. Mínimo: `lock().unwrap_or_else(|e| e.into_inner())` y degradar log a no-op.
7. **Migraciones sin `user_version`/`schema_version`** — `store.rs:146-156` encadena `migrate_0001..0009` con guards `pragma_table_info`. Funciona hoy, pero no distingue "migración aplicada" de "columna preexistente"; una futura migración con transformación de datos no tiene anclaje.
8. **Categorías: 3 fuentes de verdad persisten** — `store.rs:75` (`VALID_CATEGORIES`), `frontend/src/lib/data.svelte.ts:50-57`, prompts IA (`ai/validation.rs:27-42`, `ai/email_parser.rs:14`). Mitigado con `sanitize_category` (`store.rs:77-83`), pero `update_suggestion_data` (`store.rs:1068-1074`) escribe `category_id` sin sanitizar vía `suggestion_edit` IPC.
9. **`engine::local_ms` devuelve 0 (epoch 1970)** en horas locales inválidas por DST — `engine/mod.rs:39-45`, `from_local_datetime().single()…unwrap_or(0)`. `nl.rs:125` lo hace mejor (fallback a UTC); `notify.rs:111` cae a `Local::now()`. Fix: `earliest()` o resolución explícita.
10. **`lib.rs` monolítico y creciente** — 1218→1801 líneas, 38→59 commands (`generate_handler!` en `lib.rs:1533-1593`). El refactor a `commands/` pendiente de la auditoría 08 ahora es más caro.
11. **Log con PII en claro** — `append_log` (`lib.rs:63-71`) escribe a `%TEMP%\focusflow-spike\spike.log` asuntos/remitentes de correo (`sync.rs:405`) y títulos de tareas (`lib.rs:300`, `reminders.rs:70`). Riesgo bajo (`%TEMP%` por-usuario) pero incoherente con la promesa de privacidad del export. `data_wipe` sí vacía el log (`lib.rs:1316`) — correcto.
12. **`spec/README.md` obsoleto** — lista solo docs 01–07, apunta a rutas inexistentes (`focusflow-spike/`, `focusflow-proto/`) y afirma un "servidor activo" efímero. Numeración rota: dos docs 14, dos 15, falta el 11, y `16-Onboarding.md` vive en `spike/spec/`. Feature matrix de spec/08 desactualizada: ya implementados onboarding (FR-59), export JSON (FR-54 parcial, sin import), planning engine, Intent schema (`ai/intent*.rs`).

## 4. Hallazgos menores 🟢

13. `notif_prefs_set` acepta "99:99" como hora válida (`lib.rs:1202-1209`; `parse_hhmm` en `notify.rs:64-67`). `reminder_minutes` llega sin clamp vía `task_update` (`lib.rs:193`): un valor negativo dispara *después* del inicio; uno gigante puede desbordar `reminder_minutes * 60000` en `due_reminders` (`store.rs:685`) — en release el overflow envuelve en silencio.
14. Retención de sugerencias resueltas: **60 min** por defecto y prune con `DELETE` físico (`store.rs:1042-1048`) — el rastro de "qué se aceptó/rechazó" desaparece a la hora. Decisión consciente: retención corta para no acumular ruido; queda **documentada aquí** como diseño aceptado (la auditoría completa vive en `sync_history`).
15. IMAP sin validación real de `UIDVALIDITY` (limitación de imap 2.x; heurística de reinicio por UIDs decrecientes, `email.rs:277,361-366`) — aceptable. Fallback de secretos por variable de entorno (`AI_API_KEY`, `FF_EMAIL_PASSWORD`, `provider.rs:365,369`) — aceptado como herramienta de dev; en producción las claves van al Credential Manager.
16. Frontend: `data.svelte.ts` (1434 líneas) y `Settings.svelte` (1171) concentran demasiado; 5 warnings de svelte-check; cero `console.log`/`TODO` en src. Repo: único archivo indebido trackeado: `spike/frontend/test-results/.last-run.json`.

## 5. Positivo verificado (no era así en la auditoría 08)

- Bug crítico de zona horaria del parser NL (`day_ms % DAY`): **cerrado** con tests de regresión (`nl.rs:294`).
- Código muerto (`notify`, `widget_set_height`): eliminado.
- SQL injection: limpio (todo con `params!`; `format!` solo con nombres fijos o placeholders generados).
- CSP sin `unsafe-eval` (`tauri.conf.json:27`), capabilities mínimas (`core:default` + ventana), `withGlobalTauri: false`, hooks de test solo en `#[cfg(debug_assertions)]`.
- Secretos en Credential Manager (keyring), cero logging de claves, export con lista blanca (`EXPORTABLE_SETTINGS`, `store.rs:61-70`), TLS obligatorio en IMAP fuera de localhost (con test).
- Commits post-auditoría sólidos: dedupe por hilo de correo, retry/backoff 429, planner que no agenda en el pasado, cap de carga diaria compartido, 36 tests frontend (vitest).
- El asistente respeta "propone, no muta" (propuestas + aprobación con check de estado).
- Recordatorios comparan epoch ms puros (independientes de zona horaria).

## 6. Prioridad de actuación

1. 🔴 #1 — clonar config y soltar el guard + hacer async (descongela la app).
2. 🔴 #2 — copia rotativa del `.db` al arrancar (protege los datos del usuario).
3. 🔴 #3 + 🟡 #5 — check de `status` en `accept_suggestion` + transacciones.
4. 🔴 #4 + 🟡 #12 — actualizar README, `spec/README.md`, renumerar spec.
5. 🟡 #6 — recuperación de poison; 🟡 #7 — `user_version`; 🟡 #8 — sanitizar `update_suggestion_data`; 🟡 #9 — fallback DST.

## 7. Registro de resolución

| Hallazgo | Estado | Nota |
|----------|--------|------|
| 🟢 #13 validación `notif_prefs_set` + clamp `reminder_minutes` | ⏳ En curso | |
| 🟢 #14 retención sugerencias | ✅ Documentado | §4, diseño aceptado |
| 🟢 #15 UIDVALIDITY / env fallback | ✅ Documentado | §4, limitación/herramienta dev aceptada |
| 🟢 #16 warnings svelte-check / archivo trackeado | ⏳ En curso | |
