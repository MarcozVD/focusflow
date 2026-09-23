use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use chrono::TimeZone;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ai::{self, AiError};
use crate::email::{self, EmailConfig, RawEmail, SyncCheckpoint};
use crate::store::Db;

const SETTINGS_EMAIL_ENABLED: &str = "email.enabled";
const SETTINGS_EMAIL_CONFIG: &str = "email.config";
const SETTINGS_EMAIL_INTERVAL_HOURS: &str = "email.interval_hours";

pub fn load_email_config(db: &Db) -> EmailConfig {
    db.settings_get(SETTINGS_EMAIL_CONFIG)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<EmailConfig>(&s).ok())
        .unwrap_or_default()
}

pub fn interval_hours(db: &Db) -> u64 {
    db.settings_get(SETTINGS_EMAIL_INTERVAL_HOURS)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

#[derive(Serialize, Clone)]
pub struct MailboxSummary {
    pub mailbox: String,
    pub found: usize,
    pub processed: usize,
    pub result: String,
    pub error: String,
}

#[derive(Serialize, Clone, Default)]
pub struct SyncSummary {
    pub started_at: i64,
    pub finished_at: i64,
    pub mailboxes: Vec<MailboxSummary>,
    pub total_found: usize,
    pub total_suggestions: usize,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct SyncProgress {
    pub phase: String,
    pub mailbox: String,
    pub processed: usize,
    pub total: usize,
}

pub fn with_db<T>(app: &AppHandle, f: impl FnOnce(&Db) -> T) -> T {
    let state = app.state::<Mutex<Db>>();
    let db = crate::store::lock_recover(&state);
    f(&db)
}

/// Rollback del checkpoint cuando hubo correos excluidos por filtros: el
/// checkpoint no puede avanzar más allá del último correo procesado, o esos
/// correos nunca se reintentarían. Función pura, testeable.
///
/// - `new_cp_uid`: uid propuesto por el fetch de IMAP (el último de la bandeja).
/// - `last_decided_uid`: uid del último correo realmente procesado (0 si ninguno).
/// - `checkpoint_uid`: uid previo persistido.
/// - `excluded_count`: correos descartados por filtros.
pub fn rollback_uid(
    new_cp_uid: u32,
    last_decided_uid: u32,
    checkpoint_uid: u32,
    excluded_count: usize,
) -> u32 {
    if excluded_count == 0 {
        return new_cp_uid;
    }
    let candidate = last_decided_uid.max(checkpoint_uid);
    candidate.min(new_cp_uid)
}

/// Retención (minutos) de sugerencias resueltas antes de auto-archivarlas.
/// Compartida por la bandeja, el prune de arranque y el loop horario.
/// Valores negativos o absurdos (entrada manual en Ajustes) dejarían la
/// bandeja de resueltas vacía o eterna: corte saneado 0–7 días (bug L3).
pub fn retention_min(db: &crate::store::Db) -> i64 {
    db.settings_get("email.suggestion_retention_minutes")
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(0, 7 * 24 * 60))
        .unwrap_or(60)
}

/// Acepta una sugerencia: crea la tarea real y marca la sugerencia.
/// Idempotente: solo una sugerencia `pending` puede aceptarse; re-aceptar
/// una ya procesada devuelve la tarea existente en vez de duplicarla
/// (auditoría 17, hallazgo #3). Las tres escrituras van en una transacción
/// (#5): un fallo a mitad no deja sugerencia pending con tarea ya creada.
pub fn accept_suggestion(db: &Db, id: i64) -> Result<Vec<crate::store::TaskRow>, String> {
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
    if s.status != "pending" {
        // Excepción: el flujo de auto-aprobación inserta la sugerencia ya con
        // estado "auto_approved" y luego la acepta aquí (sin tarea aún).
        let auto_pending = s.status == "auto_approved" && s.result_task_id.is_none();
        if !auto_pending {
            if let Some(task_id) = s.result_task_id {
                if let Ok(Some(t)) = db.get_task(task_id) {
                    return Ok(vec![t]);
                }
            }
            return Err(format!(
                "la sugerencia ya fue procesada (estado: {})",
                s.status
            ));
        }
    }
    // Sin hora de inicio → tarea de Todo el día (hoy), nunca una hora inventada.
    let start = match s.start_at {
        Some(ms) => ms,
        None => {
            let day = chrono::Local::now().date_naive();
            let midnight = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap();
            chrono::Local
                .from_local_datetime(&day.and_time(midnight))
                .earliest()
                .map(|d| d.timestamp_millis())
                .unwrap_or_else(|| day.and_time(midnight).and_utc().timestamp_millis())
        }
    };
    let end = s.end_at.unwrap_or(start);
    // ¿Marcador de todo el día? Un deadline/tarea SIN hora concreta es un
    // marcador de día completo (la IA le asigna la hora del correo de origen,
    // p. ej. 23:59, que no es una hora real de evento). Un EVENT con hora
    // explícita (p. ej. "tutoría mañana a las 9") NO es all-day aunque la IA
    // no devuelva duración: conserva su hora y dura por defecto 1 h en vez de
    // colapsarse a medianoche (regresión detectada por e2e s3: un evento 09:00
    // se anclaba a 00:00 y desaparecía de la agenda).
    let has_explicit_time = s
        .start_at
        .map(|ms| {
            let t = chrono::DateTime::from_timestamp_millis(ms)
                .map(|d| d.with_timezone(&chrono::Local))
                .map(|d| d.format("%H:%M").to_string());
            t.map(|h| h != "00:00" && h != "23:59").unwrap_or(false)
        })
        .unwrap_or(false);
    let single_all_day = s.start_at.is_none()
        || s.end_at.is_none()
        || (s.start_at == s.end_at && !has_explicit_time)
        || s.kind == "availability";
    // Un marcador all-day se ancla a la MEDIANOCHE de su día: la IA resuelve
    // los deadlines sin hora a la hora del correo de origen (23:59), y aceptar
    // "11 sept 23:59" como all-day creaba un bloque 11 23:59 → 12 23:59 (la
    // tarea aparecía "inicio el 11, fin el 12, todo el día") en vez de marcar
    // únicamente el día 11 completo. Los rangos multi-día (availability)
    // también se anclan por extremo: split_range_blocks ya crea inicio +
    // "(entrega)" sobre cada día frontera.
    let (start, end) = if single_all_day {
        (
            crate::engine::local_midnight(start),
            crate::engine::local_midnight(end),
        )
    } else {
        (start, end)
    };
    let status = if s.status == "auto_approved" {
        "auto_approved"
    } else {
        "accepted"
    };
    // Rango multi-día (ventana de disponibilidad, "del 5 al 23", inicio+fin):
    // NO una tarea banner que ocupa todos los días intermedios — solo bloque
    // de inicio + bloque "(entrega)", igual que QuickAdd y el plan sugerido.
    let blocks = crate::planning::split_range_blocks(&s.title, start, end);
    let single_day = blocks.len() == 1;
    db.tx_begin().map_err(|e| e.to_string())?;
    let result = (|| {
        let mut created: Vec<crate::store::TaskRow> = Vec::new();
        for (title, bs, be, range_all_day) in &blocks {
            let all_day = if single_day {
                single_all_day
            } else {
                *range_all_day
            };
            // Marcador de día completo: una tarea all-day de UN día debe cubrir
            // [inicio, inicio + 24h), no tener duración cero. Si end <= start
            // (deadline a medianoche "vie 4 sept 00:00"), coversDay en el
            // frontend exige end > dayStart y la tarea queda invisible en
            // mes/día/semana. Igual que split_range_blocks hace con los
            // marcadores de día (s_day, s_day + DAY_MS).
            // Con hora explícita (deadline "a las 18:00" start == end) el
            // bloque dura 1 h por defecto — cero duración era invisible e
            // inútil (bug M7).
            let be = if *be <= *bs {
                if all_day {
                    *bs + crate::engine::DAY_MS
                } else {
                    *bs + crate::engine::HOUR_MS
                }
            } else {
                *be
            };
            let t = db
                .create(title, &s.category_id, &s.priority, *bs, be, all_day)
                .map_err(|e| e.to_string())?;
            created.push(t);
        }
        let task = created
            .first()
            .cloned()
            .ok_or_else(|| "tarea no creada".to_string())?;
        // Contexto de la sugerencia → descripción de la tarea: qué hay que
        // hacer + procedencia (remitente y asunto del correo).
        let mut desc = s.description.trim().to_string();
        let mut src: Vec<String> = Vec::new();
        if let Some(sender) = s
            .source_sender
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            src.push(format!("de {sender}"));
        }
        if !s.source_subject.trim().is_empty() {
            src.push(format!("\"{}\"", s.source_subject.trim()));
        }
        if !src.is_empty() {
            if !desc.is_empty() {
                desc.push_str("\n\n");
            }
            desc.push_str(&format!("Correo: {}", src.join(" · ")));
        }
        if !desc.is_empty() {
            db.set_description(task.id, &desc)
                .map_err(|e| e.to_string())?;
        }
        db.set_suggestion_status(id, status)
            .map_err(|e| e.to_string())?;
        db.set_suggestion_result_task(id, task.id)
            .map_err(|e| e.to_string())?;
        // trazabilidad de TODOS los bloques (rango multi-día = inicio +
        // "(entrega)"): revertir o borrar la sugerencia debe eliminarlos todos
        // (bug A1).
        for t in &created {
            db.link_suggestion_task(id, t.id)
                .map_err(|e| e.to_string())?;
        }
        // re-fetch main para incluir la descripción, luego reemplazar en el vec
        let main = db
            .get_task(task.id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "tarea no creada".to_string())?;
        created[0] = main;
        Ok(created)
    })();
    match result {
        Ok(tasks) => {
            db.tx_commit().map_err(|e| e.to_string())?;
            Ok(tasks)
        }
        Err(e) => {
            let _ = db.tx_rollback();
            Err(e)
        }
    }
}

/// Revierte la decisión: vuelve a "pending" y, si había tareas creadas, las
/// elimina TODAS (un rango multi-día dejó inicio + "(entrega)": borrar solo
/// result_task_id dejaba la entrega huérfana y re-aceptar la duplicaba — A1).
pub fn revert_suggestion(db: &Db, id: i64) -> Result<(), String> {
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
    // validación de estado en backend (no solo en la UI): una fusión
    // modificó una tarea existente sin guardar su estado previo — revertirla
    // no deshacía nada y re-aceptarla duplicaba la tarea. Una pendiente no
    // tiene nada que revertir.
    match s.status.as_str() {
        "accepted" | "auto_approved" | "rejected" => {}
        "merged" => {
            return Err(
                "una sugerencia fusionada no se puede revertir: edita la tarea directamente"
                    .into(),
            )
        }
        other => {
            return Err(format!(
                "no hay nada que revertir (estado: {other})"
            ))
        }
    }
    // todo el revert va en una transacción (bug L2): si el cambio de estado
    // fallaba tras borrar la tarea, la sugerencia quedaba "accepted"
    // apuntando a una tarea inexistente — ni aceptable ni reversible
    db.tx_begin().map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        let _ = db
            .unlink_and_delete_suggestion_tasks(id)
            .map_err(|e| e.to_string())?;
        db.set_suggestion_status(id, "pending")
            .map_err(|e| e.to_string())?;
        db.set_suggestion_result_task(id, 0)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            db.tx_commit().map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            let _ = db.tx_rollback();
            Err(e)
        }
    }
}

/// Fase 1 (DB, lock breve): ¿hay que procesar este correo? Devuelve false si
/// ya se deduplicó. Dos capas de defensa:
/// 1. `email_seen`: registro persistente de correos revisados (no desaparece
///    cuando el usuario rechaza/borra sugerencias ni con la retención).
/// 2. conteo de sugerencias vivas del correo (redundancia histórica).
fn prepare_email(db: &Db, raw: &RawEmail) -> Result<bool, String> {
    match db.email_seen(&raw.message_id) {
        Ok(true) => return Ok(false),
        Ok(false) => {}
        Err(e) => return Err(e.to_string()),
    }
    let already = db.suggestion_count_for_email(&raw.message_id).unwrap_or(0);
    Ok(already == 0)
}

/// Fase 2 (SOLO red, sin lock de DB): analiza el correo con la IA y devuelve
/// los compromisos accionables. `parse_email_intent` hace HTTP (hasta 90 s).
fn analyze_email(
    app: &AppHandle,
    provider: &dyn ai::AiProvider,
    configured: bool,
    raw: &RawEmail,
) -> Result<Vec<crate::ai::intent::Intent>, String> {
    let res = ai::email_intent::parse_email_intent(raw, provider, configured);
    match res {
        Err(AiError::RateLimited {
            retry_after,
            detail,
        }) => {
            crate::append_log(
                app,
                &format!(
                    "email_parse_429{} uid={} {}",
                    retry_after.map(|s| format!(" {s}")).unwrap_or_default(),
                    raw.uid,
                    detail
                ),
            );
            // formato conversable por el frontend: `ia_429 [segundos] detalle`
            Err(format!(
                "ia_429{} {detail}",
                retry_after.map(|s| format!(" {s}")).unwrap_or_default()
            ))
        }
        // IA no configurada o red caída: NO es culpa del correo → aborta sin
        // contar como fallo del correo (ver `counts_as_mail_fail`)
        // 4xx de la IA (salvo 401/403/408/429) = el PROPIO correo provoca un
        // rechazo determinista (moderación, request inválida, 413): cuenta
        // como fallo del correo para poder saltarlo tras MAX_SYNC_RETRIES
        // en vez de bloquear el buzón entero para siempre.
        Err(AiError::Http(e)) if is_per_request_4xx(&e) => Err(format!("{IA_FAIL_MAIL} {e}")),
        Err(AiError::Http(e)) | Err(AiError::NotConfigured(e)) => Err(format!("ia_fail {e}")),
        Err(AiError::BadResponse(e)) if e.contains("intención inválida") => {
            // Permanente: la IA produjo intents que nunca pasarán la
            // validación para ESTE correo (p. ej. deadline en el pasado en un
            // correo viejo). Abortar aquí dejaba el checkpoint congelado para
            // siempre: se reintentaba el mismo correo en cada sync. Se registra
            // y se trata como "sin compromisos" para poder avanzar.
            crate::append_log(app, &format!("email_intent_invalid uid={} {e}", raw.uid));
            Ok(Vec::new())
        }
        // respuesta mal formada para ESTE correo: cuenta como fallo del correo
        Err(AiError::BadResponse(e)) => Err(format!("{IA_FAIL_MAIL} {e}")),
        Err(AiError::InvalidJson(e)) => {
            crate::append_log(
                app,
                &format!("email_parse_invalid_json uid={} {e}", raw.uid),
            );
            Ok(Vec::new())
        }
        Ok(batch) => {
            let total = batch.intents.len();
            let actionable: Vec<_> = batch
                .intents
                .into_iter()
                .filter(|i| {
                    matches!(
                        i.intent_type,
                        crate::ai::intent::IntentType::Event
                            | crate::ai::intent::IntentType::Task
                            | crate::ai::intent::IntentType::Deadline
                            | crate::ai::intent::IntentType::Availability
                    )
                })
                .collect();
            if actionable.is_empty() {
                crate::append_log(
                    app,
                    &format!("email_no_intents uid={} n={}", raw.uid, total),
                );
            }
            Ok(actionable)
        }
    }
}

/// Fase 3 (DB, lock breve): inserta las sugerencias y marca el correo como
/// revisado. El visto se registra AUNQUE no haya intents: el correo ya se
/// analizó y no debe volver a la IA (con sugerencias o sin ellas).
///
/// Visto + sugerencias van en UNA transacción: antes, si fallaba un insert a
/// mitad, el correo quedaba visto (o con sugerencias parciales, que el
/// dedupe de `prepare_email` también trata como visto) y el resto de
/// compromisos se perdía. La auto-aprobación va DESPUÉS del commit porque
/// `accept_suggestion` abre su propia transacción (no anidables); si falla,
/// la sugerencia vuelve a pending como antes.
fn commit_email(
    app: &AppHandle,
    db: &Db,
    raw: &RawEmail,
    intents: &[crate::ai::intent::Intent],
) -> Result<usize, String> {
    db.tx_begin().map_err(|e| e.to_string())?;
    let inserted = (|| -> Result<Vec<(i64, bool)>, String> {
        db.email_mark_seen(&raw.message_id)
            .map_err(|e| e.to_string())?;
        intents
            .iter()
            .map(|it| insert_intent_suggestion(db, raw, it))
            .collect()
    })();
    let inserted = match inserted {
        Ok(v) => {
            db.tx_commit().map_err(|e| e.to_string())?;
            v
        }
        Err(e) => {
            let _ = db.tx_rollback();
            return Err(e);
        }
    };
    for (id, auto) in &inserted {
        if *auto {
            auto_approve(app, db, raw, *id);
        }
    }
    Ok(inserted.len())
}

/// Dominio del remitente para logs (sin la dirección completa: dato personal).
fn sender_domain(sender: &str) -> String {
    let addr = email::sender_email(sender);
    addr.split('@').nth(1).unwrap_or("?").to_string()
}

fn auto_approve(app: &AppHandle, db: &Db, raw: &RawEmail, id: i64) {
    match accept_suggestion(db, id) {
        Ok(_) => crate::append_log(
            app,
            &format!(
                "email_auto_approved uid={} dominio={} sugerencia={id}",
                raw.uid,
                sender_domain(&raw.sender)
            ),
        ),
        Err(e) => {
            // sin rollback, la bandeja muestra "Auto-aprobada" SIN tarea
            // creada y sin botón de acción (settled) — el compromiso se
            // pierde silenciosamente. Devolverla a pending la hace
            // aceptable/rechazable por el usuario (bug M5).
            crate::append_log(app, &format!("email_auto_approve_fail: {e}"));
            let _ = db.set_suggestion_status(id, "pending");
        }
    }
}

/// Inserta la sugerencia de un intent. Devuelve `(id, auto_aprobar)`; la
/// aceptación automática la hace el llamador tras cerrar la transacción.
fn insert_intent_suggestion(
    db: &Db,
    raw: &RawEmail,
    it: &crate::ai::intent::Intent,
) -> Result<(i64, bool), String> {
    use crate::ai::intent::IntentType;

    let kind = ai::email_intent::suggestion_kind(&it.intent_type);
    let (start_at, end_at, deadline_at) = match it.intent_type {
        IntentType::Deadline => (it.deadline, it.deadline, it.deadline),
        _ => (it.window.start, it.window.end, None),
    };
    // una sugerencia de disponibilidad SIEMPRE ocupa el rango completo
    let start = start_at.or(deadline_at);
    let end = if it.intent_type == IntentType::Availability {
        end_at.or(start)
    } else {
        end_at.or(start)
    };
    let prep_min = it.preparation.as_ref().map(|p| p.minutes).unwrap_or(0);

    // dedupe entre correos: mismo compromiso pendiente de otro correo
    // 0) primero, corrección dentro del MISMO hilo (References/In-Reply-To):
    //    "entrega viernes" → "corrección: entrega lunes" NO crea duplicado.
    let thread_dedupe = if raw.thread.is_empty() {
        None
    } else {
        db.find_similar_suggestion_in_thread(&it.title, &raw.thread, Some(&raw.message_id))
            .ok()
            .flatten()
    };
    let (dedupe_id, dedupe_note) = match thread_dedupe {
        Some((id, t)) => (Some(id), format!("Corrección del mismo hilo: {t}")),
        None => {
            match db.find_similar_suggestion(&it.title, start, end, Some(&raw.message_id)) {
                Ok(Some((id, t))) => (Some(id), format!("Ya detectado en otro correo: {t}")),
                _ => {
                    // y contra tareas ya existentes
                    match start {
                        Some(s) => match db.find_similar_task(&it.title, s, &raw.sender) {
                            Ok(Some((id, t))) => (Some(id), format!("Posible duplicado de: {t}")),
                            _ => (None, String::new()),
                        },
                        None => (None, String::new()),
                    }
                }
            }
        }
    };

    let trusted = db
        .is_trusted(&email::sender_email(&raw.sender))
        .unwrap_or(false);
    // auto-aprobación solo con remitente de confianza, sin duplicados y
    // con la fecha explícita (confianza alta)
    let status = if trusted && dedupe_id.is_none() && it.confidence >= 0.6 {
        "auto_approved"
    } else {
        "pending"
    };

    let id = db
        .insert_suggestion(
            "email",
            Some(&raw.message_id),
            Some(&raw.sender),
            &raw.subject,
            kind,
            &it.title,
            &it.description,
            &it.category_id,
            &priority_str(it.priority),
            start,
            end,
            deadline_at,
            prep_min,
            "",
            "[]",
            it.confidence,
            &it.reason,
            dedupe_id,
            &dedupe_note,
            status,
        )
        .map_err(|e| e.to_string())?;
    Ok((id, status == "auto_approved"))
}

fn priority_str(p: crate::ai::intent::Priority) -> String {
    match p {
        crate::ai::intent::Priority::Alta => "alta".into(),
        crate::ai::intent::Priority::Baja => "baja".into(),
        crate::ai::intent::Priority::Media => "media".into(),
    }
}

/// Error estable cuando ya hay un sync corriendo (el scheduler lo trata como
/// "skip", no como fallo).
pub const SYNC_BUSY: &str = "sincronización en curso: espera a que termine";

/// Prefijo de un fallo de IA atribuible al CORREO (respuesta mal formada
/// para ese contenido). Empieza por `ia_fail` para que el frontend y
/// `retry_delay_hint` lo sigan tratando como transitorio.
const IA_FAIL_MAIL: &str = "ia_fail_mail";

/// ¿Este error cuenta para el contador de reintentos DEL CORREO
/// (`register_fail` → saltarlo tras MAX_SYNC_RETRIES)? Solo errores
/// deterministas del contenido. 429, IA no configurada o red caída no son
/// culpa del correo: contarlos hacía que tras 3 syncs sin IA se saltara (y
/// perdiera) un correo válido.
fn counts_as_mail_fail(err: &str) -> bool {
    err.starts_with(IA_FAIL_MAIL)
}

/// `AiError::Http` empieza por el status HTTP ("400 Bad Request …"). 4xx
/// excepto credenciales (401/403), timeout (408) y cuota (429) → el error
/// depende de la petición (del correo), no del proveedor ni de la red.
fn is_per_request_4xx(http_err: &str) -> bool {
    match http_err.split_whitespace().next().and_then(|c| c.parse::<u16>().ok()) {
        Some(code) => (400..500).contains(&code) && !matches!(code, 401 | 403 | 408 | 429),
        None => false,
    }
}

static SYNC_RUNNING: AtomicBool = AtomicBool::new(false);

/// Guard de exclusión: syncs solapados (arranque + manual + scheduler)
/// duplicaban sugerencias y quemaban cuota de IA.
struct SyncGuard;

impl SyncGuard {
    fn acquire() -> Option<SyncGuard> {
        SYNC_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| SyncGuard)
    }
}

impl Drop for SyncGuard {
    fn drop(&mut self) {
        SYNC_RUNNING.store(false, Ordering::Release);
    }
}

/// Ejecuta una sincronización completa. Comandos y scheduler la llaman.
/// Cualquier `Err` (salvo "ya en curso") emite `email:sync-error` para que
/// la UI no quede atascada en "Comprobando…"; `invalid_grant` emite además
/// `auth:expired`.
pub fn run_sync(app: &AppHandle) -> Result<SyncSummary, String> {
    let Some(_guard) = SyncGuard::acquire() else {
        return Err(SYNC_BUSY.into());
    };
    let res = run_sync_locked(app);
    if let Err(e) = &res {
        if crate::auth::is_session_expired(e) {
            let _ = app.emit("auth:expired", e);
        }
        // precondiciones (email desactivado / sin configurar / sin sesión):
        // el sync ni empezó (no hubo `sync-progress`, la UI no quedó en
        // "Comprobando…"). Emitirlas desde el sync automático mostraba un
        // error en Ajustes en CADA arranque a usuarios sin email; el sync
        // manual ya recibe el Err por el `invoke`.
        if !is_precondition_error(e) {
            let _ = app.emit("email:sync-error", e);
        }
    }
    res
}

fn is_precondition_error(e: &str) -> bool {
    e.starts_with("email deshabilitado")
        || e.starts_with("email no configurado")
        || e.contains("no hay sesión de Google")
}

fn run_sync_locked(app: &AppHandle) -> Result<SyncSummary, String> {
    let started = email::now_ms();
    let mut summary = SyncSummary {
        started_at: started,
        ..Default::default()
    };

    let enabled = with_db(app, |db| {
        db.settings_get(SETTINGS_EMAIL_ENABLED)
            .ok()
            .flatten()
            .map(|v| v == "1")
            .unwrap_or(false)
    });
    if !enabled {
        return Err("email deshabilitado en Ajustes".into());
    }

    let config = with_db(app, load_email_config);
    if config.host.is_empty() || config.user.is_empty() {
        return Err("email no configurado: host y usuario requeridos".into());
    }

    // ventana temporal de revisión: solo correos de los últimos N días
    let since_days: u32 = with_db(app, |db| {
        db.settings_get("email.max_age_days")
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(7)
    });

    let ai_cfg = with_db(app, crate::ai_config_from_db);
    let provider = ai::provider_from_config(&ai_cfg).map_err(|e| e.to_string())?;
    // token OAuth2 de Google (CAMBIO 2): se lee la sesión con lock breve y el
    // refresco (red) ocurre FUERA del lock para no congelar otros comandos.
    // Guardado condicionado: no resucita la sesión si se cerró entretanto.
    let oauth_token = crate::auth::access_token_unlocked(&app.state::<Mutex<Db>>())?;
    let ai_configured = !ai_cfg.endpoint.is_empty()
        && !ai_cfg.model.is_empty()
        && ai_cfg.provider_name() != "local";

    // rescan pendiente → reiniciar checkpoints: se vuelve a repasar la
    // ventana reciente (dedup por message_id evita duplicados)
    let rescan = with_db(app, |db| {
        db.settings_get("email.rescan_pending")
            .ok()
            .flatten()
            .map(|v| v == "1")
            .unwrap_or(false)
    });
    if rescan {
        with_db(app, |db| {
            let _ = db.sync_state_clear_all();
            let _ = db.settings_set("email.rescan_pending", "0");
        });
        crate::append_log(app, "rescan_pending → checkpoints reiniciados");
    }

    let mut session = match email::connect(&config, &oauth_token) {
        Ok(s) => s,
        Err(e) => {
            crate::append_log(app, &format!("email_connect_fail: {e}"));
            return Err(format!("conexión fallida: {e}"));
        }
    };

    for mailbox in &config.mailboxes {
        let source = format!("email:{mailbox}");

        let (cp_json, checkpoint, prev_uid) = with_db(app, |db| {
            let cp_json = db
                .sync_state_get(&source)
                .ok()
                .flatten()
                .unwrap_or_default();
            let checkpoint: SyncCheckpoint =
                serde_json::from_str(&cp_json).unwrap_or_else(|_| SyncCheckpoint::empty());
            let uid = checkpoint.uid;
            (cp_json, checkpoint, uid)
        });

        match email::fetch_mailbox(&mut session, mailbox, &checkpoint, since_days) {
            Ok((emails, mut new_cp, parse_failures)) => {
                // MIME/base64 roto: no se puede reintentar con éxito; se da
                // por procesado pero queda en el log (sin contenido)
                for id in &parse_failures {
                    crate::append_log(app, &format!("email_parse_fail {source} id={id}"));
                }
                let mut mb = crate::sync::MailboxSummary {
                    mailbox: mailbox.clone(),
                    found: 0,
                    processed: 0,
                    result: "ok".into(),
                    error: String::new(),
                };
                // sin filtros → todos; con filtros → los que coinciden
                let (kept, excluded): (Vec<RawEmail>, Vec<RawEmail>) =
                    if email::has_filters(&config.filters) {
                        emails
                            .into_iter()
                            .partition(|e| email::matches_filters(e, &config.filters))
                    } else {
                        (emails, Vec::new())
                    };
                for e in &excluded {
                    // sin remitente completo ni asunto (datos personales)
                    crate::append_log(
                        app,
                        &format!(
                            "email_filtered uid={} dominio={}",
                            e.uid,
                            sender_domain(&e.sender)
                        ),
                    );
                }
                let filtered: Vec<RawEmail> = kept;
                mb.found = filtered.len();
                summary.total_found += filtered.len();

                let total = filtered.len();
                let mut last_decided_uid: u32 = 0;
                for (i, raw) in filtered.iter().enumerate() {
                    let _ = app.emit(
                        "email:sync-progress",
                        crate::sync::SyncProgress {
                            phase: "email".into(),
                            mailbox: mailbox.clone(),
                            processed: i + 1,
                            total,
                        },
                    );
                    let outcome = (|| -> Result<usize, String> {
                        // fase 1: dedupe previo (lock breve)
                        if !with_db(app, |db| prepare_email(db, raw))? {
                            return Ok(0);
                        }
                        // fase 2: IA sin lock (HTTP hasta 90 s)
                        let intents = analyze_email(app, provider.as_ref(), ai_configured, raw)?;
                        // fase 3: insertar sugerencias y marcar el correo como
                        // visto AUNQUE no se produzca ningún intent (contrato de
                        // commit_email): sin esto, un correo sin compromisos
                        // volvía a la IA en CADA sync, quemando la cuota de
                        // tokens (429 de Groq) y repartiendo el mismo trabajo.
                        with_db(app, |db| commit_email(app, db, raw, &intents))
                    })();
                    match outcome {
                        Ok(n) => {
                            last_decided_uid = last_decided_uid.max(raw.uid);
                            mb.processed += n;
                            summary.total_suggestions += n;
                        }
                        Err(e) => {
                            // fallo de red/IA → no avanzar checkpoint… pero si
                            // el MISMO correo falla MAX_SYNC_RETRIES veces
                            // seguidas (p. ej. proveedor saturado por horas),
                            // se salta para no congelar el sync entero; un
                            // rescan manual puede recuperarlo después.
                            // solo fallos deterministas del correo cuentan;
                            // 429 / IA sin configurar / red abortan sin
                            // tocar fail_count (el correo no tiene la culpa)
                            let mut cp2 = checkpoint.clone();
                            if counts_as_mail_fail(&e) {
                                let (next, skip) =
                                    crate::email::register_fail(&checkpoint, raw.uid);
                                cp2 = next;
                                if skip {
                                    crate::append_log(
                                        app,
                                        &format!("email_skip_after_retries uid={} {e}", raw.uid),
                                    );
                                    last_decided_uid = last_decided_uid.max(raw.uid);
                                    continue;
                                }
                            }
                            let cp = serde_json::to_string(&cp2).unwrap_or_default();
                            with_db(app, |db| {
                                let _ = db.sync_state_set(&source, &cp, "error", &e);
                                let _ = db.sync_history_add(
                                    &source,
                                    started,
                                    "error",
                                    filtered.len() as i64,
                                    mb.processed as i64,
                                    &e,
                                    "abortado sin avanzar checkpoint",
                                );
                            });
                            let _ = session.logout();
                            summary.error = Some(e.clone());
                            crate::append_log(app, &format!("sync_abort {source}: {e}"));
                            return Err(e);
                        }
                    }
                }

                // si algún correo quedó fuera por filtros, el checkpoint NO
                // avanza más allá del último correo realmente procesado:
                // se reintenta en la siguiente pasada (recuperable al
                // ajustar los filtros en Ajustes).
                let new_uid =
                    rollback_uid(new_cp.uid, last_decided_uid, checkpoint.uid, excluded.len());
                if new_uid < new_cp.uid {
                    crate::append_log(
                        app,
                        &format!(
                            "checkpoint_rollback {source} uid {} → {} ({} excluidos por filtros)",
                            new_cp.uid,
                            new_uid,
                            excluded.len()
                        ),
                    );
                    new_cp.uid = new_uid;
                }

                let ok_cp = serde_json::to_string(&new_cp).unwrap_or_default();
                let result = if filtered.is_empty() && prev_uid == new_cp.uid {
                    "no_new"
                } else {
                    "ok"
                };
                with_db(app, |db| {
                    let _ = db.sync_state_set(&source, &ok_cp, result, "");
                    let _ = db.sync_history_add(
                        &source,
                        started,
                        result,
                        filtered.len() as i64,
                        mb.processed as i64,
                        "",
                        &format!("uid {} → {}", prev_uid, new_cp.uid),
                    );
                });
                summary.mailboxes.push(mb);
                crate::append_log(
                    app,
                    &format!(
                        "checkpoint {source} uid {prev_uid} → {} (siguiente empieza en {})",
                        new_cp.uid,
                        new_cp.uid + 1
                    ),
                );
            }
            Err(e) => {
                let err = format!("{mailbox}: {e}");
                with_db(app, |db| {
                    let _ = db.sync_state_set(&source, &cp_json, "error", &err);
                    let _ = db.sync_history_add(&source, started, "error", 0, 0, &err, "");
                });
                summary.error = Some(err.clone());
                crate::append_log(app, &format!("sync_mailbox_error {err}"));
            }
        }
    }

    summary.finished_at = email::now_ms();
    crate::append_log(
        app,
        &format!(
            "sync_done found={} suggestions={} mbs={}",
            summary.total_found,
            summary.total_suggestions,
            summary.mailboxes.len()
        ),
    );

    if summary.total_suggestions > 0 {
        let _ = app.emit("email:new-suggestions", summary.total_suggestions);
        notify_new_suggestions(app, summary.total_suggestions);
    }
    let _ = app.emit("email:sync-done", &summary);
    Ok(summary)
}

fn notify_new_suggestions(app: &AppHandle, count: usize) {
    use tauri_plugin_notification::NotificationExt;
    let title = if count == 1 {
        "Nuevo evento detectado en tu correo".into()
    } else {
        format!("{count} nuevos eventos detectados en tu correo")
    };
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body("Revisa la bandeja de eventos en FocusFlow")
        .show();
    crate::append_log(app, &format!("notify_suggestions count={count}"));
}

/// Lazo del scheduler: corre cada `interval_hours` horas en background.
/// Fallos transitorios (429 de la IA, red, Gmail caído) programan un único
/// reintento diferido según el hint del error; no ensucia el historial.
pub fn scheduler_loop(app: AppHandle) {
    // Revisión inmediata al abrir la app: no esperar al intervalo (8 h por
    // defecto) para la primera verificación de correo. Corre en background
    // para no bloquear el arranque; si el correo está deshabilitado o sin
    // configurar, solo queda el error en el log.
    {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
            // los Err ya emiten `email:sync-error` dentro de run_sync
            let _ = tauri::async_runtime::spawn_blocking(move || match run_sync(&h) {
                Ok(s) => crate::append_log(
                    &h,
                    &format!("startup_sync_ok suggestions={}", s.total_suggestions),
                ),
                Err(e) => crate::append_log(&h, &format!("startup_sync_error: {e}")),
            })
            .await;
        });
    }
    let prune_app = app.clone();
    tauri::async_runtime::spawn(async move {
        // auto-archivo horario de sugerencias resueltas (retención 1 h por defecto)
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            let h = prune_app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let retention_min: i64 = with_db(&h, |db| crate::sync::retention_min(db));
                let pruned = with_db(&h, |db| {
                    db.prune_suggestions(email::now_ms() - retention_min * 60_000)
                });
                match pruned {
                    Ok(0) => {}
                    Ok(n) => crate::append_log(&h, &format!("suggestions_pruned count={n}")),
                    Err(e) => crate::append_log(&h, &format!("suggestions_prune_error: {e}")),
                }
            })
            .await;
        }
    });
    tauri::async_runtime::spawn(async move {
        // Duerme en pasos cortos y relee intervalo + último sync real: un
        // intervalo nuevo en Ajustes aplica ya (antes esperaba al final de la
        // espera anterior, hasta 8 h) y la hora coincide con el
        // `next_sync_at` que muestra la UI (último sync + intervalo).
        let mut last_attempt = email::now_ms(); // el sync de arranque
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let h = app.clone();
            let (interval_ms, last_sync) = with_db(&h, |db| {
                let last = db
                    .sync_history_last(1)
                    .ok()
                    .and_then(|v| v.first().map(|r| r.started_at));
                (interval_hours(db) as i64 * 3_600_000, last)
            });
            if !scheduler_due(email::now_ms(), interval_ms, last_sync, last_attempt) {
                continue;
            }
            last_attempt = email::now_ms();
            let h2 = app.clone();
            let outcome = tauri::async_runtime::spawn_blocking(move || match run_sync(&h2) {
                Ok(s) => {
                    crate::append_log(
                        &h2,
                        &format!("scheduler_sync_ok suggestions={}", s.total_suggestions),
                    );
                    // error de un buzón (429/5xx/red de Gmail) dentro de un
                    // sync "ok": también programa el reintento
                    s.error
                }
                // otro sync (manual/arranque) ya en curso: skip, no fallo
                Err(e) if e == SYNC_BUSY => {
                    crate::append_log(&h2, "scheduler_sync_skip: sincronización en curso");
                    None
                }
                Err(e) => Some(e),
            })
            .await
            .unwrap_or_else(|_| Some("sync abortado".into()));
            if let Some(e) = outcome {
                crate::append_log(&h, &format!("scheduler_sync_error: {e}"));
                // reintento único diferido si el fallo es transitorio
                if let Some(delay) = retry_delay_hint(&e) {
                    crate::append_log(
                        &h,
                        &format!("scheduler_sync_retry_scheduled delay_s={delay}"),
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    let h2 = h.clone();
                    let _ = tauri::async_runtime::spawn_blocking(move || match run_sync(&h2) {
                        Ok(s) => crate::append_log(
                            &h2,
                            &format!("scheduler_retry_ok suggestions={}", s.total_suggestions),
                        ),
                        Err(e) => crate::append_log(&h2, &format!("scheduler_retry_error: {e}")),
                    })
                    .await;
                }
            }
        }
    });
}

/// ¿Toca sync programado? `interval_ms == 0` = desactivado. Se cuenta desde
/// el último sync registrado o el último intento del scheduler (lo más
/// reciente: un sync que falla antes de escribir historial no debe
/// reintentarse cada minuto).
fn scheduler_due(now: i64, interval_ms: i64, last_sync: Option<i64>, last_attempt: i64) -> bool {
    if interval_ms <= 0 {
        return false;
    }
    let base = last_sync.unwrap_or(0).max(last_attempt);
    now >= base + interval_ms
}

/// Hint de reintento para errores transitorios de sync (segundos de espera).
/// `None` = fallo permanente (configuración), no reintentar.
///
/// Alineado con los mensajes REALES: `ia_429 [s] …` / `ia_fail…` (IA),
/// `short_gmail_error` de email.rs (429 "Límite de consultas a Gmail…
/// ~N s", 5xx/otros "…se reintentará automáticamente"), errores de red
/// `gmail api: …` y `conexión fallida: …`. Los errores de buzón llegan como
/// `"{mailbox}: {mensaje}"`, por eso se busca por contenido.
pub fn retry_delay_hint(err: &str) -> Option<u64> {
    const DEFAULT_RETRY: u64 = 300;
    if let Some(rest) = err.strip_prefix("ia_429") {
        // `ia_429 [segundos] detalle`
        let secs = rest
            .split_whitespace()
            .next()
            .and_then(|t| t.parse::<u64>().ok());
        return Some(secs.unwrap_or(DEFAULT_RETRY).clamp(30, 3600));
    }
    if err.contains("Límite de consultas a Gmail") {
        // `… Reintento automático en ~{s} s.` (Retry-After de Gmail)
        let secs = err
            .split('~')
            .nth(1)
            .and_then(|r| r.split_whitespace().next())
            .and_then(|t| t.parse::<u64>().ok());
        return Some(secs.unwrap_or(DEFAULT_RETRY).clamp(30, 3600));
    }
    if err.starts_with("ia_fail")
        || err.contains("gmail api:")
        || err.contains("gmail api body:")
        || err.contains("conexión fallida")
        || err.contains("se reintentará")
    {
        return Some(DEFAULT_RETRY);
    }
    // fallo de configuración (email deshabilitado, sin sesión de Google,
    // sesión expirada, sin permisos, …)
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::retry_delay_hint;

    #[test]
    fn retry_hint_429_con_segundos() {
        assert_eq!(retry_delay_hint("ia_429 92 FreeUsageLimitError"), Some(92));
        // sin hint → default 300 s; con hint enorme → capado a 1 h
        assert_eq!(retry_delay_hint("ia_429 oops"), Some(300));
        assert_eq!(retry_delay_hint("ia_429 99999 x"), Some(3600));
    }

    #[test]
    fn retry_hint_transitorios_y_permanentes() {
        // Corregido (auditoría lote A #9): el test usaba cadenas irreales
        // ("gmail api 429 Too Many Requests") que el cliente ya no produce;
        // ahora se prueban los mensajes REALES de short_gmail_error.
        use reqwest::StatusCode;
        let real = |code: StatusCode, retry: Option<u64>| {
            format!(
                "INBOX: {}",
                crate::email::gmail::GmailClient::short_gmail_error_for_test(code, "", retry)
            )
        };
        assert_eq!(retry_delay_hint("ia_fail timeout"), Some(300));
        assert_eq!(retry_delay_hint("ia_fail_mail falta choices"), Some(300));
        assert_eq!(
            retry_delay_hint(&real(StatusCode::TOO_MANY_REQUESTS, Some(90))),
            Some(90)
        );
        assert_eq!(
            retry_delay_hint(&real(StatusCode::TOO_MANY_REQUESTS, None)),
            Some(300)
        );
        assert_eq!(
            retry_delay_hint(&real(StatusCode::BAD_GATEWAY, None)),
            Some(300)
        );
        assert_eq!(
            retry_delay_hint("INBOX: gmail api: error sending request"),
            Some(300)
        );
        assert_eq!(
            retry_delay_hint("conexión fallida: gmail api timeout"),
            Some(300)
        );
        // sin permisos / sesión caducada → no reintentar a ciegas
        assert_eq!(retry_delay_hint(&real(StatusCode::FORBIDDEN, None)), None);
        assert_eq!(
            retry_delay_hint(crate::auth::SESSION_EXPIRED),
            None
        );
        // permanentes → sin reintento
        assert_eq!(retry_delay_hint("email deshabilitado en Ajustes"), None);
        assert_eq!(
            retry_delay_hint("no hay sesión de Google: inicia sesión para sincronizar Gmail"),
            None
        );
    }

    #[test]
    fn ai_4xx_per_request_counts_but_network_and_auth_do_not() {
        assert!(is_per_request_4xx("400 Bad Request {\"error\":\"moderation\"}"));
        assert!(is_per_request_4xx("413 Payload Too Large"));
        assert!(!is_per_request_4xx("401 Unauthorized"));
        assert!(!is_per_request_4xx("403 Forbidden"));
        assert!(!is_per_request_4xx("429 Too Many Requests"));
        assert!(!is_per_request_4xx("503 Service Unavailable"));
        assert!(!is_per_request_4xx("error sending request"));
    }

    #[test]
    fn only_deterministic_mail_errors_count_towards_skip() {
        // #3: IA sin configurar / red / 429 NO cuentan como fallo del correo
        assert!(!counts_as_mail_fail("ia_429 60 quota"));
        assert!(!counts_as_mail_fail("ia_fail sin clave"));
        assert!(!counts_as_mail_fail("ia_fail error sending request"));
        assert!(counts_as_mail_fail("ia_fail_mail falta choices[0]"));
    }

    #[test]
    fn scheduler_due_follows_interval_changes() {
        const H: i64 = 3_600_000;
        let t0 = 1_000 * H;
        // intervalo 8 h, último sync hace 2 h → no toca
        assert!(!scheduler_due(t0, 8 * H, Some(t0 - 2 * H), t0 - 2 * H));
        // el usuario baja el intervalo a 1 h → toca YA (sin esperar 8 h)
        assert!(scheduler_due(t0, H, Some(t0 - 2 * H), t0 - 2 * H));
        // desactivado
        assert!(!scheduler_due(t0, 0, None, 0));
        // sin historial (sync falló pronto): cuenta desde el último intento
        assert!(!scheduler_due(t0, H, None, t0 - 10 * 60_000));
    }

    #[test]
    fn sync_guard_is_exclusive_and_released_on_drop() {
        // #4: un segundo sync mientras corre el primero → rechazado
        let g = SyncGuard::acquire().expect("primer sync");
        assert!(SyncGuard::acquire().is_none(), "sync solapado");
        drop(g);
        let g2 = SyncGuard::acquire().expect("liberado al terminar");
        drop(g2);
    }

    #[test]
    fn no_excluded_emails_keeps_forward_progress() {
        // sin filtros → el checkpoint avanza al último uid del fetch
        assert_eq!(rollback_uid(50, 0, 10, 0), 50);
        assert_eq!(rollback_uid(50, 30, 10, 0), 50);
    }

    #[test]
    fn excluded_emails_roll_back_uid() {
        // 20 excluidos al final: nunca pasar del último procesado (30)
        assert_eq!(rollback_uid(50, 30, 10, 20), 30);
        // sin procesados (todos excluidos): se mantiene el previo
        assert_eq!(rollback_uid(50, 0, 10, 5), 10);
        // procesados hasta el final → igual que sin rollback
        assert_eq!(rollback_uid(50, 50, 10, 3), 50);
    }

    #[test]
    fn accept_suggestion_all_day_deadline_creates_full_day() {
        // Sugerencia con deadline a medianoche (start == end == misma
        // medianoche). Al aceptar, la tarea all-day debe cubrir 24h, no
        // tener duración cero — si no, coversDay la ve invisible.
        const DAY_MS: i64 = crate::engine::DAY_MS;
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let now = chrono::Local::now().timestamp_millis();
        let today = crate::engine::local_midnight(now);
        let id = db
            .insert_suggestion(
                "test",
                None,
                None,
                "test",
                "deadline",                    // kind
                "Envío enlace video catálogo", // title
                "",                            // description
                "uni",                         // category_id
                "alta",                        // priority
                Some(today),                   // start_at (hoy)
                Some(today),                   // end_at == start_at (misma medianoche)
                None,                          // deadline_at
                0,                             // prep_min
                "",                            // location
                "[]",                          // tags
                0.8,                           // confidence
                "test",                        // reason
                None,                          // dedupe_task_id
                "",                            // dedupe_note
                "pending",                     // status
            )
            .unwrap();
        let tasks = accept_suggestion(&db, id).unwrap();

        // Una tarea creada (single day, no split)
        assert_eq!(tasks.len(), 1, "debe crear exactamente 1 tarea");
        let t = &tasks[0];
        assert_eq!(t.title, "Envío enlace video catálogo");
        assert!(t.all_day, "la tarea debe ser all_day");
        assert_eq!(
            t.start_at, today,
            "start_at debe ser la medianoche de la sugerencia"
        );
        assert_eq!(
            t.end_at - t.start_at,
            DAY_MS,
            "end_at debe ser start + 24h para cubrir el día completo, no duración cero"
        );
    }

    #[test]
    fn accept_suggestion_deadline_2359_marks_only_that_day() {
        // La IA resuelve "entrega el 11" (sin hora) a la hora del correo de
        // origen (23:59 del día 11). La tarea all-day debe marcar SOLO el
        // día 11 [00:00, 00:00+24h), NO un bloque 11 23:59 → 12 23:59
        // ("inicio el 11 y fin el 12, ambas todo el día").
        const DAY_MS: i64 = crate::engine::DAY_MS;
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let day11 =
            crate::engine::local_midnight(chrono::Local::now().timestamp_millis()) + 4 * DAY_MS;
        let at_2359 = day11 + 23 * 3_600_000 + 59 * 60_000;
        let id = db
            .insert_suggestion(
                "test",
                None,
                None,
                "test",
                "deadline",
                "Analítica Digital: entrega tarea",
                "",
                "uni",
                "media",
                Some(at_2359),
                Some(at_2359), // start == end == 23:59 del día 11
                Some(at_2359),
                0,
                "",
                "[]",
                0.96,
                "fecha de entrega explícita",
                None,
                "",
                "pending",
            )
            .unwrap();
        let tasks = accept_suggestion(&db, id).unwrap();
        assert_eq!(tasks.len(), 1);
        let t = &tasks[0];
        assert!(t.all_day);
        assert_eq!(t.start_at, day11, "anclada a la medianoche del día 11");
        assert_eq!(
            t.end_at - t.start_at,
            DAY_MS,
            "cubre solo el día 11, no toca el 12"
        );
        assert!(
            t.end_at <= day11 + DAY_MS,
            "el bloque no se extiende al día 12"
        );
    }

    #[test]
    fn email_seen_survives_suggestion_rejection_and_prune() {
        // El registro de correos revisados es persistente: rechazar/borrar
        // las sugerencias (o la retención que las archiva) NO debe hacer
        // que el correo se re-analice en el siguiente sync.
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let raw = crate::email::RawEmail {
            mailbox: "INBOX".into(),
            uid: 7,
            message_id: "<canvas-1@amazonses.com>".into(),
            thread: Vec::new(),
            subject: "Tarea calificada".into(),
            sender: "Canvas <notifications@instructure.com>".into(),
            date: "2026-09-07".into(),
            body: "entrega el 11".into(),
        };
        assert!(!db.email_seen(&raw.message_id).unwrap());
        db.email_mark_seen(&raw.message_id).unwrap();
        // idempotente
        db.email_mark_seen(&raw.message_id).unwrap();
        assert!(db.email_seen(&raw.message_id).unwrap());
        assert_eq!(db.email_seen_count().unwrap(), 1);
        // sugerencia del correo, resuelta y archivada por retención
        let sid = db
            .insert_suggestion(
                "email",
                Some(&raw.message_id),
                Some(&raw.sender),
                "Tarea calificada",
                "deadline",
                "Analítica Digital: entrega tarea",
                "",
                "uni",
                "media",
                None,
                None,
                None,
                0,
                "",
                "[]",
                0.9,
                "test",
                None,
                "",
                "pending",
            )
            .unwrap();
        db.set_suggestion_status(sid, "rejected").unwrap();
        let pruned = db
            .prune_suggestions(crate::email::now_ms() + 1_000)
            .unwrap();
        assert_eq!(pruned, 1, "la retención archiva la sugerencia");
        // …y aun así el correo sigue marcado como visto
        assert!(
            db.email_seen(&raw.message_id).unwrap(),
            "el correo NO vuelve a la IA tras rechazar/archivar sus sugerencias"
        );
    }

    #[test]
    fn migrate_0011_backfills_seen_from_existing_suggestions() {
        // Instalación existente: los Message-ID que ya generaron
        // sugerencias deben quedar registrados como vistos por la
        // migración, sin esperar a re-procesar el correo.
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        // open_memory_clean_pub ya corrió migrate() (incluida 0011);
        // simular una sugerencia previa + re-ejecutar el backfill como
        // haría la migración sobre una BD vieja.
        db.email_mark_seen("<legacy@x.com>").unwrap();
        let _ = db
            .insert_suggestion(
                "email",
                Some("<legacy-2@x.com>"),
                Some("a@x.com"),
                "Asunto",
                "deadline",
                "Entrega X",
                "",
                "uni",
                "media",
                None,
                None,
                None,
                0,
                "",
                "[]",
                0.9,
                "test",
                None,
                "",
                "pending",
            )
            .unwrap();
        let n = db.email_seen_backfill().unwrap();
        assert_eq!(
            n, 1,
            "el backfill registra el correo de la sugerencia legado"
        );
        assert!(db.email_seen("<legacy-2@x.com>").unwrap());
        assert!(db.email_seen("<legacy@x.com>").unwrap());
    }

    // ---- auditoría 2026-09-15: regresiones de sugerencias/tareas ----

    fn sug(
        db: &crate::store::Db,
        kind: &str,
        title: &str,
        email_id: Option<&str>,
        start: Option<i64>,
        end: Option<i64>,
    ) -> i64 {
        db.insert_suggestion(
            "email",
            email_id,
            Some("profe@a.com"),
            "asunto",
            kind,
            title,
            "",
            "uni",
            "media",
            start,
            end,
            None,
            0,
            "",
            "[]",
            0.9,
            "test",
            None,
            "",
            "pending",
        )
        .unwrap()
    }

    #[test]
    fn revert_and_delete_remove_all_multiday_blocks() {
        // A1: una disponibilidad multi-día crea inicio + "(entrega)"; revertir
        // o borrar la sugerencia debe eliminar AMBAS tareas (antes quedaba la
        // entrega huérfana y re-aceptar la duplicaba).
        const DAY_MS: i64 = crate::engine::DAY_MS;
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let now = crate::email::now_ms();
        let id = sug(
            &db,
            "availability",
            "Ventana proyecto",
            None,
            Some(now + DAY_MS),
            Some(now + 4 * DAY_MS),
        );
        let tasks = accept_suggestion(&db, id).unwrap();
        assert_eq!(tasks.len(), 2, "inicio + (entrega)");
        assert_eq!(db.suggestion_task_ids(id).unwrap().len(), 2);

        revert_suggestion(&db, id).unwrap();
        for t in &tasks {
            assert!(db.get_task(t.id).unwrap().is_none(), "tarea huérfana");
        }
        // re-aceptar no duplica: exactamente 2 tareas otra vez
        let tasks2 = accept_suggestion(&db, id).unwrap();
        assert_eq!(tasks2.len(), 2);
        assert_eq!(
            db.list().unwrap().len(),
            2,
            "sin duplicados tras revert+accept"
        );

        // borrar la sugerencia elimina también todas sus tareas
        db.delete_suggestion(id).unwrap();
        assert!(db.list().unwrap().is_empty(), "delete limpia ambos bloques");
    }

    #[test]
    fn dedupe_matches_undated_suggestions() {
        // A2: el mismo compromiso sin fecha detectado en OTRO correo debe
        // deduplicarse; `start_at BETWEEN` con NULL nunca coincidía.
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        sug(
            &db,
            "task",
            "Llamar a la doctora",
            Some("<a@x>"),
            None,
            None,
        );
        let hit = db
            .find_similar_suggestion("Llamar a la doctora", None, None, Some("<b@x>"))
            .unwrap();
        assert!(hit.is_some(), "sugerencia sin fecha participa del dedupe");
        // una búsqueda CON fecha no se fusiona con la vaga (perdería su hora)
        let dated = db
            .find_similar_suggestion(
                "Llamar a la doctora",
                Some(crate::email::now_ms() + 86_400_000),
                None,
                Some("<b@x>"),
            )
            .unwrap();
        assert!(
            dated.is_none(),
            "la sugerencia vaga no secuestra una fechada"
        );
    }

    #[test]
    fn accept_deadline_with_explicit_time_gets_one_hour() {
        // M7: deadline "a las 18:00" (start == end con hora explícita) no debe
        // materializarse como tarea de duración cero.
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let day11 =
            crate::engine::local_midnight(crate::email::now_ms()) + 2 * crate::engine::DAY_MS;
        let at18 = day11 + 18 * 3_600_000;
        let id = sug(
            &db,
            "deadline",
            "Entregar informe",
            None,
            Some(at18),
            Some(at18),
        );
        let tasks = accept_suggestion(&db, id).unwrap();
        assert_eq!(tasks.len(), 1);
        let t = &tasks[0];
        assert!(!t.all_day, "conserva su hora");
        assert_eq!(t.end_at - t.start_at, 3_600_000, "1 h por defecto");
    }

    #[test]
    fn update_task_full_normalizes_allday_marker() {
        // M1: editar una sugerencia aceptada con inicio == fin (all-day) debe
        // quedar con 24 h de cobertura; si no, coversDay la esconde.
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let day = crate::engine::local_midnight(crate::email::now_ms()) + 3 * crate::engine::DAY_MS;
        let t = db
            .create("Entrega", "uni", "media", day, day, true)
            .unwrap();
        assert_eq!(t.end_at - t.start_at, crate::engine::DAY_MS);
        // ruta update (suggestion_edit/merge con all_day=None conserva el
        // all_day almacenado y normaliza igual)
        db.update_task_full(
            t.id, "Entrega", "uni", "media", day, day, "", "[]", "", "", None, None,
        )
        .unwrap();
        let t2 = db.get_task(t.id).unwrap().unwrap();
        assert!(t2.all_day);
        assert_eq!(
            t2.end_at - t2.start_at,
            crate::engine::DAY_MS,
            "24 h tras editar"
        );
        // y un span invertido se aplana, no se guarda negativo
        db.update_task_full(
            t.id,
            "Entrega",
            "uni",
            "media",
            day,
            day - 5_000,
            "",
            "[]",
            "",
            "",
            None,
            Some(false),
        )
        .unwrap();
        let t3 = db.get_task(t.id).unwrap().unwrap();
        assert!(t3.end_at >= t3.start_at, "sin span invertido");
    }

    #[test]
    fn create_sanitizes_priority_and_negative_span() {
        // L7/L4: prioridad inválida cae a "media" (evita el error CHECK opaco)
        // y end < start no se persiste invertido.
        let db = crate::store::Db::open_memory_clean_pub().unwrap();
        let now = crate::email::now_ms();
        let t = db
            .create("X", "uni", "urgentísima", now, now + 60_000, false)
            .unwrap();
        assert_eq!(t.priority, "media");
        let t2 = db
            .create("Y", "uni", "alta", now, now - 60_000, false)
            .unwrap();
        assert!(t2.end_at >= t2.start_at);
        // la prioridad válida NO se toca
        let t3 = db
            .create("Z", "uni", "baja", now, now + 60_000, false)
            .unwrap();
        assert_eq!(t3.priority, "baja");
    }
}
