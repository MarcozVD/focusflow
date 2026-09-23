//! FocusFlow (spike → producto): punto de entrada Tauri.
//!
//! Arquitectura en capas (C shortcuts v1, 2026-09):
//! - `commands/` — una capa fina por dominio: cada módulo agrupa los
//!   `#[tauri::command]` de UN dominio de la app. Cambiar un dominio no toca
//!   a los demás; añadir un comando = escribirlo en su módulo y sumarlo a la
//!   macro `for_each_command!` (única lista: registra el handler Y alimenta
//!   el test de inventario). Sin lógica de negocio: delegan en `store`,
//!   `sync`, `planning`, `assistant`… (las capas que implementan las reglas).
//! - este archivo — arranque de la app: setup, tray, atajos, scheduler,
//!   logging compartido y utilidades transversales.
//!
//! Reglas de la casa (se aplican en todos los módulos de comandos):
//! - lock de DB breve: leer/escribir y soltar; la red va a spawn_blocking.
//! - eventos → `app.emit(...)`: tasks:changed, suggestions:changed, …
//! - logging → [`append_log`]; saneado de entrada → [`sanitize_log_line`].

// ---------------- módulos de dominio ----------------

pub mod ai;
pub mod assistant;
pub mod auth;
pub mod commands;
pub mod email;
pub mod engine;
pub mod notify;
pub mod planning;
pub mod reminders;
pub mod store;
pub mod sync;
#[cfg(windows)]
pub mod win_toast;

// ---------------- infraestructura compartida ----------------

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use store::{lock_recover, Db};

/// Base del sitio web público de FocusFlow (landing desplegada en Cloudflare).
/// Única fuente para los enlaces legales abiertos desde Ajustes.
pub(crate) const WEB_BASE: &str = "https://flowfocus.site";

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

/// Tamaño máximo del log antes de rotarlo (renombrar a `spike.log.1`).
const LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;

/// Rotación simple: si el log supera `max` bytes se renombra a `.1`
/// (sobrescribiendo el anterior). Sin esto crecía sin límite en %TEMP%.
fn rotate_log_if_big(path: &std::path::Path, max: u64) {
    if fs::metadata(path).map(|m| m.len() > max).unwrap_or(false) {
        let _ = fs::rename(path, path.with_extension("log.1"));
    }
}

pub(crate) fn append_log(_app: &AppHandle, line: &str) {
    let Some(dir) = log_dir() else { return };
    let path = dir.join("spike.log");
    rotate_log_if_big(&path, LOG_MAX_BYTES);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
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

// ---------------- utilidades transversales (comandos ↔ arranque) ----------------

/// Cooldown por comando para las llamadas a la IA (rate limit IPC): evita
/// ráfagas de peticiones costosas de hasta 90 s cada una desde el frontend
/// (debounce no basta; un IPC repetido puede lanzarlas igualmente).
pub(crate) fn ai_cooldown(cmd: &str) -> Result<(), String> {
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

pub(crate) fn ai_config_from_db(db: &Db) -> ai::AiConfig {
    ai::AiConfig {
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

/// Lee un setting booleano ("0" = false, otro valor o ausencia = default).
pub(crate) fn setting_bool(db: &Db, key: &str, default: bool) -> bool {
    db.settings_get(key)
        .ok()
        .flatten()
        .map(|v| v != "0")
        .unwrap_or(default)
}

/// Trae la ventana principal al frente (tray, hotkey, widget, OAuth).
pub(crate) fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

// ---------------- inventario de comandos (fuente única) ----------------

/// Lista ÚNICA de comandos IPC: `nombre: ruta`. La consume
/// `tauri::generate_handler` (registro) y el test `inventory_matches_handler`
/// (verifica que la lista documentada crece junto con el handler).
///
/// AÑADIR UN COMANDO: (1) escríbelo en el módulo de su dominio bajo
/// `commands/`, (2) añade una línea aquí. Nada más.
macro_rules! for_each_command {
    ($m:ident) => {
        $m! {
            log_line: log_line,

            task_list: commands::tasks::task_list,
            task_list_range: commands::tasks::task_list_range,
            task_create: commands::tasks::task_create,
            task_complete: commands::tasks::task_complete,
            task_delete: commands::tasks::task_delete,
            task_move: commands::tasks::task_move,
            task_update: commands::tasks::task_update,
            task_duplicate: commands::tasks::task_duplicate,
            task_from_text: commands::tasks::task_from_text,

            class_list: commands::classes::class_list,
            class_instances: commands::classes::class_instances,
            class_create: commands::classes::class_create,
            class_update: commands::classes::class_update,
            class_delete: commands::classes::class_delete,
            class_conflicts: commands::classes::class_conflicts,

            study_list_range: commands::study::study_list_range,
            study_create: commands::study::study_create,
            study_update: commands::study::study_update,
            study_move: commands::study::study_move,
            study_delete: commands::study::study_delete,
            study_conflicts: commands::study::study_conflicts,

            ai_config_get: commands::email::ai_config_get,
            ai_config_set: commands::email::ai_config_set,
            ai_test: commands::email::ai_test,
            email_config_get: commands::email::email_config_get,
            email_config_set: commands::email::email_config_set,
            email_sync_now: commands::email::email_sync_now,
            email_rescan: commands::email::email_rescan,
            verify_connections: commands::email::verify_connections,

            auth_google_sign_in: commands::auth::auth_google_sign_in,
            auth_google_sign_out: commands::auth::auth_google_sign_out,
            auth_status: commands::auth::auth_status,
            sync_status: commands::auth::sync_status,

            suggestions_list: commands::suggestions::suggestions_list,
            suggestion_accept: commands::suggestions::suggestion_accept,
            suggestion_reject: commands::suggestions::suggestion_reject,
            suggestion_revert: commands::suggestions::suggestion_revert,
            suggestion_edit: commands::suggestions::suggestion_edit,
            suggestion_merge: commands::suggestions::suggestion_merge,
            suggestion_delete: commands::suggestions::suggestion_delete,
            trusted_senders_list: commands::suggestions::trusted_senders_list,
            trusted_senders_add: commands::suggestions::trusted_senders_add,
            trusted_senders_remove: commands::suggestions::trusted_senders_remove,

            plan_from_text: commands::plans::plan_from_text,
            plan_from_text_local: commands::plans::plan_from_text_local,
            plan_proposal_get: commands::plans::plan_proposal_get,
            plan_proposals_list: commands::plans::plan_proposals_list,
            plan_accept: commands::plans::plan_accept,
            plan_reject: commands::plans::plan_reject,

            assistant_turn: commands::assistant::assistant_turn,
            assistant_actions_list: commands::assistant::assistant_actions_list,
            assistant_action_accept: commands::assistant::assistant_action_accept,
            assistant_action_reject: commands::assistant::assistant_action_reject,

            toggle_widget: commands::widget::toggle_widget,
            widget_info: commands::widget::widget_info,
            open_app: commands::widget::open_app,
            widget_action: commands::widget::widget_action,

            general_settings_get: commands::ui::general_settings_get,
            general_settings_set: commands::ui::general_settings_set,
            notif_prefs_get: commands::ui::notif_prefs_get,
            notif_prefs_set: commands::ui::notif_prefs_set,
            notif_respond: commands::ui::notif_respond,
            ui_prefs_get: commands::ui::ui_prefs_get,
            ui_prefs_set: commands::ui::ui_prefs_set,
            data_export: commands::ui::data_export,
            data_import: commands::ui::data_import,
            data_wipe: commands::ui::data_wipe,
            onboarding_status: commands::ui::onboarding_status,
            onboarding_complete: commands::ui::onboarding_complete,
            onboarding_reset: commands::ui::onboarding_reset,
            open_task: commands::ui::open_task,
            open_study: commands::ui::open_study,
            open_assistant: commands::ui::open_assistant,
            open_website: commands::ui::open_website,
        }
    };
}

// ---------------- hooks de desarrollo (solo debug) ----------------

/// Expande la lista a las rutas de handler que espera generate_handler.
macro_rules! register_handlers {
    ($($name:ident : $path:path),* $(,)?) => {
        tauri::generate_handler![$($path),*]
    };
}

/// Expande la lista a los nombres expuestos al frontend (solo se usa en
/// tests; silencioso en builds release).
#[cfg_attr(not(test), allow(unused_macros))]
macro_rules! command_names {
    ($($name:ident : $path:path),* $(,)?) => {
        &[$(stringify!($name)),*]
    };
}

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
                            append_log(
                                &h,
                                &format!(
                                    "NL_TEST source={src} title={} start={} end={} cat={} prio={}",
                                    t.title, t.start_ms, t.end_ms, t.category_id, t.priority
                                ),
                            );
                            if std::env::var("FF_NL_INSERT").is_ok() {
                                let db = h.state::<Mutex<Db>>();
                                let db = lock_recover(&db);
                                match db.create(
                                    &t.title,
                                    &t.category_id,
                                    &t.priority,
                                    t.start_ms,
                                    t.end_ms,
                                    t.all_day,
                                ) {
                                    Ok(r) => append_log(
                                        &h,
                                        &format!("NL_INSERTED id={} title={}", r.id, t.title),
                                    ),
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
                Ok(s) => append_log(
                    &h,
                    &format!(
                        "SYNC_NOW ok found={} suggestions={}",
                        s.total_found, s.total_suggestions
                    ),
                ),
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
                    &format!(
                        "AI_CONFIG_INJECTED endpoint={} model={}",
                        cfg.endpoint, cfg.model
                    ),
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

// ---------------- arranque ----------------

/// Watcher del CLI: el binario `ff` escribe `cli-change.flag` en el data_dir
/// tras cada mutación; aquí se sondea y se emite `tasks:changed` para que la
/// app abierta y el widget recarguen sin reiniciar. Muy barato: lee stat del
/// archivo cada 2 s, y sólo emite si el mtime cambió.
fn cli_watch_loop(app: tauri::AppHandle, data_dir: PathBuf) {
    let flag = data_dir.join("cli-change.flag");
    std::thread::spawn(move || {
        let mut last: Option<SystemTime> = std::fs::metadata(&flag)
            .ok()
            .and_then(|m| m.modified().ok());
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let cur = std::fs::metadata(&flag)
                .ok()
                .and_then(|m| m.modified().ok());
            match (cur, last) {
                (Some(c), Some(l)) if c == l => {}
                (Some(_), _) => {
                    last = cur;
                    append_log(&app, "cli_change_detected");
                    // El CLI puede haber tocado tareas o clases: refresca ambos.
                    let _ = app.emit("tasks:changed", ());
                    let _ = app.emit("classes:changed", ());
                }
                (None, _) => {}
            }
        }
    });
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            append_log(app, "second_instance_focus_main");
            show_main(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(for_each_command!(register_handlers))
        .setup(|app| {
            let handle = app.handle().clone();

            let data_dir = app.path().app_data_dir()?;
            let db = match Db::open(&data_dir) {
                Ok(db) => db,
                Err(e) => {
                    append_log(&handle, &format!("db_open_error: {e}"));
                    // release = subsistema windows (sin consola): sin diálogo
                    // la app se cerraba sin explicación
                    fatal_dialog(&format!(
                        "FocusFlow no pudo abrir su base de datos y se cerrará.\n\n{e}\n\nCarpeta: {}",
                        data_dir.display()
                    ));
                    return Err(format!("db_open_error: {e}").into());
                }
            };
            let count = db.count().unwrap_or(-1);
            append_log(&handle, &format!("db_ready at {} tasks={count}", data_dir.display()));
            db.settings_default("email.enabled", "0").ok();
            db.settings_default("email.interval_hours", "8").ok();
            db.settings_default("general.start_with_windows", "0").ok();
            db.settings_default("general.start_minimized", "0").ok();
            db.settings_default("general.close_to_tray_widget", "1").ok();
            db.settings_default("ui.theme", "").ok();
            db.settings_default(commands::ui::SETTINGS_ONBOARDING_COMPLETED, "0").ok();
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
                    commands::widget::create_widget(&handle)?;
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
            cli_watch_loop(handle.clone(), data_dir.clone());
            // prune inicial al arrancar: limpiar resoluciones viejas pendientes de archivar
            {
                let db = app.state::<Mutex<Db>>();
                let db = lock_recover(&db);
                let retention_min: i64 = crate::sync::retention_min(&db);
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
                commands::widget::auto_start_behavior(&handle, &lock_recover(&db));
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
                        let _ = commands::widget::create_widget(app);
                    }
                    if let Some(w) = app.get_webview_window("widget") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .map_err(|e| {
            // error de arranque (setup, webview…): diálogo en vez de panic mudo
            fatal_dialog(&format!("FocusFlow no pudo iniciarse:\n\n{e}"));
            Box::new(e) as Box<dyn std::error::Error>
        })?;
    Ok(())
}

/// Diálogo nativo de error fatal (MessageBoxW de user32, ya enlazada por
/// Tauri/WebView2: sin dependencias nuevas). No-op fuera de Windows.
fn fatal_dialog(msg: &str) {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                hwnd: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                utype: u32,
            ) -> i32;
        }
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let text = wide(msg);
        let caption = wide("FocusFlow");
        const MB_OK_ICONERROR: u32 = 0x0000_0010;
        // SAFETY: cadenas UTF-16 terminadas en NUL que viven durante la
        // llamada; hwnd nulo = sin ventana propietaria.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    let _ = msg;
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
        assert!(
            sanitize_log_line(&long).chars().count() <= 2000,
            "tope de longitud"
        );
    }

    #[test]
    fn log_rotates_when_over_limit() {
        let dir = std::env::temp_dir().join(format!("ff-logrot-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("spike.log");
        fs::write(&p, vec![b'x'; 100]).unwrap();
        rotate_log_if_big(&p, 1000);
        assert!(p.exists(), "bajo el límite no rota");
        rotate_log_if_big(&p, 50);
        assert!(!p.exists(), "sobre el límite se renombra");
        assert!(dir.join("spike.log.1").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inventory_matches_handler() {
        // La lista única alimenta handler e inventario; este test asegura que
        // la expansión de nombres está completa y sin duplicados (un nombre
        // repetido sería un bug silencioso de registro).
        let names: &[&str] = for_each_command!(command_names);
        assert!(names.contains(&"log_line"));
        assert!(names.contains(&"task_from_text"));
        assert!(names.contains(&"suggestion_accept"));
        assert!(names.contains(&"assistant_turn"));
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        let uniq = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), uniq, "nombres de comando duplicados");
    }
}
