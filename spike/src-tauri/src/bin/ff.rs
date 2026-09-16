//! `ff` — CLI de FocusFlow.
//!
//! Añade y gestiona tareas desde el cmd reutilizando el mismo crate: abre la
//! MISMA base SQLite que la app (`%APPDATA%\com.focusflow.spike`), usa el
//! parser de lenguaje natural local (`ai::nl::parse_task_nl`), los bloques
//! multi-día (`planning::split_range_blocks`) y recordatorios. Tras un cambio,
//! deja una marca (`cli-change.flag`) que el hilo de la app vigila para emitir
//! `tasks:changed` y refrescar app + widget en vivo.
//!
//! Override de tests: `FF_DATA_DIR=<dir>` usa otro directorio de datos.

use std::io::Write as _;
use std::path::PathBuf;

use chrono::TimeZone as _;

use focusflow_spike_lib::{ai, planning, reminders, store, store::Db};

#[path = "ff/classes.rs"]
mod classes;

#[path = "ff/sesiones.rs"]
mod sesiones;

fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FF_DATA_DIR") {
        return PathBuf::from(d);
    }
    // Misma ruta que la app: Tauri v2 app_data_dir() en Windows =
    // %APPDATA%\<identifier> (com.focusflow.spike).
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("com.focusflow.spike")
}

/// Marca de cambio para el hilo de la app: el CLI escribe `now_ms` dentro y
/// la app (que conoce el mismo data_dir) emite `tasks:changed`.
fn touch_flag(dir: &std::path::Path) {
    let f = dir.join("cli-change.flag");
    if let Ok(mut fh) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&f)
    {
        let _ = writeln!(fh, "{}", store::now_ms());
        let _ = fh.flush();
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn fmt_date(ms: i64) -> String {
    use chrono::Local;
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.format("%a %d %b %Y").to_string())
        .unwrap_or_else(|| ms.to_string())
}

fn fmt_time(ms: i64) -> String {
    use chrono::Local;
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.format("%H:%M").to_string())
        .unwrap_or_else(|| ms.to_string())
}

fn fmt_span(start: i64, end: i64, all_day: bool) -> String {
    if all_day {
        fmt_date(start)
    } else {
        format!(
            "{} {} (fin {})",
            fmt_date(start),
            fmt_time(start),
            fmt_time(end)
        )
    }
}

/// "2026-09-10" → medianoche local en ms (reutiliza local_ms del parser).
fn parse_date_ms(s: &str) -> Option<i64> {
    let d = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
    Some(ai::nl::local_ms(d.and_hms_opt(0, 0, 0)?))
}

/// "HH:MM" → minutos desde medianoche.
fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

fn json_task(t: &store::TaskRow) -> serde_json::Value {
    serde_json::json!({
        "id": t.id,
        "title": t.title,
        "category_id": t.category_id,
        "priority": t.priority,
        "status": t.status,
        "start_at": t.start_at,
        "end_at": t.end_at,
        "all_day": t.all_day,
        "completed_at": t.completed_at,
        "created_at": t.created_at,
        "reminder_minutes": t.reminder_minutes,
    })
}

fn open_db() -> Result<Db, String> {
    let dir = data_dir();
    Db::open(&dir).map_err(|e| format!("no se pudo abrir la BD en {}: {e}", dir.display()))
}

const CATS: &[&str] = &["uni", "trab", "per", "fin", "sal", "otr"];

const USAGE: &str = "\
ff — CLI de FocusFlow

USO
  ff add \"<texto libre>\" [--cat uni|trab|per|fin|sal|otr] [--prio alta|media|baja]
                          [--at HH:MM] [--dur MINUTOS] [--today] [--json]
      Texto libre con lenguaje natural: \"Examen mañana a las 5\",
      \"Pagar factura urgente el viernes\", \"Gym 2 horas pasado mañana\".
      --cat/--prio/--at/--dur sobreescriben lo que detecte el parser.
  ff list [--all] [--json]     próximas 7 días (o todas con --all)
  ff show <id>
  ff done <id>                 marca completada
  ff undo <id>                 vuelve a pendiente
  ff status <id> <pendiente|en-curso|completada>
  ff move <id> <YYYY-MM-DD> [HH:MM]   mueve la tarea a otra fecha/hora
  ff rm <id>                   borra
  ff clases list [--json]            asignaturas del horario
  ff clases add \"<nombre>\" --day lun|mar|mie|jue|vie|sab|dom
                   --from HH:MM --to HH:MM [--desde YYYY-MM-DD] [--hasta YYYY-MM-DD]
      Vigencia por defecto: hoy → hoy+12 semanas (~cuatrimestre).
  ff clases rm <id>            borra asignatura
  ff clases week [--semana YYYY-MM-DD]   horario real de esa semana
  ff sesiones list [--all] [--json]      sesiones de estudio próximas (7 días)
  ff sesiones add \"<título>\" [--fecha YYYY-MM-DD] --from HH:MM
                    (--to HH:MM | --dur MIN) [--task <id>] [--notas \"…\"]
      Bloque de tiempo RESERVADO para estudiar/trabajar (no es una tarea).
      --fecha por defecto: hoy.
  ff sesiones move <id> <YYYY-MM-DD> [--from HH:MM] [--to HH:MM | --dur MIN]
  ff sesiones edit <id> [--task <id>|none] [--notas \"…\"]
  ff sesiones rm <id>
  ff sesiones week [--semana YYYY-MM-DD] sesiones de esa semana
  ff cats                      categorías válidas
  ff ruta                      directorio de datos
Flags globales: --json (salida máquina en add/list/show/done/undo/status/move/rm)";

fn fail(msg: &str) -> i32 {
    eprintln!("{msg}");
    1
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(args));
}

fn run(args: Vec<String>) -> i32 {
    let json_out = args.iter().any(|a| a == "--json");
    let rest: Vec<String> = args.into_iter().filter(|a| a != "--json").collect();
    let mut it = rest.into_iter();
    let cmd = it.next().unwrap_or_default();
    let positional: Vec<String> = it.collect();

    match cmd.as_str() {
        "add" => cmd_add(positional, json_out),
        "list" => cmd_list(positional, json_out),
        "show" => cmd_show(positional, json_out),
        "done" => cmd_done(positional, true, json_out),
        "undo" => cmd_done(positional, false, json_out),
        "status" => cmd_status(positional, json_out),
        "move" => cmd_move(positional, json_out),
        "rm" => cmd_rm(positional, json_out),
        "clases" => classes::cmd_clases(positional, json_out),
        "sesiones" => sesiones::cmd_sesiones(positional, json_out),
        "cats" => cmd_cats(json_out),
        "ruta" => cmd_ruta(),
        "--help" | "-h" | "help" | "" => {
            println!("{USAGE}");
            0
        }
        other => {
            eprintln!("comando desconocido: {other}\n\n{USAGE}");
            2
        }
    }
}

fn cmd_add(args: Vec<String>, json_out: bool) -> i32 {
    let mut text = String::new();
    let mut cat: Option<String> = None;
    let mut prio: Option<String> = None;
    let mut at: Option<String> = None;
    let mut dur: Option<i64> = None;
    let mut today_flag = false;

    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--cat" => match it.next() {
                Some(v) => cat = Some(v),
                None => return fail("flag --cat necesita un valor"),
            },
            "--prio" => match it.next() {
                Some(v) => prio = Some(v),
                None => return fail("flag --prio necesita un valor"),
            },
            "--at" => match it.next() {
                Some(v) => at = Some(v),
                None => return fail("flag --at necesita HH:MM"),
            },
            "--dur" => match it.next().and_then(|v| v.parse::<i64>().ok()) {
                Some(v) => dur = Some(v),
                None => return fail("flag --dur necesita minutos (número)"),
            },
            "--today" => today_flag = true,
            _ => {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&a);
            }
        }
    }

    let text = text.trim();
    if text.is_empty() {
        return fail("falta el texto de la tarea: ff add \"comprar pan mañana\"");
    }
    if let Some(c) = &cat {
        if !CATS.contains(&c.as_str()) {
            return fail(&format!(
                "--cat inválida '{c}'. Válidas: {}",
                CATS.join(", ")
            ));
        }
    }
    if let Some(p) = &prio {
        if !matches!(p.as_str(), "alta" | "media" | "baja") {
            return fail("--prio debe ser alta|media|baja");
        }
    }
    if let Some(v) = &at {
        if parse_hhmm(v).is_none() {
            return fail("--at debe ser HH:MM (00:00–23:59)");
        }
    }
    if let Some(d) = dur {
        if !(1..=4 * 7 * 24 * 60).contains(&d) {
            return fail("--dur debe estar entre 1 y 40320 minutos (4 semanas)");
        }
    }

    // Parser NL local (mismo pipeline que la app, sin IA remota): título
    // limpio, categoría, prioridad, día y horario.
    let mut parsed = match ai::nl::parse_task_nl(text) {
        Some(p) => p,
        None => return fail("no se pudo interpretar el texto"),
    };
    // flags explícitos mandan sobre el parser
    if today_flag {
        let today = chrono::Local::now().date_naive();
        parsed.start_ms = ai::nl::local_ms(today.and_hms_opt(0, 0, 0).unwrap());
        parsed.end_ms = parsed.start_ms + 86_399_000;
        parsed.all_day = true;
    }
    if let Some(c) = cat {
        parsed.category_id = c;
    }
    if let Some(p) = prio {
        parsed.priority = p;
    }
    if let Some(v) = at.and_then(|v| parse_hhmm(&v)) {
        let day = ai::nl::day_start_ms(parsed.start_ms).unwrap_or(parsed.start_ms);
        parsed.start_ms = day + v as i64 * 60_000;
        if let Some(d) = dur {
            parsed.end_ms = parsed.start_ms + d * 60_000;
        } else if parsed.all_day || parsed.end_ms <= parsed.start_ms {
            parsed.end_ms = parsed.start_ms + 3_600_000;
        }
        parsed.all_day = false;
    } else if let Some(d) = dur {
        if !parsed.all_day {
            parsed.end_ms = parsed.start_ms + d * 60_000;
        }
    }

    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    // Rango multi-día → bloque inicio + bloque "(entrega)", igual que la app.
    let blocks = planning::split_range_blocks(&parsed.title, parsed.start_ms, parsed.end_ms);
    // transacción como task_from_text: un fallo a mitad no deja el rango a
    // medias en la BD compartida (bug L5)
    if let Err(e) = db.tx_begin() {
        return fail(&format!("error iniciando transacción: {e}"));
    }
    let mut created: Vec<store::TaskRow> = Vec::with_capacity(blocks.len());
    for (title, s, e, all_day) in &blocks {
        match db.create(
            title,
            &parsed.category_id,
            &parsed.priority,
            *s,
            *e,
            *all_day,
        ) {
            Ok(t) => created.push(t),
            Err(e) => {
                let _ = db.tx_rollback();
                return fail(&format!("error creando tarea: {e}"));
            }
        }
    }
    if let Some(min) = parsed
        .reminders
        .first()
        .and_then(|s| reminders::parse_reminder_minutes(s))
    {
        if let Some(t) = created.first() {
            if let Err(e) = db.set_task_reminder(t.id, min) {
                let _ = db.tx_rollback();
                return fail(&format!("error asignando recordatorio: {e}"));
            }
        }
    }
    if let Err(e) = db.tx_commit() {
        let _ = db.tx_rollback();
        return fail(&format!("error confirmando: {e}"));
    }
    touch_flag(&data_dir());

    if json_out {
        let arr: Vec<_> = created.iter().map(json_task).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        for t in &created {
            let rem = t
                .reminder_minutes
                .map(|m| format!(", recordatorio {m} min"))
                .unwrap_or_default();
            println!(
                "✓ #{} «{}» [{}] {} — {}{}",
                t.id,
                t.title,
                t.category_id,
                t.priority,
                fmt_span(t.start_at, t.end_at, t.all_day),
                rem
            );
        }
    }
    0
}

fn cmd_list(args: Vec<String>, json_out: bool) -> i32 {
    let all = args.iter().any(|a| a == "--all");
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    let tasks = match db.list() {
        Ok(v) => v,
        Err(e) => return fail(&format!("error listando: {e}")),
    };
    let now = now_ms();
    let horizon = now + 7 * 86_400_000;
    let mut v: Vec<store::TaskRow> = tasks
        .into_iter()
        .filter(|t| t.status != "completada")
        .filter(|t| {
            all || if t.all_day {
                t.end_at >= now && t.start_at <= horizon
            } else {
                t.end_at >= now
            }
        })
        .collect();
    v.sort_by_key(|t| (t.start_at, t.id));

    if json_out {
        let arr: Vec<_> = v.iter().map(json_task).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return 0;
    }
    if v.is_empty() {
        println!("sin tareas próximas");
        return 0;
    }
    for t in &v {
        println!(
            "#{} [{}] {} {} — {}",
            t.id,
            t.status,
            t.title,
            if t.all_day { "(todo el día)" } else { "" },
            fmt_span(t.start_at, t.end_at, t.all_day)
        );
    }
    0
}

fn parse_id(args: &[String]) -> Result<i64, String> {
    args.first()
        .ok_or_else(|| "falta <id>".to_string())?
        .parse::<i64>()
        .map_err(|_| format!("id inválido: {}", args[0]))
}

fn cmd_show(args: Vec<String>, json_out: bool) -> i32 {
    let id = match parse_id(&args) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    match db.get_task(id) {
        Ok(Some(t)) => {
            if json_out {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json_task(&t)).unwrap_or_default()
                );
            } else {
                println!(
                    "#{} «{}»\n  estado: {} | prioridad: {} | categoría: {}\n  cuándo: {}\n  recordatorio: {}\n  creada: {}",
                    t.id,
                    t.title,
                    t.status,
                    t.priority,
                    t.category_id,
                    fmt_span(t.start_at, t.end_at, t.all_day),
                    t.reminder_minutes
                        .map(|m| format!("{m} min antes"))
                        .unwrap_or_else(|| "—".into()),
                    fmt_date(t.created_at),
                );
                if !t.description.is_empty() {
                    println!("  descripción: {}", t.description);
                }
            }
            0
        }
        Ok(None) => fail(&format!("no existe la tarea #{id}")),
        Err(e) => fail(&format!("error: {e}")),
    }
}

fn cmd_done(args: Vec<String>, done: bool, json_out: bool) -> i32 {
    let id = match parse_id(&args) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    if let Err(e) = db.set_completed(id, done) {
        return fail(&format!("error: {e}"));
    }
    touch_flag(&data_dir());
    if json_out {
        match db.get_task(id) {
            Ok(Some(t)) => println!(
                "{}",
                serde_json::to_string_pretty(&json_task(&t)).unwrap_or_default()
            ),
            _ => println!("{{\"id\": {id}}}"),
        }
    } else {
        println!("{} #{}", if done { "completada" } else { "reabierta" }, id);
    }
    0
}

fn cmd_status(args: Vec<String>, json_out: bool) -> i32 {
    if args.len() < 2 {
        return fail("uso: ff status <id> <pendiente|en-curso|completada>");
    }
    let id = match args[0].parse::<i64>() {
        Ok(v) => v,
        Err(_) => return fail(&format!("id inválido: {}", args[0])),
    };
    let status = args[1].as_str();
    if !matches!(status, "pendiente" | "en-curso" | "completada") {
        return fail("estado inválido: usa pendiente | en-curso | completada");
    }
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    // done/undo sólo crean pendiente|completada; en-curso pasa por aquí.
    let res = match status {
        "completada" => db.set_completed(id, true),
        _ => db.set_task_status(id, status),
    };
    if let Err(e) = res {
        return fail(&format!("error: {e}"));
    }
    touch_flag(&data_dir());
    if json_out {
        match db.get_task(id) {
            Ok(Some(t)) => println!(
                "{}",
                serde_json::to_string_pretty(&json_task(&t)).unwrap_or_default()
            ),
            _ => println!("{{\"id\": {id}, \"status\": \"{status}\"}}"),
        }
    } else {
        println!("#{} → {status}", id);
    }
    0
}

fn cmd_move(args: Vec<String>, json_out: bool) -> i32 {
    if args.is_empty() {
        return fail("uso: ff move <id> <YYYY-MM-DD> [HH:MM]");
    }
    let id = match args[0].parse::<i64>() {
        Ok(v) => v,
        Err(_) => return fail(&format!("id inválido: {}", args[0])),
    };
    let day = match parse_date_ms(args.get(1).map(|s| s.as_str()).unwrap_or("")) {
        Some(v) => v,
        None => return fail("fecha inválida: usa YYYY-MM-DD"),
    };
    let (start, end, all_day): (i64, i64, bool) = match args.get(2).and_then(|s| parse_hhmm(s)) {
        Some(min) => {
            let s = day + min as i64 * 60_000;
            (s, s + 3_600_000, false)
        }
        None => (day, day + 86_399_000, true),
    };
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    if let Err(e) = db.move_to(id, start, end, Some(all_day)) {
        return fail(&format!("error: {e}"));
    }
    touch_flag(&data_dir());
    if json_out {
        match db.get_task(id) {
            Ok(Some(t)) => println!(
                "{}",
                serde_json::to_string_pretty(&json_task(&t)).unwrap_or_default()
            ),
            _ => println!("{{\"id\": {id}}}"),
        }
    } else {
        println!("#{} movida a {}", id, fmt_span(start, end, all_day));
    }
    0
}

fn cmd_rm(args: Vec<String>, json_out: bool) -> i32 {
    let id = match parse_id(&args) {
        Ok(v) => v,
        Err(e) => return fail(&e),
    };
    let db = match open_db() {
        Ok(d) => d,
        Err(e) => return fail(&e),
    };
    if let Err(e) = db.delete(id) {
        return fail(&format!("error: {e}"));
    }
    touch_flag(&data_dir());
    if json_out {
        println!("{{\"id\": {id}, \"deleted\": true}}");
    } else {
        println!("eliminada #{id}");
    }
    0
}

fn cmd_cats(json_out: bool) -> i32 {
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&CATS).unwrap_or_default()
        );
    } else {
        println!("{}", CATS.join(", "));
    }
    0
}

fn cmd_ruta() -> i32 {
    println!("{}", data_dir().display());
    0
}
