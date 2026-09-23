//! Dominio TAREAS: CRUD del calendario + creación por lenguaje natural.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::store::{lock_recover, Db, TaskRow};
use crate::{ai, append_log, planning, reminders};

use super::with_db;

#[tauri::command]
pub fn task_list(state: State<'_, Mutex<Db>>) -> Result<Vec<TaskRow>, String> {
    with_db(&state, |db| db.list().map_err(|e| e.to_string()))
}

#[tauri::command]
pub fn task_list_range(
    state: State<'_, Mutex<Db>>,
    start_at: i64,
    end_at: i64,
) -> Result<Vec<TaskRow>, String> {
    with_db(&state, |db| {
        db.list_range(start_at, end_at).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn task_create(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    title: String,
    category_id: String,
    priority: String,
    start_at: i64,
    end_at: i64,
    all_day: bool,
) -> Result<TaskRow, String> {
    let task = with_db(&state, |db| {
        // misma política de conflictos que task_move (bug L4: task_create los
        // ignoraba por completo): con conflict_check activo, en modo estricto
        // el solape se bloquea; en modo laxo solo se registra el aviso.
        let check = crate::setting_bool(db, "calendar.conflict_check", true) && all_day != true;
        if check {
            if let Some((_, other)) = db
                .find_overlap(-1, start_at, end_at)
                .map_err(|e| e.to_string())?
            {
                if crate::setting_bool(db, "calendar.conflict_strict", false) {
                    return Err(format!("conflicto: se solapa con '{other}'"));
                }
                append_log(
                    &app,
                    &format!("task_create_overlap title={title} con={other}"),
                );
            }
        }
        db.create(&title, &category_id, &priority, start_at, end_at, all_day)
            .map_err(|e| e.to_string())
    })?;
    let _ = app.emit("tasks:changed", ());
    Ok(task)
}

#[tauri::command]
pub fn task_complete(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    done: bool,
) -> Result<(), String> {
    with_db(&state, |db| {
        db.set_completed(id, done).map_err(|e| e.to_string())
    })?;
    let _ = app.emit("tasks:changed", ());
    Ok(())
}

#[tauri::command]
pub fn task_delete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    with_db(&state, |db| db.delete(id).map_err(|e| e.to_string()))?;
    let _ = app.emit("tasks:changed", ());
    Ok(())
}

#[derive(Serialize)]
pub struct TaskMoveResult {
    pub conflict: Option<String>,
}

/// Política de conflictos común a task_move / task_update / widget
/// (posponer): con `calendar.conflict_check` activo, un solape con otra
/// tarea con hora se bloquea en modo estricto (`calendar.conflict_strict`)
/// y en modo laxo se devuelve como aviso. Los all-day no ocupan horas.
pub(crate) fn check_conflict(
    db: &Db,
    id: i64,
    start_at: i64,
    end_at: i64,
    all_day: bool,
) -> Result<Option<String>, String> {
    if all_day || !crate::setting_bool(db, "calendar.conflict_check", true) {
        return Ok(None);
    }
    match db
        .find_overlap(id, start_at, end_at)
        .map_err(|e| e.to_string())?
    {
        Some((_, other)) if crate::setting_bool(db, "calendar.conflict_strict", false) => {
            Err(format!("conflicto: se solapa con '{other}'"))
        }
        Some((_, other)) => Ok(Some(other)),
        None => Ok(None),
    }
}

#[tauri::command]
pub fn task_move(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    start_at: i64,
    end_at: i64,
    all_day: Option<bool>,
) -> Result<TaskMoveResult, String> {
    let conflict;
    {
        let db = lock_recover(&state);
        // validación de conflictos configurables (solapamiento) antes de guardar.
        // Por defecto el movimiento se permite y solo se avisa; si el usuario activa
        // `calendar.conflict_strict` (restricciones), el movimiento conflictivo se bloquea.
        conflict = check_conflict(&db, id, start_at, end_at, all_day == Some(true))?;
        db.move_to(id, start_at, end_at, all_day)
            .map_err(|e| e.to_string())?;
    }
    append_log(
        &app,
        &format!("task_moved id={id} start={start_at} end={end_at} all_day={all_day:?} conflict={conflict:?}"),
    );
    let _ = app.emit("tasks:changed", ());
    Ok(TaskMoveResult { conflict })
}

#[tauri::command]
pub fn task_update(
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
) -> Result<TaskMoveResult, String> {
    let conflict = with_db(&state, |db| -> Result<Option<String>, String> {
        // misma política de conflictos que task_create/task_move: editar la
        // hora desde el formulario no validaba solapes
        // solo si cambia el horario: en modo estricto una tarea que YA se
        // solapaba (import, correo, asistente) no se podía editar en nada,
        // ni siquiera el título
        let prev = db.get_task(id).map_err(|e| e.to_string())?;
        let time_changed = prev
            .map(|t| t.start_at != start_at || t.end_at != end_at || t.all_day != all_day)
            .unwrap_or(true);
        let conflict = if time_changed {
            check_conflict(db, id, start_at, end_at, all_day)?
        } else {
            None
        };
        db.update_task_full(
            id,
            &title,
            &category_id,
            &priority,
            start_at,
            end_at,
            &description,
            &tags,
            &notes,
            &links,
            reminder_minutes,
            Some(all_day),
        )
        .map_err(|e| e.to_string())?;
        Ok(conflict)
    })?;
    append_log(
        &app,
        &format!("task_updated id={id} title={title} all_day={all_day} conflict={conflict:?}"),
    );
    let _ = app.emit("tasks:changed", ());
    Ok(TaskMoveResult { conflict })
}

#[tauri::command]
pub fn task_duplicate(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<TaskRow, String> {
    let t = with_db(&state, |db| {
        db.duplicate(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "tarea no encontrada".to_string())
    })?;
    let _ = app.emit("tasks:changed", ());
    Ok(t)
}

#[derive(Serialize)]
pub struct TaskFromTextResult {
    pub task: TaskRow,
    /// Todos los bloques creados (un rango multi-día genera inicio +
    /// "(entrega)"). El frontend los cachea todos, no solo `task`.
    pub created: Vec<TaskRow>,
    pub source: String,
    pub used_ai: bool,
}

/// Interpreta texto libre → tarea(s). La llamada a la IA es bloqueante
/// (hasta 90 s) y corre en otro hilo, FUERA del mutex, para que la app siga
/// respondiendo mientras se interpreta el texto.
#[tauri::command]
pub async fn task_from_text(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
) -> Result<TaskFromTextResult, String> {
    crate::ai_cooldown("task_from_text")?;
    let cfg = with_db(&state, crate::ai_config_from_db);

    let log_app = app.clone();
    let (parsed, source, used_ai) = tauri::async_runtime::spawn_blocking(move || {
        let mut used_ai = false;
        let parsed: (ai::validation::ParsedTask, String) = match ai::provider_from_config(&cfg) {
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
        Ok::<(ai::validation::ParsedTask, String, bool), String>((parsed.0, parsed.1, used_ai))
    })
    .await
    .map_err(|e| e.to_string())??;

    let (task, created) = {
        let db = lock_recover(&state);
        // Rango multi-día ("inicia hoy y finaliza el lunes a las 4pm") →
        // bloque de inicio + bloque "(entrega)"; un solo día → una tarea.
        // Sin hora (parsed.all_day) → marcador de día completo de 24 h, no una
        // tarea 00:00–00:00 invisible que el backlog flexible reprogramaba.
        let blocks = planning::text_task_blocks(
            &parsed.title,
            parsed.start_ms,
            parsed.end_ms,
            parsed.all_day,
        );
        // transacción: si un bloque falla, no queda el rango a medias (bug L5)
        db.tx_begin().map_err(|e| e.to_string())?;
        let result = (|| -> Result<(TaskRow, Vec<TaskRow>), String> {
            let mut created = Vec::with_capacity(blocks.len());
            for (title, s, e, all_day) in &blocks {
                let t = db
                    .create(
                        title,
                        &parsed.category_id,
                        &parsed.priority,
                        *s,
                        *e,
                        *all_day,
                    )
                    .map_err(|e| e.to_string())?;
                created.push(t);
            }
            let t = created
                .first()
                .cloned()
                .ok_or_else(|| "no se pudo crear la tarea".to_string())?;
            // conservar el recordatorio sugerido por la IA ("1d", "3h", ...)
            if let Some(min) = parsed
                .reminders
                .first()
                .and_then(|s| reminders::parse_reminder_minutes(s))
            {
                db.set_task_reminder(t.id, min).map_err(|e| e.to_string())?;
            }
            Ok((t, created))
        })();
        match result {
            Ok(v) => {
                db.tx_commit().map_err(|e| e.to_string())?;
                v
            }
            Err(e) => {
                let _ = db.tx_rollback();
                return Err(e);
            }
        }
    };
    append_log(
        &app,
        &format!(
            "nl_task source={source} ai={used_ai} title={} start={}",
            parsed.title, parsed.start_ms
        ),
    );
    let _ = app.emit("tasks:changed", ());
    Ok(TaskFromTextResult {
        task,
        created,
        source,
        used_ai,
    })
}
