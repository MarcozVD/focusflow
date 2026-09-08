//! Dominio SUGERENCIAS: bandeja de compromisos detectados en el correo
//! (aceptar / rechazar / revertir / editar / fusionar / eliminar) y
//! remitentes de confianza que habilitan la auto-aprobación.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use crate::store::{lock_recover, Db, TaskRow};
use crate::{append_log, sync};

use super::with_db;

fn retention_min(db: &Db) -> i64 {
    db.settings_get("email.suggestion_retention_minutes")
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(60)
}

#[tauri::command]
pub fn suggestions_list(
    state: State<'_, Mutex<Db>>,
    only_pending: bool,
) -> Result<Vec<crate::store::SuggestionRow>, String> {
    with_db(&state, |db| {
        let retention = retention_min(db);
        db.list_suggestions(only_pending, retention * 60_000)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn suggestion_accept(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<Vec<TaskRow>, String> {
    let tasks = with_db(&state, |db| sync::accept_suggestion(db, id))?;
    append_log(
        &app,
        &format!("suggestion_accepted id={id} tasks={}", tasks.len()),
    );
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(tasks)
}

#[tauri::command]
pub fn suggestion_reject(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<(), String> {
    with_db(&state, |db| {
        db.set_suggestion_status(id, "rejected")
            .map_err(|e| e.to_string())
    })?;
    append_log(&app, &format!("suggestion_rejected id={id}"));
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
pub fn suggestion_revert(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<(), String> {
    with_db(&state, |db| sync::revert_suggestion(db, id))?;
    append_log(&app, &format!("suggestion_reverted id={id}"));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

/// Elimina la sugerencia por completo (control del usuario). Si tenía tarea
/// creada, la borra también. No se puede recuperar.
#[tauri::command]
pub fn suggestion_delete(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<(), String> {
    with_db(&state, |db| {
        db.delete_suggestion(id).map_err(|e| e.to_string())
    })?;
    append_log(&app, &format!("suggestion_deleted id={id}"));
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

#[tauri::command]
pub fn suggestion_edit(
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
    db.update_suggestion_data(
        id,
        &title,
        &category_id,
        &priority,
        start_at,
        end_at,
        &description,
    )
    .map_err(|e| e.to_string())?;
    // si la sugerencia ya fue aceptada, la tarea creada se mantiene en sincronía
    // (solo los campos del formulario; description/tags/notas/links y el
    // recordatorio de la tarea se preservan leyendo la tarea actual)
    if let Some(task_id) = db
        .get_suggestion(id)
        .ok()
        .flatten()
        .and_then(|s| s.result_task_id)
    {
        if let Some(t) = db.get_task(task_id).ok().flatten() {
            db.update_task_full(
                task_id,
                &title,
                &category_id,
                &priority,
                start_at,
                end_at,
                &description,
                &t.tags,
                &t.notes,
                &t.links,
                t.reminder_minutes,
                Some(all_day),
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
pub fn suggestion_merge(
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
    let title = if s.title.trim().is_empty() {
        existing.title.clone()
    } else {
        s.title.clone()
    };
    let start = s.start_at.unwrap_or(existing.start_at);
    let end = s.end_at.unwrap_or(existing.end_at);
    let priority = if s.priority == "media" {
        existing.priority.clone()
    } else {
        s.priority.clone()
    };
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
        task_id,
        &title,
        &s.category_id,
        &priority,
        start,
        end,
        &description,
        &existing.tags,
        &existing.notes,
        &existing.links,
        existing.reminder_minutes,
        None,
    )
    .map_err(|e| e.to_string())?;
    db.set_suggestion_status(id, "merged")
        .map_err(|e| e.to_string())?;
    drop(db);
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

// ---------------- remitentes de confianza ----------------

#[tauri::command]
pub fn trusted_senders_list(state: State<'_, Mutex<Db>>) -> Result<Vec<String>, String> {
    with_db(&state, |db| db.trusted_list().map_err(|e| e.to_string()))
}

#[tauri::command]
pub fn trusted_senders_add(state: State<'_, Mutex<Db>>, sender: String) -> Result<(), String> {
    with_db(&state, |db| {
        db.trusted_add(&sender).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn trusted_senders_remove(state: State<'_, Mutex<Db>>, sender: String) -> Result<(), String> {
    with_db(&state, |db| {
        db.trusted_remove(&sender).map_err(|e| e.to_string())
    })
}
