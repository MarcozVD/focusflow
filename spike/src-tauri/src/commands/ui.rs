//! Dominio UI/AJUSTES: generales, notificaciones, preferencias visuales,
//! onboarding, datos (export/borrado), reporte de errores y navegación
//! abierta desde el widget (open_task/open_agenda/open_assistant/website).

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::store::{lock_recover, Db};
use crate::{append_log, auth, report, sync};

use super::with_db;

// ---------------- ajustes generales ----------------

fn setting_bool(db: &Db, key: &str, default: bool) -> bool {
    crate::setting_bool(db, key, default)
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn autostart_set(enabled: bool) -> Result<(), String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
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
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_READ) {
        Ok(k) => k.get_value::<String, _>("FocusFlow").is_ok(),
        Err(_) => false,
    }
}

#[derive(Serialize)]
pub struct GeneralSettingsView {
    pub start_with_windows: bool,
    pub start_minimized: bool,
    pub close_to_tray_widget: bool,
    pub conflict_strict: bool,
    pub autostart_actual: bool,
}

#[tauri::command]
pub fn general_settings_get(state: State<'_, Mutex<Db>>) -> GeneralSettingsView {
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
pub fn general_settings_set(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    start_with_windows: bool,
    start_minimized: bool,
    close_to_tray_widget: bool,
    conflict_strict: bool,
) -> Result<(), String> {
    {
        let db = lock_recover(&state);
        db.settings_set(
            "general.start_with_windows",
            if start_with_windows { "1" } else { "0" },
        )
        .map_err(|e| e.to_string())?;
        db.settings_set(
            "general.start_minimized",
            if start_minimized { "1" } else { "0" },
        )
        .map_err(|e| e.to_string())?;
        db.settings_set(
            "general.close_to_tray_widget",
            if close_to_tray_widget { "1" } else { "0" },
        )
        .map_err(|e| e.to_string())?;
        db.settings_set(
            "calendar.conflict_strict",
            if conflict_strict { "1" } else { "0" },
        )
        .map_err(|e| e.to_string())?;
    }
    autostart_set(start_with_windows)?;
    append_log(
        &app,
        &format!("general_settings_set start_win={start_with_windows} minimized={start_minimized} tray={close_to_tray_widget} conflict_strict={conflict_strict}"),
    );
    Ok(())
}

// ---------------- notificaciones contextuales (fase 11) ----------------

#[tauri::command]
pub fn notif_prefs_get(state: State<'_, Mutex<Db>>) -> crate::notify::NotifPrefsView {
    let db = lock_recover(&state);
    crate::notify::prefs_view(&db)
}

#[tauri::command]
pub fn notif_prefs_set(
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
        let (h, m) = s
            .split_once(':')
            .ok_or_else(|| format!("formato inválido: {s} (espera HH:MM)"))?;
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
    db.settings_set("notif.enabled", if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    db.settings_set("notif.quiet_start", &quiet_start)
        .map_err(|e| e.to_string())?;
    db.settings_set("notif.quiet_end", &quiet_end)
        .map_err(|e| e.to_string())?;
    db.settings_set("notif.daily_cap", &cap.to_string())
        .map_err(|e| e.to_string())?;
    db.settings_set("notif.free_minutes", &free.to_string())
        .map_err(|e| e.to_string())?;
    append_log(
        &app,
        &format!("notif_prefs_set enabled={enabled} quiet={quiet_start}-{quiet_end} cap={cap} free_min={free}"),
    );
    Ok(())
}

#[tauri::command]
pub fn notif_respond(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    status: String,
) -> Result<(), String> {
    with_db(&state, |db| {
        db.set_notif_status(id, &status).map_err(|e| e.to_string())
    })?;
    append_log(&app, &format!("notif_respond id={id} status={status}"));
    Ok(())
}

// ---------------- preferencias de UI (tema + acento) ----------------

#[derive(Serialize, Clone)]
pub struct UiPrefsView {
    pub theme: String,
    pub accent: String,
}

#[tauri::command]
pub fn ui_prefs_get(state: State<'_, Mutex<Db>>) -> UiPrefsView {
    let db = lock_recover(&state);
    UiPrefsView {
        theme: db
            .settings_get("ui.theme")
            .ok()
            .flatten()
            .unwrap_or_default(),
        accent: db
            .settings_get("ui.accent")
            .ok()
            .flatten()
            .filter(|v| v.starts_with('#') && v.len() == 7)
            .unwrap_or_else(|| "#2563EB".into()),
    }
}

#[tauri::command]
pub fn ui_prefs_set(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    theme: String,
    accent: String,
) -> Result<(), String> {
    let theme = if theme == "light" || theme == "dark" {
        theme
    } else {
        String::new()
    };
    let accent = if accent.starts_with('#') && accent.len() == 7 {
        accent
    } else {
        "#2563EB".into()
    };
    {
        let db = lock_recover(&state);
        db.settings_set("ui.theme", &theme)
            .map_err(|e| e.to_string())?;
        db.settings_set("ui.accent", &accent)
            .map_err(|e| e.to_string())?;
    }
    append_log(
        &app,
        &format!("ui_prefs_set theme={theme:?} accent={accent}"),
    );
    let _ = app.emit("ui:prefs", UiPrefsView { theme, accent });
    Ok(())
}

// ---------------- onboarding (primer arranque) ----------------

pub const SETTINGS_ONBOARDING_COMPLETED: &str = "onboarding.completed";

#[derive(Serialize)]
pub struct OnboardingAiView {
    pub endpoint: String,
    pub model: String,
    pub effective_endpoint: String,
    pub effective_model: String,
}

#[derive(Serialize)]
pub struct OnboardingStatusView {
    pub completed: bool,
    pub ai: OnboardingAiView,
    pub email: Option<crate::email::EmailConfig>,
}

#[tauri::command]
pub fn onboarding_status(state: State<'_, Mutex<Db>>) -> OnboardingStatusView {
    let db = lock_recover(&state);
    let completed = setting_bool(&db, SETTINGS_ONBOARDING_COMPLETED, false);
    let ai_cfg = crate::ai_config_from_db(&db);
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
                crate::ai::default_endpoint()
            } else {
                ai_cfg.endpoint.clone()
            },
            effective_model: if ai_cfg.model.is_empty() {
                crate::ai::default_model()
            } else {
                ai_cfg.model.clone()
            },
        },
        email,
    }
}

#[tauri::command]
pub fn onboarding_complete(app: AppHandle, state: State<'_, Mutex<Db>>) -> Result<(), String> {
    with_db(&state, |db| {
        db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "1")
            .map_err(|e| e.to_string())
    })?;
    append_log(&app, "onboarding_completed");
    Ok(())
}

#[tauri::command]
pub fn onboarding_reset(state: State<'_, Mutex<Db>>) -> Result<(), String> {
    with_db(&state, |db| {
        db.settings_set(SETTINGS_ONBOARDING_COMPLETED, "0")
            .map_err(|e| e.to_string())
    })
}

// ---------------- privacidad (fase 12) ----------------

/// Exporta los datos del usuario en JSON (sin secretos: ni claves ni
/// contraseñas, que viven solo en el Credential Manager del SO).
#[tauri::command]
pub fn data_export(state: State<'_, Mutex<Db>>) -> Result<String, String> {
    let db = lock_recover(&state);
    let v = db.export_data().map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

/// Borra TODO: datos en DB y log local. Destructivo e irreversible.
/// Los secretos ya no viven en Credential Manager (ver CAMBIO 1); el logout
/// de Google se hace con `auth_google_sign_out` (borra tokens de la DB).
#[tauri::command]
pub fn data_wipe(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    confirmation: String,
) -> Result<(), String> {
    // borrado irreversible: exige token explícito del frontend para que una
    // llamada accidental (o un webview comprometido) no pueda borrar sin
    // confirmación del usuario
    if confirmation != "WIPE" {
        append_log(&app, "data_wipe canceled (sin confirmación)");
        return Err("borrado cancelado: falta confirmación".into());
    }
    with_db(&state, |db| db.wipe_data().map_err(|e| e.to_string()))?;
    if let Some(dir) = crate::log_dir() {
        let _ = std::fs::write(dir.join("spike.log"), "");
    }
    let _ = app.emit("data:wipe", ());
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("suggestions:changed", ());
    append_log(&app, "data_wipe done");
    Ok(())
}

// ---------------- reporte de errores (módulo opcional) ----------------

/// MÓDULO OPCIONAL de reporte de errores (ver report.rs): envía un correo con
/// la descripción y los últimos errores del log usando la cuenta configurada.
#[tauri::command]
pub fn report_send(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    description: String,
) -> Result<String, String> {
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

// ---------------- navegación abierta desde el widget ----------------

#[tauri::command]
pub fn open_task(app: AppHandle, id: i64) -> Result<(), String> {
    crate::show_main(&app);
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
pub fn open_agenda(app: AppHandle) -> Result<(), String> {
    crate::show_main(&app);
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

/// Abre una URL del sitio web de FocusFlow (directivas/privacidad/condiciones/
/// guía de primeros pasos) en el navegador del sistema. Misma cadena de fallback
/// que auth.rs.
#[tauri::command]
pub fn open_website(app: AppHandle, url: String) -> Result<(), String> {
    let base = crate::WEB_BASE;
    let allowed = url == format!("{base}/legal/directivas.html")
        || url == format!("{base}/legal/privacidad.html")
        || url == format!("{base}/legal/condiciones.html")
        || url == format!("{base}/legal/empezar.html");
    if !allowed {
        return Err("url_no_permitida".into());
    }
    if open::that(&url).is_ok() {
        append_log(&app, &format!("open_website {url}"));
        return Ok(());
    }
    let rundll32 = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", &url])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if rundll32 {
        append_log(&app, &format!("open_website {url}"));
        return Ok(());
    }
    append_log(&app, &format!("open_website_failed {url}"));
    Err("no_se_pudo_abrir_navegador".into())
}

/// "Pregunta a FocusFlow": abre la app en la vista del Asistente (fase 9/10).
#[tauri::command]
pub fn open_assistant(app: AppHandle) -> Result<(), String> {
    crate::show_main(&app);
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
