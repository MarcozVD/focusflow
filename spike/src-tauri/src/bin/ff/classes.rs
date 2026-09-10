//! `ff clases` — horario de universidad desde el cmd.
//!
//! Mismo patrón que las tareas: abre la MISMA BD que la app y, tras cada
//! mutación, deja `cli-change.flag` para que la app abierta refresque el
//! horario en vivo. Valida con las mismas reglas que `store::validate_class`
//! (que es `pub(crate)` en el lib; aquí se replican los mensajes).

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use serde_json::json;

use focusflow_spike_lib::commands::classes::class_instances_in_range;
use focusflow_spike_lib::store::ClassRow;

use super::{data_dir, fail, now_ms, open_db, touch_flag, USAGE};

const DIAS: [&str; 7] = ["lun", "mar", "mie", "jue", "vie", "sab", "dom"];

fn hhmm(min: i64) -> String {
    format!("{:02}:{:02}", min / 60, min % 60)
}

fn fmt_vig(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|| ms.to_string())
}

/// "lunes"|"lun"|"mon"… → 0..=6 (lunes primero). Acepta acentos y mayúsculas.
fn parse_dow(s: &str) -> Option<i64> {
    let t = s
        .to_lowercase()
        .replace('é', "e")
        .replace('á', "a")
        .replace('í', "i")
        .replace('ó', "o")
        .replace('ú', "u");
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    let full = [
        "lunes",
        "martes",
        "miercoles",
        "jueves",
        "viernes",
        "sabado",
        "domingo",
    ];
    let en = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    for (i, k) in DIAS.iter().enumerate() {
        if t == *k || t == full[i] || t == en[i] {
            return Some(i as i64);
        }
    }
    None
}

/// "HH:MM" → minutos desde medianoche (24:00 válido solo como fin).
fn parse_min(s: &str, allow_2400: bool) -> Option<i64> {
    let (h, m) = s.split_once(':')?;
    let h: i64 = h.parse().ok()?;
    let m: i64 = m.parse().ok()?;
    let v = h * 60 + m;
    let max = if allow_2400 { 1440 } else { 1439 };
    (m <= 59 && (0..=max).contains(&v)).then_some(v)
}

/// "YYYY-MM-DD" → medianoche local en ms (vigencia igual que la UI).
fn parse_date_ms(s: &str) -> Option<i64> {
    let d = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    day_ms(d)
}

fn day_ms(d: NaiveDate) -> Option<i64> {
    Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .earliest()
        .map(|dt| dt.timestamp_millis())
}

fn json_class(c: &ClassRow) -> serde_json::Value {
    json!({
        "id": c.id,
        "title": c.title,
        "day_of_week": c.day_of_week,
        "start_min": c.start_min,
        "end_min": c.end_min,
        "start_date": c.start_date,
        "end_date": c.end_date,
        "created_at": c.created_at,
        "updated_at": c.updated_at,
    })
}

fn print_class(c: &ClassRow) {
    println!(
        "#{} {} [{}] {}–{} vigente {}..{}",
        c.id,
        c.title,
        DIAS[(c.day_of_week % 7) as usize],
        hhmm(c.start_min),
        hhmm(c.end_min),
        fmt_vig(c.start_date),
        fmt_vig(c.end_date),
    );
}

pub fn cmd_clases(args: Vec<String>, json_out: bool) -> i32 {
    let mut it = args.into_iter();
    let sub = it.next().unwrap_or_default();
    match sub.as_str() {
        "list" => class_list(json_out),
        "add" => class_add(it.collect(), json_out),
        "rm" => class_rm(it.collect(), json_out),
        "week" => class_week(it.collect(), json_out),
        _ => {
            eprintln!("uso: ff clases <list|add|rm|week> […]\n\n{USAGE}");
            2
        }
    }
}

fn class_list(json_out: bool) -> i32 {
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let classes = match db.class_list() {
        Ok(v) => v,
        Err(e) => return fail(&format!("error listando clases: {e}")),
    };
    if json_out {
        let arr: Vec<_> = classes.iter().map(json_class).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else if classes.is_empty() {
        println!(
            "sin clases — ejemplo: ff clases add \"Cálculo\" --day lun --from 08:00 --to 09:30"
        );
    } else {
        for c in &classes {
            print_class(c);
        }
    }
    0
}

fn class_add(args: Vec<String>, json_out: bool) -> i32 {
    let mut title = String::new();
    let mut dow: Option<i64> = None;
    let mut from: Option<i64> = None;
    let mut to: Option<i64> = None;
    let mut desde: Option<i64> = None;
    let mut hasta: Option<i64> = None;

    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let need = |flag: &str, it: &mut std::vec::IntoIter<String>| -> Option<String> {
            let v = it.next();
            if v.is_none() {
                eprintln!("flag {flag} necesita un valor");
            }
            v
        };
        match a.as_str() {
            "--day" => {
                let Some(v) = need("--day", &mut it) else {
                    return 1;
                };
                match parse_dow(&v) {
                    Some(d) => dow = Some(d),
                    None => {
                        return fail(&format!(
                            "día inválido: {v} (usa lun|mar|mie|jue|vie|sab|dom)"
                        ))
                    }
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
            "--desde" | "--hasta" => {
                let Some(v) = need(&a, &mut it) else {
                    return 1;
                };
                let Some(ms) = parse_date_ms(&v) else {
                    return fail(&format!("{a} inválida: {v} (usa YYYY-MM-DD)"));
                };
                if a == "--desde" {
                    desde = Some(ms);
                } else {
                    hasta = Some(ms);
                }
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
        return fail("falta el nombre de la clase: ff clases add \"Cálculo\" --day lun --from 08:00 --to 09:30");
    }
    let Some(dow) = dow else {
        return fail("falta --day (lun|mar|mie|jue|vie|sab|dom)");
    };
    let (Some(from), Some(to)) = (from, to) else {
        return fail("faltan --from/--to (HH:MM)");
    };
    if to <= from {
        return fail("la hora de fin debe ser posterior a la de inicio");
    }
    // Vigencia por defecto: hoy → hoy+12 semanas (~cuatrimestre), como el formulario.
    let today = Local::now().date_naive();
    let start_date = desde.unwrap_or_else(|| day_ms(today).unwrap_or(0));
    let end_date =
        hasta.unwrap_or_else(|| day_ms(today + chrono::Duration::weeks(12)).unwrap_or(start_date));
    if end_date < start_date {
        return fail("--hasta es anterior a --desde");
    }

    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let row = match db.class_create(&title, dow, from, to, start_date, end_date) {
        Ok(r) => r,
        Err(e) => return fail(&format!("error creando clase: {e}")),
    };
    touch_flag(&data_dir());
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&json_class(&row)).unwrap_or_default()
        );
    } else {
        print!("✓ ");
        print_class(&row);
    }
    0
}

fn class_rm(args: Vec<String>, json_out: bool) -> i32 {
    let Some(id) = args.first().and_then(|s| s.parse::<i64>().ok()) else {
        return fail("uso: ff clases rm <id>");
    };
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    match db.class_delete(id) {
        Ok(true) => {
            touch_flag(&data_dir());
            if json_out {
                println!("{{\"id\": {id}, \"deleted\": true}}");
            } else {
                println!("clase eliminada #{id}");
            }
            0
        }
        Ok(false) => fail(&format!("no existe la clase #{id}")),
        Err(e) => fail(&format!("error: {e}")),
    }
}

/// Instancias reales de la semana (lun-dom) que contiene `--semana YYYY-MM-DD`
/// o la actual: mismo materializador `class_instances_in_range` que la app.
fn class_week(args: Vec<String>, json_out: bool) -> i32 {
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
    let classes = match db.class_list() {
        Ok(v) => v,
        Err(e) => return fail(&format!("error: {e}")),
    };
    let base_ms = anchor.unwrap_or_else(now_ms);
    let Some(monday) = Local
        .timestamp_millis_opt(base_ms)
        .earliest()
        .map(|dt| dt.date_naive())
        .and_then(|d| {
            day_ms(d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64))
        })
    else {
        return fail("fecha fuera de rango");
    };
    let insts = class_instances_in_range(&classes, monday, monday + 7 * 86_400_000 - 1);
    if json_out {
        let arr: Vec<_> = insts
            .iter()
            .map(|i| {
                json!({
                    "class_id": i.class_id,
                    "title": i.title,
                    "day_of_week": i.day_of_week,
                    "date_ms": i.date_ms,
                    "start_at": i.start_at,
                    "end_at": i.end_at,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return 0;
    }
    if insts.is_empty() {
        println!("sin clases esa semana");
        return 0;
    }
    for i in &insts {
        let dow = Local
            .timestamp_millis_opt(i.start_at)
            .earliest()
            .map(|d| d.format("%a %d %b").to_string())
            .unwrap_or_default();
        println!(
            "{dow} {}–{} {}",
            hhmm((i.start_at - i.date_ms) / 60_000),
            hhmm((i.end_at - i.date_ms) / 60_000),
            i.title
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dow_acepta_varios_formatos() {
        assert_eq!(parse_dow("lunes"), Some(0));
        assert_eq!(parse_dow("LUN"), Some(0));
        assert_eq!(parse_dow("miércoles"), Some(2));
        assert_eq!(parse_dow("mie"), Some(2));
        assert_eq!(parse_dow("fri"), Some(4));
        assert_eq!(parse_dow("domingo"), Some(6));
        assert_eq!(parse_dow("lunr"), None);
        assert_eq!(parse_dow(""), None);
    }

    #[test]
    fn minutos_rango() {
        assert_eq!(parse_min("08:00", false), Some(480));
        assert_eq!(parse_min("23:59", false), Some(1439));
        assert_eq!(parse_min("24:00", false), None);
        assert_eq!(parse_min("24:00", true), Some(1440));
        assert_eq!(parse_min("9:5", false), Some(545));
        assert_eq!(parse_min("08:60", false), None);
    }

    #[test]
    fn fecha_a_ms_medianoche_local() {
        let ms = parse_date_ms("2026-09-14").unwrap();
        let d = Local.timestamp_millis_opt(ms).earliest().unwrap();
        assert_eq!(d.format("%Y-%m-%d %H:%M").to_string(), "2026-09-14 00:00");
        assert!(parse_date_ms("14/09/2026").is_none());
    }
}
