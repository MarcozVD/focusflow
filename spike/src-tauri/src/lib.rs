use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::TimeZone;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

pub mod ai;
pub mod assistant;
pub mod auth;
pub mod email;
pub mod engine;
pub mod notify;
pub mod planning;
pub mod reminders;
pub mod report; // MÓDULO OPCIONAL de reporte de errores — ver cabecera de report.rs para retirarlo
pub mod store;
pub mod sync;
#[cfg(windows)]
pub mod win_toast;

use ai::{validation::ParsedTask, AiConfig};
use store::{lock_recover, Db, TaskRow};

/// Directorio de log en %TEMP%. Nunca panic: si no se puede crear, el log
/// se degrada a no-op (auditoría 17, hallazgo #6).
pub(crate) fn log_dir() -> Option<PathBuf> {
    let d = std::env::temp_dir().join("focusflow-spike");
    fs::create_dir_all(&d).ok()?;
    Some(d)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// Cooldown por comando para las llamadas a la IA (rate limit IPC): evita
/// ráfagas de peticiones costosas de hasta 90 s cada una desde el frontend
/// (debounce no basta; un IPC repetido puede lanzarlas igualmente).
fn ai_cooldown(cmd: &str) -> Result<(), String> {
    static LAST: std::sync::OnceLock<Mutex<std::collections::HashMap<String, std::time::Instant>>> =
        std::sync::OnceLock::new();
    const MIN_GAP_MS: u128 = 800;
    let map = LAST.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut m = lock_recover(map);
    let now = std::time::Instant::now();
    if let Some(prev) = m.get(cmd) {
        if now.duration_since(*prev).as_millis() < MIN_GAP_MS {
            return Err("demasiadas peticiones seguidas: espera un momento.".into());
        }
    }
    m.insert(cmd.to_string(), now);
    Ok(())
}

pub(crate) fn append_log(_app: &AppHandle, line: &str) {
    let Some(dir) = log_dir() else { return };
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("spike.log"))
    {
        let _ = writeln!(f, "[{}] {}", now_ms(), sanitize_log_line(line));
    }
}

/// Sanea una línea de log: sin saltos de línea ni caracteres de control (un
/// asunto/cuerpo de correo malicioso no puede forjar líneas), y con tope de
/// longitud. Se aplica a TODA entrada, incluidas las del comando `log_line`.
pub(crate) fn sanitize_log_line(line: &str) -> String {
    const MAX: usize = 2000;
    let out: String = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    out.chars().take(MAX).collect()
}

#[tauri::command]
fn log_line(app: AppHandle, line: String) {
    append_log(&app, &line);
}

// ---------------- tasks ----------------

#[tauri::command]
fn task_list(state: State<'_, Mutex<Db>>) -> Result<Vec<TaskRow>, String> {
    lock_recover(&state).list().map_err(|e| e.to_string())
}

#[tauri::command]
fn task_list_range(state: State<'_, Mutex<Db>>, start_at: i64, end_at: i64) -> Result<Vec<TaskRow>, String> {
    state
        .lock()
        .unwrap()
        .list_range(start_at, end_at)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn task_create(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    title: String,
    category_id: String,
    priority: String,
    start_at: i64,
    end_at: i64,
    all_day: bool,
) -> Result<TaskRow, String> {
    let task = state
        .lock()
        .unwrap()
        .create(&title, &category_id, &priority, start_at, end_at, all_day)
        .map_err(|e| e.to_string())?;
    let _ = app.emit("tasks:changed", ());
    Ok(task)
}

#[tauri::command]
fn task_complete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64, done: bool) -> Result<(), String> {
    lock_recover(&state).set_completed(id, done).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks:changed", ());
    Ok(())
}

#[tauri::command]
fn task_delete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    lock_recover(&state).delete(id).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks:changed", ());
    Ok(())
}

/// MÓDULO OPCIONAL de reporte de errores (ver report.rs): envía un correo con
/// la descripción y los últimos errores del log usando la cuenta configurada.
#[tauri::command]
fn report_send(app: AppHandle, state: State<'_, Mutex<Db>>, description: String) -> Result<String, String> {
    let (cfg, token) = {
        let db = lock_recover(&state);
        let cfg = sync::load_email_config(&db);
        let token = auth::access_token(&db)?;
        (cfg, token)
    };
    let r = report::send_report(&cfg, &token, &description);
    append_log(&app, &format!("report_send ok={}", r.is_ok()));
    r
}

#[derive(Serialize)]
struct TaskMoveResult {
    conflict: Option<String>,
}

#[tauri::command]
fn task_move(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    start_at: i64,
    end_at: i64,
    all_day: Option<bool>,
) -> Result<TaskMoveResult, String> {
    let db = lock_recover(&state);
    // validación de conflictos configurables (solapamiento) antes de guardar.
    // Por defecto el movimiento se permite y solo se avisa; si el usuario activa
    // `calendar.conflict_strict` (restricciones), el movimiento conflictivo se bloquea.
    let check_conflicts = setting_bool(&db, "calendar.conflict_check", true);
    let strict = setting_bool(&db, "calendar.conflict_strict", false);
    let mut conflict = None;
    if check_conflicts && all_day != Some(true) {
        if let Some((_, other)) = db.find_overlap(id, start_at, end_at).map_err(|e| e.to_string())? {
            if strict {
                return Err(format!("conflicto: se solapa con '{other}'"));
            }
            conflict = Some(other);
        }
    }
    db.move_to(id, start_at, end_at, all_day).map_err(|e| e.to_string())?;
    drop(db);
    append_log(
        &app,
        &format!("task_moved id={id} start={start_at} end={end_at} all_day={all_day:?} conflict={conflict:?}"),
    );
    let _ = app.emit("tasks:changed", ());
    Ok(TaskMoveResult { conflict })
}

#[tauri::command]
fn task_update(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    title: String,
    category_id: String,
    priority: String,
    start_at: i64,
    end_at: i64,
    description: String,
    tags: String,
    notes: String,
    links: String,
    reminder_minutes: Option<i64>,
    all_day: bool,
) -> Result<(), String> {
    state
        .lock()
        .unwrap()
        .update_task_full(
            id, &title, &category_id, &priority, start_at, end_at,
            &description, &tags, &notes, &links, reminder_minutes, Some(all_day),
        )
        .map_err(|e| e.to_string())?;
    append_log(&app, &format!("task_updated id={id} title={title} all_day={all_day}"));
    let _ = app.emit("tasks:changed", ());
    Ok(())
}

#[tauri::command]
fn task_duplicate(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<TaskRow, String> {
    let db = lock_recover(&state);
    let t = db.duplicate(id).map_err(|e| e.to_string())?.ok_or_else(|| "tarea no encontrada".to_string())?;
    drop(db);
    let _ = app.emit("tasks:changed", ());
    Ok(t)
}

// ---------------- módulo 1: IA (lenguaje natural) ----------------

pub(crate) fn ai_config_from_db(db: &Db) -> AiConfig {
    AiConfig {
        endpoint: db
            .settings_get("ai.endpoint")
            .ok()
            .flatten()
            .unwrap_or_else(|| ai::default_endpoint()),
        model: db
            .settings_get("ai.model")
            .ok()
            .flatten()
            .unwrap_or_else(|| ai::default_model()),
        provider: db
            .settings_get("ai.provider")
            .ok()
            .flatten()
            .unwrap_or_else(|| ai::default_provider()),
    }
}

#[derive(Serialize)]
struct TaskFromTextResult {
    task: TaskRow,
    source: String,
    used_ai: bool,
}

#[tauri::command]
async fn task_from_text(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
) -> Result<TaskFromTextResult, String> {
    ai_cooldown("task_from_text")?;
    // La configuración se lee bajo un lock corto. La llamada a la IA es
    // bloqueante (hasta 90 s) y se ejecuta en otro hilo, fuera del mutex,
    // para que la app siga respondiendo mientras se interpreta el texto.
    let cfg = {
        let db = lock_recover(&state);
        ai_config_from_db(&db)
    };

    let log_app = app.clone();
    let (parsed, source, used_ai) = tauri::async_runtime::spawn_blocking(move || {
        let mut used_ai = false;
        let parsed: (ParsedTask, String) = match ai::provider_from_config(&cfg) {
            Ok(provider) => {
                used_ai = true;
                match ai::task_parser::parse_task_text(&text, provider.as_ref(), true) {
                    Ok(p) => p,
                    Err(e) => {
                        append_log(&log_app, &format!("nl_ai_fail: {e}"));
                        ai::nl::parse_task_nl(&text)
                            .map(|t| (t, "local".into()))
                            .ok_or_else(|| "no se pudo interpretar el texto".to_string())?
                    }
                }
            }
            Err(_) => ai::nl::parse_task_nl(&text)
                .map(|t| (t, "local".into()))
                .ok_or_else(|| "no se pudo interpretar el texto".to_string())?,
        };
        Ok::<(ParsedTask, String, bool), String>((parsed.0, parsed.1, used_ai))
    })
    .await
    .map_err(|e| e.to_string())??;

    let task = {
        let db = lock_recover(&state);
        let t = db
            .create(&parsed.title, &parsed.category_id, &parsed.priority, parsed.start_ms, parsed.end_ms, parsed.all_day)
            .map_err(|e| e.to_string())?;
        // conservar el recordatorio sugerido por la IA ("1d", "3h", ...)
        if let Some(min) = parsed.reminders.first().and_then(|s| reminders::parse_reminder_minutes(s)) {
            db.set_task_reminder(t.id, min).map_err(|e| e.to_string())?;
        }
        t
    };
    append_log(
        &app,
        &format!("nl_task source={source} ai={used_ai} title={} start={}", parsed.title, parsed.start_ms),
    );
    let _ = app.emit("tasks:changed", ());
    Ok(TaskFromTextResult { task, source, used_ai })
}

#[derive(Serialize)]
struct AiConfigView {
    endpoint: String,
    model: String,
    effective_endpoint: String,
    effective_model: String,
    configured: bool,
}

#[tauri::command]
fn ai_config_get(state: State<'_, Mutex<Db>>) -> AiConfigView {
    let db = lock_recover(&state);
    let cfg = ai_config_from_db(&db);
    AiConfigView {
        endpoint: cfg.endpoint.clone(),
        model: cfg.model.clone(),
        effective_endpoint: if cfg.endpoint.is_empty() {
            ai::default_endpoint()
        } else {
            cfg.endpoint.clone()
        },
        effective_model: if cfg.model.is_empty() {
            ai::default_model()
        } else {
            cfg.model.clone()
        },
        configured: !cfg.endpoint.is_empty() && !cfg.model.is_empty() && cfg.provider_name() != "local",
    }
}

#[tauri::command]
fn ai_config_set(state: State<'_, Mutex<Db>>, endpoint: String, model: String) -> Result<(), String> {
    // Auto-completado: si el usuario deja un campo vacío y existe un default
    // (variable de entorno AI_ENDPOINT/AI_MODEL), se materializa al guardar.
    // Evita que el frontend borre el default al guardar con el campo en blanco.
    let endpoint = if endpoint.trim().is_empty() {
        ai::default_endpoint()
    } else {
        endpoint
    };
    let model = if model.trim().is_empty() {
        ai::default_model()
    } else {
        model
    };
    let db = lock_recover(&state);
    db.settings_set("ai.endpoint", &endpoint).map_err(|e| e.to_string())?;
    db.settings_set("ai.model", &model).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize)]
struct AiTestResult {
    ok: bool,
    latency_ms: u64,
    model: String,
    error: String,
}

#[tauri::command]
async fn ai_test(state: State<'_, Mutex<Db>>, app: AppHandle) -> Result<AiTestResult, String> {
    ai_cooldown("ai_test").ok(); // prueba manual: cooldown suave, no bloquea
    // Lock corto solo para leer la config; la llamada de red (hasta ~4,5 min
    // con reintentos 429) va a otro hilo y fuera del mutex, o congela todos
    // los comandos IPC que tocan la DB (auditoría 17, hallazgo #1).
    let cfg = {
        let db = lock_recover(&state);
        ai_config_from_db(&db)
    };
    let t0 = std::time::Instant::now();
    let res = tauri::async_runtime::spawn_blocking(move || {
        match ai::provider_from_config(&cfg) {
            Ok(provider) => {
                let model = cfg.model.clone();
                match provider.chat_json("Devuelve exactamente: {\"ok\": true}", "ping", r#"{"ok": true}"#) {
                    Ok(_) => AiTestResult {
                        ok: true,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        model,
                        error: String::new(),
                    },
                    Err(e) => AiTestResult {
                        ok: false,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        model,
                        error: e.to_string(),
                    },
                }
            }
            Err(e) => AiTestResult {
                ok: false,
                latency_ms: 0,
                model: cfg.model.clone(),
                error: e.to_string(),
            },
        }
    })
    .await;
    match res {
        Ok(r) => {
            if r.ok {
                append_log(&app, "ai_test_ok");
            }
            Ok(r)
        }
        Err(e) => Ok(AiTestResult {
            ok: false,
            latency_ms: 0,
            model: String::new(),
            error: e.to_string(),
        }),
    }
}

// ---------------- email + sync ----------------

#[derive(Serialize)]
struct EmailConfigView {
    config: email::EmailConfig,
    enabled: bool,
    interval_hours: u64,
    max_age_days: u32,
    trusted: Vec<String>,
}

#[tauri::command]
fn email_config_get(state: State<'_, Mutex<Db>>) -> EmailConfigView {
    let db = lock_recover(&state);
    let config = sync::load_email_config(&db);
    EmailConfigView {
        enabled: db
            .settings_get("email.enabled")
            .ok()
            .flatten()
            .map(|v| v == "1")
            .unwrap_or(false),
        interval_hours: sync::interval_hours(&db),
        max_age_days: db
            .settings_get("email.max_age_days")
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(7),
        trusted: db.trusted_list().unwrap_or_default(),
        config,
    }
}

#[tauri::command]
fn email_config_set(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    config: email::EmailConfig,
    enabled: bool,
    interval_hours: u64,
    max_age_days: u32,
) -> Result<(), String> {
    let db = lock_recover(&state);
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    db.settings_set("email.config", &json).map_err(|e| e.to_string())?;
    db.settings_set("email.enabled", if enabled { "1" } else { "0" }).map_err(|e| e.to_string())?;
    db.settings_set("email.interval_hours", &interval_hours.to_string()).map_err(|e| e.to_string())?;
    db.settings_set("email.max_age_days", &max_age_days.to_string()).map_err(|e| e.to_string())?;
    append_log(&app, &format!("email_config_saved user={} mailboxes={:?} enabled={enabled} max_age_days={max_age_days}", config.user, config.mailboxes));
    Ok(())
}

#[tauri::command]
fn email_sync_now(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match sync::run_sync(&handle) {
            Ok(s) => append_log(&handle, &format!("manual_sync_ok found={} suggestions={}", s.total_found, s.total_suggestions)),
            Err(e) => {
                append_log(&handle, &format!("manual_sync_error: {e}"));
                let _ = handle.emit("email:sync-error", e);
            }
        }
    });
    Ok(())
}

/// Reescanear la ventana reciente: reinicia checkpoints y sincroniza.
/// Recupera correos que quedaron fuera por filtros o errores previos
/// (la deduplicación por message_id evita duplicados).
#[tauri::command]
fn email_rescan(app: AppHandle, state: State<'_, Mutex<Db>>) -> Result<(), String> {
    {
        let db = lock_recover(&state);
        db.settings_set("email.rescan_pending", "1").map_err(|e| e.to_string())?;
    }
    append_log(&app, "email_rescan_requested");
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match sync::run_sync(&handle) {
            Ok(s) => append_log(&handle, &format!("rescan_ok found={} suggestions={}", s.total_found, s.total_suggestions)),
            Err(e) => {
                append_log(&handle, &format!("rescan_error: {e}"));
                let _ = handle.emit("email:sync-error", e);
            }
        }
    });
    Ok(())
}

#[derive(Serialize)]
struct ConnectionCheck {
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
struct VerifyResult {
    ai: ConnectionCheck,
    email: ConnectionCheck,
}

/// Prueba ambas conexiones: API de IA (OpenCode Zen) y correo (IMAP).
/// Async + spawn_blocking: la red (IA + IMAP, potencialmente minutos con
/// reintentos) no puede correr en el hilo principal o congela la UI
/// (auditoría 17, hallazgo #1).
#[tauri::command]
async fn verify_connections(state: State<'_, Mutex<Db>>, app: AppHandle) -> Result<VerifyResult, String> {
    ai_cooldown("verify_connections").ok(); // onboarding: cooldown suave
    let (ai_cfg, email_cfg) = {
        let db = lock_recover(&state);
        (ai_config_from_db(&db), sync::load_email_config(&db))
    };
    let app2 = app.clone();

    let res = tauri::async_runtime::spawn_blocking(move || {
        let ai = match ai::provider_from_config(&ai_cfg) {
            Ok(provider) => {
                let t0 = std::time::Instant::now();
                match provider.chat_json("Devuelve exactamente: {\"ok\": true}", "ping", r#"{"ok": true}"#) {
                    Ok(_) => ConnectionCheck {
                        ok: true,
                        detail: format!("API OK ({}, {} ms)", ai_cfg.model, t0.elapsed().as_millis()),
                    },
                    Err(e) => ConnectionCheck {
                        ok: false,
                        detail: format!("{} ({} ms)", e, t0.elapsed().as_millis()),
                    },
                }
            }
            Err(e) => ConnectionCheck { ok: false, detail: e.to_string() },
        };

        let token = crate::sync::with_db(&app2, |db| crate::auth::access_token(db)).unwrap_or_default();
        let email = match email::test_connection(&email_cfg, &token) {
            Ok((mailbox, n)) => ConnectionCheck {
                ok: true,
                detail: format!("Conectado a {mailbox} ({n} correos)"),
            },
            Err(e) => ConnectionCheck { ok: false, detail: e },
        };
        (ai, email)
    })
    .await;

    let result = match res {
        Ok((ai, email)) => {
            append_log(
                &app,
                &format!(
                    "verify ai={} email={}",
                    if ai.ok { "ok" } else { "fail" },
                    if email.ok { "ok" } else { "fail" }
                ),
            );
            VerifyResult { ai, email }
        }
        Err(e) => VerifyResult {
            ai: ConnectionCheck { ok: false, detail: e.to_string() },
            email: ConnectionCheck { ok: false, detail: "no ejecutado".into() },
        },
    };
    Ok(result)
}

// ---------------- Google OAuth (CAMBIO 2) ----------------

/// Inicia el flujo OAuth2 PKCE completo (navegador + callback + intercambio).
/// Async + spawn_blocking: puede tardar hasta ~2 min mientras el usuario
/// autoriza en el navegador; la DB no se retiene durante el flujo.
#[tauri::command]
async fn auth_google_sign_in(app: AppHandle) -> Result<auth::AuthSessionView, String> {
    let app2 = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let session = auth::perform_login()?;
        // persistir sesión + materializar config de Gmail (lock breve)
        crate::sync::with_db(&app2, |db| {
            db.auth_save(&session).map_err(|e| e.to_string())?;
            let cfg = auth::gmail_email_config(&session.email);
            let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
            db.settings_set("email.config", &json).map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })?;
        Ok::<_, String>(auth::to_view(&session))
    })
    .await;
    match res {
        Ok(Ok(v)) => {
            append_log(&app, &format!("auth_sign_in ok user={}", v.email));
            // enfoca la ventana principal: el usuario acaba de autorizar en el
            // navegador y la app debe traerse a primer plano automáticamente
            show_main(&app);
            Ok(v)
        }
        Ok(Err(e)) => {
            append_log(&app, &format!("auth_sign_in error: {e}"));
            Err(e)
        }
        Err(e) => {
            append_log(&app, &format!("auth_sign_in thread error: {e}"));
            Err(format!("error interno: {e}"))
        }
    }
}

/// Cierra sesión: borra los tokens de la DB (refresh_token incluido).
#[tauri::command]
fn auth_google_sign_out(app: AppHandle, state: State<'_, Mutex<Db>>) -> Result<(), String> {
    let db = lock_recover(&state);
    db.auth_clear().map_err(|e| e.to_string())?;
    append_log(&app, "auth_sign_out");
    Ok(())
}

/// Estado actual de la sesión (sin red, sin refresco).
#[tauri::command]
fn auth_status(state: State<'_, Mutex<Db>>) -> Option<auth::AuthSessionView> {
    let db = lock_recover(&state);
    auth::status(&db)
}

#[derive(Serialize)]
struct SyncStatusView {
    states: Vec<store::SyncStateRow>,
    today: Vec<store::SyncHistoryRow>,
    last_history: Vec<store::SyncHistoryRow>,
    last_sync_at: Option<i64>,
    next_sync_at: Option<i64>,
    interval_hours: u64,
}

#[tauri::command]
fn sync_status(state: State<'_, Mutex<Db>>) -> SyncStatusView {
    let db = lock_recover(&state);
    let now = chrono::Local::now();
    let start_of_day = chrono::Local
        .from_local_datetime(&now.date_naive().and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
    let today = db.sync_history_today(start_of_day).unwrap_or_default();
    let last_history = db.sync_history_last(10).unwrap_or_default();
    let last_sync_at = last_history.first().map(|h| h.started_at);
    let interval = sync::interval_hours(&db);
    let next_sync_at = last_sync_at.map(|t| t + interval as i64 * 3_600_000);
    SyncStatusView {
        states: db.sync_state_all().unwrap_or_default(),
        today,
        last_history,
        last_sync_at,
        next_sync_at,
        interval_hours: interval,
    }
}

// ---------------- bandeja de eventos detectados ----------------

#[tauri::command]
fn suggestions_list(state: State<'_, Mutex<Db>>, only_pending: bool) -> Result<Vec<store::SuggestionRow>, String> {
    let db = lock_recover(&state);
    let retention_min: i64 = db
        .settings_get("email.suggestion_retention_minutes")
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(60);
    db.list_suggestions(only_pending, retention_min * 60_000)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn suggestion_accept(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<TaskRow, String> {
    let db = lock_recover(&state);
    let task = sync::accept_suggestion(&db, id)?;
    drop(db);
    append_log(&app, &format!("suggestion_accepted id={id} task={}", task.id));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(task)
}

#[tauri::command]
fn suggestion_reject(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    state
        .lock()
        .unwrap()
        .set_suggestion_status(id, "rejected")
        .map_err(|e| e.to_string())?;
    append_log(&app, &format!("suggestion_rejected id={id}"));
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
fn suggestion_revert(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    let db = lock_recover(&state);
    sync::revert_suggestion(&db, id)?;
    drop(db);
    append_log(&app, &format!("suggestion_reverted id={id}"));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

/// Elimina la sugerencia por completo (control del usuario). Si tenía tarea
/// creada, la borra también. No se puede recuperar.
#[tauri::command]
fn suggestion_delete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    let db = lock_recover(&state);
    db.delete_suggestion(id).map_err(|e| e.to_string())?;
    drop(db);
    append_log(&app, &format!("suggestion_deleted id={id}"));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
fn suggestion_edit(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    title: String,
    category_id: String,
    priority: String,
    start_at: i64,
    end_at: i64,
    description: String,
    all_day: bool,
) -> Result<(), String> {
    let db = lock_recover(&state);
    db.update_suggestion_data(id, &title, &category_id, &priority, start_at, end_at, &description)
        .map_err(|e| e.to_string())?;
    // si la sugerencia ya fue aceptada, la tarea creada se mantiene en sincronía
    // (solo los campos del formulario; description/tags/notas/links y el
    // recordatorio de la tarea se preservan leyendo la tarea actual)
    if let Some(task_id) = db.get_suggestion(id).ok().flatten().and_then(|s| s.result_task_id) {
        if let Some(t) = db.get_task(task_id).ok().flatten() {
            db.update_task_full(
                task_id, &title, &category_id, &priority, start_at, end_at,
                &description, &t.tags, &t.notes, &t.links, t.reminder_minutes, Some(all_day),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    drop(db);
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
fn suggestion_merge(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    task_id: i64,
) -> Result<(), String> {
    let db = lock_recover(&state);
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
    let existing = db
        .get_task(task_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "tarea no encontrada".to_string())?;
    let title = if s.title.trim().is_empty() { existing.title.clone() } else { s.title.clone() };
    let start = s.start_at.unwrap_or(existing.start_at);
    let end = s.end_at.unwrap_or(existing.end_at);
    let priority = if s.priority == "media" { existing.priority.clone() } else { s.priority.clone() };
    // fusionar conserva los campos enriquecidos de la tarea existente
    // (tags/notas/links/recordatorio); la descripción toma el contexto de la
    // sugerencia cuando la tarea no lo tiene, o lo añade si aporta algo nuevo
    let sug_desc = s.description.trim();
    let description = if existing.description.trim().is_empty() {
        sug_desc.to_string()
    } else if !sug_desc.is_empty() && !existing.description.contains(sug_desc) {
        format!("{}\n\n{}", existing.description.trim(), sug_desc)
    } else {
        existing.description.clone()
    };
    db.update_task_full(
        task_id, &title, &s.category_id, &priority, start, end,
        &description, &existing.tags, &existing.notes, &existing.links,
        existing.reminder_minutes, None,
    )
    .map_err(|e| e.to_string())?;
    db.set_suggestion_status(id, "merged").map_err(|e| e.to_string())?;
    drop(db);
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
fn trusted_senders_list(state: State<'_, Mutex<Db>>) -> Result<Vec<String>, String> {
    lock_recover(&state).trusted_list().map_err(|e| e.to_string())
}

#[tauri::command]
fn trusted_senders_add(state: State<'_, Mutex<Db>>, sender: String) -> Result<(), String> {
    lock_recover(&state).trusted_add(&sender).map_err(|e| e.to_string())
}

#[tauri::command]
fn trusted_senders_remove(state: State<'_, Mutex<Db>>, sender: String) -> Result<(), String> {
    lock_recover(&state).trusted_remove(&sender).map_err(|e| e.to_string())
}

// ---------------- propuestas de planificación (fase 7) ----------------

#[tauri::command]
async fn plan_from_text(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
) -> Result<planning::PlanProposalView, String> {
    ai_cooldown("plan_from_text")?;
    let cfg = {
        let db = lock_recover(&state);
        ai_config_from_db(&db)
    };
    let log_app = app.clone();
    // La IA es bloqueante: se interpreta el texto fuera del mutex.
    let text_ai = text.clone();
    let (intents, source) = tauri::async_runtime::spawn_blocking(move || {
        let (provider, configured) = match ai::provider_from_config(&cfg) {
            Ok(p) => (Some(p), true),
            Err(_) => (None, false),
        };
        let batch = match ai::intent_parser::parse_intent(&text_ai, provider.as_deref(), configured) {
            Ok(b) => b,
            Err(e) => {
                append_log(&log_app, &format!("plan_ai_fail: {e}"));
                return Err("no se pudo interpretar el texto".to_string());
            }
        };
        Ok::<(Vec<ai::intent::Intent>, String), String>((batch.intents, batch.source))
    })
    .await
    .map_err(|e| e.to_string())??;

    let view = {
        let db = lock_recover(&state);
        planning::plan_from_text(&db, &text, &intents, &source)?
    };
    append_log(&app, &format!("plan_created id={} source={} items={}", view.id, view.source, view.items.len()));
    let _ = app.emit("plans:changed", ());
    Ok(view)
}

/// Variante 100% local de plan_from_text: sin IA ni cooldown, para cuando el
/// proveedor está lento/saturado y el usuario elige la interpretación rápida.
#[tauri::command]
async fn plan_from_text_local(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
) -> Result<planning::PlanProposalView, String> {
    let text_ai = text.clone();
    let (intents, source) = tauri::async_runtime::spawn_blocking(move || {
        ai::intent_parser::parse_intent(&text_ai, None, false)
            .map(|b| (b.intents, b.source))
            .map_err(|e| format!("no se pudo interpretar el texto: {e}"))
    })
    .await
    .map_err(|e| e.to_string())??;

    let view = {
        let db = lock_recover(&state);
        planning::plan_from_text(&db, &text, &intents, &source)?
    };
    append_log(&app, &format!("plan_created id={} source={} items={} (local)", view.id, view.source, view.items.len()));
    let _ = app.emit("plans:changed", ());
    Ok(view)
}

#[tauri::command]
fn plan_proposal_get(state: State<'_, Mutex<Db>>, id: i64) -> Result<Option<planning::PlanProposalView>, String> {
    planning::get_plan(&lock_recover(&state), id)
}

#[tauri::command]
fn plan_proposals_list(state: State<'_, Mutex<Db>>, only_pending: bool) -> Result<Vec<store::PlanProposalRow>, String> {
    lock_recover(&state).list_plan_proposals(only_pending).map_err(|e| e.to_string())
}

#[tauri::command]
fn plan_accept(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    edit: Option<planning::EditedPlan>,
) -> Result<Vec<TaskRow>, String> {
    let db = lock_recover(&state);
    let tasks = planning::accept_plan(&db, id, &edit.unwrap_or_default())?;
    drop(db);
    append_log(&app, &format!("plan_accepted id={id} tasks={}", tasks.len()));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("plans:changed", ());
    Ok(tasks)
}

#[tauri::command]
fn plan_reject(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    let db = lock_recover(&state);
    planning::reject_plan(&db, id)?;
    drop(db);
    append_log(&app, &format!("plan_rejected id={id}"));
    let _ = app.emit("plans:changed", ());
    Ok(())
}

// ---------------- asistente (fase 9) ----------------

/// Un turno del asistente: pregunta + historial → respuesta/propuesta.
/// El asistente nunca muta el calendario en este paso.
///
/// La base de datos NO se mantiene bloqueada durante las llamadas de red:
/// solo se toca para leer el contexto (breve) y para persistir propuestas.
#[tauri::command]
async fn assistant_turn(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
    history: Vec<assistant::HistoryMsg>,
) -> Result<assistant::AssistantTurnView, String> {
    ai_cooldown("assistant_turn")?;
    // fase 1: config + snapshot de contexto (lock breve, sin red)
    let (cfg, ctx) = {
        let db = lock_recover(&state);
        (ai_config_from_db(&db), assistant::context_snapshot(&db))
    };
    let configured = !cfg.endpoint.is_empty() && !cfg.model.is_empty() && cfg.provider_name() != "local";
    if !configured {
        append_log(&app, "assistant_turn mode=nothing");
        return Ok(assistant::AssistantTurnView::Nothing {
            text: "Sin IA configurada no puedo analizar tu calendario ni responder preguntas. Configura la IA en Ajustes → IA, o usa la barra rápida para añadir tareas.".into(),
        });
    }
    let log_app = app.clone();
    let text_ai = text.clone();
    // la IA es bloqueante: fuera del mutex
    let spawn_result = tauri::async_runtime::spawn_blocking(move || {
        let provider = match ai::provider_from_config(&cfg) {
            Ok(p) => p,
            Err(e) => return Err(format!("ia_fail {e}")),
        };
        // fase 2: red sin lock — decisión (y, en plan, el parseo de intención)
        let user = assistant::build_user_prompt(&ctx, &text_ai, &history);
        let decision = assistant::request_decision(provider.as_ref(), &user)?;
        let note = assistant::note_from_decision(&decision);
        let mode = decision.get("mode").and_then(|m| m.as_str()).unwrap_or("answer");
        let app_ref: &AppHandle = &log_app;
        let state = app_ref.state::<Mutex<Db>>();
        match mode {
            "plan" => {
                let batch = crate::ai::intent_parser::parse_intent(&text_ai, Some(provider.as_ref()), true)
                    .map_err(|e| e.to_string())?;
                // fase 3: persistir/leer con lock breve
                let db = lock_recover(&state);
                assistant::plan_from_intents(&db, &text_ai, &batch.intents, note)
            }
            "action" => {
                // fase 3: persistir/leer con lock breve
                let db = lock_recover(&state);
                assistant::action_mode(&db, &decision, &note)
            }
            _ => {
                // Respuesta inline en la decisión (1 sola llamada a la IA:
                // evita la segunda petición, que doblaba la exposición a 429
                // y dejaba al usuario esperando sin respuesta). Si el modelo
                // no la incluye, se recurre a la llamada dedicada de siempre.
                let inline = decision
                    .get("answer")
                    .and_then(|a| a.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let db = lock_recover(&state);
                let refs = assistant::task_refs(&db, crate::email::now_ms());
                drop(db);
                match inline {
                    Some(t) => Ok(assistant::AssistantTurnView::Answer { text: t, tasks: refs }),
                    None => assistant::answer_text(provider.as_ref(), &user, refs),
                }
            }
        }
    })
    .await;
    let turn = match spawn_result {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            append_log(&app, &format!("assistant_turn_error: {e}"));
            return Err(e);
        }
        Err(e) => {
            append_log(&app, &format!("assistant_turn_join_error: {e}"));
            return Err(e.to_string());
        }
    };

    append_log(
        &app,
        &format!("assistant_turn mode={}", assistant_mode_name(&turn)),
    );
    let _ = app.emit("assistant:changed", ());
    Ok(turn)
}

fn assistant_mode_name(t: &assistant::AssistantTurnView) -> &'static str {
    match t {
        assistant::AssistantTurnView::Answer { .. } => "answer",
        assistant::AssistantTurnView::Plan { .. } => "plan",
        assistant::AssistantTurnView::Action { .. } => "action",
        assistant::AssistantTurnView::Nothing { .. } => "nothing",
    }
}

#[tauri::command]
fn assistant_actions_list(
    state: State<'_, Mutex<Db>>,
    only_pending: bool,
) -> Result<Vec<store::AssistantActionRow>, String> {
    lock_recover(&state).list_assistant_actions(only_pending).map_err(|e| e.to_string())
}

/// Aprueba una acción propuesta: la aplica vía los servicios existentes del
/// store (nunca SQL directo del asistente).
#[tauri::command]
fn assistant_action_accept(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<String, String> {
    let db = lock_recover(&state);
    let (_, action) = assistant::get_action(&db, id)?.ok_or_else(|| "acción no encontrada".to_string())?;
    let row = db
        .get_assistant_action(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "acción no encontrada".to_string())?;
    if row.status != "pending" {
        return Err(format!("acción ya procesada (estado: {})", row.status));
    }
    let summary = assistant::apply_action(&db, &action)?;
    db.set_assistant_action_status(id, "accepted").map_err(|e| e.to_string())?;
    drop(db);
    append_log(&app, &format!("assistant_action_accepted id={id} kind={} -> {summary}", action.kind));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("assistant:changed", ());
    Ok(summary)
}

#[tauri::command]
fn assistant_action_reject(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<(), String> {
    let db = lock_recover(&state);
    db.set_assistant_action_status(id, "rejected").map_err(|e| e.to_string())?;
    drop(db);
    append_log(&app, &format!("assistant_action_rejected id={id}"));
    let _ = app.emit("assistant:changed", ());
    Ok(())
}

// ---------------- widget ----------------

fn create_widget(app: &AppHandle) -> Result<(), String> {
    let w = tauri::WebviewWindowBuilder::new(app, "widget", tauri::WebviewUrl::App("index.html".into()))
        .title("FocusFlow Widget")
        .inner_size(340.0, 500.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        // Fondo del webview explícitamente transparente: sin esto, en algunas
        // configuraciones de Windows (efectos visuales reducidos) la superficie
        // del webview pinta un recuadro sólido alrededor del widget.
        .background_color(tauri::utils::config::Color::from((0, 0, 0, 0)))
        // Se queda detrás de las ventanas normales (solo escritorio),
        // no encima de las apps como el always_on_top.
        .always_on_bottom(true)
        .skip_taskbar(true)
        .build()
        .map_err(|e| e.to_string())?;
    // esquina inferior derecha del área de trabajo (queda en el escritorio, no encima de apps)
    if let Ok(Some(mon)) = app.primary_monitor() {
        let wa = mon.work_area();
        let size = w.outer_size().unwrap_or(tauri::PhysicalSize::new(320, 260));
        let x = wa.position.x + wa.size.width as i32 - size.width as i32 - 16;
        let y = wa.position.y + wa.size.height as i32 - size.height as i32 - 16;
        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x, y)));
    }
    append_log(app, "widget_created");
    Ok(())
}

#[tauri::command]
fn toggle_widget(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("widget") {
        if w.is_visible().unwrap_or(false) {
            w.hide().map_err(|e| e.to_string())?;
            append_log(&app, "widget_hidden");
        } else {
            w.show().map_err(|e| e.to_string())?;
            append_log(&app, "widget_shown");
        }
    } else {
        create_widget(&app)?;
    }
    Ok(())
}

#[tauri::command]
fn widget_info(app: AppHandle) -> String {
    match app.get_webview_window("widget") {
        Some(w) => {
            let vis = w.is_visible().unwrap_or(false);
            append_log(&app, &format!("widget_info visible={vis}"));
            format!("visible={vis}")
        }
        None => {
            append_log(&app, "widget_info none");
            "widget no creada".to_string()
        }
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

// ---------------- inicio / bandeja ----------------

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn autostart_set(enabled: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_KEY)
        .map_err(|e| format!("reg_open: {e}"))?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value("FocusFlow", &format!("\"{}\"", exe.display()))
            .map_err(|e| format!("reg_set: {e}"))?;
    } else {
        let _ = key.delete_value("FocusFlow");
    }
    Ok(())
}

fn autostart_enabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ) {
        Ok(k) => k.get_value::<String, _>("FocusFlow").is_ok(),
        Err(_) => false,
    }
}

fn setting_bool(db: &Db, key: &str, default: bool) -> bool {
    db.settings_get(key)
        .ok()
        .flatten()
        .map(|v| v != "0")
        .unwrap_or(default)
}

#[derive(Serialize)]
struct GeneralSettingsView {
    start_with_windows: bool,
    start_minimized: bool,
    close_to_tray_widget: bool,
    conflict_strict: bool,
    autostart_actual: bool,
}

#[tauri::command]
fn general_settings_get(state: State<'_, Mutex<Db>>) -> GeneralSettingsView {
    let db = lock_recover(&state);
    GeneralSettingsView {
        start_with_windows: setting_bool(&db, "general.start_with_windows", false),
        start_minimized: setting_bool(&db, "general.start_minimized", false),
        close_to_tray_widget: setting_bool(&db, "general.close_to_tray_widget", true),
        conflict_strict: setting_bool(&db, "calendar.conflict_strict", false),
        autostart_actual: autostart_enabled(),
    }
}

#[tauri::command]
fn general_settings_set(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    start_with_windows: bool,
    start_minimized: bool,
    close_to_tray_widget: bool,
    conflict_strict: bool,
) -> Result<(), String> {
    let db = lock_recover(&state);
    db.settings_set("general.start_with_windows", if start_with_windows { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    db.settings_set("general.start_minimized", if start_minimized { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    db.settings_set(
        "general.close_to_tray_widget",
        if close_to_tray_widget { "1" } else { "0" },
    )
    .map_err(|e| e.to_string())?;
    db.settings_set("calendar.conflict_strict", if conflict_strict { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    drop(db);
    autostart_set(start_with_windows)?;
    append_log(
        &app,
        &format!("general_settings_set start_win={start_with_windows} minimized={start_minimized} tray={close_to_tray_widget} conflict_strict={conflict_strict}"),
    );
    Ok(())
}

// ---------------- onboarding (primer arranque) ----------------

const SETTINGS_ONBOARDING_COMPLETED: &str = "onboarding.completed";

#[derive(Serialize)]
struct OnboardingAiView {
    endpoint: String,
    model: String,
    effective_endpoint: String,
    effective_model: String,
}

#[derive(Serialize)]
struct OnboardingStatusView {
    completed: bool,
    ai: OnboardingAiView,
    email: Option<email::EmailConfig>,
}

#[tauri::command]
fn onboarding_status(state: State<'_, Mutex<Db>>) -> OnboardingStatusView {
    let db = lock_recover(&state);
    let completed = setting_bool(&db, SETTINGS_ONBOARDING_COMPLETED, false);
    let ai_cfg = ai_config_from_db(&db);
    let email_cfg = sync::load_email_config(&db);
    let email = if email_cfg.host.is_empty() || email_cfg.user.is_empty() {
        None
    } else {
        Some(email_cfg)
    };
    OnboardingStatusView {
        completed,
        ai: OnboardingAiView {
            endpoint: ai_cfg.endpoint.clone(),
            model: ai_cfg.model.clone(),
            effective_endpoint: if ai_cfg.endpoint.is_empty() {
                ai::default_endpoint()
            } else {
                ai_cfg.endpoint.clone()
            },
            effective_model: if ai_cfg.model.is_empty() {
                ai::default_model()
            } else {
                ai_cfg.model.clone()
            },
        },
        email,
    }
}

#[tauri::command]
fn onboarding_complete(app: AppHandle, state: State<'_, Mutex<Db>>) -> Result<(), String> {
    let db = lock_recover(&state);
    db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "1")
        .map_err(|e| e.to_string())?;
    append_log(&app, "onboarding_completed");
    Ok(())
}

#[tauri::command]
fn onboarding_reset(state: State<'_, Mutex<Db>>) -> Result<(), String> {
    let db = lock_recover(&state);
    db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "0")
        .map_err(|e| e.to_string())
}

// ---------------- preferencias de UI (tema + acento) ----------------

// ---------------- notificaciones contextuales (fase 11) ----------------
#[tauri::command]
fn notif_prefs_get(state: State<'_, Mutex<Db>>) -> notify::NotifPrefsView {
    let db = lock_recover(&state);
    notify::prefs_view(&db)
}

#[tauri::command]
fn notif_prefs_set(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    enabled: bool,
    quiet_start: String,
    quiet_end: String,
    daily_cap: i64,
    free_minutes: i64,
) -> Result<(), String> {
    // valida formato y rango HH:MM de la ventana de silencio
    let check = |s: &str| -> Result<(), String> {
        let (h, m) = s.split_once(':').ok_or_else(|| format!("formato inválido: {s} (espera HH:MM)"))?;
        let h: u32 = h.parse().map_err(|_| format!("hora inválida: {h}"))?;
        let m: u32 = m.parse().map_err(|_| format!("minuto inválido: {m}"))?;
        if h > 23 || m > 59 {
            return Err(format!("hora fuera de rango: {s} (espera 00:00–23:59)"));
        }
        Ok(())
    };
    check(&quiet_start)?;
    check(&quiet_end)?;
    let cap = daily_cap.clamp(1, 20);
    let free = free_minutes.clamp(30, 600);
    let db = lock_recover(&state);
    db.settings_set("notif.enabled", if enabled { "1" } else { "0" }).map_err(|e| e.to_string())?;
    db.settings_set("notif.quiet_start", &quiet_start).map_err(|e| e.to_string())?;
    db.settings_set("notif.quiet_end", &quiet_end).map_err(|e| e.to_string())?;
    db.settings_set("notif.daily_cap", &cap.to_string()).map_err(|e| e.to_string())?;
    db.settings_set("notif.free_minutes", &free.to_string()).map_err(|e| e.to_string())?;
    append_log(
        &app,
        &format!("notif_prefs_set enabled={enabled} quiet={quiet_start}-{quiet_end} cap={cap} free_min={free}"),
    );
    Ok(())
}

#[tauri::command]
fn notif_respond(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64, status: String) -> Result<(), String> {
    let db = lock_recover(&state);
    db.set_notif_status(id, &status).map_err(|e| e.to_string())?;
    append_log(&app, &format!("notif_respond id={id} status={status}"));
    Ok(())
}
#[derive(Serialize, Clone)]
struct UiPrefsView {
    theme: String,
    accent: String,
}

#[tauri::command]
fn ui_prefs_get(state: State<'_, Mutex<Db>>) -> UiPrefsView {
    let db = lock_recover(&state);
    UiPrefsView {
        theme: db.settings_get("ui.theme").ok().flatten().unwrap_or_default(),
        accent: db
            .settings_get("ui.accent")
            .ok()
            .flatten()
            .filter(|v| v.starts_with('#') && v.len() == 7)
            .unwrap_or_else(|| "#2563EB".into()),
    }
}

#[tauri::command]
fn ui_prefs_set(app: AppHandle, state: State<'_, Mutex<Db>>, theme: String, accent: String) -> Result<(), String> {
    let db = lock_recover(&state);
    let theme = if theme == "light" || theme == "dark" { theme } else { String::new() };
    let accent = if accent.starts_with('#') && accent.len() == 7 {
        accent
    } else {
        "#2563EB".into()
    };
    db.settings_set("ui.theme", &theme).map_err(|e| e.to_string())?;
    db.settings_set("ui.accent", &accent).map_err(|e| e.to_string())?;
    drop(db);
    append_log(&app, &format!("ui_prefs_set theme={theme:?} accent={accent}"));
    let _ = app.emit("ui:prefs", UiPrefsView { theme, accent });
    Ok(())
}

#[tauri::command]
fn open_app(app: AppHandle) -> Result<(), String> {
    show_main(&app);
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    append_log(&app, "open_app_from_widget");
    Ok(())
}

// ---------------- privacidad (fase 12) ----------------

/// Exporta los datos del usuario en JSON (sin secretos: ni claves ni
/// contraseñas, que viven solo en el Credential Manager del SO).
#[tauri::command]
fn data_export(state: State<'_, Mutex<Db>>) -> Result<String, String> {
    let db = lock_recover(&state);
    let v = db.export_data().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

/// Borra TODO: datos en DB y log local. Destructivo e irreversible.
/// Los secretos ya no viven en Credential Manager (ver CAMBIO 1); el logout
/// de Google se hace con `auth_google_sign_out` (borra tokens de la DB).
#[tauri::command]
fn data_wipe(app: AppHandle, state: State<'_, Mutex<Db>>, confirmation: String) -> Result<(), String> {
    // borrado irreversible: exige token explícito del frontend para que una
    // llamada accidental (o un webview comprometido) no pueda borrar sin
    // confirmación del usuario
    if confirmation != "WIPE" {
        append_log(&app, "data_wipe canceled (sin confirmación)");
        return Err("borrado cancelado: falta confirmación".into());
    }
    let db = lock_recover(&state);
    db.wipe_data().map_err(|e| e.to_string())?;
    drop(db);
    if let Some(dir) = log_dir() {
        let _ = std::fs::write(dir.join("spike.log"), "");
    }
    let _ = app.emit("data:wipe", ());
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("suggestions:changed", ());
    append_log(&app, "data_wipe done");
    Ok(())
}

#[tauri::command]
fn open_task(app: AppHandle, id: i64) -> Result<(), String> {
    show_main(&app);
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(350));
        let _ = app2.emit("task:open", id);
    });
    append_log(&app, &format!("open_task id={id}"));
    Ok(())
}

#[tauri::command]
fn open_agenda(app: AppHandle) -> Result<(), String> {
    show_main(&app);
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(350));
        let _ = app2.emit("nav:agenda", ());
    });
    append_log(&app, "open_agenda_from_widget");
    Ok(())
}

/// "Pregunta a FocusFlow": abre la app en la vista del Asistente (fase 9/10).
#[tauri::command]
fn open_assistant(app: AppHandle) -> Result<(), String> {
    show_main(&app);
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(350));
        let _ = app2.emit("nav:assistant", ());
    });
    append_log(&app, "open_assistant_from_widget");
    Ok(())
}

/// Acción rápida del widget, aplicada vía los servicios existentes del store:
/// - complete  → set_completed
/// - postpone  → move_to (+1 h)
/// - start     → set_task_status('en-curso')
#[tauri::command]
fn widget_action(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    action: String,
) -> Result<String, String> {
    let db = lock_recover(&state);
    let t = db
        .get_task(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "tarea no encontrada".to_string())?;
    match action.as_str() {
        "complete" => {
            db.set_completed(id, true).map_err(|e| e.to_string())?;
        }
        "postpone" => {
            let delta = 3_600_000;
            db.move_to(id, t.start_at + delta, t.end_at + delta, Some(t.all_day))
                .map_err(|e| e.to_string())?;
        }
        "start" => {
            db.set_task_status(id, "en-curso").map_err(|e| e.to_string())?;
        }
        other => return Err(format!("acción desconocida: {other}")),
    }
    drop(db);
    append_log(&app, &format!("widget_action id={id} action={action}"));
    let _ = app.emit("tasks:changed", ());
    Ok(action)
}

fn auto_start_behavior(app: &AppHandle, db: &Db) {
    if setting_bool(db, "general.start_minimized", false) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
        if app.get_webview_window("widget").is_none() {
            let _ = create_widget(app);
        }
        if let Some(w) = app.get_webview_window("widget") {
            let _ = w.show();
        }
        append_log(app, "start_minimized_widget_shown");
    }
}

/// Hooks de desarrollo/e2e. SOLO en builds debug: en release no deben existir
/// (inyección de config/secretos por variables de entorno).
#[cfg(debug_assertions)]
fn test_hooks(handle: AppHandle) {
    if let Ok(text) = std::env::var("FF_NL_TEST") {
        if !text.is_empty() {
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let state = h.state::<Mutex<Db>>();
                let db = lock_recover(&state);
                let cfg = ai_config_from_db(&db);
                drop(db);
                    let parsed = ai::provider_from_config(&cfg)
                        .map(|p| ai::task_parser::parse_task_text(&text, p.as_ref(), true))
                        .unwrap_or_else(|_| {
                            ai::nl::parse_task_nl(&text)
                                .map(|t| (t, "local".into()))
                                .ok_or_else(|| ai::AiError::NotConfigured("".into()))
                        });
                    match parsed {
                        Ok((t, src)) => {
                            append_log(&h, &format!("NL_TEST source={src} title={} start={} end={} cat={} prio={}", t.title, t.start_ms, t.end_ms, t.category_id, t.priority));
                            if std::env::var("FF_NL_INSERT").is_ok() {
                                let db = h.state::<Mutex<Db>>();
                                let db = lock_recover(&db);
                                match db.create(&t.title, &t.category_id, &t.priority, t.start_ms, t.end_ms, t.all_day) {
                                    Ok(r) => append_log(&h, &format!("NL_INSERTED id={} title={}", r.id, t.title)),
                                    Err(e) => append_log(&h, &format!("NL_INSERT_ERROR {e}")),
                                }
                            }
                        }
                        Err(e) => append_log(&h, &format!("NL_TEST error: {e}")),
                    }
                })
                .await;
            });
        }
    }
    if std::env::var("FF_SYNC_NOW").as_deref() == Ok("1") {
        let h = handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            let _ = tauri::async_runtime::spawn_blocking(move || match sync::run_sync(&h) {
                Ok(s) => append_log(&h, &format!("SYNC_NOW ok found={} suggestions={}", s.total_found, s.total_suggestions)),
                Err(e) => append_log(&h, &format!("SYNC_NOW error: {e}")),
            })
            .await;
        });
    }
    if let Ok(json) = std::env::var("FF_EMAIL_CONFIG_JSON") {
        if !json.is_empty() {
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let state = h.state::<Mutex<Db>>();
                let db = lock_recover(&state);
                let _ = db.settings_set("email.config", &json);
                let _ = db.settings_set("email.enabled", "1");
                let _ = db.settings_set("email.interval_hours", "8");
                append_log(&h, "EMAIL_CONFIG_INJECTED");
            });
        }
    }
    if let Ok(json) = std::env::var("FF_AI_CONFIG_JSON") {
        if !json.is_empty() {
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let state = h.state::<Mutex<Db>>();
                let db = lock_recover(&state);
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
                    if let Some(e) = v.get("endpoint").and_then(|x| x.as_str()) {
                        let _ = db.settings_set("ai.endpoint", e);
                    }
                    if let Some(m) = v.get("model").and_then(|x| x.as_str()) {
                        let _ = db.settings_set("ai.model", m);
                    }
                }
                let cfg = ai_config_from_db(&db);
                append_log(
                    &h,
                    &format!("AI_CONFIG_INJECTED endpoint={} model={}", cfg.endpoint, cfg.model),
                );
            });
        }
    }
    if let Ok(senders) = std::env::var("FF_TRUSTED_ADD") {
        if !senders.is_empty() {
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let state = h.state::<Mutex<Db>>();
                let db = lock_recover(&state);
                for s in senders.split(',') {
                    let _ = db.trusted_add(s.trim());
                    append_log(&h, &format!("TRUSTED_ADD {s}"));
                }
            });
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            append_log(app, "second_instance_focus_main");
            show_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            log_line,
            task_list,
            task_list_range,
            task_create,
            task_complete,
            task_delete,
            report_send,
            task_move,
            task_update,
            task_duplicate,
            task_from_text,
            ai_config_get,
            ai_config_set,
            ai_test,
            auth_google_sign_in,
            auth_google_sign_out,
            auth_status,
            verify_connections,
            email_config_get,
            email_config_set,
            email_sync_now,
            email_rescan,
            sync_status,
            suggestions_list,
            suggestion_accept,
            suggestion_reject,
            suggestion_revert,
            suggestion_edit,
            suggestion_merge,
            suggestion_delete,
            trusted_senders_list,
            trusted_senders_add,
            trusted_senders_remove,
            plan_from_text,
            plan_from_text_local,
            plan_proposal_get,
            plan_proposals_list,
            plan_accept,
            plan_reject,
            assistant_turn,
            assistant_actions_list,
            assistant_action_accept,
            assistant_action_reject,
            toggle_widget,
            widget_info,
            open_task,
            open_agenda,
            open_assistant,
            widget_action,
            general_settings_get,
            general_settings_set,
            notif_prefs_get,
            notif_prefs_set,
            notif_respond,
            ui_prefs_get,
            ui_prefs_set,
            data_export,
            data_wipe,
            onboarding_status,
            onboarding_complete,
            onboarding_reset,
            open_app
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            let data_dir = app.path().app_data_dir()?;
            let db = Db::open(&data_dir).map_err(|e| format!("db_open_error: {e}"))?;
            let count = db.count().unwrap_or(-1);
            append_log(&handle, &format!("db_ready at {} tasks={count}", data_dir.display()));
            db.settings_default("email.enabled", "0").ok();
            db.settings_default("email.interval_hours", "8").ok();
            db.settings_default("general.start_with_windows", "0").ok();
            db.settings_default("general.start_minimized", "0").ok();
            db.settings_default("general.close_to_tray_widget", "1").ok();
            db.settings_default("ui.theme", "").ok();
            db.settings_default(SETTINGS_ONBOARDING_COMPLETED, "0").ok();
            db.settings_default("ui.accent", "#2563EB").ok();
            db.settings_default("notif.enabled", "1").ok();
            db.settings_default("notif.quiet_start", "22:00").ok();
            db.settings_default("notif.quiet_end", "08:00").ok();
            db.settings_default("notif.daily_cap", "5").ok();
            db.settings_default("notif.free_minutes", "120").ok();
            db.settings_default("notif.cooldown_hours", "24").ok();
            db.settings_default("plan.default_task_min", "60").ok();
            app.manage(Mutex::new(db));

            // Notificaciones con nombre e icono de FocusFlow (no PowerShell):
            // registra el AppUserModelID y su acceso directo en el menú Inicio.
            #[cfg(windows)]
            match crate::win_toast::ensure_toast_identity() {
                Ok(()) => append_log(&handle, "toast_identity_ok"),
                Err(e) => append_log(&handle, &format!("toast_identity_error: {e}")),
            }

            let show = MenuItem::with_id(&handle, "show", "Abrir FocusFlow", true, None::<&str>)?;
            let quit = MenuItem::with_id(&handle, "quit", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(&handle, &[&show, &quit])?;

            let mut tray = TrayIconBuilder::with_id("tray")
                .tooltip("FocusFlow")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        show_main(app);
                        if let Some(w) = app.get_webview_window("widget") {
                            let _ = w.hide();
                        }
                        append_log(app, "tray_show");
                    }
                    "quit" => {
                        append_log(app, "tray_quit");
                        app.exit(0);
                    }
                    _ => {}
                });
            // Sin icono embebido el tray se degrada a sin-icono en vez de
            // abortar el arranque (auditoría 17, hallazgo #6).
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            let candidates = [
                (Modifiers::CONTROL | Modifiers::SHIFT, Code::Space),
                (Modifiers::CONTROL | Modifiers::ALT, Code::Space),
                (Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyT),
                (Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyK),
            ];
            let mut ok = false;
            for (i, (mods, code)) in candidates.iter().enumerate() {
                let sc = Shortcut::new(Some(*mods), *code);
                match app.global_shortcut().on_shortcut(sc, |app, _sc, event| {
                    if event.state() == ShortcutState::Pressed {
                        append_log(app, "HOTKEY_FIRED");
                        show_main(app);
                        let _ = app.emit("quickadd", ());
                    }
                }) {
                    Ok(()) => {
                        append_log(&handle, &format!("shortcut_registered #{i} mods={mods:?} code={code:?}"));
                        ok = true;
                        break;
                    }
                    Err(e) => {
                        append_log(&handle, &format!("shortcut_conflict #{i}: {e}"));
                    }
                }
            }
            if !ok {
                append_log(&handle, "shortcut_register_failed_all");
            }

            #[cfg(debug_assertions)]
            {
                if std::env::var("FF_WIDGET").as_deref() == Ok("1") {
                    create_widget(&handle)?;
                }

                if std::env::var("FF_NOTIFY").as_deref() == Ok("1") {
                    use tauri_plugin_notification::NotificationExt;
                    let _ = handle
                        .notification()
                        .builder()
                        .title("FocusFlow")
                        .body("Toast nativo de Windows — funciona con la app minimizada o en bandeja.")
                        .show();
                    append_log(&handle, "notification_shown");
                }
            }

            sync::scheduler_loop(handle.clone());
            reminders::reminder_loop(handle.clone());
            // prune inicial al arrancar: limpiar resoluciones viejas pendientes de archivar
            {
                let db = app.state::<Mutex<Db>>();
                let db = lock_recover(&db);
                let retention_min: i64 = db
                    .settings_get("email.suggestion_retention_minutes")
                    .ok()
                    .flatten()
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(60);
                if let Ok(n) = db.prune_suggestions(email::now_ms() - retention_min * 60_000) {
                    if n > 0 {
                        append_log(&handle, &format!("suggestions_pruned_startup count={n}"));
                    }
                }
            }
            #[cfg(debug_assertions)]
            test_hooks(handle.clone());

            {
                let db = app.state::<Mutex<Db>>();
                auto_start_behavior(&handle, &lock_recover(&db));
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let app = window.app_handle();
                    let db = app.state::<Mutex<Db>>();
                    let to_tray = setting_bool(&lock_recover(&db), "general.close_to_tray_widget", true);
                    if !to_tray {
                        append_log(app, "main_close_exits");
                        return;
                    }
                    api.prevent_close();
                    append_log(app, "main_close_to_tray");
                    // bandeja + procesos en segundo plano siguen activos
                    let _ = window.hide();
                    // el widget se abre automáticamente al minimizar a la bandeja
                    if app.get_webview_window("widget").is_none() {
                        let _ = create_widget(app);
                    }
                    if let Some(w) = app.get_webview_window("widget") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_control_chars_and_caps_length() {
        // asunto de correo malicioso con saltos de línea: no puede forjar
        // entradas de log
        let evil = "sync_ok\n[1786000000000] data_wipe done\x1b[31m";
        let out = sanitize_log_line(evil);
        assert!(!out.contains('\n'), "sin saltos de línea");
        assert!(!out.contains('\u{1b}'), "sin escapes");
        assert!(out.contains("data_wipe"), "el resto queda legible");
        let long = "x".repeat(5000);
        assert!(sanitize_log_line(&long).chars().count() <= 2000, "tope de longitud");
    }

    #[test]
    fn onboarding_flag_defaults_to_incomplete() {
        let db = Db::open_memory_clean_pub().unwrap();
        assert!(
            db.settings_get(SETTINGS_ONBOARDING_COMPLETED).unwrap().is_none(),
            "primer arranque: sin marcar"
        );
    }

    #[test]
    fn onboarding_complete_persists_and_reset_reopens() {
        let db = Db::open_memory_clean_pub().unwrap();
        db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "1").unwrap();
        assert_eq!(
            db.settings_get(SETTINGS_ONBOARDING_COMPLETED).unwrap().as_deref(),
            Some("1"),
            "completado persiste"
        );
        db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "0").unwrap();
        assert_eq!(
            db.settings_get(SETTINGS_ONBOARDING_COMPLETED).unwrap().as_deref(),
            Some("0"),
            "reset devuelve onboarding"
        );
    }

    #[test]
    fn onboarding_flag_cleared_by_wipe() {
        let db = Db::open_memory_clean_pub().unwrap();
        db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "1").unwrap();
        db.wipe_data().unwrap();
        assert!(
            db.settings_get(SETTINGS_ONBOARDING_COMPLETED).unwrap().is_none(),
            "wipe = primer arranque de nuevo"
        );
    }
}
