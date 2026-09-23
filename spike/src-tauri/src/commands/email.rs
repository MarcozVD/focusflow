//! Dominio IA: configuración, prueba de conexión y utilidades de correo/sync
//! que solo usa Ajustes (config + verificación de conexiones + rescan).

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::store::{lock_recover, Db};
use crate::{ai, append_log, email, sync};

use super::with_db;

#[derive(Serialize)]
pub struct AiConfigView {
    pub endpoint: String,
    pub model: String,
    pub effective_endpoint: String,
    pub effective_model: String,
    pub configured: bool,
}

#[tauri::command]
pub fn ai_config_get(state: State<'_, Mutex<Db>>) -> AiConfigView {
    let cfg = with_db(&state, crate::ai_config_from_db);
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
        configured: !cfg.endpoint.is_empty()
            && !cfg.model.is_empty()
            && cfg.provider_name() != "local",
    }
}

#[tauri::command]
pub fn ai_config_set(
    state: State<'_, Mutex<Db>>,
    endpoint: String,
    model: String,
) -> Result<(), String> {
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
    with_db(&state, |db| {
        db.settings_set("ai.endpoint", &endpoint)
            .map_err(|e| e.to_string())?;
        db.settings_set("ai.model", &model)
            .map_err(|e| e.to_string())?;
        Ok(())
    })
}

#[derive(Serialize)]
pub struct AiTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub model: String,
    pub error: String,
}

#[tauri::command]
pub async fn ai_test(state: State<'_, Mutex<Db>>, app: AppHandle) -> Result<AiTestResult, String> {
    crate::ai_cooldown("ai_test").ok(); // prueba manual: cooldown suave, no bloquea
                                        // Lock corto solo para leer la config; la llamada de red (hasta ~4,5 min
                                        // con reintentos 429) va a otro hilo y fuera del mutex, o congela todos
                                        // los comandos IPC que tocan la DB (auditoría 17, hallazgo #1).
    let cfg = with_db(&state, crate::ai_config_from_db);
    let t0 = std::time::Instant::now();
    let res = tauri::async_runtime::spawn_blocking(move || match ai::provider_from_config(&cfg) {
        Ok(provider) => {
            let model = cfg.model.clone();
            match provider.chat_json(
                "Devuelve exactamente: {\"ok\": true}",
                "ping",
                r#"{"ok": true}"#,
            ) {
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

// ---------------- config de correo + verificación de conexiones ----------------

#[derive(Serialize)]
pub struct EmailConfigView {
    pub config: email::EmailConfig,
    pub enabled: bool,
    pub interval_hours: u64,
    pub max_age_days: u32,
    pub trusted: Vec<String>,
}

#[tauri::command]
pub fn email_config_get(state: State<'_, Mutex<Db>>) -> EmailConfigView {
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
pub fn email_config_set(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    config: email::EmailConfig,
    enabled: bool,
    interval_hours: u64,
    max_age_days: u32,
) -> Result<(), String> {
    let db = lock_recover(&state);
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    db.settings_set("email.config", &json)
        .map_err(|e| e.to_string())?;
    db.settings_set("email.enabled", if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    db.settings_set("email.interval_hours", &interval_hours.to_string())
        .map_err(|e| e.to_string())?;
    db.settings_set("email.max_age_days", &max_age_days.to_string())
        .map_err(|e| e.to_string())?;
    append_log(&app, &format!("email_config_saved dominio={} mailboxes={:?} enabled={enabled} max_age_days={max_age_days}", config.user.split('@').nth(1).unwrap_or("?"), config.mailboxes));
    Ok(())
}

#[tauri::command]
pub fn email_sync_now(app: AppHandle) -> Result<(), String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || match sync::run_sync(&handle) {
        Ok(s) => append_log(
            &handle,
            &format!(
                "manual_sync_ok found={} suggestions={}",
                s.total_found, s.total_suggestions
            ),
        ),
        // run_sync ya emite `email:sync-error` (evita el evento duplicado)
        Err(e) => append_log(&handle, &format!("manual_sync_error: {e}")),
    });
    Ok(())
}

/// Reescanear la ventana reciente: reinicia checkpoints y sincroniza.
/// Recupera correos que quedaron fuera por filtros o errores previos
/// (la deduplicación por message_id evita duplicados).
#[tauri::command]
pub fn email_rescan(app: AppHandle, state: State<'_, Mutex<Db>>) -> Result<(), String> {
    with_db(&state, |db| {
        db.settings_set("email.rescan_pending", "1")
            .map_err(|e| e.to_string())
    })?;
    append_log(&app, "email_rescan_requested");
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || match sync::run_sync(&handle) {
        Ok(s) => append_log(
            &handle,
            &format!(
                "rescan_ok found={} suggestions={}",
                s.total_found, s.total_suggestions
            ),
        ),
        // run_sync ya emite `email:sync-error` (evita el evento duplicado)
        Err(e) => append_log(&handle, &format!("rescan_error: {e}")),
    });
    Ok(())
}

#[derive(Serialize)]
pub struct ConnectionCheck {
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize)]
pub struct VerifyResult {
    pub ai: ConnectionCheck,
    pub email: ConnectionCheck,
}

/// Prueba ambas conexiones: API de IA (OpenCode Zen) y Gmail (REST API).
/// Async + spawn_blocking: la red (IA + Gmail REST, potencialmente minutos con
/// reintentos) no puede correr en el hilo principal o congela la UI
/// (auditoría 17, hallazgo #1).
#[tauri::command]
pub async fn verify_connections(
    state: State<'_, Mutex<Db>>,
    app: AppHandle,
) -> Result<VerifyResult, String> {
    crate::ai_cooldown("verify_connections").ok(); // onboarding: cooldown suave
    let (ai_cfg, email_cfg) = with_db(&state, |db| {
        (crate::ai_config_from_db(db), sync::load_email_config(db))
    });
    let app2 = app.clone();

    let res = tauri::async_runtime::spawn_blocking(move || {
        let ai = match ai::provider_from_config(&ai_cfg) {
            Ok(provider) => {
                let t0 = std::time::Instant::now();
                match provider.chat_json(
                    "Devuelve exactamente: {\"ok\": true}",
                    "ping",
                    r#"{"ok": true}"#,
                ) {
                    Ok(_) => ConnectionCheck {
                        ok: true,
                        detail: format!(
                            "API OK ({}, {} ms)",
                            ai_cfg.model,
                            t0.elapsed().as_millis()
                        ),
                    },
                    Err(e) => ConnectionCheck {
                        ok: false,
                        detail: format!("{} ({} ms)", e, t0.elapsed().as_millis()),
                    },
                }
            }
            Err(e) => ConnectionCheck {
                ok: false,
                detail: e.to_string(),
            },
        };

        // refresco del token FUERA del lock (POST hasta 60 s): antes se hacía
        // con la DB bloqueada y congelaba todos los comandos IPC
        let token = match crate::auth::access_token_unlocked(&app2.state::<Mutex<Db>>()) {
            Ok(t) => Ok(t),
            Err(e) => {
                if crate::auth::is_session_expired(&e) {
                    let _ = app2.emit("auth:expired", &e);
                }
                Err(e)
            }
        };
        let email = match token.and_then(|t| email::test_connection(&email_cfg, &t)) {
            Ok((mailbox, n)) => ConnectionCheck {
                ok: true,
                detail: format!("Conectado a {mailbox} ({n} correos)"),
            },
            Err(e) => ConnectionCheck {
                ok: false,
                detail: e,
            },
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
            ai: ConnectionCheck {
                ok: false,
                detail: e.to_string(),
            },
            email: ConnectionCheck {
                ok: false,
                detail: "no ejecutado".into(),
            },
        },
    };
    Ok(result)
}
