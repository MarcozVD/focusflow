//! Comandos del dominio "Horario" (clases semanales con vigencia).
//!
//! Las clases son una entidad INDEPENDIENTE de las tareas: bloques de tiempo
//! fijo en los que el usuario está ocupado. No tienen estado, no cuentan como
//! pendientes ni afectan estadísticas; el planificador sí las consulta como
//! tiempo ocupado (ver `planning::engine_with_calendar`).

use super::with_db;
use crate::append_log;
use crate::store::{lock_recover, ClassRow, Db};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// Resultado de crear/actualizar: `class_conflicts` devuelve la lista de
/// clases activas que se cruzan con la franja (para avisos futuros en UI).
#[derive(serde::Serialize)]
pub struct ClassOpResult {
    pub class: ClassRow,
}

fn emit_changed(app: &AppHandle) {
    let _ = app.emit("classes:changed", ());
}

#[tauri::command]
pub fn class_list(state: State<'_, Mutex<Db>>) -> Result<Vec<ClassRow>, String> {
    with_db(&state, |db| db.class_list().map_err(|e| e.to_string()))
}

/// Instancias concretas de clases activas que tocan el rango [start_at,
/// end_at] en ms ÉPOCA (local). El frontend las usa para pintar la semana/el
/// día y para detectar conflictos. Cada fila trae `date_ms` (medianoche local
/// del día) más los datos de la clase; `start_at`/`end_at` en ms absolutos.
#[tauri::command]
pub fn class_instances(
    state: State<'_, Mutex<Db>>,
    start_at: i64,
    end_at: i64,
) -> Result<Vec<ClassInstance>, String> {
    let db = lock_recover(&state);
    let classes = db.class_list().map_err(|e| e.to_string())?;
    Ok(class_instances_in_range(&classes, start_at, end_at))
}

/// Instancia concreta de una clase en una fecha (serializada al frontend).
#[derive(serde::Serialize, Clone, Debug)]
pub struct ClassInstance {
    pub class_id: i64,
    pub title: String,
    pub day_of_week: i64,
    /// Medianoche local del día en que cae la instancia.
    pub date_ms: i64,
    pub start_at: i64,
    pub end_at: i64,
}

/// Dado el catálogo de clases, materializa las instancias activas cuya fecha
/// (medianoche local) cae en [range_start, range_end] (ambos inclusivos).
///
/// Vigencia (regla 30): una clase produce instancias solo si el día consultado
/// cumple `start_date <= fecha <= end_date` (días ÉPOCA locales, inclusivos).
pub fn class_instances_in_range(
    classes: &[ClassRow],
    range_start: i64,
    range_end: i64,
) -> Vec<ClassInstance> {
    use chrono::{Datelike, Local, NaiveDate, TimeZone};
    // Medianoche de una fecha de calendario en la zona local. Si un cambio
    // de horario de verano hiciera inexistente esa hora, toma el primer
    // instante del día.
    fn ms_of(d: NaiveDate) -> i64 {
        d.and_hms_opt(0, 0, 0)
            .and_then(|naive| {
                Local
                    .from_local_datetime(&naive)
                    .single()
                    .or_else(|| Local.from_local_datetime(&naive).earliest())
            })
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0)
    }
    // Hora de PARED `min` minutos tras la medianoche de `d` (DST): sumar
    // min*60_000 a la medianoche desplazaba 1 h las clases del día del
    // cambio de horario. 1440 = medianoche del día siguiente.
    fn wall_ms(d: NaiveDate, min: i64) -> i64 {
        let (d, min) = if min >= 1440 {
            (d.succ_opt().unwrap_or(d), min - 1440)
        } else {
            (d, min)
        };
        d.and_hms_opt((min / 60) as u32, (min % 60) as u32, 0)
            .and_then(|naive| Local.from_local_datetime(&naive).earliest())
            .map(|dt| dt.timestamp_millis())
            // hora inexistente (salto de primavera): medianoche + min
            .unwrap_or_else(|| ms_of(d) + min * 60_000)
    }
    let midnight = crate::engine::local_midnight;
    let date_of = |ms: i64| {
        Local
            .timestamp_millis_opt(ms)
            .single()
            .or_else(|| Local.timestamp_millis_opt(ms).earliest())
            .map(|dt| dt.date_naive())
    };
    let mut out = Vec::new();
    // recorre por días: el rango visible es ≤ ~6 semanas; barato y exacto.
    // Se avanza por FECHAS de calendario (succ_opt), no sumando 24 h: así un
    // día de 23/25 h por DST no desalinea la rejilla (dow, vigencia, huecos).
    let Some(mut date) = date_of(midnight(range_start.min(range_end))) else {
        return out;
    };
    let Some(end_date) = date_of(midnight(range_end.max(range_start))) else {
        return out;
    };
    while date <= end_date {
        let day = ms_of(date);
        let dow = date.weekday().num_days_from_monday() as i64; // 0 = lunes
        for c in classes {
            if c.day_of_week != dow {
                continue;
            }
            if day < c.start_date || day > c.end_date {
                continue; // fuera de la vigencia (regla 21/23)
            }
            out.push(ClassInstance {
                class_id: c.id,
                title: c.title.clone(),
                day_of_week: c.day_of_week,
                date_ms: day,
                start_at: wall_ms(date, c.start_min),
                end_at: wall_ms(date, c.end_min),
            });
        }
        match date.succ_opt() {
            Some(next) => date = next,
            None => break,
        }
    }
    out.sort_by(|a, b| {
        a.start_at
            .cmp(&b.start_at)
            .then(a.class_id.cmp(&b.class_id))
    });
    out
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn class_create(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    title: String,
    day_of_week: i64,
    start_min: i64,
    end_min: i64,
    start_date: i64,
    end_date: i64,
) -> Result<ClassRow, String> {
    let row = with_db(&state, |db| {
        db.class_create(
            &title,
            day_of_week,
            start_min,
            end_min,
            start_date,
            end_date,
        )
        .map_err(crate::store::Db::rusqlite_error_text)
    })?;
    append_log(
        &app,
        &format!(
            "class_created id={} title={} dow={day_of_week} {start_min}-{end_min} vig={start_date}..{end_date}",
            row.id,
            row.title
        ),
    );
    emit_changed(&app);
    Ok(row)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn class_update(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
    title: String,
    day_of_week: i64,
    start_min: i64,
    end_min: i64,
    start_date: i64,
    end_date: i64,
) -> Result<ClassRow, String> {
    let row = with_db(&state, |db| {
        db.class_update(
            id,
            &title,
            day_of_week,
            start_min,
            end_min,
            start_date,
            end_date,
        )
        .map_err(crate::store::Db::rusqlite_error_text)
    })?;
    append_log(
        &app,
        &format!("class_updated id={id} title={title} dow={day_of_week} {start_min}-{end_min}"),
    );
    emit_changed(&app);
    Ok(row)
}

#[tauri::command]
pub fn class_delete(app: AppHandle, state: State<'_, Mutex<Db>>, id: i64) -> Result<(), String> {
    let deleted = with_db(&state, |db| db.class_delete(id).map_err(|e| e.to_string()))?;
    if !deleted {
        return Err("clase no encontrada".into());
    }
    append_log(&app, &format!("class_deleted id={id}"));
    emit_changed(&app);
    Ok(())
}

/// Clases activas (por vigencia y día) que se cruzan con la franja absoluta
/// [start_at, end_at] de una TAREA. La UI lo usa para el aviso
/// "Este horario coincide con una clase" (reglas 7 y 24).
#[tauri::command]
pub fn class_conflicts(
    state: State<'_, Mutex<Db>>,
    start_at: i64,
    end_at: i64,
) -> Result<Vec<ClassInstance>, String> {
    let db = lock_recover(&state);
    let classes = db.class_list().map_err(|e| e.to_string())?;
    // instancias del día(s) que toca la franja; filtra solape real
    Ok(class_instances_in_range(&classes, start_at, end_at)
        .into_iter()
        .filter(|i| i.start_at < end_at && i.end_at > start_at)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::local_midnight;
    use crate::planning::engine_with_calendar;
    use chrono::TimeZone;

    const DAY: i64 = 86_400_000;
    /// Medianoche local de "hoy" (referencia para construir fechas de prueba).
    fn today() -> i64 {
        local_midnight(crate::email::now_ms())
    }

    /// Próximo lunes (medianoche local) relativo a `t`.
    fn next_monday(t: i64) -> i64 {
        let dow_now = chrono::Local
            .timestamp_millis_opt(t)
            .single()
            .map(|dt| dt.format("%u").to_string().parse::<i64>().unwrap_or(1) - 1)
            .unwrap_or(0);
        t + (7 - dow_now) % 7 * DAY
    }

    #[test]
    fn class_instance_uses_wall_clock_time() {
        // Lote B #15: la instancia se construye con la hora de pared local
        // (misma salida que medianoche + min en un día normal)
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today();
        let mon = next_monday(t);
        d.class_create("Física", 0, 540, 1440, mon - DAY, mon + 7 * DAY)
            .unwrap();
        let inst = class_instances_in_range(&d.class_list().unwrap(), mon, mon + DAY - 1);
        assert_eq!(inst.len(), 1);
        let expected = chrono::Local
            .timestamp_millis_opt(mon)
            .unwrap()
            .date_naive()
            .and_hms_opt(9, 0, 0)
            .unwrap();
        let expected = chrono::Local
            .from_local_datetime(&expected)
            .earliest()
            .unwrap()
            .timestamp_millis();
        assert_eq!(inst[0].start_at, expected, "09:00 de pared");
        assert_eq!(inst[0].end_at, local_midnight(mon + DAY + 3_600_000), "fin 24:00 = medianoche siguiente");
    }

    #[test]
    fn class_crud_roundtrip_and_validation() {
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today();
        // crear
        let c = d
            .class_create("Cálculo", 1, 540, 660, t, t + 30 * DAY)
            .unwrap();
        assert_eq!(c.title, "Cálculo");
        assert_eq!(d.class_list().unwrap().len(), 1);
        // editar (reutiliza validación)
        let u = d
            .class_update(c.id, "Cálculo II", 1, 600, 720, t, t + 60 * DAY)
            .unwrap();
        assert_eq!(u.title, "Cálculo II");
        assert_eq!(u.end_min, 720);
        // validaciones (regla 3 y 28)
        assert!(d.class_create("", 1, 540, 660, t, t).is_err());
        assert!(d.class_create("X", 1, 660, 540, t, t).is_err()); // fin <= inicio
        assert!(d.class_create("X", 8, 540, 660, t, t).is_err()); // día inválido
        assert!(d.class_create("X", 1, 540, 660, t + DAY, t).is_err()); // fin < inicio
        assert!(d.class_create("X", 1, -5, 60, t, t).is_err()); // hora fuera de rango
                                                                // borrar
        assert!(d.class_delete(c.id).unwrap());
        assert!(!d.class_delete(c.id).unwrap()); // ya no existe
        assert!(d.class_list().unwrap().is_empty());
    }

    #[test]
    fn instances_respect_weekday_and_vigency() {
        let t = today();
        // buscar el próximo lunes real como ancla de fecha
        let monday = next_monday(t);
        let d = Db::open_memory_clean_pub().unwrap();
        // clase de lunes vigente solo la semana del `monday`
        d.class_create("Semana A", 0, 600, 720, monday, monday + 6 * DAY)
            .unwrap();
        let inside = class_instances_in_range(&d.class_list().unwrap(), monday, monday + 6 * DAY);
        assert_eq!(inside.len(), 1, "dentro de la vigencia se materializa");
        assert_eq!(inside[0].start_at, monday + 600 * 60_000);
        // fuera (semana siguiente) → nada (regla 21/22)
        let after = class_instances_in_range(
            &d.class_list().unwrap(),
            monday + 7 * DAY,
            monday + 13 * DAY,
        );
        assert!(after.is_empty(), "clase vencida no aparece");
        // antes del inicio tampoco
        let before =
            class_instances_in_range(&d.class_list().unwrap(), monday - 7 * DAY, monday - 1 * DAY);
        assert!(
            before.is_empty(),
            "clase futura no aparece antes de empezar"
        );
    }

    #[test]
    fn conflicts_only_with_active_classes() {
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today();
        let monday = next_monday(t);
        let c = d
            .class_create("Programación", 0, 600, 720, monday, monday + 6 * DAY)
            .unwrap();
        // tarea 11:00–12:00 de ese lunes → conflicto (regla 7)
        let task_start = monday + 660 * 60_000;
        let task_end = monday + 720 * 60_000;
        let conf = class_instances_in_range(&d.class_list().unwrap(), task_start, task_end)
            .into_iter()
            .filter(|i| i.start_at < task_end && i.end_at > task_start)
            .collect::<Vec<_>>();
        assert_eq!(conf.len(), 1);
        assert_eq!(conf[0].class_id, c.id);
        // misma hora una semana después de la vigencia → sin conflicto (regla 24)
        let late_start = task_start + 7 * DAY;
        let conf2 = class_instances_in_range(
            &d.class_list().unwrap(),
            late_start,
            late_start + 60 * 60_000,
        );
        assert!(conf2.is_empty());
    }

    #[test]
    fn planner_hard_blocks_avoid_class_hours() {
        // regla 9/12/13: el planificador trata clases activas como ocupado
        let d = Db::open_memory_clean_pub().unwrap();
        let t = today();
        let monday = next_monday(t);
        // clase lunes 10:00–12:00 vigente ahora
        d.class_create("Matemáticas", 0, 600, 720, t - DAY, monday + 28 * DAY)
            .unwrap();
        let e = engine_with_calendar(&d);
        // hay al menos un bloque duro con la franja de la clase
        let has = e.blocks.iter().any(|b| {
            b.severity == crate::engine::Severity::Hard
                && b.label == "Matemáticas"
                && b.interval.start == monday + 600 * 60_000
                && b.interval.end == monday + 720 * 60_000
        });
        assert!(has, "la clase vigente entra como bloque duro del motor");
        // y la clase NO es tarea: no aparece en list()
        assert!(d.list().unwrap().is_empty(), "las clases no son tareas");
    }
}
