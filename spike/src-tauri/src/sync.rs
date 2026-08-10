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
    let db = state.lock().unwrap();
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

/// Acepta una sugerencia: crea la tarea real y marca la sugerencia.
pub fn accept_suggestion(db: &Db, id: i64) -> Result<crate::store::TaskRow, String> {
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
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
    // Una ventana de disponibilidad se acepta como UNA tarea de todo el día
    // que abarca el rango completo (nunca una tarea por día).
    let all_day = s.start_at.is_none() || s.end_at.is_none() || s.start_at == s.end_at || s.kind == "availability";
    let task = db
        .create(&s.title, &s.category_id, &s.priority, start, end, all_day)
        .map_err(|e| e.to_string())?;
    let status = if s.status == "auto_approved" { "auto_approved" } else { "accepted" };
    db.set_suggestion_status(id, status).map_err(|e| e.to_string())?;
    db.set_suggestion_result_task(id, task.id).map_err(|e| e.to_string())?;
    Ok(task)
}

/// Revierte la decisión: vuelve a "pending" y, si había tarea creada, la elimina.
pub fn revert_suggestion(db: &Db, id: i64) -> Result<(), String> {
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
    if let Some(task_id) = s.result_task_id {
        let _ = db.delete(task_id);
    }
    db.set_suggestion_status(id, "pending").map_err(|e| e.to_string())?;
    db.set_suggestion_result_task(id, 0).map_err(|e| e.to_string())?;
    Ok(())
}

/// Fase 1 (DB, lock breve): ¿hay que procesar este correo? Devuelve false si
/// ya se deduplicó (mismo message_id ya procesado).
fn prepare_email(db: &Db, raw: &RawEmail) -> Result<bool, String> {
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
        Err(AiError::Http(e)) | Err(AiError::NotConfigured(e)) | Err(AiError::BadResponse(e)) => {
            Err(format!("ia_fail {e}"))
        }
        Err(AiError::InvalidJson(e)) => {
            crate::append_log(app, &format!("email_parse_invalid_json uid={} {e}", raw.uid));
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

/// Fase 3 (DB, lock breve): deduplica e inserta las sugerencias.
fn commit_email(
    app: &AppHandle,
    db: &Db,
    raw: &RawEmail,
    intents: &[crate::ai::intent::Intent],
) -> Result<usize, String> {
    let mut count = 0;
    for it in intents {
        count += insert_intent_suggestion(app, db, raw, it)?;
    }
    Ok(count)
}

/// Procesa un correo con la IA: compromisos → sugerencias (o auto-aprobación).
/// Fase 8: usa el pipeline de intenciones (event | deadline | availability |
/// task), minimiza el cuerpo antes de mandarlo a la IA y deduplica entre
/// correos (mismo compromiso en varios correos → una sola sugerencia).
///
/// Combina las tres fases; `run_sync` las ejecuta por separado para no
/// mantener el `Mutex<Db>` durante la llamada HTTP de la IA.
fn process_email(
    app: &AppHandle,
    db: &Db,
    provider: &dyn ai::AiProvider,
    configured: bool,
    raw: &RawEmail,
) -> Result<usize, String> {
    if !prepare_email(db, raw)? {
        return Ok(0);
    }
    let intents = analyze_email(app, provider, configured, raw)?;
    commit_email(app, db, raw, &intents)
}

fn insert_intent_suggestion(
    app: &AppHandle,
    db: &Db,
    raw: &RawEmail,
    it: &crate::ai::intent::Intent,
) -> Result<usize, String> {
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
    let (dedupe_id, dedupe_note) = match db.find_similar_suggestion(&it.title, start, end, Some(&raw.message_id)) {
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
    };

    let trusted = db.is_trusted(&email::sender_email(&raw.sender)).unwrap_or(false);
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

    if status == "auto_approved" {
        match accept_suggestion(db, id) {
            Ok(_) => crate::append_log(
                app,
                &format!("email_auto_approved uid={} sender={} kind={kind}", raw.uid, raw.sender),
            ),
            Err(e) => crate::append_log(app, &format!("email_auto_approve_fail: {e}")),
        }
    }
    Ok(1)
}

fn priority_str(p: crate::ai::intent::Priority) -> String {
    match p {
        crate::ai::intent::Priority::Alta => "alta".into(),
        crate::ai::intent::Priority::Baja => "baja".into(),
        crate::ai::intent::Priority::Media => "media".into(),
    }
}

/// Ejecuta una sincronización completa. Comandos y scheduler la llaman.
pub fn run_sync(app: &AppHandle) -> Result<SyncSummary, String> {
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
    if ai::get_email_credentials(&config.user).is_none() {
        return Err("falta la contraseña de aplicación del correo (Ajustes → Correo)".into());
    }
    let ai_configured = !ai_cfg.endpoint.is_empty() && !ai_cfg.model.is_empty() && ai_cfg.provider_name() != "local";

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

    let mut session = match email::connect(&config) {
        Ok(s) => s,
        Err(e) => {
            crate::append_log(app, &format!("email_connect_fail: {e}"));
            return Err(format!("conexión fallida: {e}"));
        }
    };

    for mailbox in &config.mailboxes {
        let source = format!("email:{mailbox}");

        let (cp_json, checkpoint, prev_uid) = with_db(app, |db| {
            let cp_json = db.sync_state_get(&source).ok().flatten().unwrap_or_default();
            let checkpoint: SyncCheckpoint = serde_json::from_str(&cp_json).unwrap_or_else(|_| SyncCheckpoint::empty());
            let uid = checkpoint.uid;
            (cp_json, checkpoint, uid)
        });

            match email::fetch_mailbox(&mut session, mailbox, &checkpoint, since_days) {
                Ok((emails, mut new_cp)) => {
                    let mut mb = crate::sync::MailboxSummary {
                        mailbox: mailbox.clone(),
                        found: 0,
                        processed: 0,
                        result: "ok".into(),
                        error: String::new(),
                    };
                    // sin filtros → todos; con filtros → los que coinciden
                    let (kept, excluded): (Vec<RawEmail>, Vec<RawEmail>) = if email::has_filters(&config.filters) {
                        emails.into_iter().partition(|e| email::matches_filters(e, &config.filters))
                    } else {
                        (emails, Vec::new())
                    };
                    for e in &excluded {
                        crate::append_log(
                            app,
                            &format!("email_filtered uid={} sender={} asunto={}", e.uid, e.sender, e.subject),
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
                            if intents.is_empty() {
                                return Ok(0);
                            }
                            // fase 3: insertar sugerencias (lock breve)
                            with_db(app, |db| commit_email(app, db, raw, &intents))
                        })();
                        match outcome {
                            Ok(n) => {
                                last_decided_uid = last_decided_uid.max(raw.uid);
                                mb.processed += n;
                                summary.total_suggestions += n;
                            }
                        Err(e) => {
                            // fallo de red/IA → no avanzar checkpoint
                            let cp = serde_json::to_string(&checkpoint).unwrap_or_default();
                            with_db(app, |db| {
                                let _ = db.sync_state_set(&source, &cp, "error", &e);
                                let _ = db.sync_history_add(
                                    &source, started, "error", filtered.len() as i64,
                                    mb.processed as i64, &e, "abortado sin avanzar checkpoint",
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
                let new_uid = rollback_uid(new_cp.uid, last_decided_uid, checkpoint.uid, excluded.len());
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
                let result = if filtered.is_empty() && prev_uid == new_cp.uid { "no_new" } else { "ok" };
                with_db(app, |db| {
                    let _ = db.sync_state_set(&source, &ok_cp, result, "");
                    let _ = db.sync_history_add(
                        &source, started, result, filtered.len() as i64,
                        mb.processed as i64, "", &format!("uid {} → {}", prev_uid, new_cp.uid),
                    );
                });
                summary.mailboxes.push(mb);
                crate::append_log(
                    app,
                    &format!("checkpoint {source} uid {prev_uid} → {} (siguiente empieza en {})", new_cp.uid, new_cp.uid + 1),
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
            summary.total_found, summary.total_suggestions, summary.mailboxes.len()
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
pub fn scheduler_loop(app: AppHandle) {
    // Revisión inmediata al abrir la app: no esperar al intervalo (8 h por
    // defecto) para la primera verificación de correo. Corre en background
    // para no bloquear el arranque; si el correo está deshabilitado o sin
    // configurar, solo queda el error en el log.
    {
        let h = app.clone();
        tauri::async_runtime::spawn(async move {
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
                let retention_min: i64 = with_db(&h, |db| {
                    db.settings_get("email.suggestion_retention_minutes")
                        .ok()
                        .flatten()
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(60)
                });
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
        loop {
            let h = app.clone();
            let interval_ms = with_db(&h, interval_hours) * 3_600_000;
            if interval_ms == 0 {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                continue;
            }
            tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
            let h2 = app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || match run_sync(&h2) {
                Ok(s) => crate::append_log(
                    &h2,
                    &format!("scheduler_sync_ok suggestions={}", s.total_suggestions),
                ),
                Err(e) => crate::append_log(&h2, &format!("scheduler_sync_error: {e}")),
            })
            .await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
