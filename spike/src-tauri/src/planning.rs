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

use crate::ai::intent::{Intent, IntentType, Priority};
use crate::engine::planner::Planner;
use crate::engine::{ConstraintEngine, Severity};
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
        format!("{} {}", day_name(&s), s.format("%d/%m"))
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
                e.commitments.push(crate::engine::Block {
                    interval: crate::engine::Interval { start: t.start_at, end: t.end_at },
                    label: t.title.clone(),
                    severity: Severity::Hard,
                });
            }
        }
    }
    e
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
    let planner = Planner { engine, ..Planner::default() };
    let report = planner.plan(intents);

    let items: Vec<PlanItemView> = report
        .items
        .into_iter()
        .map(|it| {
            let intent = intent_for(intents, &it.title);
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
        understanding: understanding(intents),
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
pub fn accept_plan(db: &Db, id: i64, edit: &EditedPlan) -> Result<Vec<TaskRow>, String> {
    let Some(plan) = get_plan(db, id)? else {
        return Err("propuesta no encontrada".into());
    };
    if plan.status != "pending" {
        return Err(format!("propuesta ya procesada (estado: {})", plan.status));
    }
    let sessions = effective_sessions(&plan, edit)?;

    // validación contra el calendario actual (puede haber cambiado desde la
    // propuesta: nuevas tareas, reubicaciones, borrados)
    let mut created: Vec<TaskRow> = Vec::new();
    for (item_idx, s) in &sessions {
        let item = &plan.items[*item_idx];
        if let Some((_, other)) = db.find_overlap(-1, s.start_ms, s.end_ms).map_err(|e| e.to_string())? {
            return Err(format!(
                "'{}' ({}) se solapa con '{}'. Edita los bloques o cancela.",
                item.title,
                fmt_when(s.start_ms, s.end_ms),
                other
            ));
        }
    }

    // 1. eventos fijos del texto (examen, reunión, cita…)
    for u in &plan.understanding {
        if u.intent_type == IntentType::Event {
            if let Some(start) = u.window_start {
                let end = u.window_end.unwrap_or(start + 3_600_000);
                if end > start {
                    let t = db
                        .create(&u.title, &u.category_id, priority_str(u.priority), start, end, u.all_day)
                        .map_err(|e| e.to_string())?;
                    db.set_task_metadata(t.id, &plan_link_meta(id, "event")).map_err(|e| e.to_string())?;
                    let t = db.get_task(t.id).map_err(|e| e.to_string())?.ok_or("tarea no creada")?;
                    created.push(t);
                }
            }
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
        let d = db();
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
}
