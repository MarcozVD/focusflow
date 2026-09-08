//! Dominio PLANES: propuestas de planificación (fase 7) generadas desde
//! texto libre con IA (o la variante 100% local sin IA).

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use crate::store::{Db, TaskRow};
use crate::{ai, append_log, planning};

#[tauri::command]
pub async fn plan_from_text(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
) -> Result<planning::PlanProposalView, String> {
    crate::ai_cooldown("plan_from_text")?;
    let cfg = crate::commands::with_db(&state, crate::ai_config_from_db);
    let log_app = app.clone();
    // La IA es bloqueante: se interpreta el texto fuera del mutex.
    let text_ai = text.clone();
    let (intents, source) = tauri::async_runtime::spawn_blocking(move || {
        let (provider, configured) = match ai::provider_from_config(&cfg) {
            Ok(p) => (Some(p), true),
            Err(_) => (None, false),
        };
        let batch = match ai::intent_parser::parse_intent(&text_ai, provider.as_deref(), configured)
        {
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

    let view = crate::commands::with_db(&state, |db| {
        planning::plan_from_text(db, &text, &intents, &source)
    })?;
    append_log(
        &app,
        &format!(
            "plan_created id={} source={} items={}",
            view.id,
            view.source,
            view.items.len()
        ),
    );
    let _ = app.emit("plans:changed", ());
    Ok(view)
}

/// Variante 100% local de plan_from_text: sin IA ni cooldown, para cuando el
/// proveedor está lento/saturado y el usuario elige la interpretación rápida.
#[tauri::command]
pub async fn plan_from_text_local(
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

    let view = crate::commands::with_db(&state, |db| {
        planning::plan_from_text(db, &text, &intents, &source)
    })?;
    append_log(
        &app,
        &format!(
            "plan_created id={} source={} items={} (local)",
            view.id,
            view.source,
            view.items.len()
        ),
    );
    let _ = app.emit("plans:changed", ());
    Ok(view)
}

#[tauri::command]
pub fn plan_proposal_get(
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<Option<planning::PlanProposalView>, String> {
    crate::commands::with_db(&state, |db| planning::get_plan(db, id))
}

#[tauri::command]
pub fn plan_proposals_list(
    state: State<'_, Mutex<Db>>,
    only_pending: bool,
) -> Result<Vec<crate::store::PlanProposalRow>, String> {
    crate::commands::with_db(&state, |db| {
        db.list_plan_proposals(only_pending)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn plan_accept(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    edit: Option<planning::EditedPlan>,
) -> Result<Vec<TaskRow>, String> {
    let tasks = crate::commands::with_db(&state, |db| {
        planning::accept_plan(db, id, &edit.unwrap_or_default())
    })?;
    append_log(
        &app,
        &format!("plan_accepted id={id} tasks={}", tasks.len()),
    );
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("plans:changed", ());
    Ok(tasks)
}

#[tauri::command]
pub fn plan_reject(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    crate::commands::with_db(&state, |db| planning::reject_plan(db, id))?;
    append_log(&app, &format!("plan_rejected id={id}"));
    let _ = app.emit("plans:changed", ());
    Ok(())
}
