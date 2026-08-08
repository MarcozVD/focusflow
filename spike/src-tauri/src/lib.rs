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
pub mod email;
pub mod engine;
pub mod reminders;
mod store;
pub mod sync;

use ai::{validation::ParsedTask, AiConfig};
use store::{Db, TaskRow};

pub(crate) fn log_dir() -> PathBuf {
    let d = std::env::temp_dir().join("focusflow-spike");
    fs::create_dir_all(&d).unwrap();
    d
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub(crate) fn append_log(_app: &AppHandle, line: &str) {
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir().join("spike.log"))
    {
        let _ = writeln!(f, "[{}] {}", now_ms(), line);
    }
}

#[tauri::command]
fn log_line(app: AppHandle, line: String) {
    append_log(&app, &line);
}

// ---------------- tasks ----------------

#[tauri::command]
fn task_list(state: State<'_, Mutex<Db>>) -> Result<Vec<TaskRow>, String> {
    state.lock().unwrap().list().map_err(|e| e.to_string())
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
    state.lock().unwrap().set_completed(id, done).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks:changed", ());
    Ok(())
}

#[tauri::command]
fn task_delete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    state.lock().unwrap().delete(id).map_err(|e| e.to_string())?;
    let _ = app.emit("tasks:changed", ());
    Ok(())
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
    let db = state.lock().unwrap();
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
    let db = state.lock().unwrap();
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
    // La configuración se lee bajo un lock corto. La llamada a la IA es
    // bloqueante (hasta 90 s) y se ejecuta en otro hilo, fuera del mutex,
    // para que la app siga respondiendo mientras se interpreta el texto.
    let cfg = {
        let db = state.lock().unwrap();
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
        let db = state.lock().unwrap();
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
    has_key: bool,
    effective_endpoint: String,
    effective_model: String,
}

#[tauri::command]
fn ai_config_get(state: State<'_, Mutex<Db>>) -> AiConfigView {
    let db = state.lock().unwrap();
    let cfg = ai_config_from_db(&db);
    AiConfigView {
        endpoint: cfg.endpoint.clone(),
        model: cfg.model.clone(),
        has_key: ai::get_ai_key().is_some(),
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
    }
}

#[tauri::command]
fn ai_config_set(state: State<'_, Mutex<Db>>, endpoint: String, model: String) -> Result<(), String> {
    let db = state.lock().unwrap();
    db.settings_set("ai.endpoint", &endpoint).map_err(|e| e.to_string())?;
    db.settings_set("ai.model", &model).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn ai_set_key(key: String) -> Result<(), String> {
    ai::keyring_set(ai::AI_KEY_USER, &key)
}

#[tauri::command]
fn ai_clear_key() -> Result<(), String> {
    ai::keyring_delete(ai::AI_KEY_USER)
}

#[derive(Serialize)]
struct AiTestResult {
    ok: bool,
    latency_ms: u64,
    model: String,
    error: String,
}

#[tauri::command]
fn ai_test(state: State<'_, Mutex<Db>>, app: AppHandle) -> AiTestResult {
    let db = state.lock().unwrap();
    let cfg = ai_config_from_db(&db);
    let t0 = std::time::Instant::now();
    match ai::provider_from_config(&cfg) {
        Ok(provider) => {
            let model = cfg.model.clone();
            match provider.chat_json("Devuelve exactamente: {\"ok\": true}", "ping", r#"{"ok": true}"#) {
                Ok(_) => {
                    append_log(&app, "ai_test_ok");
                    AiTestResult {
                        ok: true,
                        latency_ms: t0.elapsed().as_millis() as u64,
                        model,
                        error: String::new(),
                    }
                }
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
}

// ---------------- email + sync ----------------

#[derive(Serialize)]
struct EmailConfigView {
    config: email::EmailConfig,
    enabled: bool,
    interval_hours: u64,
    max_age_days: u32,
    has_password: bool,
    trusted: Vec<String>,
}

#[tauri::command]
fn email_config_get(state: State<'_, Mutex<Db>>) -> EmailConfigView {
    let db = state.lock().unwrap();
    let config = sync::load_email_config(&db);
    EmailConfigView {
        has_password: ai::get_email_credentials(&config.user).is_some(),
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
    password: Option<String>,
    enabled: bool,
    interval_hours: u64,
    max_age_days: u32,
) -> Result<(), String> {
    let db = state.lock().unwrap();
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    db.settings_set("email.config", &json).map_err(|e| e.to_string())?;
    db.settings_set("email.enabled", if enabled { "1" } else { "0" }).map_err(|e| e.to_string())?;
    db.settings_set("email.interval_hours", &interval_hours.to_string()).map_err(|e| e.to_string())?;
    db.settings_set("email.max_age_days", &max_age_days.to_string()).map_err(|e| e.to_string())?;
    if let Some(p) = password {
        if !p.is_empty() {
            ai::set_email_credentials(&config.user, &p)?;
        }
    }
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
#[tauri::command]
fn verify_connections(state: State<'_, Mutex<Db>>, app: AppHandle) -> VerifyResult {
    let db = state.lock().unwrap();
    let ai_cfg = ai_config_from_db(&db);
    let email_cfg = sync::load_email_config(&db);
    drop(db);

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

    let email = match email::test_connection(&email_cfg) {
        Ok((mailbox, n)) => ConnectionCheck {
            ok: true,
            detail: format!("Conectado a {mailbox} ({n} correos)"),
        },
        Err(e) => ConnectionCheck { ok: false, detail: e },
    };

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
    let db = state.lock().unwrap();
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
    let db = state.lock().unwrap();
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
    let db = state.lock().unwrap();
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
    let db = state.lock().unwrap();
    sync::revert_suggestion(&db, id)?;
    drop(db);
    append_log(&app, &format!("suggestion_reverted id={id}"));
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
    let db = state.lock().unwrap();
    db.update_suggestion_data(id, &title, &category_id, &priority, start_at, end_at, &description)
        .map_err(|e| e.to_string())?;
    // si la sugerencia ya fue aceptada, la tarea creada se mantiene en sincronía
    if let Some(task_id) = db.get_suggestion(id).ok().flatten().and_then(|s| s.result_task_id) {
        let _ = db.update_task_full(task_id, &title, &category_id, &priority, start_at, end_at, "", "[]", "", "", None, Some(all_day));
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
    let db = state.lock().unwrap();
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
    db.update_task_full(task_id, &title, &s.category_id, &priority, start, end, "", "[]", "", "", None, None)
        .map_err(|e| e.to_string())?;
    db.set_suggestion_status(id, "merged").map_err(|e| e.to_string())?;
    drop(db);
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
fn trusted_senders_list(state: State<'_, Mutex<Db>>) -> Result<Vec<String>, String> {
    state.lock().unwrap().trusted_list().map_err(|e| e.to_string())
}

#[tauri::command]
fn trusted_senders_add(state: State<'_, Mutex<Db>>, sender: String) -> Result<(), String> {
    state.lock().unwrap().trusted_add(&sender).map_err(|e| e.to_string())
}

#[tauri::command]
fn trusted_senders_remove(state: State<'_, Mutex<Db>>, sender: String) -> Result<(), String> {
    state.lock().unwrap().trusted_remove(&sender).map_err(|e| e.to_string())
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
    let db = state.lock().unwrap();
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
    let db = state.lock().unwrap();
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

// ---------------- preferencias de UI (tema + acento) ----------------

#[derive(Serialize, Clone)]
struct UiPrefsView {
    theme: String,
    accent: String,
}

#[tauri::command]
fn ui_prefs_get(state: State<'_, Mutex<Db>>) -> UiPrefsView {
    let db = state.lock().unwrap();
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
    let db = state.lock().unwrap();
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

fn test_hooks(handle: AppHandle) {
    if let Ok(text) = std::env::var("FF_NL_TEST") {
        if !text.is_empty() {
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let state = h.state::<Mutex<Db>>();
                let db = state.lock().unwrap();
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
                                let db = db.lock().unwrap();
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
                let db = state.lock().unwrap();
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
                let db = state.lock().unwrap();
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
                let db = state.lock().unwrap();
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
            task_move,
            task_update,
            task_duplicate,
            task_from_text,
            ai_config_get,
            ai_config_set,
            ai_set_key,
            ai_clear_key,
            ai_test,
            verify_connections,
            email_config_get,
            email_config_set,
            email_sync_now,
            sync_status,
            suggestions_list,
            suggestion_accept,
            suggestion_reject,
            suggestion_revert,
            suggestion_edit,
            suggestion_merge,
            trusted_senders_list,
            trusted_senders_add,
            trusted_senders_remove,
            toggle_widget,
            widget_info,
            open_task,
            open_agenda,
            general_settings_get,
            general_settings_set,
            ui_prefs_get,
            ui_prefs_set,
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
            db.settings_default("ui.accent", "#2563EB").ok();
            app.manage(Mutex::new(db));

            let show = MenuItem::with_id(&handle, "show", "Abrir FocusFlow", true, None::<&str>)?;
            let quit = MenuItem::with_id(&handle, "quit", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(&handle, &[&show, &quit])?;

            TrayIconBuilder::with_id("tray")
                .icon(app.default_window_icon().unwrap().clone())
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
                })
                .build(app)?;

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

            sync::scheduler_loop(handle.clone());
            reminders::reminder_loop(handle.clone());
            // prune inicial al arrancar: limpiar resoluciones viejas pendientes de archivar
            {
                let db = app.state::<Mutex<Db>>();
                let db = db.lock().unwrap();
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
            test_hooks(handle.clone());

            {
                let db = app.state::<Mutex<Db>>();
                auto_start_behavior(&handle, &db.lock().unwrap());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let app = window.app_handle();
                    let db = app.state::<Mutex<Db>>();
                    let to_tray = setting_bool(&db.lock().unwrap(), "general.close_to_tray_widget", true);
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
