//! Conector fase 7: intent → plan → propuesta → aprobación → calendario.
//!
//! El flujo completo:
//! 1. `plan_from_text` interpreta el texto (IA o heurística local),
//! 2. construye un motor con el calendario real (tareas existentes como
//!    compromisos) más los intents del texto,
//! 3. planifica con [crate::engine::planner::Planner],
//! 4. guarda la propuesta como `pending` (no toca el calendario),
//! 5. `accept_plan` crea las tareas reales (una por sesión + eventos),
//! 6. `reject_plan` descarta la propuesta sin cambios.

use chrono::{Datelike, Local, TimeZone, Timelike, Weekday};
use serde::{Deserialize, Serialize};

use crate::ai::intent::{Duration, Intent, IntentType, Priority, TimeWindow};
use crate::engine::planner::{PlannedItem, Planner};
use crate::engine::{ConstraintEngine, DAY_MS, local_midnight};
use crate::store::{Db, TaskRow};

/// Sesión propuesta, tal como llega al frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionView {
    pub start_ms: i64,
    pub end_ms: i64,
    pub is_prep: bool,
}

/// Ítem planificado de la propuesta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItemView {
    pub title: String,
    pub intent_type: IntentType,
    pub priority: Priority,
    pub category_id: String,
    pub deadline_bound_ms: Option<i64>,
    pub prep_min: u32,
    pub task_min: u32,
    pub required_min: u32,
    pub planned_min: u32,
    pub complete: bool,
    pub notes: Vec<String>,
    pub sessions: Vec<SessionView>,
}

/// Resumen "Entendido" por intent: qué entendió la app y cuándo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderstoodView {
    pub title: String,
    pub intent_type: IntentType,
    pub category_id: String,
    pub priority: Priority,
    pub when_label: String,
    pub deadline: Option<i64>,
    pub window_start: Option<i64>,
    pub window_end: Option<i64>,
    pub all_day: bool,
    pub prep_min: u32,
    pub task_min: u32,
    pub reminders_min_before: Vec<u32>,
}

/// Propuesta completa para el frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanProposalView {
    pub id: i64,
    pub text: String,
    pub status: String,
    pub source: String,
    pub understanding: Vec<UnderstoodView>,
    pub items: Vec<PlanItemView>,
    pub created_at: i64,
}

/// Sesión editada por el usuario (aceptación con modificaciones).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EditedSession {
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Sesiones editadas por ítem: `items[i].sessions` reemplaza las de la
/// propuesta. Ítems ausentes o con lista vacía conservan lo propuesto.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct EditedPlan {
    pub items: Vec<Vec<EditedSession>>,
}

fn priority_str(p: Priority) -> &'static str {
    match p {
        Priority::Alta => "alta",
        Priority::Media => "media",
        Priority::Baja => "baja",
    }
}

/// Duración por defecto (min) para tareas flexibles del backlog cuando el
/// texto pide organizar sin dar duraciones. Configurable en settings
/// (`plan.default_task_min`, rango 15-480).
pub const DEFAULT_FLEX_MIN: u32 = 60;

/// Máximo de tareas del backlog que entran en un "organiza mi semana".
const MAX_BACKLOG_ITEMS: usize = 10;

/// Duración efectiva para tareas flexibles: lee la setting, si no, 60.
fn flex_min(db: &Db) -> u32 {
    db.settings_get("plan.default_task_min")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|m| (15..=480).contains(m))
        .unwrap_or(DEFAULT_FLEX_MIN)
}

/// ¿El texto pide organizar el horario sin tareas concretas?
/// ("organiza mi semana", "planifica mi día", ...).
fn is_organize_directive(text: &str) -> bool {
    let t = text.to_lowercase();
    ["organiz", "planific", "estructur", "distribu"].iter().any(|k| t.contains(k))
}

/// ¿La directiva de organizar apunta a la semana actual?
/// ("organiza mi semana" sí; "organiza mi día" no).
fn is_week_directive(text: &str) -> bool {
    is_organize_directive(text) && text.to_lowercase().contains("semana")
}

/// Semana actual (local): (lunes 00:00, domingo 23:59:59.999) en ms.
fn current_week_bounds() -> (i64, i64) {
    let today = Local::now().date_naive();
    let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let monday_ms = crate::engine::local_ms(monday.and_hms_opt(0, 0, 0).unwrap());
    (monday_ms, monday_ms + 7 * DAY_MS - 1)
}

/// Días de horizonte para "organiza mi semana": desde hoy hasta el domingo
/// de la semana actual (si hoy es domingo → solo hoy).
fn days_until_sunday() -> u32 {
    6 - Local::now().date_naive().weekday().num_days_from_monday()
}

/// Tareas flexibles pendientes: sin horario fijo (`end_at <= start_at`,
/// compromisos reales son `end_at > start_at`) y sin completar. Se ordenan
/// por prioridad (Alta → Media → Baja) y antigüedad, con tope
/// [MAX_BACKLOG_ITEMS]. Nunca tocan compromisos fijos: el motor ya los
/// bloquea como commitments.
/// Con `week = Some((from_ms, to_ms))` (lunes–domingo de la semana actual)
/// solo entran las tareas con `start_at` dentro de la semana o atrasadas
/// (anteriores al lunes); las de semanas futuras quedan fuera.
fn flexible_backlog(db: &Db, week: Option<(i64, i64)>) -> Vec<Intent> {
    let Ok(tasks) = db.list() else { return Vec::new() };
    let rank = |p: &str| match p {
        "alta" => 0u8,
        "media" => 1,
        _ => 2,
    };
    let mut flex: Vec<TaskRow> = tasks
        .into_iter()
        .filter(|t| t.status != "completada" && t.end_at <= t.start_at)
        .filter(|t| match week {
            Some((from, to)) => t.start_at < from || (from..=to).contains(&t.start_at),
            None => true,
        })
        .collect();
    flex.sort_by(|a, b| (rank(&a.priority), a.start_at).cmp(&(rank(&b.priority), b.start_at)));
    flex.truncate(MAX_BACKLOG_ITEMS);
    flex.into_iter()
        .map(|t| Intent {
            intent_type: IntentType::Task,
            title: t.title,
            description: String::new(),
            category_id: t.category_id,
            priority: match t.priority.as_str() {
                "alta" => Priority::Alta,
                "media" => Priority::Media,
                _ => Priority::Baja,
            },
            window: TimeWindow { start: None, end: None, all_day: false },
            duration: Some(Duration { minutes: flex_min(db) }),
            deadline: None,
            preparation: None,
            recurrence: None,
            reminders: Vec::new(),
            constraints: Vec::new(),
            confidence: 0.9,
            reason: "backlog".into(),
            source: "local".into(),
        })
        .collect()
}

fn day_name(d: &chrono::DateTime<Local>) -> &'static str {
    match d.weekday() {
        Weekday::Mon => "lunes",
        Weekday::Tue => "martes",
        Weekday::Wed => "miércoles",
        Weekday::Thu => "jueves",
        Weekday::Fri => "viernes",
        Weekday::Sat => "sábado",
        Weekday::Sun => "domingo",
    }
}

fn fmt_when(start: i64, end: i64) -> String {
    let s = Local.timestamp_millis_opt(start).earliest().unwrap_or_else(|| Local::now());
    let e = Local.timestamp_millis_opt(end).earliest().unwrap_or_else(|| Local::now());
    if s.hour() == 0 && s.minute() == 0 && e.hour() == 0 && e.minute() == 0 {
        let days = (e.date_naive() - s.date_naive()).num_days();
        if days >= 2 {
            // multi-día: "lunes 07/09 – viernes 11/09" (el fin es el día de
            // cierre, fecha límite al final de ese día)
            format!("{} {} – {} {}", day_name(&s), s.format("%d/%m"), day_name(&e), e.format("%d/%m"))
        } else {
            format!("{} {}", day_name(&s), s.format("%d/%m"))
        }
    } else {
        format!("{} {}–{}", day_name(&s), s.format("%H:%M"), e.format("%H:%M"))
    }
}

fn understanding(intents: &[Intent]) -> Vec<UnderstoodView> {
    intents
        .iter()
        .map(|i| {
            let when_label = match i.intent_type {
                IntentType::Event | IntentType::Task => match (i.window.start, i.window.end) {
                    (Some(s), Some(e)) => fmt_when(s, e),
                    (Some(s), None) => format!("{} {}", day_name(&Local.timestamp_millis_opt(s).earliest().unwrap_or_else(Local::now)), s),
                    _ => "sin horario fijo".into(),
                },
                _ => match i.window.start {
                    Some(s) => fmt_when(s, i.window.end.unwrap_or(s + 3_600_000)),
                    None => "sin horario fijo".into(),
                },
            };
            UnderstoodView {
                title: i.title.clone(),
                intent_type: i.intent_type,
                category_id: i.category_id.clone(),
                priority: i.priority,
                when_label,
                deadline: i.deadline,
                window_start: i.window.start,
                window_end: i.window.end,
                all_day: i.window.all_day,
                prep_min: i.preparation.as_ref().map(|p| p.minutes).unwrap_or(0),
                task_min: i.duration.as_ref().map(|d| d.minutes).unwrap_or(0),
                reminders_min_before: i
                    .reminders
                    .iter()
                    .filter_map(|r| r.minutes_before)
                    .collect(),
            }
        })
        .collect()
}

/// Motor con el calendario real: toda tarea activa no completada cuenta
/// como compromiso duro para la planificación.
pub fn engine_with_calendar(db: &Db) -> ConstraintEngine {
    let mut e = ConstraintEngine::default();
    if let Ok(tasks) = db.list() {
        for t in tasks {
            if t.status != "completada" && t.end_at > t.start_at {
                e.push_commitment(t.start_at, t.end_at, t.all_day, t.title.clone());
            }
        }
    }
    e
}

/// Ítem sintetizado para un evento all-day multi-día sin duración: una
/// sesión por cada ventana libre de cada día del rango (el día de inicio lo
/// bloquea el propio evento, el día de fin se incluye hasta el cierre).
fn fill_range_item(engine: &ConstraintEngine, i: &Intent, start: i64, end: i64) -> Option<PlannedItem> {
    let start_day = local_midnight(start);
    let end_day = local_midnight(end);
    let d0 = Local.timestamp_millis_opt(start_day).earliest()?.date_naive();
    let d1 = Local.timestamp_millis_opt(end_day).earliest()?.date_naive();
    // fin del día en ms (medianoche del día siguiente)
    let f_end = |d: chrono::NaiveDate| crate::engine::local_ms((d + chrono::Duration::days(1)).into());
    let mut sessions: Vec<crate::engine::planner::PlanSession> = Vec::new();
    let mut total = 0u32;
    let mut d = d0;
    while d <= d1 {
        // `end` es medianoche del día de fin → el día completo entra (hasta el
        // cierre del horario laboral). Con hora de cierre se recorta.
        let is_last = d == d1;
        let day_clip_end = if is_last && end == end_day { f_end(d) } else { end };
        for f in engine.allowed_on(d) {
            let s = f.start.max(start);
            let e = f.end.min(day_clip_end);
            if e - s < 30 * 60_000 {
                continue;
            }
            sessions.push(crate::engine::planner::PlanSession { start_ms: s, end_ms: e, is_prep: false });
            total += ((e - s) / 60_000) as u32;
        }
        d += chrono::Duration::days(1);
    }
    if sessions.is_empty() {
        return None;
    }
    Some(PlannedItem {
        title: i.title.clone(),
        intent_type: i.intent_type,
        priority: i.priority,
        deadline_bound_ms: Some(end),
        prep_min: 0,
        task_min: total,
        required_min: total,
        planned_min: total,
        sessions,
        complete: true,
        notes: Vec::new(),
    })
}

fn apply_intents(base: &mut ConstraintEngine, intents: &[Intent]) {
    let ie = ConstraintEngine::from_intents(intents);
    base.commitments.extend(ie.commitments);
    base.availability.extend(ie.availability);
    base.deadlines.extend(ie.deadlines);
    if ie.working_hours.is_some() && ie.working_hours != base.working_hours {
        base.working_hours = ie.working_hours;
    }
}

/// Relaciona ítems planificados con su intent original (por título, en
/// orden). Los ítems devuelven los datos del intent que los originó.
fn intent_for<'a>(intents: &'a [Intent], title: &str) -> Option<&'a Intent> {
    intents.iter().find(|i| i.title == title)
}

/// Texto → propuesta persistida (`pending`). No muta el calendario.
pub fn plan_from_text(
    db: &Db,
    text: &str,
    intents: &[Intent],
    source: &str,
) -> Result<PlanProposalView, String> {
    let mut engine = engine_with_calendar(db);
    apply_intents(&mut engine, intents);
    // "organiza mi semana": el horizonte se limita al domingo de la semana
    // actual (no 14 días). Otras directivas ("organiza mi día", ...) usan el
    // horizonte por defecto.
    let week_mode = is_week_directive(text);
    let planner = if week_mode {
        Planner { engine: engine.clone(), horizon_days: days_until_sunday(), ..Planner::default() }
    } else {
        Planner { engine: engine.clone(), ..Planner::default() }
    };
    let mut all = intents.to_vec();
    let mut report = planner.plan(intents);
    // "organiza mi semana": el texto no dimensiona nada (intents vacíos o sin
    // duración) → planificar el backlog flexible pendiente con la duración
    // por defecto (configurable). Los compromisos fijos ya bloquean el motor.
    if report.items.is_empty() && is_organize_directive(text) {
        // modo semana: solo el backlog de esta semana (o atrasado); las
        // flexibles de semanas futuras no entran en el plan
        let backlog = flexible_backlog(db, if week_mode { Some(current_week_bounds()) } else { None });
        if !backlog.is_empty() {
            let extra = planner.plan(&backlog);
            report.items.extend(extra.items);
            all.extend(backlog);
        }
    }
    // Eventos all-day multi-día sin duración: no hay sesiones dimensionables,
    // pero el usuario espera cubrir el rango. Sintetizar sesiones por día
    // (todo el horario libre de cada día, día inicio bloqueado por el propio
    // evento, día fin incluido).
    for i in intents {
        if i.intent_type != IntentType::Event || i.duration.is_some() || i.preparation.is_some() {
            continue;
        }
        let (Some(s), Some(en)) = (i.window.start, i.window.end) else { continue };
        if !i.window.all_day || en - s <= DAY_MS {
            continue;
        }
        if report.items.iter().any(|it| it.title == i.title) {
            continue;
        }
        if let Some(item) = fill_range_item(&engine, i, s, en) {
            report.items.push(item);
        }
    }

    let items: Vec<PlanItemView> = report
        .items
        .into_iter()
        .map(|it| {
            let intent = intent_for(&all, &it.title);
            PlanItemView {
                category_id: intent.map(|i| i.category_id.clone()).unwrap_or_else(|| "otr".into()),
                title: it.title,
                intent_type: it.intent_type,
                priority: it.priority,
                deadline_bound_ms: it.deadline_bound_ms,
                prep_min: it.prep_min,
                task_min: it.task_min,
                required_min: it.required_min,
                planned_min: it.planned_min,
                complete: it.complete,
                notes: it.notes,
                sessions: it
                    .sessions
                    .into_iter()
                    .map(|s| SessionView { start_ms: s.start_ms, end_ms: s.end_ms, is_prep: s.is_prep })
                    .collect(),
            }
        })
        .collect();

    let view = PlanProposalView {
        id: 0,
        text: text.to_string(),
        status: "pending".into(),
        source: source.to_string(),
        understanding: understanding(&all),
        items,
        created_at: 0,
    };
    let payload = serde_json::to_string(&view).map_err(|e| e.to_string())?;
    let id = db.insert_plan_proposal(text, &payload, source).map_err(|e| e.to_string())?;
    let created = db.get_plan_proposal(id).map_err(|e| e.to_string())?.map(|p| p.created_at).unwrap_or(0);
    Ok(PlanProposalView { id, created_at: created, ..view })
}

/// Propuesta guardada, reconstruida para el frontend.
pub fn get_plan(db: &Db, id: i64) -> Result<Option<PlanProposalView>, String> {
    let Some(row) = db.get_plan_proposal(id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let mut view: PlanProposalView = serde_json::from_str(&row.payload).map_err(|e| e.to_string())?;
    view.id = row.id;
    view.text = row.text;
    view.status = row.status;
    view.source = row.source;
    view.created_at = row.created_at;
    Ok(Some(view))
}

/// Sesiones efectivas a crear: las editadas por ítem si las hay, si no las
/// de la propuesta original.
fn effective_sessions(plan: &PlanProposalView, edit: &EditedPlan) -> Result<Vec<(usize, SessionView)>, String> {
    let mut out: Vec<(usize, SessionView)> = Vec::new();
    for (idx, item) in plan.items.iter().enumerate() {
        let edited = edit.items.get(idx);
        if let Some(ed) = edited {
            if ed.is_empty() {
                continue; // ítem eliminado por el usuario
            }
            for s in ed {
                if s.end_ms <= s.start_ms {
                    return Err(format!("'{}': el bloque debe terminar después de empezar", item.title));
                }
                if (s.end_ms - s.start_ms) < 15 * 60_000 {
                    return Err(format!("'{}': bloque de menos de 15 minutos", item.title));
                }
                out.push((idx, SessionView { start_ms: s.start_ms, end_ms: s.end_ms, is_prep: false }));
            }
        } else {
            out.extend(item.sessions.iter().map(|s| (idx, s.clone())));
        }
    }
    // sin solapamientos entre los bloques que se van a crear
    let mut by_start: Vec<(i64, i64, String)> = out
        .iter()
        .map(|(idx, s)| (s.start_ms, s.end_ms, plan.items[*idx].title.clone()))
        .collect();
    by_start.sort_by_key(|t| t.0);
    for w in by_start.windows(2) {
        if w[1].0 < w[0].1 {
            return Err(format!(
                "los bloques de '{}' y '{}' se solapan",
                w[0].2, w[1].2
            ));
        }
    }
    Ok(out)
}

/// Acepta una propuesta pendiente: valida el calendario actual, crea las
/// tareas reales (una por sesión + eventos del texto) y marca `accepted`.
/// Si algo falla a mitad de camino, las tareas ya creadas se eliminan para
/// no dejar el calendario parcialmente mutado.
pub fn accept_plan(db: &Db, id: i64, edit: &EditedPlan) -> Result<Vec<TaskRow>, String> {
    let Some(plan) = get_plan(db, id)? else {
        return Err("propuesta no encontrada".into());
    };
    if plan.status != "pending" {
        return Err(format!("propuesta ya procesada (estado: {})", plan.status));
    }
    let sessions = effective_sessions(&plan, edit)?;

    let mut created: Vec<TaskRow> = Vec::new();

    // 1. validar los eventos del texto contra el calendario y entre sí
    let mut event_spans: Vec<(i64, i64, String)> = plan
        .understanding
        .iter()
        .filter(|u| u.intent_type == IntentType::Event)
        .filter_map(|u| u.window_start.map(|s| (s, u.window_end.unwrap_or(s + 3_600_000), u.title.clone())))
        .filter(|(s, e, _)| e > s)
        .collect();
    event_spans.sort_by_key(|(s, _, _)| *s);
    for w in event_spans.windows(2) {
        if w[1].0 < w[0].1 {
            return Err(format!(
                "los eventos '{}' y '{}' del texto se solapan",
                w[0].2, w[1].2
            ));
        }
    }
    for (s, e, title) in &event_spans {
        if let Some((_, other)) = db.find_overlap(-1, *s, *e).map_err(|e2| e2.to_string())? {
            return Err(format!(
                "'{}' ({}) se solapa con '{}'. Edita o cancela.",
                title,
                fmt_when(*s, *e),
                other
            ));
        }
    }

    // 2. crear eventos fijos — primero, para que la validación de sesiones
    // los conozca (los excluimos: las sesiones del plan ya los rodean)
    let mut event_ids: Vec<i64> = Vec::new();
    for (start, end, title) in &event_spans {
        let u = plan.understanding.iter().find(|u| u.title == *title).expect("evento");
        let t = db
            .create(title, &u.category_id, priority_str(u.priority), *start, *end, u.all_day)
            .map_err(|e| e.to_string())?;
        db.set_task_metadata(t.id, &plan_link_meta(id, "event")).map_err(|e| e.to_string())?;
        let t = db.get_task(t.id).map_err(|e| e.to_string())?.ok_or("tarea no creada")?;
        event_ids.push(t.id);
        created.push(t);
    }

    // validación de sesiones contra el calendario actual CON los eventos del
    // plan ya insertados (excluidos de la comprobación de solape)
    let result = (|| -> Result<(), String> {
        for (item_idx, s) in &sessions {
            let item = &plan.items[*item_idx];
            if let Some((_, other)) = db.find_overlap_excluding(&event_ids, s.start_ms, s.end_ms).map_err(|e| e.to_string())? {
                return Err(format!(
                    "'{}' ({}) se solapa con '{}'. Edita los bloques o cancela.",
                    item.title,
                    fmt_when(s.start_ms, s.end_ms),
                    other
                ));
            }
        }

        // 2. sesiones de trabajo/preparación
        for (item_idx, s) in &sessions {
            let item = &plan.items[*item_idx];
            let t = db
                .create(&item.title, &item.category_id, priority_str(item.priority), s.start_ms, s.end_ms, false)
                .map_err(|e| e.to_string())?;
            db.set_task_metadata(t.id, &plan_link_meta(id, "session")).map_err(|e| e.to_string())?;
            let u = plan.understanding.iter().find(|u| u.title == item.title);
            if s.is_prep {
                if let Some(min) = u.and_then(|u| u.reminders_min_before.first()).map(|m| *m as i64) {
                    db.set_task_reminder(t.id, min).map_err(|e| e.to_string())?;
                }
            }
            let t = db.get_task(t.id).map_err(|e| e.to_string())?.ok_or("tarea no creada")?;
            created.push(t);
        }
        Ok(())
    })();

    if let Err(e) = result {
        // rollback compensatorio: no dejar el calendario a medias
        for t in &created {
            let _ = db.delete(t.id);
        }
        return Err(e);
    }

    db.set_plan_proposal_status(id, "accepted").map_err(|e| e.to_string())?;
    Ok(created)
}

fn plan_link_meta(proposal_id: i64, kind: &str) -> String {
    format!(r#"{{"plan_proposal_id":{proposal_id},"plan_kind":"{kind}"}}"#)
}

/// Descarta una propuesta pendiente. No toca el calendario.
pub fn reject_plan(db: &Db, id: i64) -> Result<(), String> {
    let Some(plan) = get_plan(db, id)? else {
        return Err("propuesta no encontrada".into());
    };
    if plan.status != "pending" {
        return Err(format!("propuesta ya procesada (estado: {})", plan.status));
    }
    db.set_plan_proposal_status(id, "rejected").map_err(|e| e.to_string())
}

/// Propuesta pendiente sin interpretar (borrador de texto) — no se usa.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::intent::{Duration, Preparation, TimeWindow};
    use crate::engine::MIN_MS;

    fn db() -> Db {
        Db::open_memory_pub().unwrap()
    }

    /// DB sin datos de demostración (el seed cruza medianoche según la hora
    /// real y ensucia los días de prueba).
    fn clean_db() -> Db {
        Db::open_memory_clean_pub().unwrap()
    }

    fn intent(title: &str, kind: IntentType, minutes: u32) -> Intent {
        Intent {
            intent_type: kind,
            title: title.into(),
            description: String::new(),
            category_id: "uni".into(),
            priority: Priority::Media,
            window: TimeWindow { start: None, end: None, all_day: false },
            duration: if minutes > 0 { Some(Duration { minutes }) } else { None },
            deadline: None,
            preparation: None,
            recurrence: None,
            reminders: Vec::new(),
            constraints: Vec::new(),
            confidence: 0.9,
            reason: "test".into(),
            source: "local".into(),
        }
    }

    fn exam_intent(title: &str, deadline: i64) -> Intent {
        let mut i = intent(title, IntentType::Event, 0);
        i.preparation = Some(Preparation { minutes: 240, note: String::new() });
        i.deadline = Some(deadline);
        i.window = TimeWindow { start: Some(deadline), end: Some(deadline + 2 * 3_600_000), all_day: false };
        i
    }

    fn day(n: i64) -> i64 {
        let t = Local::now().date_naive() + chrono::Duration::days(n);
        let s = crate::engine::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
        s
    }

    #[test]
    fn pipeline_produces_pending_proposal() {
        let d = db();
        let intents = vec![exam_intent("Examen de cálculo", day(6) + 12 * 3_600_000)];
        let view = plan_from_text(&d, "tengo examen el viernes y necesito 4 horas", &intents, "ai").unwrap();
        assert_eq!(view.status, "pending");
        assert_eq!(view.understanding.len(), 1);
        assert_eq!(view.understanding[0].prep_min, 240);
        assert!(!view.items.is_empty(), "planifica la preparación");
        assert!(view.items[0].sessions.len() >= 2, "se divide en varias sesiones");
        let row = d.get_plan_proposal(view.id).unwrap().unwrap();
        assert_eq!(row.status, "pending");
        assert!(row.payload.contains("Examen de cálculo"));
    }

    #[test]
    fn existing_tasks_block_planning() {
        let d = db();
        // lunes completo ocupado con una tarea real del calendario
        let start = day(1);
        d.create("Reunión", "trab", "alta", start, start + 9 * 3_600_000, false).unwrap();
        let intents = vec![intent("Escribir informe", IntentType::Task, 120)];
        let view = plan_from_text(&d, "escribir informe 2 horas", &intents, "local").unwrap();
        let item = &view.items[0];
        assert!(item.complete, "120 min caben fuera del lunes");
        for s in &item.sessions {
            assert!(s.end_ms <= start || s.start_ms >= start + 9 * 3_600_000, "nunca choca con la reunión");
        }
    }

    #[test]
    fn multiday_allday_blocks_only_external_days() {
        let d = clean_db();
        // "proyecto del lunes al jueves": cubre lunes, martes y miércoles;
        // el jueves es el día de fin (sin hora de cierre).
        d.create("Proyecto", "trab", "alta", day(1), day(4), true).unwrap();
        let e = engine_with_calendar(&d);
        let hour = crate::engine::HOUR_MS;
        // día inicial: bloqueado completo
        assert_eq!(e.available_minutes(day(1) + 9 * hour, day(1) + 10 * hour), 0);
        assert_eq!(e.available_minutes(day(1), day(1) + 24 * hour), 0);
        // días intermedios: libres (06:00–22:00 = 16h)
        assert_eq!(e.available_minutes(day(2), day(2) + 24 * hour), 16 * 60);
        assert_eq!(e.available_minutes(day(3) + 9 * hour, day(3) + 10 * hour), 60);
        // día de fin sin hora de cierre: libre, con fecha límite 22:00
        assert_eq!(e.available_minutes(day(4), day(4) + 24 * hour), 16 * 60);
        let dl = e
            .deadlines
            .iter()
            .find(|x| x.label == "Proyecto")
            .expect("deadline del día de fin");
        assert_eq!(dl.at_ms, day(4) + 22 * hour, "fecha límite al final del día (22:00)");
    }

    #[test]
    fn multiday_allday_without_duration_fills_range() {
        let d = clean_db();
        let mut i = intent("Proyecto", IntentType::Event, 0);
        i.window = TimeWindow { start: Some(day(1)), end: Some(day(4)), all_day: true };
        let view = plan_from_text(&d, "proyecto del lunes al jueves", &[i], "local").unwrap();
        assert_eq!(view.items.len(), 1, "se sintetiza el ítem del rango");
        let it = &view.items[0];
        assert_eq!(it.title, "Proyecto");
        assert_eq!(it.sessions.len(), 3, "día inicio bloqueado por el evento, 3 días libres");
        for s in &it.sessions {
            let st = Local.timestamp_millis_opt(s.start_ms).earliest().unwrap();
            let et = Local.timestamp_millis_opt(s.end_ms).earliest().unwrap();
            assert_eq!(st.hour(), 6, "empiezan a las 06:00");
            assert_eq!(et.hour(), 22, "terminan a las 22:00");
        }
        assert_eq!(it.required_min, 3 * 16 * 60, "3 días x 16 h");
        assert!(it.complete);
        // aceptar crea el evento + las sesiones
        let created = accept_plan(&d, view.id, &EditedPlan::default()).unwrap();
        assert_eq!(created.len(), 4, "evento + 3 sesiones");
    }

    #[test]
    fn single_day_allday_without_duration_no_sessions() {
        let d = clean_db();
        let mut i = intent("Clase", IntentType::Event, 0);
        i.window = TimeWindow { start: Some(day(1)), end: Some(day(2)), all_day: true };
        let view = plan_from_text(&d, "clase el lunes", &[i], "local").unwrap();
        assert!(view.items.is_empty(), "un solo día no se llena con sesiones");
    }

    #[test]
    fn multiday_allday_with_close_time_blocks_two_hours_before() {
        let d = clean_db();
        // cierra el jueves a las 22:00 → ocupa 20:00–22:00 de ese día
        d.create("Proyecto", "trab", "alta", day(1), day(4) + 22 * 3_600_000, true).unwrap();
        let e = engine_with_calendar(&d);
        let hour = crate::engine::HOUR_MS;
        assert_eq!(e.available_minutes(day(4) + 20 * hour, day(4) + 22 * hour), 0, "2 h antes del cierre ocupadas");
        assert_eq!(e.available_minutes(day(4) + 14 * hour, day(4) + 15 * hour), 60, "resto del día libre");
        let dl = e.deadlines.iter().find(|x| x.label == "Proyecto").expect("deadline = hora de cierre");
        assert_eq!(dl.at_ms, day(4) + 22 * hour);
    }

    #[test]
    fn single_day_allday_blocks_full_day() {
        let d = clean_db();
        d.create("Examen", "uni", "alta", day(2), day(3), true).unwrap();
        let engine = engine_with_calendar(&d);
        let hour = crate::engine::HOUR_MS;
        assert_eq!(engine.available_minutes(day(2), day(2) + 24 * hour), 0, "todo el día sigue ocupado");
        assert_eq!(engine.available_minutes(day(1) + 12 * hour, day(2)), 10 * 60, "día anterior libre (12:00–22:00)");
        assert!(engine.deadlines.is_empty(), "todo el día simple no crea fecha límite");
    }

    #[test]
    fn accept_creates_tasks_and_marks_accepted() {
        let d = db();
        let intents = vec![exam_intent("Examen", day(3) + 12 * 3_600_000)];
        let view = plan_from_text(&d, "examen con preparación", &intents, "local").unwrap();
        let before = d.count().unwrap();
        let tasks = accept_plan(&d, view.id, &EditedPlan::default()).unwrap();
        assert!(!tasks.is_empty());
        assert_eq!(d.count().unwrap(), before + tasks.len() as i64);
        let row = d.get_plan_proposal(view.id).unwrap().unwrap();
        assert_eq!(row.status, "accepted");
        // evento fijo (el examen) + sesiones de prep
        // evento fijo (el examen) + sesiones de prep
        eprintln!("DBG tasks: {:?}", tasks.iter().map(|t| (&t.title, t.start_at, &t.metadata)).collect::<Vec<_>>());
        let ev = tasks.iter().find(|t| t.metadata.contains("plan_kind")).unwrap();
        assert_eq!(ev.start_at, view.understanding[0].window_start.unwrap());
        // todas las tareas enlazadas a la propuesta
        for t in &tasks {
            assert!(t.metadata.contains(&format!(r#""plan_proposal_id":{}"#, view.id)));
        }
    }

    #[test]
    fn accept_twice_fails() {
        let d = db();
        let intents = vec![intent("Estudiar", IntentType::Task, 60)];
        let view = plan_from_text(&d, "estudiar", &intents, "local").unwrap();
        accept_plan(&d, view.id, &EditedPlan::default()).unwrap();
        let err = accept_plan(&d, view.id, &EditedPlan::default()).unwrap_err();
        assert!(err.contains("ya procesada"), "{err}");
    }

    #[test]
    fn reject_makes_no_changes() {
        let d = db();
        let intents = vec![intent("Estudiar", IntentType::Task, 60)];
        let view = plan_from_text(&d, "estudiar", &intents, "local").unwrap();
        let before = d.count().unwrap();
        reject_plan(&d, view.id).unwrap();
        assert_eq!(d.count().unwrap(), before, "rechazar no crea tareas");
        let row = d.get_plan_proposal(view.id).unwrap().unwrap();
        assert_eq!(row.status, "rejected");
        let err = reject_plan(&d, view.id).unwrap_err();
        assert!(err.contains("ya procesada"), "{err}");
    }

    #[test]
    fn edited_sessions_replace_proposal() {
        let d = clean_db(); // seed demo solapa el slot fijo de edición (20:00 día 2)
        let intents = vec![intent("Estudiar", IntentType::Task, 120)];
        let view = plan_from_text(&d, "estudiar 2 horas", &intents, "local").unwrap();
        let slot = day(2) + 20 * 3_600_000; // 20:00 del día 2
        let edit = EditedPlan {
            items: vec![vec![EditedSession { start_ms: slot, end_ms: slot + 120 * MIN_MS }]],
        };
        let tasks = accept_plan(&d, view.id, &edit).unwrap();
        assert_eq!(tasks.len(), 1, "una sola sesión editada");
        assert_eq!(tasks[0].start_at, slot);
        assert_eq!(tasks[0].end_at, slot + 120 * MIN_MS);
        let t = d.get_task(tasks[0].id).unwrap().unwrap();
        assert_eq!(t.start_at, slot);
    }

    #[test]
    fn edited_conflict_is_rejected() {
        let d = db();
        let intents = vec![intent("Estudiar", IntentType::Task, 60)];
        let view = plan_from_text(&d, "estudiar", &intents, "local").unwrap();
        let slot = day(1) + 10 * 3_600_000;
        d.create("Clase", "uni", "alta", slot, slot + 3_600_000, false).unwrap();
        let edit = EditedPlan {
            items: vec![vec![EditedSession { start_ms: slot, end_ms: slot + 3_600_000 }]],
        };
        let err = accept_plan(&d, view.id, &edit).unwrap_err();
        assert!(err.contains("se solapa"), "{err}");
        let row = d.get_plan_proposal(view.id).unwrap().unwrap();
        assert_eq!(row.status, "pending", "sigue pendiente tras el conflicto");
    }

    #[test]
    fn overlapping_edited_sessions_are_rejected() {
        let d = db();
        let intents = vec![intent("Estudiar", IntentType::Task, 120)];
        let view = plan_from_text(&d, "estudiar", &intents, "local").unwrap();
        let a = day(2) + 10 * 3_600_000;
        let edit = EditedPlan {
            items: vec![vec![
                EditedSession { start_ms: a, end_ms: a + 3_600_000 },
                EditedSession { start_ms: a + 1_800_000, end_ms: a + 3_600_000 },
            ]],
        };
        let err = accept_plan(&d, view.id, &edit).unwrap_err();
        assert!(err.contains("se solapan"), "{err}");
    }

    #[test]
    fn deleted_event_does_not_break_proposal() {
        // borrar una tarea existente no rompe propuestas pendientes: el
        // aceptar revalida contra el calendario actual (sin la tarea)
        let d = db();
        let intents = vec![intent("Estudiar", IntentType::Task, 60)];
        let view = plan_from_text(&d, "estudiar", &intents, "local").unwrap();
        let slot = day(1) + 10 * 3_600_000;
        let clash = d.create("Reunión", "trab", "alta", slot, slot + 3_600_000, false).unwrap();
        // la reunión se borra → el hueco queda libre
        d.delete(clash.id).unwrap();
        let edit = EditedPlan {
            items: vec![vec![EditedSession { start_ms: slot, end_ms: slot + 3_600_000 }]],
        };
        let tasks = accept_plan(&d, view.id, &edit).unwrap();
        assert_eq!(tasks.len(), 1, "se acepta con el hueco liberado");
    }

    // ------------------------------------------------------------------
    // "organiza mi semana": backlog flexible
    // ------------------------------------------------------------------

    #[test]
    fn organize_week_plans_flexible_backlog() {
        let d = clean_db();
        // flexibles = sin horario fijo (end_at <= start_at); fechas dentro de
        // la semana actual (hoy / atrasada) para el filtro de semana
        d.create("Pagar internet", "otr", "media", day(0), day(0), false).unwrap();
        d.create("Estudiar cálculo", "uni", "alta", day(-1), day(-1), false).unwrap();
        let view = plan_from_text(&d, "organiza mi semana", &[], "local").unwrap();
        assert_eq!(view.items.len(), 2, "planifica el backlog: {:?}", view.items.iter().map(|i| &i.title).collect::<Vec<_>>());
        assert_eq!(view.items[0].title, "Estudiar cálculo", "prioridad Alta primero");
        assert_eq!(view.understanding.len(), 2, "el backlog aparece en 'Entendido'");
        for it in &view.items {
            assert!(it.complete);
            assert_eq!(it.task_min, DEFAULT_FLEX_MIN, "duración por defecto 60");
            for s in &it.sessions {
                assert_eq!(((s.end_ms - s.start_ms) / 60_000) as u32, DEFAULT_FLEX_MIN);
            }
        }
    }

    #[test]
    fn organize_week_respects_fixed_commitments() {
        let d = clean_db();
        // compromiso fijo 10:00-12:00 bloquea; el flexible se planifica fuera
        d.create("Clase", "uni", "alta", day(1) + 10 * 3_600_000, day(1) + 12 * 3_600_000, false).unwrap();
        d.create("Estudiar", "uni", "media", day(1), day(1), false).unwrap();
        let view = plan_from_text(&d, "organiza mi día", &[], "local").unwrap();
        assert_eq!(view.items.len(), 1);
        for s in &view.items[0].sessions {
            let in_clase = s.start_ms < day(1) + 12 * 3_600_000 && s.end_ms > day(1) + 10 * 3_600_000;
            assert!(!in_clase, "sesión solapa la clase fija");
        }
    }

    #[test]
    fn organize_week_without_flexible_keeps_empty() {
        let d = clean_db();
        let view = plan_from_text(&d, "organiza mi semana", &[], "local").unwrap();
        assert!(view.items.is_empty(), "sin backlog no hay nada que planificar");
        assert!(view.understanding.is_empty());
    }

    #[test]
    fn organize_week_uses_configured_duration() {
        let d = clean_db();
        d.settings_set("plan.default_task_min", "90").unwrap();
        d.create("Escribir informe", "trab", "media", day(0), day(0), false).unwrap();
        let view = plan_from_text(&d, "organiza mi semana", &[], "local").unwrap();
        assert_eq!(view.items.len(), 1);
        let it = &view.items[0];
        assert_eq!(it.task_min, 90, "duración desde settings");
        for s in &it.sessions {
            assert_eq!(((s.end_ms - s.start_ms) / 60_000) as u32, 90);
        }
    }

    #[test]
    fn organize_week_caps_backlog_at_max() {
        let d = clean_db();
        for i in 0..15 {
            d.create(&format!("Tarea {i}"), "otr", "media", day(0), day(0), false).unwrap();
        }
        let view = plan_from_text(&d, "organiza mi semana", &[], "local").unwrap();
        assert_eq!(view.items.len(), MAX_BACKLOG_ITEMS, "el backlog se recorta al tope");
    }

    #[test]
    fn organize_week_sessions_stay_within_current_week() {
        let d = clean_db();
        let (monday, sunday_end) = current_week_bounds();
        // flexible con start_at dentro de la semana actual (miércoles 10:00)
        let mid = monday + 2 * DAY_MS + 10 * 3_600_000;
        d.create("Estudiar cálculo", "uni", "alta", mid, mid, false).unwrap();
        let view = plan_from_text(&d, "organiza mi semana", &[], "local").unwrap();
        assert_eq!(view.items.len(), 1, "la tarea de esta semana entra en el plan");
        assert!(!view.items[0].sessions.is_empty(), "se planifica: {:?}", view.items[0].notes);
        for s in &view.items[0].sessions {
            assert!(s.end_ms - 1 <= sunday_end, "sesión dentro de la semana (<= domingo 23:59): {s:?}");
            assert!(s.start_ms >= monday, "nunca antes del lunes de esta semana: {s:?}");
        }
    }

    #[test]
    fn organize_week_excludes_future_week_tasks() {
        let d = clean_db();
        let (monday, _) = current_week_bounds();
        // atrasada (antes del lunes de esta semana) → entra
        d.create("Atrasada", "otr", "alta", monday - 2 * DAY_MS, monday - 2 * DAY_MS, false).unwrap();
        // dentro de la semana actual → entra
        let mid = monday + 2 * DAY_MS + 10 * 3_600_000;
        d.create("De esta semana", "otr", "media", mid, mid, false).unwrap();
        // semana próxima y dentro de 3 semanas → fuera del plan
        let next = monday + 8 * DAY_MS;
        d.create("Semana próxima", "otr", "alta", next, next, false).unwrap();
        let far = monday + 17 * DAY_MS;
        d.create("Lejana", "otr", "media", far, far, false).unwrap();
        let view = plan_from_text(&d, "organiza mi semana", &[], "local").unwrap();
        let titles: Vec<&str> = view.items.iter().map(|i| i.title.as_str()).collect();
        assert!(titles.contains(&"Atrasada"), "entra la atrasada: {titles:?}");
        assert!(titles.contains(&"De esta semana"), "entra la de esta semana: {titles:?}");
        assert!(!titles.contains(&"Semana próxima"), "semana futura fuera: {titles:?}");
        assert!(!titles.contains(&"Lejana"), "a 3 semanas fuera: {titles:?}");
    }

    #[test]
    fn organize_day_keeps_default_horizon() {
        // "organiza mi día" (sin "semana") conserva el horizonte de 14 días y
        // no filtra el backlog por semana: una flexible de la semana próxima
        // (dentro del horizonte) sí entra.
        let d = clean_db();
        let far = day(10);
        d.create("Lejana", "otr", "media", far, far, false).unwrap();
        let view = plan_from_text(&d, "organiza mi día", &[], "local").unwrap();
        assert_eq!(view.items.len(), 1, "sin 'semana' no hay filtro de semana");
        assert_eq!(view.items[0].title, "Lejana");
    }
}
