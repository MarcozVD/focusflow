//! Planificador determinista (Fase 6).
//!
//! Transforma `Intent + Calendario + Restricciones` → **propuesta de
//! horario** (spec/11). Nunca muta el calendario: produce un `PlanReport`
//! que el usuario aprueba y una capa externa aplica.
//!
//! Principios (en orden):
//! 1. restricciones hard (las hereda del motor: eventos, bloques, sueño,
//!    horario laboral, disponibilidad),
//! 2. vencimientos (todo termina antes del límite),
//! 3. prioridad Alta primero,
//! 4. preparación requerida (sesiones de prep antes de la tarea),
//! 5. preferencias del usuario (horario preferido),
//! 6. carga equilibrada (tope por día y puntaje de balance).
//!
//! Evita: horarios imposibles, solapamientos, fuera de horas permitidas,
//! fragmentación excesiva, amontonar todo en el mismo hueco y destruir
//! compromisos existentes.

use chrono::{Datelike, Local, NaiveDate, TimeZone, Timelike, Weekday};

use crate::ai::intent::{Intent, IntentType, Priority};

use super::{clamp_today, time_of_day_min, Block, ConstraintEngine, Interval, Severity, MIN_MS};

/// Sesión individual propuesta.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanSession {
    pub start_ms: i64,
    pub end_ms: i64,
    /// true = bloque de preparación; false = bloque de trabajo/estudio.
    pub is_prep: bool,
}

/// Item ya planificado.
#[derive(Debug, Clone)]
pub struct PlannedItem {
    pub title: String,
    pub intent_type: IntentType,
    pub priority: Priority,
    /// Límite duro: todo termina antes de este instante (si existe).
    pub deadline_bound_ms: Option<i64>,
    pub prep_min: u32,
    pub task_min: u32,
    pub required_min: u32,
    pub planned_min: u32,
    pub sessions: Vec<PlanSession>,
    pub complete: bool,
    pub notes: Vec<String>,
}

impl PlannedItem {
    pub fn hours(&self) -> String {
        let h = self.required_min as f64 / 60.0;
        if h.fract() == 0.0 {
            format!("{} horas", h as u32)
        } else {
            format!("{h:.1} horas")
        }
    }
}

/// Reporte completo: un ítem por intent planificable.
#[derive(Debug, Clone)]
pub struct PlanReport {
    pub items: Vec<PlannedItem>,
}

impl PlanReport {
    /// Texto para mostrar al usuario (formato del enunciado).
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for it in &self.items {
            if it.required_min == 0 {
                out.push_str(&format!("Plan: {}\nSin duración declarada: no se puede dimensionar.\n\n", it.title));
                continue;
            }
            out.push_str(&format!("Plan:\n{}\n\n{} requeridos\n", it.title, it.hours()));
            if it.prep_min > 0 {
                out.push_str(&format!("Incluye {} de preparación\n", it.prep_min));
            }
            if it.complete {
                out.push_str("Proposed:\n");
            } else {
                out.push_str(&format!(
                    "INCOMPLETO — solo {} de {} planificados\nProposed:\n",
                    it.planned_min, it.required_min
                ));
            }
            for s in &it.sessions {
                let (day_name, hm) = fmt_session(s.start_ms, s.end_ms);
                let tag = if s.is_prep { " (prep)" } else { "" };
                out.push_str(&format!("{day_name} {}{}\n", hm, tag));
            }
            for n in &it.notes {
                out.push_str(&format!("Nota: {n}\n"));
            }
            out.push('\n');
        }
        out.trim_end().to_string()
    }
}

/// Candidato a sesión: (día, inicio, duración).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Candidate {
    day: NaiveDate,
    start_ms: i64,
    len_min: u32,
}

/// Puntaje de un candidato. Menor = mejor. Fórmula documentada en spec/11:
/// ```text
/// score = +40 000 si empieza antes del horario preferido (se evita si hay
///                alternativa)
///        +1 000  * sesiones ya creadas        (costo de fragmentación)
///        +1      * min de carga del día       (equilibrio de carga)
///        −60     * min(len,120)/30            (preferir sesiones largas)
///        −5      * días antes del vencimiento (planificar temprano)
/// ```
/// Empate → el candidato que empieza antes.
fn score_candidate(c: &Candidate, sessions_so_far: usize, day_load_min: u32, preferred_after_min: Option<u32>, deadline_bound_ms: Option<i64>) -> i64 {
    let mut sc: i64 = 0;
    if let Some(p) = preferred_after_min {
        if time_of_day_min(c.start_ms) < p {
            sc += 40_000;
        }
    }
    sc += 1_000 * sessions_so_far as i64;
    sc += day_load_min as i64;
    sc -= 60 * (c.len_min.min(120) / 30) as i64;
    if let Some(d) = deadline_bound_ms {
        if let Some(dd) = chrono::Local.timestamp_millis_opt(d).earliest() {
            let days = (dd.date_naive() - c.day).num_days();
            if days > 0 {
                sc -= 5 * days;
            }
        }
    }
    sc
}

/// Especificación interna de un ítem a planificar.
struct ItemSpec {
    title: String,
    intent_type: IntentType,
    priority: Priority,
    deadline_bound_ms: Option<i64>,
    prep_min: u32,
    task_min: u32,
    required_min: u32,
}

fn order_items(intents: &[Intent]) -> Vec<ItemSpec> {
    let mut items: Vec<ItemSpec> = Vec::new();
    for i in intents {
        // Eventos con ventana fija son compromisos; no se mueven. Solo se
        // planifica su preparación.
        let is_fixed = i.intent_type == IntentType::Event
            && (i.window.start.is_some() || i.window.end.is_some());
        let task_min = i.duration.map(|d| d.minutes).unwrap_or(0);
        let prep_min = i.preparation.as_ref().map(|p| p.minutes).unwrap_or(0);
        if task_min == 0 && prep_min == 0 {
            continue; // no dimensionable (availability/constraint/task backlog)
        }
        let bound = if is_fixed {
            i.window.start
        } else {
            i.deadline
        };
        items.push(ItemSpec {
            title: i.title.clone(),
            intent_type: i.intent_type,
            priority: i.priority,
            deadline_bound_ms: bound,
            prep_min,
            task_min,
            required_min: task_min + prep_min,
        });
    }
    items.sort_by(|a, b| {
        (priority_rank(a.priority), a.deadline_bound_ms.unwrap_or(i64::MAX), &a.title)
            .cmp(&(priority_rank(b.priority), b.deadline_bound_ms.unwrap_or(i64::MAX), &b.title))
    });
    items
}

fn priority_rank(p: Priority) -> u8 {
    match p {
        Priority::Alta => 0,
        Priority::Media => 1,
        Priority::Baja => 2,
    }
}

/// Planificador determinista.
#[derive(Debug, Clone)]
pub struct Planner {
    pub engine: ConstraintEngine,
    /// Cuántos días hacia adelante se planifica (horizonte).
    pub horizon_days: u32,
    /// Sesión más corta aceptable (evita migajas → fragmentación excesiva).
    pub min_session_min: u32,
    /// Máximo de sesiones por ítem.
    pub max_sessions_per_item: usize,
    /// Tope de minutos por día por ítem (equilibrio de carga). None = sin
    /// tope (no recomendado).
    pub per_day_max_min: Option<u32>,
}

impl Default for Planner {
    fn default() -> Self {
        Planner {
            engine: ConstraintEngine::default(),
            horizon_days: 14,
            min_session_min: 30,
            max_sessions_per_item: 6,
            per_day_max_min: Some(120),
        }
    }
}

impl Planner {
    /// Preferencia de horario declarada en el motor (primer `StartAfter`).
    pub fn preferred_after_min(&self) -> Option<u32> {
        self.engine.preferences.iter().find_map(|p| match p {
            super::SoftPreference::StartAfter { minute } => Some(*minute),
            _ => None,
        })
    }

    /// Planifica todos los ítems. Los ítems ya planificados pasan a ser
    /// compromisos del motor (sin solapamientos ni doble reserva).
    pub fn plan(&self, intents: &[Intent]) -> PlanReport {
        let mut engine = self.engine.clone();
        let mut report = PlanReport { items: Vec::new() };
        for it in order_items(intents) {
            let planned = self.plan_item(&mut engine, &it);
            report.items.push(planned);
        }
        report
    }

    fn plan_item(&self, engine: &mut ConstraintEngine, it: &ItemSpec) -> PlannedItem {
        let from = Local::now().date_naive();
        let mut sessions: Vec<PlanSession> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let mut remaining = it.required_min;
        let mut day_load: std::collections::HashMap<NaiveDate, u32> = Default::default();
        let overdue = it
            .deadline_bound_ms
            .is_some_and(|d| {
                chrono::Local
                    .timestamp_millis_opt(d)
                    .earliest()
                    .is_some_and(|t| t.date_naive() < from)
            });
        if overdue {
            notes.push("vencida: se agenda hoy para recuperarla".into());
        }

        while remaining > 0 {
            let so_far = sessions.len();
            let Some(c) = self.best_candidate(engine, from, it.deadline_bound_ms, remaining, so_far, &day_load) else {
                notes.push("no hay tiempo disponible en el horizonte (restricciones hard)".into());
                break;
            };
            if c.len_min < self.min_session_min {
                if remaining - c.len_min == 0 {
                    // migaja final que completa el ítem → aceptar
                } else if so_far + 1 >= self.max_sessions_per_item {
                    notes.push("demasiada fragmentación: se descarta el resto".into());
                    break;
                } else {
                    notes.push(format!(
                        "hueco de {} min menor que el mínimo ({})",
                        c.len_min, self.min_session_min
                    ));
                }
            }
            if so_far >= self.max_sessions_per_item {
                notes.push("se alcanzó el máximo de sesiones por ítem".into());
                break;
            }
            let prep_left = it.prep_min.saturating_sub(
                sessions.iter().filter(|s| s.is_prep).map(|s| s.end_ms - s.start_ms).sum::<i64>() as u32 / 60_000,
            );
            let is_prep = prep_left > 0;
            sessions.push(PlanSession {
                start_ms: c.start_ms,
                end_ms: c.start_ms + c.len_min as i64 * MIN_MS,
                is_prep,
            });
            // reservar de inmediato: el siguiente candidato no puede
            // reutilizar el mismo hueco (evita doble reserva)
            engine.commitments.push(Block {
                interval: Interval { start: c.start_ms, end: c.start_ms + c.len_min as i64 * MIN_MS },
                label: it.title.clone(),
                severity: Severity::Hard,
            });
            *day_load.entry(c.day).or_insert(0) += c.len_min;
            remaining -= c.len_min;
        }

        let planned_min: u32 = sessions.iter().map(|s| ((s.end_ms - s.start_ms) / MIN_MS) as u32).sum();
        PlannedItem {
            title: it.title.clone(),
            intent_type: it.intent_type,
            priority: it.priority,
            deadline_bound_ms: it.deadline_bound_ms,
            prep_min: it.prep_min,
            task_min: it.task_min,
            required_min: it.required_min,
            planned_min,
            complete: planned_min >= it.required_min,
            sessions,
            notes,
        }
    }

    /// Encuentra el mejor candidato (puntaje más bajo, empate → el más
    /// temprano) para los minutos que faltan.
    fn best_candidate(
        &self,
        engine: &ConstraintEngine,
        from: NaiveDate,
        deadline_bound_ms: Option<i64>,
        remaining: u32,
        sessions_so_far: usize,
        day_load: &std::collections::HashMap<NaiveDate, u32>,
    ) -> Option<Candidate> {
        let pref = self.preferred_after_min();
        let cap = self.per_day_max_min.unwrap_or(u32::MAX);
        let mut best: Option<Candidate> = None;
        let mut best_score = i64::MAX;
        let deadline_day = deadline_bound_ms.and_then(|d| chrono::Local.timestamp_millis_opt(d).earliest().map(|t| t.date_naive()));
        // Vencida (deadline anterior a hoy): se agenda HOY para recuperarla,
        // nunca salta a mañana. El horizonte se reduce al día actual.
        let overdue = deadline_day.is_some_and(|dd| dd < from);
        let horizon = if overdue {
            from
        } else {
            from + chrono::Duration::days(self.horizon_days as i64)
        };
        let mut day = from;
        while day <= horizon {
            if let Some(dd) = deadline_day {
                // solo limita cuando el vencimiento es futuro; vencida ya
                // está acotada por `horizon = from`
                if !overdue && day > dd {
                    break;
                }
            }
            let day_cap_left = cap.saturating_sub(*day_load.get(&day).unwrap_or(&0));
            if day_cap_left > 0 {
                for f in engine.allowed_on(day) {
                    // hoy: el intervalo se recorta a partir de "ahora" (no
                    // se planifica en horas pasadas del día actual)
                    let f = clamp_today(f, day);
                    let iv_len = ((f.end - f.start) / MIN_MS) as u32;
                    if iv_len == 0 {
                        continue;
                    }
                    let start = if let Some(p) = pref {
                        // recortar el intervalo a la preferencia cuando cae dentro
                        let (day_start, _) = super::day_bounds(day);
                        let pref_ms = day_start + p as i64 * MIN_MS;
                        if pref_ms > f.start && pref_ms < f.end {
                            pref_ms
                        } else {
                            f.start
                        }
                    } else {
                        f.start
                    };
                    if start >= f.end {
                        continue;
                    }
                    let len_min = ((f.end - start) / MIN_MS) as u32;
                    let len = len_min.min(remaining).min(day_cap_left);
                    if len == 0 {
                        continue;
                    }
                    // candidato alineado al grid del motor (15')
                    let len = (len / self.engine.step_min.max(1)) * self.engine.step_min.max(1);
                    if len == 0 {
                        continue;
                    }
                    let cand = Candidate { day, start_ms: start, len_min: len };
                    let load = *day_load.get(&day).unwrap_or(&0);
                    let sc = score_candidate(&cand, sessions_so_far, load, pref, deadline_bound_ms);
                    let replace = sc < best_score || (sc == best_score && start < best.map(|b| b.start_ms).unwrap_or(i64::MAX));
                    if replace {
                        best_score = sc;
                        best = Some(cand);
                    }
                }
            }
            day += chrono::Duration::days(1);
        }
        best
    }
}

/// "Mar 19:00–20:00" — nombre de día en español + rango horario.
pub fn fmt_session(start_ms: i64, end_ms: i64) -> (String, String) {
    let s = Local.timestamp_millis_opt(start_ms).earliest().unwrap();
    let e = Local.timestamp_millis_opt(end_ms).earliest().unwrap();
    let name = match s.weekday() {
        Weekday::Mon => "Lun",
        Weekday::Tue => "Mar",
        Weekday::Wed => "Mié",
        Weekday::Thu => "Jue",
        Weekday::Fri => "Vie",
        Weekday::Sat => "Sáb",
        Weekday::Sun => "Dom",
    };
    let hm = format!(
        "{:02}:{:02}–{:02}:{:02}",
        s.hour(),
        s.minute(),
        e.hour(),
        e.minute()
    );
    (name.to_string(), hm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::intent::TimeWindow;
    use super::super::{DayWindow, SoftPreference, DAY_MS, HOUR_MS};

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    fn dt((y, mo, d): (i32, u32, u32), h: u32, m: u32) -> i64 {
        super::super::local_ms(chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, m, 0).unwrap())
    }

    fn day(offset: i64) -> (i32, u32, u32) {
        let t = Local::now().date_naive() + chrono::Duration::days(offset);
        (t.year(), t.month(), t.day())
    }

    fn intent(title: &str, kind: IntentType, priority: Priority, minutes: u32, prep: u32, deadline: Option<i64>) -> Intent {
        Intent {
            intent_type: kind,
            title: title.into(),
            description: String::new(),
            category_id: "uni".into(),
            priority,
            window: TimeWindow { start: None, end: None, all_day: false },
            duration: if minutes > 0 { Some(crate::ai::intent::Duration { minutes }) } else { None },
            deadline,
            preparation: if prep > 0 { Some(crate::ai::intent::Preparation { minutes: prep, note: String::new() }) } else { None },
            recurrence: None,
            reminders: Vec::new(),
            constraints: Vec::new(),
            confidence: 0.9,
            reason: "test".into(),
            source: "local".into(),
        }
    }

    fn engine_free() -> ConstraintEngine {
        ConstraintEngine::default()
    }

    fn planner(engine: ConstraintEngine) -> Planner {
        Planner { engine, ..Planner::default() }
    }

    fn block(e: &mut ConstraintEngine, start: i64, end: i64, label: &str) {
        e.blocks.push(Block { interval: Interval { start, end }, label: label.into(), severity: Severity::Hard });
    }

    fn commit(e: &mut ConstraintEngine, start: i64, end: i64, label: &str) {
        e.commitments.push(Block { interval: Interval { start, end }, label: label.into(), severity: Severity::Hard });
    }

    /// Bloquea hoy para que la planificación arranque en day(1).
    fn no_today(mut e: ConstraintEngine) -> ConstraintEngine {
        let t = Local::now().date_naive();
        let start = super::super::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
        block(&mut e, start, start + DAY_MS, "hoy");
        e
    }

    fn sessions_min(it: &PlannedItem) -> u32 {
        it.sessions.iter().map(|s| ((s.end_ms - s.start_ms) / MIN_MS) as u32).sum()
    }

    fn assert_no_overlap_with(engine: &ConstraintEngine, it: &PlannedItem) {
        for s in &it.sessions {
            assert!(
                engine.is_available(s.start_ms, s.end_ms).is_empty(),
                "sesión {}–{} no es libre: {:?}",
                s.start_ms, s.end_ms,
                engine.is_available(s.start_ms, s.end_ms)
            );
        }
    }

    // ------------------------------------------------------------------
    // casos del enunciado
    // ------------------------------------------------------------------

    #[test]
    fn example_exam_preparation_split() {
        // "I have a calculus exam Friday and need four hours to prepare."
        let friday = Local::now().date_naive() + chrono::Duration::days(6);
        let friday_ms = super::super::local_ms(friday.and_hms_opt(12, 0, 0).unwrap());
        let e = no_today(engine_free());
        let p = planner(e.clone());
        let items = vec![intent("Examen de cálculo", IntentType::Deadline, Priority::Media, 0, 240, Some(friday_ms))];
        let report = p.plan(&items);
        let it = &report.items[0];
        assert!(it.complete, "4h completas: {report:?}");
        assert_eq!(sessions_min(it), 240);
        assert!(it.sessions.len() >= 2, "se divide en varias sesiones");
        assert!(it.sessions.len() <= 6, "fragmentación controlada");
        for s in &it.sessions {
            assert!(s.is_prep);
            assert!(s.end_ms <= friday_ms, "todo termina antes del examen");
            assert!(s.start_ms >= super::super::local_ms(Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap()));
        }
        // per_day_max 120 → 240 min repartidos en ≥2 días (carga equilibrada)
        let days_used = {
            let mut ds: Vec<_> = it.sessions.iter().map(|s| chrono::Local.timestamp_millis_opt(s.start_ms).earliest().unwrap().date_naive()).collect();
            ds.sort();
            ds.dedup();
            ds.len()
        };
        assert!(days_used >= 2, "no amontona todo el mismo día");
        assert_no_overlap_with(&e, it);
    }

    #[test]
    fn no_available_time() {
        let mut e = no_today(engine_free());
        for d in 0..20i64 {
            let t = Local::now().date_naive() + chrono::Duration::days(d);
            let s = super::super::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
            block(&mut e, s, s + DAY_MS, "todo bloqueado");
        }
        let p = planner(e);
        let report = p.plan(&[intent("Tarea", IntentType::Task, Priority::Alta, 60, 0, None)]);
        let it = &report.items[0];
        assert!(!it.complete);
        assert_eq!(it.planned_min, 0);
        assert!(!it.notes.is_empty(), "explica por qué");
        assert!(it.notes[0].contains("no hay tiempo"));
    }

    #[test]
    fn partial_availability() {
        // solo 3h libres en todo el horizonte (mañana del día 1) → planifica lo que cabe
        let mut e = no_today(engine_free());
        for d in 1..15i64 {
            let t = Local::now().date_naive() + chrono::Duration::days(d);
            let s = super::super::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
            if d == 1 {
                block(&mut e, s + 9 * HOUR_MS, s + 24 * HOUR_MS, "tarde ocupada");
            } else {
                block(&mut e, s + 6 * HOUR_MS, s + 24 * HOUR_MS, "día ocupado");
            }
        }
        let p = Planner { engine: e, per_day_max_min: Some(300), ..Planner::default() };
        let report = p.plan(&[intent("Escribir", IntentType::Task, Priority::Alta, 300, 0, None)]);
        let it = &report.items[0];
        assert_eq!(it.planned_min, 180, "solo hay 3h: {report:?}");
        assert!(!it.complete);
    }

    #[test]
    fn conflicting_events_are_avoided() {
        let mut e = no_today(engine_free());
        commit(&mut e, dt(day(1), 9, 0), dt(day(1), 11, 0), "clase");
        commit(&mut e, dt(day(1), 14, 0), dt(day(1), 16, 0), "reunión");
        let p = planner(e.clone());
        let report = p.plan(&[intent("Estudiar", IntentType::Task, Priority::Media, 120, 0, None)]);
        let it = &report.items[0];
        assert!(it.complete);
        assert_no_overlap_with(&e, it);
        for s in &it.sessions {
            assert!(!(s.start_ms < dt(day(1), 11, 0) && s.end_ms > dt(day(1), 9, 0)), "no solapa la clase");
            assert!(!(s.start_ms < dt(day(1), 16, 0) && s.end_ms > dt(day(1), 14, 0)), "no solapa la reunión");
        }
    }

    #[test]
    fn multiple_tasks_no_double_booking() {
        let e = no_today(engine_free());
        let p = planner(e.clone());
        let items = vec![
            intent("A", IntentType::Task, Priority::Media, 120, 0, None),
            intent("B", IntentType::Task, Priority::Media, 120, 0, None),
        ];
        let report = p.plan(&items);
        for it in &report.items {
            assert!(it.complete);
            assert_no_overlap_with(&e, it);
        }
        // las sesiones de A y B no se solapan entre sí
        let mut all: Vec<(i64, i64, &str)> = Vec::new();
        for it in &report.items {
            for s in &it.sessions {
                all.push((s.start_ms, s.end_ms, it.title.as_str()));
            }
        }
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                let a = all[i];
                let b = all[j];
                assert!(
                    a.1 <= b.0 || b.1 <= a.0,
                    "doble reserva: {} {}–{} vs {} {}–{}",
                    a.2, a.0, a.1, b.2, b.0, b.1
                );
            }
        }
        assert_eq!(report.items[0].title, "A");
        assert_eq!(report.items[1].title, "B");
    }

    #[test]
    fn multiple_deadlines_respected() {
        let friday = Local::now().date_naive() + chrono::Duration::days(6);
        let monday = Local::now().date_naive() + chrono::Duration::days(3);
        let friday_ms = super::super::local_ms(friday.and_hms_opt(23, 59, 0).unwrap());
        let monday_ms = super::super::local_ms(monday.and_hms_opt(23, 59, 0).unwrap());
        let e = no_today(engine_free());
        let p = planner(e.clone());
        let items = vec![
            intent("Informe lunes", IntentType::Deadline, Priority::Media, 180, 0, Some(monday_ms)),
            intent("Examen viernes", IntentType::Deadline, Priority::Media, 180, 0, Some(friday_ms)),
        ];
        let report = p.plan(&items);
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.items[0].title, "Informe lunes", "vencimiento más cercano primero");
        for it in &report.items {
            assert!(it.complete);
            for s in &it.sessions {
                assert!(s.end_ms <= it.deadline_bound_ms.unwrap(), "{} respeta su vencimiento", it.title);
            }
            assert_no_overlap_with(&e, it);
        }
    }

    #[test]
    fn priorities_high_first() {
        let e = no_today(engine_free());
        let p = planner(e.clone());
        // solo 2h libres el día 1 → la prioridad Alta se queda el hueco temprano
        let mut e2 = e.clone();
        block(&mut e2, dt(day(1), 11, 0), dt(day(1), 18, 0), "bloque");
        let p2 = planner(e2);
        let report = p2.plan(&[
            intent("Baja", IntentType::Task, Priority::Baja, 120, 0, None),
            intent("Alta", IntentType::Task, Priority::Alta, 120, 0, None),
        ]);
        let alta = report.items.iter().find(|i| i.title == "Alta").unwrap();
        let baja = report.items.iter().find(|i| i.title == "Baja").unwrap();
        assert!(alta.complete, "Alta planificada primero");
        assert!(baja.complete, "Baja también, en otro hueco");
        assert!(
            alta.sessions[0].start_ms <= baja.sessions[0].start_ms,
            "Alta se agenda antes: {:?} vs {:?}",
            alta.sessions, baja.sessions
        );
        assert!(report.items[0].title == "Alta", "orden por prioridad");
        let _ = p;
    }

    #[test]
    fn insufficient_time_reported() {
        // 5h requeridas; horizonte corto con 2h libres por día → 4h máximas
        let e = no_today(engine_free());
        let p = Planner {
            engine: e.clone(),
            horizon_days: 2,
            ..Planner::default()
        };
        let report = p.plan(&[intent("Tesis", IntentType::Task, Priority::Alta, 300, 0, None)]);
        let it = &report.items[0];
        assert!(!it.complete);
        assert_eq!(it.planned_min, 2 * 2 * 60, "planifica lo que hay: {report:?}");
        assert!(it.planned_min < it.required_min);
        assert!(!it.notes.is_empty());
        let _ = e;
    }

    #[test]
    fn preparation_sessions_are_prep_until_full() {
        // "need four hours to prepare" → todas las sesiones son prep
        let friday = Local::now().date_naive() + chrono::Duration::days(6);
        let friday_ms = super::super::local_ms(friday.and_hms_opt(12, 0, 0).unwrap());
        let e = no_today(engine_free());
        let p = planner(e);
        let items = vec![intent("Examen", IntentType::Deadline, Priority::Media, 60, 240, Some(friday_ms))];
        let report = p.plan(&items);
        let it = &report.items[0];
        assert!(it.complete);
        assert_eq!(sessions_min(it), 300);
        let prep_sessions: u32 = it.sessions.iter().filter(|s| s.is_prep).map(|s| ((s.end_ms - s.start_ms) / MIN_MS) as u32).sum();
        assert_eq!(prep_sessions, 240, "las primeras 240 min son preparación");
        // los 60 min finales son la tarea
        assert!(it.sessions.last().map(|s| !s.is_prep).unwrap_or(false));
        for s in &it.sessions {
            assert!(s.end_ms <= friday_ms);
        }
    }

    #[test]
    fn max_sessions_limits_fragmentation() {
        let mut e = no_today(engine_free());
        // solo huecos de 30 min (17:30-18:00) en todo el horizonte: 300 min
        // posibles, pero a 30 min/sesión exige > max_sessions
        for d in 1..15i64 {
            let t = Local::now().date_naive() + chrono::Duration::days(d);
            let s = super::super::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
            block(&mut e, s + 6 * HOUR_MS, s + 21 * HOUR_MS + 30 * MIN_MS, "día ocupado");
        }
        let p = planner(e);
        let report = p.plan(&[intent("Tarea", IntentType::Task, Priority::Media, 300, 0, None)]);
        let it = &report.items[0];
        assert!(it.sessions.len() <= p.max_sessions_per_item, "solo {} sesiones: {:?}", it.sessions.len(), report.to_text());
        assert!(!it.complete, "no se puede completar sin fragmentar de más");
        assert_eq!(it.planned_min, 6 * 30, "6 sesiones de 30 min");
    }

    #[test]
    fn preference_respected_when_possible() {
        let mut e = no_today(engine_free());
        e.working_hours = Some(DayWindow { start_min: 9 * 60, end_min: 21 * 60 });
        e.preferences.push(SoftPreference::StartAfter { minute: 18 * 60 });
        let p = planner(e);
        let report = p.plan(&[intent("Estudiar", IntentType::Task, Priority::Media, 120, 0, None)]);
        let it = &report.items[0];
        assert!(it.complete);
        for s in &it.sessions {
            assert!(time_of_day_min(s.start_ms) >= 18 * 60, "respeta estudiar después de las 18");
        }
    }

    #[test]
    fn preference_yields_if_unavoidable() {
        let mut e = no_today(engine_free());
        e.preferences.push(SoftPreference::StartAfter { minute: 17 * 60 });
        // todo el horizonte con la tarde ocupada → no existe opción ≥17:00
        for d in 1..15i64 {
            let t = Local::now().date_naive() + chrono::Duration::days(d);
            let s = super::super::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
            commit(&mut e, s + 12 * HOUR_MS, s + 24 * HOUR_MS, "tarde ocupada");
        }
        let p = planner(e);
        let report = p.plan(&[intent("Estudiar", IntentType::Task, Priority::Media, 120, 0, None)]);
        let it = &report.items[0];
        assert!(it.complete, "cede y usa la mañana: {report:?}");
        assert!(time_of_day_min(it.sessions[0].start_ms) < 12 * 60, "usa la mañana: {:?}", it.sessions);
    }

    #[test]
    fn fixed_events_are_not_moved() {
        // evento con ventana fija → no se planifica (es compromiso); su prep sí
        let e = no_today(engine_free());
        let mut ev = intent("Examen", IntentType::Event, Priority::Alta, 0, 120, None);
        ev.window = TimeWindow { start: Some(dt(day(2), 10, 0)), end: Some(dt(day(2), 12, 0)), all_day: false };
        let p = planner(e.clone());
        let report = p.plan(&[ev]);
        assert_eq!(report.items.len(), 1);
        let it = &report.items[0];
        assert!(it.complete);
        assert_eq!(sessions_min(it), 120, "solo planifica la preparación");
        for s in &it.sessions {
            assert!(s.end_ms <= dt(day(2), 10, 0), "prep antes del examen fijo");
            assert!(e.is_available(s.start_ms, s.end_ms).is_empty());
        }
    }

    #[test]
    fn not_schedulable_intents_skipped() {
        let p = planner(no_today(engine_free()));
        let report = p.plan(&[
            intent("Disponibilidad", IntentType::Availability, Priority::Media, 0, 0, None),
            intent("Backlog", IntentType::Task, Priority::Media, 0, 0, None),
            intent("Tarea", IntentType::Task, Priority::Media, 60, 0, None),
        ]);
        assert_eq!(report.items.len(), 1, "solo lo dimensionable se planifica");
        assert_eq!(report.items[0].title, "Tarea");
    }

    #[test]
    fn deterministic_repeated_planning() {
        let e = no_today(engine_free());
        let p = planner(e);
        let items = vec![
            intent("A", IntentType::Task, Priority::Media, 150, 30, Some(dt(day(5), 23, 59))),
            intent("B", IntentType::Deadline, Priority::Baja, 90, 0, Some(dt(day(8), 12, 0))),
        ];
        let r1 = p.plan(&items);
        let r2 = p.plan(&items);
        let s1: Vec<_> = r1.items.iter().map(|i| i.sessions.clone()).collect();
        let s2: Vec<_> = r2.items.iter().map(|i| i.sessions.clone()).collect();
        assert_eq!(s1, s2, "mismo estado → mismo plan");
    }

    #[test]
    fn plan_never_schedules_in_past_on_today() {
        // hoy libre (sin no_today): el planner recorta los intervalos a
        // partir de "ahora" → ninguna sesión empieza en horas pasadas.
        let e = engine_free();
        let p = planner(e);
        // "ahora" se captura ANTES de planificar: el planner agenda desde su
        // propio ahora (≥ este valor); capturarlo después creaba una carrera
        // de milisegundos que flakeaba el test.
        let now = Local::now().timestamp_millis();
        let report = p.plan(&[intent("Hoy", IntentType::Task, Priority::Media, 60, 0, None)]);
        for s in &report.items[0].sessions {
            assert!(s.start_ms >= now, "sesión en el pasado: {s:?} (ahora {now})");
        }
    }

    #[test]
    fn overdue_deadline_schedules_today_not_later() {
        // Si queda una tarea vencida (deadline de ayer), el plan de hoy la
        // agenda HOY para recuperarla; no se salta a mañana ni la ignora.
        let e = engine_free();
        let p = planner(e);
        let today = Local::now().date_naive();
        let past = super::super::local_ms((today - chrono::Duration::days(1)).and_hms_opt(12, 0, 0).unwrap());
        let report = p.plan(&[intent("Vencida", IntentType::Task, Priority::Alta, 120, 0, Some(past))]);
        let it = &report.items[0];
        assert!(
            it.sessions
                .iter()
                .all(|s| Local.timestamp_millis_opt(s.start_ms).earliest().unwrap().date_naive() == today),
            "vencida → sesiones HOY, nunca mañana ni más tarde: {report:?}"
        );
        assert!(
            it.notes.iter().any(|n| n.contains("vencida")),
            "explica la decisión: {:?}",
            it.notes
        );
        assert!(it.complete || !it.sessions.is_empty(), "al menos intenta hoy: {report:?}");
    }

    #[test]
    fn default_working_hours_are_06_to_22() {
        let e = engine_free();
        assert_eq!(
            e.working_hours,
            Some(DayWindow { start_min: 6 * 60, end_min: 22 * 60 }),
            "horario unificado 06:00-22:00"
        );
        let day = Local::now().date_naive() + chrono::Duration::days(1);
        let start = super::super::local_ms(day.and_hms_opt(0, 0, 0).unwrap());
        assert_eq!(e.free_intervals_on(day), vec![Interval { start: start + 6 * HOUR_MS, end: start + 22 * HOUR_MS }]);
    }

    #[test]
    fn report_text_matches_example_shape() {
        let friday = Local::now().date_naive() + chrono::Duration::days(6);
        let friday_ms = super::super::local_ms(friday.and_hms_opt(23, 59, 0).unwrap());
        let e = no_today(engine_free());
        let p = planner(e);
        let report = p.plan(&[intent("Calculus preparation", IntentType::Deadline, Priority::Media, 0, 240, Some(friday_ms))]);
        let text = report.to_text();
        assert!(text.contains("Plan:"), "{text}");
        assert!(text.contains("4 horas requeridos"), "{text}");
        assert!(text.contains("Proposed:"), "{text}");
        assert!(text.contains("–"), "rangos con –: {text}");
        println!("{text}");
    }
}
