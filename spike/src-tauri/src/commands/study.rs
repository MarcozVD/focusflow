//! Comandos del dominio "Sesiones de estudio".
//!
//! Una sesión de estudio es un bloque de tiempo RESERVADO por el usuario para
//! estudiar o trabajar sobre un tema/tarea. Es una entidad INDEPENDIENTE:
//! - de las TAREAS (no tiene estado, no es pendiente, no cuenta en contadores
//!   ni estadísticas; `task_id` es solo un vínculo opcional),
//! - de las CLASES (una clase es tiempo ocupado fijo y recurrente; una sesión
//!   es tiempo reservado, movible y editable como una tarea).
//!
//! El planificador las consulta como tiempo ya reservado
//! (`planning::engine_with_calendar`), así nunca propone una tarea encima.

use super::with_db;
use crate::append_log;
use crate::store::{lock_recover, Db, StudyRow};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

fn emit_changed(app: &AppHandle) {
    // Evento propio: el frontend recarga solo las sesiones (las tareas y las
    // clases no cambian al crear/mover una sesión).
    let _ = app.emit("study:changed", ());
}

/// Sesión que toca el rango pedido (solape real semiabierto).
#[tauri::command]
pub fn study_list_range(
    state: State<'_, Mutex<Db>>,
    start_at: i64,
    end_at: i64,
) -> Result<Vec<StudyRow>, String> {
    with_db(&state, |db| {
        db.study_list_range(start_at, end_at)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn study_create(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    title: String,
    start_at: i64,
    end_at: i64,
    task_id: Option<i64>,
    notes: String,
) -> Result<StudyRow, String> {
    let row = with_db(&state, |db| {
        db.study_create(&title, start_at, end_at, task_id, &notes)
            .map_err(Db::rusqlite_error_text)
    })?;
    append_log(
        &app,
        &format!(
            "study_created id={} title={} {}..{} task={:?}",
            row.id, row.title, row.start_at, row.end_at, row.task_id
        ),
    );
    emit_changed(&app);
    Ok(row)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn study_update(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    title: String,
    start_at: i64,
    end_at: i64,
    task_id: Option<i64>,
    notes: String,
) -> Result<StudyRow, String> {
    let row = with_db(&state, |db| {
        db.study_update(id, &title, start_at, end_at, task_id, &notes)
            .map_err(Db::rusqlite_error_text)
    })?;
    append_log(
        &app,
        &format!(
            "study_updated id={id} title={title} {}..{} task={:?}",
            row.start_at, row.end_at, row.task_id
        ),
    );
    emit_changed(&app);
    Ok(row)
}

/// Mueve/redimensiona una sesión desde el calendario (drag & drop). Solo
/// cambia la ventana temporal: la decisión de superponer es del usuario.
#[tauri::command]
pub fn study_move(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    start_at: i64,
    end_at: i64,
) -> Result<StudyRow, String> {
    let row = with_db(&state, |db| {
        db.study_move(id, start_at, end_at)
            .map_err(Db::rusqlite_error_text)
    })?;
    append_log(
        &app,
        &format!("study_moved id={id} {}..{}", row.start_at, row.end_at),
    );
    emit_changed(&app);
    Ok(row)
}

#[tauri::command]
pub fn study_delete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    let deleted = with_db(&state, |db| db.study_delete(id).map_err(|e| e.to_string()))?;
    if !deleted {
        return Err("sesión de estudio no encontrada".into());
    }
    append_log(&app, &format!("study_deleted id={id}"));
    emit_changed(&app);
    Ok(())
}

/// Conflicto de una ventana con el resto del calendario del usuario.
/// - `classes`: clases activas (vigencia + día) que se solapan → el frontend
///   muestra "Esta sesión coincide con una clase" con las opciones
///   Editar horario / Continuar de todas formas / Cancelar (regla 10).
/// - `tasks`: tareas que se solapan → aviso suave, NUNCA bloqueo: ambas
///   entidades coexisten sin que el sistema mueva ninguna (regla 11).
#[derive(serde::Serialize, Clone, Debug)]
pub struct StudyConflicts {
    pub classes: Vec<super::classes::ClassInstance>,
    pub tasks: Vec<StudyTaskHit>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct StudyTaskHit {
    pub id: i64,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
}

#[tauri::command]
pub fn study_conflicts(
    state: State<'_, Mutex<Db>>,
    start_at: i64,
    end_at: i64,
    exclude_id: Option<i64>,
) -> Result<StudyConflicts, String> {
    let db = lock_recover(&state);
    let classes = db.class_list().map_err(|e| e.to_string())?;
    let classes = super::classes::class_instances_in_range(&classes, start_at, end_at)
        .into_iter()
        .filter(|i| i.start_at < end_at && i.end_at > start_at)
        .collect();
    let mut tasks = Vec::new();
    if let Ok(rows) = db.list() {
        for t in rows {
            if t.status == "completada" {
                continue;
            }
            if t.start_at < end_at && t.end_at > start_at {
                tasks.push(StudyTaskHit {
                    id: t.id,
                    title: t.title,
                    start_at: t.start_at,
                    end_at: t.end_at,
                });
            }
        }
    }
    let _ = exclude_id; // reservado: hoy una sesión nunca choca consigo misma
    Ok(StudyConflicts { classes, tasks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::engine_with_calendar;
    use chrono::{Datelike, TimeZone};

    const DAY: i64 = 86_400_000;
    const HOUR: i64 = 3_600_000;

    fn today() -> i64 {
        crate::engine::local_midnight(crate::email::now_ms())
    }

    #[test]
    fn study_crud_roundtrip_and_validation() {
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today() + 10 * HOUR;

        let s = d.study_create("Estudiar cálculo", t, t + 2 * HOUR, None, "integrales");
        let s = s.unwrap();
        assert_eq!(s.title, "Estudiar cálculo");
        assert_eq!(s.task_id, None);
        assert_eq!(d.study_list().unwrap().len(), 1);

        // editar (mismo camino de validación)
        let u = d
            .study_update(s.id, "Estudiar integrales", t, t + 3 * HOUR, None, "cap. 4")
            .unwrap();
        assert_eq!(u.title, "Estudiar integrales");
        assert_eq!(u.end_at, t + 3 * HOUR);

        // validaciones: título, duración > 0 y fecha
        assert!(d.study_create("   ", t, t + HOUR, None, "").is_err());
        assert!(d.study_create("X", t, t, None, "").is_err()); // duración 0
        assert!(d.study_create("X", t + HOUR, t, None, "").is_err()); // fin < inicio
        assert!(d.study_create("X", 0, HOUR, None, "").is_err()); // fecha inválida

        // borrar
        assert!(d.study_delete(s.id).unwrap());
        assert!(!d.study_delete(s.id).unwrap());
        assert!(d.study_list().unwrap().is_empty());
    }

    #[test]
    fn study_links_to_task_without_becoming_a_task() {
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today() + 9 * HOUR;
        let task = d
            .create(
                "Preparar parcial de cálculo",
                "uni",
                "alta",
                t,
                t + HOUR,
                false,
            )
            .unwrap();

        let s = d
            .study_create("Estudiar integrales", t, t + 2 * HOUR, Some(task.id), "")
            .unwrap();
        assert_eq!(s.task_id, Some(task.id));

        // la tarea sigue existiendo por sí sola: la sesión NO duplica pendientes
        assert_eq!(d.list().unwrap().len(), 1);
        assert_eq!(d.list().unwrap()[0].title, "Preparar parcial de cálculo");

        // vínculo inválido → error legible
        assert!(d
            .study_create("Fantasma", t, t + HOUR, Some(task.id + 999), "")
            .is_err());
    }

    #[test]
    fn studies_are_neither_pending_nor_counted() {
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today() + 11 * HOUR;
        let s = d.study_create("Sesión", t, t + HOUR, None, "").unwrap();
        // sin estado de completado y fuera de la lista de tareas/pendientes
        assert!(d.list().unwrap().is_empty(), "una sesión no es una tarea");
        assert_eq!(
            d.count().unwrap(),
            0,
            "no cuenta en los contadores de tareas"
        );
        // sigue existiendo y se puede mover (movible y editable)
        let m = d.study_move(s.id, t + HOUR, t + 3 * HOUR).unwrap();
        assert_eq!(m.start_at, t + HOUR);
        assert!(d.study_move(s.id, t + HOUR, t + HOUR).is_err());
    }

    #[test]
    fn studies_reserve_time_in_planner() {
        // reglas 12–15: el planificador trata las sesiones como TIEMPO YA
        // RESERVADO (bloque duro), así nunca propone una tarea encima.
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today();
        let start = t + 14 * HOUR;
        let s = d
            .study_create(
                "Sesión de estudio: Cálculo",
                start,
                start + 2 * HOUR,
                None,
                "",
            )
            .unwrap();
        let e = engine_with_calendar(&d);
        let has = e.blocks.iter().any(|b| {
            b.severity == crate::engine::Severity::Hard
                && b.label == s.title
                && b.interval.start == start
                && b.interval.end == start + 2 * HOUR
        });
        assert!(has, "la sesión reservada entra como bloque duro del motor");
        // y sigue sin ser una tarea pendiente
        assert!(d.list().unwrap().is_empty());
    }

    #[test]
    fn conflicts_detect_classes_and_tasks_but_never_block() {
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today();
        let start = t + 10 * HOUR;
        // clase activa hoy 10:00–12:00 (dow de hoy, vigencia amplia)
        let dow = chrono::Local
            .timestamp_millis_opt(start)
            .unwrap()
            .weekday()
            .num_days_from_monday() as i64;
        d.class_create("Programación", dow, 600, 720, t - DAY, t + 28 * DAY)
            .unwrap();
        d.create(
            "Informe de redes",
            "uni",
            "media",
            t + 11 * HOUR,
            t + 12 * HOUR,
            false,
        )
        .unwrap();

        // sesión 11:00–13:00: solapa con la clase y con la tarea
        let s_start = t + 11 * HOUR;
        let s_end = t + 13 * HOUR;
        let classes = super::super::classes::class_instances_in_range(
            &d.class_list().unwrap(),
            s_start,
            s_end,
        )
        .into_iter()
        .filter(|i| i.start_at < s_end && i.end_at > s_start)
        .count();
        assert_eq!(classes, 1, "detecta el solape con la clase activa");
        let task_hits = d
            .list()
            .unwrap()
            .into_iter()
            .filter(|x| x.start_at < s_end && x.end_at > s_start)
            .count();
        assert_eq!(task_hits, 1, "detecta el solape con la tarea");

        // aun con conflicto, la sesión se crea y convive (el sistema no mueve nada)
        let s = d.study_create("Sesión", s_start, s_end, None, "").unwrap();
        assert_eq!(d.study_list().unwrap().len(), 1);
        assert_eq!(d.study_get(s.id).unwrap().unwrap().start_at, s_start);
    }
}
