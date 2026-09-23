//! Dominio SUGERENCIAS: bandeja de compromisos detectados en el correo
//! (aceptar / rechazar / revertir / editar / fusionar / eliminar) y
//! remitentes de confianza que habilitan la auto-aprobación.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};

use crate::store::{Db, TaskRow};
use crate::{append_log, sync};

use super::with_db;

fn retention_min(db: &Db) -> i64 {
    sync::retention_min(db)
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
    with_db(&state, |db| reject_suggestion(db, id))?;
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
    with_db(&state, |db| {
        edit_suggestion(
            db,
            id,
            &SuggestionEdit {
                title: &title,
                category_id: &category_id,
                priority: &priority,
                start_at,
                end_at,
                description: &description,
                all_day,
            },
        )
    })?;
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
    with_db(&state, |db| merge_suggestion(db, id, task_id))?;
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("email:new-suggestions", ());
    Ok(())
}

/// Rechaza una sugerencia. Solo una `pending` puede rechazarse: rechazar una
/// aceptada dejaba su tarea en el calendario con la sugerencia "rechazada"
/// (y revertirla luego borraba la tarea sin que el usuario lo esperara).
pub fn reject_suggestion(db: &Db, id: i64) -> Result<(), String> {
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
    if s.status != "pending" {
        return Err(format!(
            "solo se puede rechazar una sugerencia pendiente (estado: {})",
            s.status
        ));
    }
    db.set_suggestion_status(id, "rejected")
        .map_err(|e| e.to_string())
}

/// Campos editables de una sugerencia (formulario de edición).
pub struct SuggestionEdit<'a> {
    pub title: &'a str,
    pub category_id: &'a str,
    pub priority: &'a str,
    pub start_at: i64,
    pub end_at: i64,
    pub description: &'a str,
    pub all_day: bool,
}

/// Edita la sugerencia y, si ya fue aceptada, mantiene en sincronía TODAS
/// sus tareas (un rango multi-día creó inicio + "(entrega)"). Antes solo se
/// actualizaba `result_task_id` (el primer bloque) aplicándole el rango
/// entero, y sin transacción. Ahora, dentro de una transacción, se recalcula
/// `split_range_blocks`: si el número de bloques coincide con las tareas
/// vivas se actualiza cada una (preservando tags/notas/links/recordatorio);
/// si no, se borran y se recrean enlazadas.
pub fn edit_suggestion(db: &Db, id: i64, e: &SuggestionEdit) -> Result<(), String> {
    db.tx_begin().map_err(|e| e.to_string())?;
    let r = edit_suggestion_inner(db, id, e);
    match r {
        Ok(()) => db.tx_commit().map_err(|e| e.to_string()),
        Err(err) => {
            let _ = db.tx_rollback();
            Err(err)
        }
    }
}

fn edit_suggestion_inner(db: &Db, id: i64, e: &SuggestionEdit) -> Result<(), String> {
    // rango previo: si no cambia, las tareas conservan SUS horas (el usuario
    // pudo moverlas) y no se recalculan bloques — `split_range_blocks` ancla
    // el bloque de inicio a "ahora" cuando el rango ya empezó, así que
    // cualquier edición (aunque fuera solo el título) movía la tarea y
    // re-armaba su recordatorio, y además resucitaba bloques borrados.
    let prev_range = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .map(|s| (s.start_at, s.end_at));
    let range_changed = prev_range != Some((Some(e.start_at), Some(e.end_at)));
    db.update_suggestion_data(
        id,
        e.title,
        e.category_id,
        e.priority,
        e.start_at,
        e.end_at,
        e.description,
    )
    .map_err(|e| e.to_string())?;
    let ids = db.suggestion_task_ids(id).map_err(|e| e.to_string())?;
    let mut alive: Vec<TaskRow> = Vec::new();
    for tid in ids {
        if let Some(t) = db.get_task(tid).map_err(|e| e.to_string())? {
            alive.push(t);
        }
    }
    // sin tareas vivas (pendiente, rechazada, fusionada o tarea borrada por
    // el usuario): solo se edita la sugerencia
    if alive.is_empty() {
        return Ok(());
    }
    if !range_changed {
        for (i, t) in alive.iter().enumerate() {
            // título: el bloque principal toma el nuevo; los "(entrega)"
            // conservan su sufijo sobre el título nuevo
            let title = if i == 0 || !t.title.ends_with(" (entrega)") {
                e.title.to_string()
            } else {
                format!("{} (entrega)", e.title)
            };
            let desc = if i == 0 { e.description } else { t.description.as_str() };
            db.update_task_full(
                t.id,
                &title,
                e.category_id,
                e.priority,
                t.start_at,
                t.end_at,
                desc,
                &t.tags,
                &t.notes,
                &t.links,
                t.reminder_minutes,
                Some(t.all_day),
            )
            .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    let blocks = crate::planning::split_range_blocks(e.title, e.start_at, e.end_at);
    let single = blocks.len() == 1;
    let block_all_day = |b: &(String, i64, i64, bool)| if single { e.all_day } else { b.3 };
    // un bloque all-day se ancla a medianoche local + 24 h (el formulario
    // puede traer horas: 10:00–12:00 daba un "todo el día" de 2 h a las 10)
    let span = |b: &(String, i64, i64, bool)| {
        if block_all_day(b) {
            let s = crate::engine::local_midnight(b.1);
            (s, s + crate::engine::DAY_MS)
        } else {
            (b.1, b.2)
        }
    };
    if blocks.len() == alive.len() {
        for (i, (b, t)) in blocks.iter().zip(alive.iter()).enumerate() {
            // la descripción del formulario va al bloque principal; los
            // demás conservan la suya
            let desc = if i == 0 { e.description } else { t.description.as_str() };
            db.update_task_full(
                t.id,
                &b.0,
                e.category_id,
                e.priority,
                span(b).0,
                span(b).1,
                desc,
                &t.tags,
                &t.notes,
                &t.links,
                t.reminder_minutes,
                Some(block_all_day(b)),
            )
            .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    // cambió la forma del rango (1 día ↔ multi-día): recrear los bloques
    let main = alive[0].clone();
    db.unlink_and_delete_suggestion_tasks(id)
        .map_err(|e| e.to_string())?;
    let mut first: Option<i64> = None;
    for (i, b) in blocks.iter().enumerate() {
        let t = db
            .create(&b.0, e.category_id, e.priority, span(b).0, span(b).1, block_all_day(b))
            .map_err(|e| e.to_string())?;
        if i == 0 {
            db.update_task_full(
                t.id,
                &t.title,
                &t.category_id,
                &t.priority,
                t.start_at,
                t.end_at,
                e.description,
                &main.tags,
                &main.notes,
                &main.links,
                main.reminder_minutes,
                None,
            )
            .map_err(|e| e.to_string())?;
            first = Some(t.id);
        }
        db.link_suggestion_task(id, t.id)
            .map_err(|e| e.to_string())?;
    }
    if let Some(tid) = first {
        db.set_suggestion_result_task(id, tid)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Fusiona una sugerencia pendiente con una tarea existente. Las dos
/// escrituras (tarea + estado) van en una transacción: antes un fallo al
/// marcar `merged` dejaba la tarea modificada y la sugerencia `pending`
/// (aceptarla luego duplicaba). Una fusión NO es reversible (no se guarda
/// snapshot de la tarea previa): `revert_suggestion` la rechaza.
pub fn merge_suggestion(db: &Db, id: i64, task_id: i64) -> Result<(), String> {
    db.tx_begin().map_err(|e| e.to_string())?;
    match merge_suggestion_inner(db, id, task_id) {
        Ok(()) => db.tx_commit().map_err(|e| e.to_string()),
        Err(err) => {
            let _ = db.tx_rollback();
            Err(err)
        }
    }
}

fn merge_suggestion_inner(db: &Db, id: i64, task_id: i64) -> Result<(), String> {
    let s = db
        .get_suggestion(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "sugerencia no encontrada".to_string())?;
    // solo una sugerencia pendiente puede fusionarse: una ya aceptada tiene
    // (o debe tener) su propia tarea, y fusionarla la duplicaba (bug M2)
    if s.status != "pending" {
        return Err(format!(
            "la sugerencia ya fue procesada (estado: {})",
            s.status
        ));
    }
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
