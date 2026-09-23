//! Dominio GOOGLE AUTH + estado de sincronización.

use chrono::TimeZone;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::store::{lock_recover, Db};
use crate::{append_log, auth, sync};

use super::with_db;

/// Inicia el flujo OAuth2 PKCE completo (navegador + callback + intercambio).
/// Async + spawn_blocking: puede tardar hasta ~2 min mientras el usuario
/// autoriza en el navegador; la DB no se retiene durante el flujo.
#[tauri::command]
pub async fn auth_google_sign_in(app: AppHandle) -> Result<auth::AuthSessionView, String> {
    let app2 = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || {
        let session = auth::perform_login()?;
        // persistir sesión + materializar config de Gmail (lock breve)
        let state = app2.state::<std::sync::Mutex<Db>>();
        {
            let db = lock_recover(&state);
            // otra cuenta distinta a la anterior → no heredar el checkpoint
            // del correo (se saltaría su correo reciente)
            let prev_user = db.auth_load().ok().flatten().map(|s| s.user_id);
            if prev_user.is_some_and(|u| u != session.user_id) {
                let _ = db.sync_state_clear_all();
            }
            db.auth_save(&session).map_err(|e| e.to_string())?;
            let cfg = auth::gmail_email_config(&session.email);
            let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
            db.settings_set("email.config", &json)
                .map_err(|e| e.to_string())?;
        }
        Ok::<_, String>(auth::to_view(&session))
    })
    .await;
    match res {
        Ok(Ok(v)) => {
            append_log(&app, &format!("auth_sign_in ok dominio={}", v.email.split('@').nth(1).unwrap_or("?")));
            // enfoca la ventana principal: el usuario acaba de autorizar en el
            // navegador y la app debe traerse a primer plano automáticamente
            crate::show_main(&app);
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

/// Cierra sesión: borra los tokens de la DB (refresh_token incluido) y el
/// checkpoint del correo (ver `auth::sign_out`).
#[tauri::command]
pub fn auth_google_sign_out(app: AppHandle, state: State<'_, Mutex<Db>>) -> Result<(), String> {
    with_db(&state, auth::sign_out)?;
    append_log(&app, "auth_sign_out");
    Ok(())
}

/// Estado actual de la sesión (sin red, sin refresco).
#[tauri::command]
pub fn auth_status(state: State<'_, Mutex<Db>>) -> Option<auth::AuthSessionView> {
    let db = lock_recover(&state);
    auth::status(&db)
}

#[derive(Serialize)]
pub struct SyncStatusView {
    pub states: Vec<crate::store::SyncStateRow>,
    pub today: Vec<crate::store::SyncHistoryRow>,
    pub last_history: Vec<crate::store::SyncHistoryRow>,
    pub last_sync_at: Option<i64>,
    pub next_sync_at: Option<i64>,
    pub interval_hours: u64,
}

#[tauri::command]
pub fn sync_status(state: State<'_, Mutex<Db>>) -> SyncStatusView {
    let db = lock_recover(&state);
    let now = chrono::Local::now();
    let start_of_day = chrono::Local
        .from_local_datetime(&now.date_naive().and_hms_opt(0, 0, 0).unwrap())
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| {
            now.date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis()
        });
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
