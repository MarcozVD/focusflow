//! `ff sesiones` — sesiones de estudio desde el cmd.
//!
//! Mismo patrón que `ff clases`: abre la MISMA BD que la app (`store::Db`) y,
//! tras cada mutación, deja `cli-change.flag` para que la app abierta emita
//! `study:changed` y refresque calendario/vista en vivo. Las sesiones NO son
//! tareas: son bloques de tiempo reservado (título + ventana + tarea opcional
//! + notas); aquí se crean/listan/mueven/borran con la misma validación que
//! la UI (`store::validate_study`).

use chrono::{Datelike, Local, TimeZone};
use serde_json::json;

use focusflow_spike_lib::store::StudyRow;

use super::{data_dir, fail, now_ms, open_db, touch_flag, USAGE};

fn hhmm(ms_of_day: i64) -> String {
    let min = ms_of_day / 60_000;
    format!("{:02}:{:02}", min / 60, min % 60)
}

/// HH:MM LOCAL de un instante (ms ÉPOCA): igual que `fmt_time` en ff.rs.
fn hhmm_at(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.format("%H:%M").to_string())
        .unwrap_or_else(|| hhmm(ms % 86_400_000))
}

/// "HH:MM" → minutos desde medianoche. `24:00` válido solo como fin.
fn parse_min(s: &str, allow_2400: bool) -> Option<i64> {
    let (h, m) = s.split_once(':')?;
    let h: i64 = h.parse().ok()?;
    let m: i64 = m.parse().ok()?;
    let v = h * 60 + m;
    let max = if allow_2400 { 1440 } else { 1439 };
    (m <= 59 && (0..=max).contains(&v)).then_some(v)
}

/// "YYYY-MM-DD" → medianoche local en ms.
fn parse_date_ms(s: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .earliest()
        .map(|dt| dt.timestamp_millis())
}

/// Lunes (medianoche local) de la semana que contiene `ms`.
fn monday_of(ms: i64) -> Option<i64> {
    let d = Local.timestamp_millis_opt(ms).earliest()?.date_naive();
    let monday = d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64);
    Local
        .from_local_datetime(&monday.and_hms_opt(0, 0, 0)?)
        .earliest()
        .map(|dt| dt.timestamp_millis())
}

fn day_label(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.format("%a %d %b").to_string())
        .unwrap_or_else(|| ms.to_string())
}

fn json_study(s: &StudyRow) -> serde_json::Value {
    json!({
        "id": s.id,
        "title": s.title,
        "start_at": s.start_at,
        "end_at": s.end_at,
        "task_id": s.task_id,
        "notes": s.notes,
        "created_at": s.created_at,
        "updated_at": s.updated_at,
    })
}

fn print_study(s: &StudyRow, task_title: Option<&str>) {
    let linked = task_title.map(|t| format!(" → «{t}»")).unwrap_or_default();
    let notes = if s.notes.is_empty() {
        String::new()
    } else {
        format!("  # {}", s.notes)
    };
    println!(
        "#{} {} {} {}–{}{}{}",
        s.id,
        s.title,
        day_label(s.start_at),
        hhmm_at(s.start_at),
        hhmm_at(s.end_at),
        linked,
        notes,
    );
}

/// Título de tarea relacionada (para el listado legible).
fn task_title_of(db: &focusflow_spike_lib::store::Db, id: Option<i64>) -> Option<String> {
    let id = id?;
    let t = db.get_task(id).ok()??;
    Some(t.title)
}

pub fn cmd_sesiones(args: Vec<String>, json_out: bool) -> i32 {
    let mut it = args.into_iter();
    let sub = it.next().unwrap_or_default();
    match sub.as_str() {
        "list" => sesion_list(it.collect(), json_out),
        "add" => sesion_add(it.collect(), json_out),
        "move" => sesion_move(it.collect(), json_out),
        "edit" => sesion_edit(it.collect(), json_out),
        "rm" => sesion_rm(it.collect(), json_out),
        "week" => sesion_week(it.collect(), json_out),
        _ => {
            eprintln!("uso: ff sesiones <list|add|move|edit|rm|week> […]\n\n{USAGE}");
            2
        }
    }
}

fn sesion_list(args: Vec<String>, json_out: bool) -> i32 {
    let all = args.iter().any(|a| a == "--all");
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let now = now_ms();
    let horizon = now + 7 * 86_400_000;
    let mut sessions = match db.study_list() {
        Ok(v) => v,
        Err(e) => return fail(&format!("error listando sesiones: {e}")),
    };
    sessions.retain(|s| all || s.end_at >= now && s.start_at <= horizon);
    if json_out {
        let arr: Vec<_> = sessions.iter().map(json_study).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return 0;
    }
    if sessions.is_empty() {
        println!(
            "sin sesiones próximas — ejemplo: ff sesiones add \"Estudiar cálculo\" --fecha {} --from 10:00 --to 12:00",
            Local::now().format("%Y-%m-%d")
        );
        return 0;
    }
    for s in &sessions {
        print_study(s, task_title_of(&db, s.task_id).as_deref());
    }
    0
}

fn sesion_add(args: Vec<String>, json_out: bool) -> i32 {
    let mut title = String::new();
    let mut fecha: Option<i64> = None;
    let mut from: Option<i64> = None;
    let mut to: Option<i64> = None;
    let mut dur: Option<i64> = None;
    let mut task_id: Option<i64> = None;
    let mut notes = String::new();

    let mut it = args.into_iter();
    let need = |flag: &str, it: &mut std::vec::IntoIter<String>| -> Option<String> {
        let v = it.next();
        if v.is_none() {
            eprintln!("flag {flag} necesita un valor");
        }
        v
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--fecha" => {
                let Some(v) = need("--fecha", &mut it) else {
                    return 1;
                };
                match parse_date_ms(&v) {
                    Some(ms) => fecha = Some(ms),
                    None => return fail(&format!("--fecha inválida: {v} (usa YYYY-MM-DD)")),
                }
            }
            "--from" => {
                let Some(v) = need("--from", &mut it) else {
                    return 1;
                };
                match parse_min(&v, false) {
                    Some(m) => from = Some(m),
                    None => return fail(&format!("--from inválido: {v} (HH:MM)")),
                }
            }
            "--to" => {
                let Some(v) = need("--to", &mut it) else {
                    return 1;
                };
                match parse_min(&v, true) {
                    Some(m) => to = Some(m),
                    None => return fail(&format!("--to inválido: {v} (HH:MM, permite 24:00)")),
                }
            }
            "--dur" => match it.next().and_then(|v| v.parse::<i64>().ok()) {
                Some(v) if v > 0 => dur = Some(v),
                _ => return fail("--dur necesita minutos (número > 0)"),
            },
            "--task" => match it.next().and_then(|v| v.parse::<i64>().ok()) {
                Some(v) => task_id = Some(v),
                None => return fail("--task necesita el id de una tarea (ff list)"),
            },
            "--notas" => {
                let Some(v) = need("--notas", &mut it) else {
                    return 1;
                };
                if !notes.is_empty() {
                    notes.push(' ');
                }
                notes.push_str(&v);
            }
            _ => {
                if !title.is_empty() {
                    title.push(' ');
                }
                title.push_str(&a);
            }
        }
    }

    if title.trim().is_empty() {
        return fail("falta el título: ff sesiones add \"Estudiar cálculo\" --fecha 2026-09-11 --from 10:00 --to 12:00");
    }
    // Fecha por defecto: hoy.
    let fecha = fecha.unwrap_or_else(|| {
        let d = Local::now().date_naive();
        Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
            .earliest()
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0)
    });
    let (Some(from), Some(to_or_dur)) = (from, to.or(dur)) else {
        return fail("faltan --from + (--to | --dur)");
    };
    // --to es hora de fin; --dur es duración desde --from.
    let (start_at, end_at) = if to.is_some() {
        let end = fecha + to_or_dur * 60_000;
        if end <= fecha + from * 60_000 {
            return fail("la hora de fin debe ser posterior a la de inicio");
        }
        (fecha + from * 60_000, end)
    } else {
        let s = fecha + from * 60_000;
        (s, s + to_or_dur * 60_000)
    };

    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let row = match db.study_create(&title, start_at, end_at, task_id, &notes) {
        Ok(r) => r,
        Err(e) => {
            return fail(&format!(
                "error creando sesión: {}",
                focusflow_spike_lib::store::Db::rusqlite_error_text(e)
            ))
        }
    };
    touch_flag(&data_dir());
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_study(&row)).unwrap_or_default()
        );
    } else {
        print!("✓ ");
        print_study(&row, task_title_of(&db, row.task_id).as_deref());
    }
    0
}

fn sesion_move(args: Vec<String>, json_out: bool) -> i32 {
    let Some(id) = args.first().and_then(|s| s.parse::<i64>().ok()) else {
        return fail(
            "uso: ff sesiones move <id> <YYYY-MM-DD> [--from HH:MM] [--to HH:MM | --dur MIN]",
        );
    };
    let mut fecha: Option<i64> = None;
    let mut from: Option<i64> = None;
    let mut to: Option<i64> = None;
    let mut dur: Option<i64> = None;
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => match it.next().and_then(|v| parse_min(&v, false)) {
                Some(m) => from = Some(m),
                None => return fail("--from inválido (HH:MM)"),
            },
            "--to" => match it.next().and_then(|v| parse_min(&v, true)) {
                Some(m) => to = Some(m),
                None => return fail("--to inválido (HH:MM, permite 24:00)"),
            },
            "--dur" => match it.next().and_then(|v| v.parse::<i64>().ok()) {
                Some(v) if v > 0 => dur = Some(v),
                _ => return fail("--dur necesita minutos (número > 0)"),
            },
            other => match parse_date_ms(other) {
                Some(ms) => fecha = Some(ms),
                None => return fail(&format!("fecha inválida: {other} (usa YYYY-MM-DD)")),
            },
        }
    }
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let cur = match db.study_get(id) {
        Ok(Some(s)) => s,
        Ok(None) => return fail(&format!("no existe la sesión #{id}")),
        Err(e) => return fail(&format!("error: {e}")),
    };
    // Defaults: la ventana actual; conservar duración si solo se da fecha/from.
    let cur_day = cur.start_at - cur.start_at % 86_400_000;
    let day = fecha.unwrap_or(cur_day);
    let from_min = from.unwrap_or((cur.start_at - cur_day) / 60_000);
    let dur_min = dur
        .or_else(|| to.map(|t| t - from_min))
        .unwrap_or((cur.end_at - cur.start_at) / 60_000);
    if dur_min <= 0 {
        return fail("la duración resultante debe ser > 0");
    }
    let start_at = day + from_min * 60_000;
    let end_at = start_at + dur_min * 60_000;
    let row = match db.study_move(id, start_at, end_at) {
        Ok(r) => r,
        Err(e) => {
            return fail(&format!(
                "error moviendo: {}",
                focusflow_spike_lib::store::Db::rusqlite_error_text(e)
            ))
        }
    };
    touch_flag(&data_dir());
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_study(&row)).unwrap_or_default()
        );
    } else {
        print!("movida → ");
        print_study(&row, task_title_of(&db, row.task_id).as_deref());
    }
    0
}

fn sesion_edit(args: Vec<String>, json_out: bool) -> i32 {
    let Some(id) = args.first().and_then(|s| s.parse::<i64>().ok()) else {
        return fail("uso: ff sesiones edit <id> [--task <id>|--task none] [--notas \"…\"]");
    };
    let mut task_id: Option<i64> = None;
    let mut task_given = false;
    let mut notes: Option<String> = None;
    let mut it = args.into_iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--task" => {
                task_given = true;
                match it.next() {
                    Some(v) if v == "none" || v == "-" => task_id = None,
                    Some(v) => match v.parse::<i64>() {
                        Ok(t) => task_id = Some(t),
                        Err(_) => return fail(&format!("--task inválida: {v} (id o 'none')")),
                    },
                    None => return fail("--task necesita un id o 'none'"),
                }
            }
            "--notas" => match it.next() {
                Some(v) => notes = Some(v),
                None => return fail("--notas necesita un valor"),
            },
            other => return fail(&format!("argumento inesperado: {other}")),
        }
    }
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let cur = match db.study_get(id) {
        Ok(Some(s)) => s,
        Ok(None) => return fail(&format!("no existe la sesión #{id}")),
        Err(e) => return fail(&format!("error: {e}")),
    };
    let new_task = if task_given { task_id } else { cur.task_id };
    let new_notes = notes.unwrap_or_else(|| cur.notes.clone());
    let row = match db.study_update(
        id,
        &cur.title,
        cur.start_at,
        cur.end_at,
        new_task,
        &new_notes,
    ) {
        Ok(r) => r,
        Err(e) => {
            return fail(&format!(
                "error editando: {}",
                focusflow_spike_lib::store::Db::rusqlite_error_text(e)
            ))
        }
    };
    touch_flag(&data_dir());
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_study(&row)).unwrap_or_default()
        );
    } else {
        print!("editada → ");
        print_study(&row, task_title_of(&db, row.task_id).as_deref());
    }
    0
}

fn sesion_rm(args: Vec<String>, json_out: bool) -> i32 {
    let Some(id) = args.first().and_then(|s| s.parse::<i64>().ok()) else {
        return fail("uso: ff sesiones rm <id>");
    };
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    match db.study_delete(id) {
        Ok(true) => {
            touch_flag(&data_dir());
            if json_out {
                println!("{{\"id\": {id}, \"deleted\": true}}");
            } else {
                println!("sesión eliminada #{id}");
            }
            0
        }
        Ok(false) => fail(&format!("no existe la sesión #{id}")),
        Err(e) => fail(&format!("error: {e}")),
    }
}

/// Instancias de la semana (lun–dom) que contiene `--semana YYYY-MM-DD` o la
/// actual: mismo criterio de render que la vista Semana de la app.
fn sesion_week(args: Vec<String>, json_out: bool) -> i32 {
    let mut it = args.into_iter();
    let mut anchor: Option<i64> = None;
    while let Some(a) = it.next() {
        if a == "--semana" {
            match it.next().and_then(|v| parse_date_ms(&v)) {
                Some(v) => anchor = Some(v),
                None => return fail("--semana necesita YYYY-MM-DD"),
            }
        }
    }
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let base = anchor.unwrap_or_else(now_ms);
    let Some(monday) = monday_of(base) else {
        return fail("fecha fuera de rango");
    };
    let sessions = match db.study_list_range(monday, monday + 7 * 86_400_000 - 1) {
        Ok(v) => v,
        Err(e) => return fail(&format!("error: {e}")),
    };
    if json_out {
        let arr: Vec<_> = sessions.iter().map(json_study).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return 0;
    }
    if sessions.is_empty() {
        println!("sin sesiones esa semana");
        return 0;
    }
    for s in &sessions {
        print_study(s, task_title_of(&db, s.task_id).as_deref());
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutos_rango() {
        assert_eq!(parse_min("08:00", false), Some(480));
        assert_eq!(parse_min("24:00", true), Some(1440));
        assert_eq!(parse_min("24:00", false), None);
        assert_eq!(parse_min("08:60", false), None);
    }

    #[test]
    fn fecha_a_medianoche() {
        let ms = parse_date_ms("2026-09-14").unwrap();
        let d = Local.timestamp_millis_opt(ms).earliest().unwrap();
        assert_eq!(d.format("%Y-%m-%d %H:%M").to_string(), "2026-09-14 00:00");
        assert!(parse_date_ms("14/09/2026").is_none());
    }

    #[test]
    fn lunes_de_la_semana() {
        let ms = parse_date_ms("2026-09-16").unwrap(); // miércoles
        let monday = monday_of(ms).unwrap();
        let d = Local.timestamp_millis_opt(monday).earliest().unwrap();
        assert_eq!(d.weekday(), chrono::Weekday::Mon);
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-09-14");
    }
}
