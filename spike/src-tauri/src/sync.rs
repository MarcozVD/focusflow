use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::ai::validation::ParsedEvent;
use crate::ai::{self, AiError, AiResult};
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
            day.and_time(midnight).and_utc().timestamp_millis()
        }
    };
    let end = s.end_at.unwrap_or(start);
    let all_day = s.start_at.is_none() || s.end_at.is_none() || s.start_at == s.end_at;
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

/// Procesa un correo con la IA: relevancia → eventos → sugerencias (o auto-aprobación).
fn process_email(
    app: &AppHandle,
    db: &Db,
    provider: &dyn ai::AiProvider,
    raw: &RawEmail,
) -> Result<usize, String> {
    let already = db
        .suggestion_count_for_email(&raw.message_id)
        .unwrap_or(0);
    if already > 0 {
        return Ok(0);
    }
    let res: AiResult<ai::validation::EmailParseResult> =
        ai::email_parser::parse_email(&raw.subject, &raw.sender, &raw.date, &raw.body, provider);
    match res {
        Err(AiError::Http(e)) | Err(AiError::NotConfigured(e)) | Err(AiError::BadResponse(e)) => {
            Err(format!("ia_fail {e}"))
        }
        Err(AiError::InvalidJson(e)) => {
            crate::append_log(app, &format!("email_parse_invalid_json uid={} {e}", raw.uid));
            Ok(0)
        }
        Ok(parsed) => {
            if !parsed.is_relevant {
                crate::append_log(
                    app,
                    &format!("email_not_relevant uid={} reason={}", raw.uid, parsed.reason),
                );
                return Ok(0);
            }
            if parsed.events.is_empty() {
                crate::append_log(app, &format!("email_no_events uid={} reason={}", raw.uid, parsed.reason));
                return Ok(0);
            }
            let mut count = 0;
            for ev in &parsed.events {
                count += insert_event_suggestion(app, db, raw, ev.clone(), parsed.confidence, &parsed.reason)?;
            }
            Ok(count)
        }
    }
}

fn insert_event_suggestion(
    app: &AppHandle,
    db: &Db,
    raw: &RawEmail,
    ev: ParsedEvent,
    confidence: f64,
    reason: &str,
) -> Result<usize, String> {
    let (dedupe_id, dedupe_note) = match db.find_similar_task(&ev.title, ev.start_ms, &raw.sender) {
        Ok(Some((id, t))) => (Some(id), format!("Posible duplicado de: {t}")),
        _ => (None, String::new()),
    };

    let trusted = db.is_trusted(&email::sender_email(&raw.sender)).unwrap_or(false);
    let status = if trusted && dedupe_id.is_none() { "auto_approved" } else { "pending" };
    let tags = serde_json::to_string(&ev.tags).unwrap_or_else(|_| "[]".into());

    let id = db
        .insert_suggestion(
            "email",
            Some(&raw.message_id),
            Some(&raw.sender),
            &ev.title,
            &ev.description,
            &ev.category_id,
            &ev.priority,
            Some(ev.start_ms),
            Some(ev.end_ms),
            &ev.location,
            &tags,
            confidence,
            reason,
            dedupe_id,
            &dedupe_note,
            status,
        )
        .map_err(|e| e.to_string())?;

    if status == "auto_approved" {
        match accept_suggestion(db, id) {
            Ok(_) => crate::append_log(
                app,
                &format!("email_auto_approved uid={} sender={}", raw.uid, raw.sender),
            ),
            Err(e) => crate::append_log(app, &format!("email_auto_approve_fail: {e}")),
        }
    }
    Ok(1)
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
            Ok((emails, new_cp)) => {
                let mut mb = crate::sync::MailboxSummary {
                    mailbox: mailbox.clone(),
                    found: 0,
                    processed: 0,
                    result: "ok".into(),
                    error: String::new(),
                };
                let filtered: Vec<RawEmail> = emails
                    .into_iter()
                    .filter(|e| email::matches_filters(e, &config.filters))
                    .collect();
                mb.found = filtered.len();
                summary.total_found += filtered.len();

                let total = filtered.len();
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
                    let outcome = with_db(app, |db| process_email(app, db, provider.as_ref(), raw));
                    match outcome {
                        Ok(n) => {
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
