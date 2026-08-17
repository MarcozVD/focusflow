//! Motor de restricciones determinista (Fase 5).
//!
//! Separa dos responsabilidades (spec/10):
//! - **Interpretación** (módulo `ai`): texto libre → `Intent`. Puede usar LLM.
//! - **Cálculo de horario** (este módulo): qué se puede y qué no se puede
//!   agendar. Pura aritmética de intervalos, sin LLM, sin aleatoriedad.
//!
//! Modelo:
//! - Todo tiempo se expresa en ms epoch **local** (igual que el modelo de
//!   datos, spec/03).
//! - Los intervalos son semiabiertos `[start, end)`: dos eventos adyacentes
//!   (`[9,10)` y `[10,11)`) no se solapan.
//! - Restricciones **hard** (no violables): compromisos existentes, bloques
//!   explícitos, vencimientos, sueño, horario laboral, ventana de
//!   disponibilidad del usuario.
//! - Restricciones **soft** (violables con penalización): horario preferido,
//!   orden preferido.
//! - `suggest_slot` es determinista: escaneo por pasos de `step_min` (15'),
//!   `lookahead_days` (14) desde hoy, primero sin penalización soft y luego
//!   con la menor penalización; empate → el slot más temprano.

pub mod planner;

pub use planner::{PlanReport, PlanSession, PlannedItem, Planner};

use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};

use crate::ai::intent::{ConstraintKind, Intent, IntentType};

pub const MIN_MS: i64 = 60_000;
pub const HOUR_MS: i64 = 3_600_000;
pub const DAY_MS: i64 = 24 * HOUR_MS;

/// Inicio por defecto del horario laboral (06:00). El cap "don't schedule
/// before HH:MM" lo reemplaza; sobre horarios explícitos solo lo eleva.
pub const DEFAULT_WORK_START_MIN: u32 = 6 * 60;

/// ms epoch local para un NaiveDateTime (mismo convenio que `ai::nl`).
/// Gap/solape DST: `earliest()` resuelve de forma determinista; si la hora
/// local no existe (p. ej. medianoche en el salto), se interpreta como UTC
/// en vez de devolver 0 (epoch 1970) — auditoría 17, hallazgo #9.
pub fn local_ms(dt: NaiveDateTime) -> i64 {
    match Local.from_local_datetime(&dt) {
        chrono::LocalResult::Single(d) => d.timestamp_millis(),
        chrono::LocalResult::Ambiguous(d, _) => d.timestamp_millis(),
        chrono::LocalResult::None => dt.and_utc().timestamp_millis(),
    }
}

/// Medianoche local (ms) del día que contiene `ms`.
pub fn local_midnight(ms: i64) -> i64 {
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| local_ms(d.date_naive().and_hms_opt(0, 0, 0).unwrap()))
        .unwrap_or(ms)
}

/// Intervalo semiabierto `[start, end)` en ms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
}

impl Interval {
    pub fn minutes(&self) -> i64 {
        (self.end - self.start) / MIN_MS
    }

    pub fn contains_ms(&self, ms: i64) -> bool {
        self.start <= ms && ms < self.end
    }

    /// Solape real (semiabierto): `[9,10)` y `[10,11)` NO se solapan.
    pub fn overlaps(&self, o: Interval) -> bool {
        self.start < o.end && o.start < self.end
    }

    /// `self` contiene por completo a `o`.
    pub fn contains_interval(&self, o: &Interval) -> bool {
        self.start <= o.start && o.end <= self.end
    }
}

/// Une intervalos solapados de una lista (ordenada o no). Los adyacentes
/// (`[9,10)` + `[10,11)`) NO se fusionan: siguen siendo dos bloques.
pub fn merge(mut list: Vec<Interval>) -> Vec<Interval> {
    if list.len() < 2 {
        return list;
    }
    list.sort_by_key(|i| i.start);
    let mut out: Vec<Interval> = Vec::new();
    for iv in list {
        match out.last_mut() {
            Some(last) if iv.start < last.end => {
                if iv.end > last.end {
                    last.end = iv.end;
                }
            }
            _ => out.push(iv),
        }
    }
    out
}

/// Resta bloques a un intervalo permitido → trozos libres.
/// `blocks` debe estar ordenado (ver [merge]).
pub fn subtract(allowed: &Interval, blocks: &[Interval]) -> Vec<Interval> {
    let mut out = Vec::new();
    let mut cursor = allowed.start;
    for b in blocks {
        if b.end <= cursor {
            continue;
        }
        if b.start > allowed.end {
            break;
        }
        if b.start > cursor {
            out.push(Interval { start: cursor, end: b.start.min(allowed.end) });
        }
        cursor = cursor.max(b.end);
        if cursor >= allowed.end {
            break;
        }
    }
    if cursor < allowed.end {
        out.push(Interval { start: cursor, end: allowed.end });
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Hard,
    Soft,
}

/// Bloqueo con etiqueta (para reportar *qué* bloquea).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub interval: Interval,
    pub label: String,
    pub severity: Severity,
}

/// Horario laboral diario, en minutos desde medianoche local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayWindow {
    pub start_min: u32,
    pub end_min: u32,
}

/// Horas de sueño. `end_min <= start_min` indica que cruza la medianoche
/// (ej: 23:00 → 07:00).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Night {
    pub start_min: u32,
    pub end_min: u32,
}

/// Preferencias soft del usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoftPreference {
    /// "Prefiero estudiar después de las 16:00" → el inicio del evento debe
    /// caer en o después de `minute` (penalización = minutos de adelanto).
    StartAfter { minute: u32 },
    /// "Hacer A antes que B": orden preferido entre títulos. El motor la
    /// conserva y la reporta; el planner (Fase 6) la aplica al ordenar.
    Order { first: String, second: String },
}

/// Vencimiento duro: el ítem debe terminar antes de `at_ms`.
#[derive(Debug, Clone, PartialEq)]
pub struct Deadline {
    pub at_ms: i64,
    pub label: String,
}

/// Resultado de consultar si un intervalo es viable.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
}

/// Propuesta de slot generada por [ConstraintEngine::suggest_slot].
#[derive(Debug, Clone, PartialEq)]
pub struct SlotProposal {
    /// Inicio del bloque de preparación (si `prep_min > 0`).
    pub prep_start_ms: Option<i64>,
    pub task_start_ms: i64,
    pub task_end_ms: i64,
    /// Violaciones soft aceptadas por el slot elegido (vacío = sin soft).
    pub soft_violations: Vec<Violation>,
}

impl SlotProposal {
    pub fn prep_end_ms(&self) -> Option<i64> {
        self.prep_start_ms.map(|s| s + (self.task_start_ms - s))
    }
}

#[derive(Debug, Clone)]
pub struct ConstraintEngine {
    /// Eventos existentes: no se puede agendar encima (hard).
    pub commitments: Vec<Block>,
    /// Bloques explícitos ("tengo clase de 2 a 4") (hard).
    pub blocks: Vec<Block>,
    /// Vencimientos (hard).
    pub deadlines: Vec<Deadline>,
    /// Ventanas de disponibilidad declaradas por el usuario; si no está
    /// vacía, define la región permitida (hard). Vacía → se usa
    /// `working_hours` (o 24h si tampoco hay).
    pub availability: Vec<Interval>,
    /// Horario laboral diario (hard). `None` → 24h.
    pub working_hours: Option<DayWindow>,
    /// Sueño (hard).
    pub sleep: Option<Night>,
    /// Preferencias soft.
    pub preferences: Vec<SoftPreference>,
    /// Duración mínima exigida para agendar un ítem (hard, configuración).
    pub min_duration_min: Option<u32>,
    /// Cuántos días hacia adelante escanea `suggest_slot`.
    pub lookahead_days: u32,
    /// Paso del grid de candidatos, en minutos.
    pub step_min: u32,
}

impl Default for ConstraintEngine {
    fn default() -> Self {
        ConstraintEngine {
            commitments: Vec::new(),
            blocks: Vec::new(),
            deadlines: Vec::new(),
            availability: Vec::new(),
            working_hours: Some(DayWindow { start_min: 6 * 60, end_min: 22 * 60 }),
            sleep: None,
            preferences: Vec::new(),
            min_duration_min: None,
            lookahead_days: 14,
            step_min: 15,
        }
    }
}

impl ConstraintEngine {
    // ------------------------------------------------------------------
    // Construcción desde intents (puente con la fase 3)
    // ------------------------------------------------------------------

    /// Compromiso duro para un bloque `[start, end)` con `label`.
    ///
    /// Un "todo el día" de varios días ("proyecto del lunes al viernes") NO
    /// ocupa las 24 h de cada día cubierto: solo bloquean los días externos.
    /// - Día inicial: completo.
    /// - Días intermedios: libres.
    /// - Día final: si trae hora de cierre (`end` no es medianoche), bloquea
    ///   las 2 h previas; si no, el día queda libre pero la fecha límite cae
    ///   al final del día (22:00) y se registra como vencimiento.
    pub fn push_commitment(&mut self, start: i64, end: i64, all_day: bool, label: String) {
        if !all_day {
            self.commitments.push(Block {
                interval: Interval { start, end },
                label,
                severity: Severity::Hard,
            });
            return;
        }
        let start_day = local_midnight(start);
        let end_day = local_midnight(end);
        if end_day - start_day <= DAY_MS {
            self.commitments.push(Block {
                interval: Interval { start, end },
                label,
                severity: Severity::Hard,
            });
            return;
        }
        let first_end = (start_day + DAY_MS).min(end);
        if first_end > start {
            self.commitments.push(Block {
                interval: Interval { start, end: first_end },
                label: label.clone(),
                severity: Severity::Hard,
            });
        }
        let deadline = if end == end_day {
            end_day + 22 * HOUR_MS
        } else {
            end
        };
        self.deadlines.push(Deadline { at_ms: deadline, label: label.clone() });
        if end != end_day {
            let s = (end - 2 * HOUR_MS).max(end_day);
            if s < end {
                self.commitments.push(Block {
                    interval: Interval { start: s, end },
                    label,
                    severity: Severity::Hard,
                });
            }
        }
    }

    /// Mapea un batch de `Intent` al estado del motor:
    /// - `event` con ventana → compromiso (hard).
    /// - `availability` → ventana de disponibilidad (hard).
    /// - `deadline` → vencimiento (hard).
    /// - `constraint` `daily_cap` (`"HH:MM"`) → inicio mínimo del horario
    ///   laboral (hard).
    pub fn from_intents(intents: &[Intent]) -> ConstraintEngine {
        let mut e = ConstraintEngine::default();
        for i in intents {
            match i.intent_type {
                IntentType::Event => match (i.window.start, i.window.end) {
                    (Some(s), Some(en)) if en > s => {
                        e.push_commitment(s, en, i.window.all_day, i.title.clone());
                    }
                    (Some(s), _) => {
                        e.push_commitment(s, s + DAY_MS, i.window.all_day, i.title.clone());
                    }
                    _ => {}
                },
                IntentType::Availability => {
                    if let (Some(s), Some(en)) = (i.window.start, i.window.end) {
                        if en > s {
                            e.availability.push(Interval { start: s, end: en });
                        }
                    }
                }
                IntentType::Deadline => {
                    if let Some(d) = i.deadline {
                        e.deadlines.push(Deadline { at_ms: d, label: i.title.clone() });
                    }
                }
                IntentType::Constraint => {
                    for c in &i.constraints {
                        if c.kind == ConstraintKind::DailyCap {
                            if let Some(v) = &c.value {
                                if let Some(min) = parse_hhmm(v) {
                                    match e.working_hours {
                                        // cap más tarde → sube el inicio
                                        Some(w) if min > w.start_min => {
                                            e.working_hours = Some(DayWindow {
                                                start_min: min,
                                                end_min: w.end_min,
                                            });
                                        }
                                        // cap más temprano sobre el horario por
                                        // defecto (06:00) → reemplaza el inicio
                                        Some(w) if w.start_min == DEFAULT_WORK_START_MIN => {
                                            e.working_hours = Some(DayWindow {
                                                start_min: min,
                                                end_min: w.end_min,
                                            });
                                        }
                                        Some(_) => {}
                                        None => {
                                            e.working_hours = Some(DayWindow {
                                                start_min: min,
                                                end_min: 23 * 60 + 59,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        e
    }

    // ------------------------------------------------------------------
    // Consultas
    // ------------------------------------------------------------------

    /// ¿Está libre este intervalo? Devuelve los bloqueos que lo impiden
    /// (vacío = libre). Un intervalo inválido (`end <= start`) se reporta
    /// como bloqueo hard "intervalo inválido".
    pub fn is_available(&self, start_ms: i64, end_ms: i64) -> Vec<Block> {
        let iv = Interval { start: start_ms, end: end_ms };
        if end_ms <= start_ms {
            return vec![Block {
                interval: iv,
                label: "intervalo inválido".into(),
                severity: Severity::Hard,
            }];
        }
        let mut out = Vec::new();
        if !self.allowed_region_contains(iv) {
            out.push(Block {
                interval: iv,
                label: "fuera de la ventana permitida".into(),
                severity: Severity::Hard,
            });
        }
        out.extend(self.overlaps_in_interval(iv));
        out
    }

    /// Minutos disponibles dentro de `[from, to)` (solo región permitida).
    pub fn available_minutes(&self, from_ms: i64, to_ms: i64) -> i64 {
        let mut total = 0i64;
        let d0 = ms_to_day(from_ms);
        let d1 = ms_to_day(to_ms - 1);
        let mut d = d0;
        while d <= d1 {
            let (dd0, dd1) = day_bounds(d);
            let clip = Interval { start: from_ms.max(dd0), end: to_ms.min(dd1) };
            for f in self.allowed_on(d) {
                if f.overlaps(clip) {
                    total += f.end.min(clip.end) - f.start.max(clip.start);
                }
            }
            d += chrono::Duration::days(1);
        }
        total / MIN_MS
    }

    /// Todos los bloqueos hard de un día (compromisos, bloques, sueño),
    /// recortados al día y ordenados por inicio.
    pub fn blocked_intervals_on(&self, day: NaiveDate) -> Vec<Block> {
        let (d0, d1) = day_bounds(day);
        let clip = |iv: Interval| {
            let s = iv.start.max(d0);
            let e = iv.end.min(d1);
            if s >= e {
                None
            } else {
                Some(Interval { start: s, end: e })
            }
        };
        let mut out: Vec<Block> = Vec::new();
        for b in &self.commitments {
            if let Some(c) = clip(b.interval) {
                out.push(Block { interval: c, label: b.label.clone(), severity: Severity::Hard });
            }
        }
        for b in &self.blocks {
            if let Some(c) = clip(b.interval) {
                out.push(Block { interval: c, label: b.label.clone(), severity: Severity::Hard });
            }
        }
        out.extend(self.sleep_blocks_on(day));
        out.sort_by_key(|b| b.interval.start);
        out
    }

    /// Intervalos libres de un día (región permitida menos todo lo hard).
    pub fn free_intervals_on(&self, day: NaiveDate) -> Vec<Interval> {
        self.allowed_on(day)
    }

    /// Restricciones registradas, con severidad. Respuesta a
    /// "¿cuáles son soft?" → filtrar `Severity::Soft`.
    pub fn all_constraints(&self) -> Vec<(String, Severity)> {
        let mut out: Vec<(String, Severity)> = Vec::new();
        for b in &self.commitments {
            out.push((format!("compromiso: {}", b.label), Severity::Hard));
        }
        for b in &self.blocks {
            out.push((format!("bloqueo: {}", b.label), Severity::Hard));
        }
        for d in &self.deadlines {
            out.push((format!("vencimiento: {}", d.label), Severity::Hard));
        }
        if !self.availability.is_empty() {
            out.push(("disponibilidad declarada".into(), Severity::Hard));
        }
        if let Some(w) = self.working_hours {
            out.push((
                format!("horario laboral {:02}:{:02}-{:02}:{:02}", w.start_min / 60, w.start_min % 60, w.end_min / 60, w.end_min % 60),
                Severity::Hard,
            ));
        }
        if self.sleep.is_some() {
            out.push(("sueño".into(), Severity::Hard));
        }
        if let Some(m) = self.min_duration_min {
            out.push((format!("duración mínima {} min", m), Severity::Hard));
        }
        for p in &self.preferences {
            out.push((format!("{p:?}"), Severity::Soft));
        }
        out
    }

    /// Violaciones (hard y soft) de un intervalo propuesto, con opción de
    /// vencimiento externo (distinto de los `deadlines` del motor).
    pub fn violations(&self, start_ms: i64, end_ms: i64, deadline_ms: Option<i64>) -> Vec<Violation> {
        let iv = Interval { start: start_ms, end: end_ms };
        let mut v: Vec<Violation> = Vec::new();
        if end_ms <= start_ms {
            v.push(Violation {
                rule: "intervalo inválido".into(),
                severity: Severity::Hard,
                message: "el final debe ser posterior al inicio".into(),
            });
            return v;
        }
        if !self.allowed_region_contains(iv) {
            v.push(Violation {
                rule: "ventana permitida".into(),
                severity: Severity::Hard,
                message: "fuera de disponibilidad/horario laboral".into(),
            });
        }
        for b in self.overlaps_in_interval(iv) {
            v.push(Violation {
                rule: b.label.clone(),
                severity: Severity::Hard,
                message: format!("choca con {}", b.label),
            });
        }
        for d in &self.deadlines {
            if end_ms > d.at_ms {
                v.push(Violation {
                    rule: format!("vencimiento: {}", d.label),
                    severity: Severity::Hard,
                    message: format!("termina después del vencimiento ({})", d.label),
                });
            }
        }
        if let Some(dl) = deadline_ms {
            if end_ms > dl {
                v.push(Violation {
                    rule: "vencimiento externo".into(),
                    severity: Severity::Hard,
                    message: "termina después del vencimiento pedido".into(),
                });
            }
        }
        for p in &self.preferences {
            if let SoftPreference::StartAfter { minute } = p {
                if time_of_day_min(start_ms) < *minute {
                    v.push(Violation {
                        rule: format!("preferencia: empezar después de las {:02}:{:02}", minute / 60, minute % 60),
                        severity: Severity::Soft,
                        message: "empieza antes del horario preferido".into(),
                    });
                }
            }
        }
        v
    }

    // ------------------------------------------------------------------
    // Planeo determinista de un slot
    // ------------------------------------------------------------------

    /// Primer slot viable (determinista) para una tarea de `duration_min`
    /// con `prep_min` de preparación contigua previa.
    ///
    /// Reglas:
    /// - `duration_min < min_duration_min` → `None` (duración insuficiente).
    /// - Escanea `lookahead_days` días desde hoy, en pasos de `step_min`
    ///   sobre los intervalos libres. En el día actual solo se consideran
    ///   horas a partir de "ahora" (nunca en el pasado).
    /// - Solo candidatos sin violación hard (región permitida, sin choques,
    ///   termina antes de `deadline_ms`).
    /// - Penalización soft: minutos que el inicio de la tarea se adelanta a
    ///   `preferred_after_min` (si se pide).
    /// - Gana el candidato con menor penalización; empate → el más temprano.
    pub fn suggest_slot(
        &self,
        duration_min: u32,
        prep_min: u32,
        deadline_ms: Option<i64>,
        preferred_after_min: Option<u32>,
    ) -> Option<SlotProposal> {
        if let Some(m) = self.min_duration_min {
            if duration_min < m {
                return None;
            }
        }
        let total = (duration_min + prep_min) as i64 * MIN_MS;
        let step = self.step_min.max(1) as i64 * MIN_MS;
        let today = Local::now().date_naive();
        let mut best: Option<(i64, i64, SlotProposal)> = None;
        for d in 0..self.lookahead_days {
            let day = today + chrono::Duration::days(d as i64);
            for f in self.allowed_on(day) {
                // hoy: nunca antes de "ahora" (horas pasadas no se agendan)
                let f = clamp_today(f, day);
                if f.end - f.start < total {
                    continue;
                }
                let mut s = grid_ceil(f.start, step);
                while s + total <= f.end {
                    let task_start = s + prep_min as i64 * MIN_MS;
                    let task_end = task_start + duration_min as i64 * MIN_MS;
                    let mut ok = true;
                    if let Some(dl) = deadline_ms {
                        if task_end > dl {
                            ok = false;
                        }
                    }
                    if ok {
                        let penalty = match preferred_after_min {
                            Some(a) => {
                                let tod = time_of_day_min(task_start);
                                if tod < a {
                                    (a - tod) as i64
                                } else {
                                    0
                                }
                            }
                            None => 0,
                        };
                        let prop = SlotProposal {
                            prep_start_ms: if prep_min > 0 { Some(s) } else { None },
                            task_start_ms: task_start,
                            task_end_ms: task_end,
                            soft_violations: Vec::new(),
                        };
                        let replace = match &best {
                            Some((bp, bs, _)) => (penalty, task_start) < (*bp, *bs),
                            None => true,
                        };
                        if replace {
                            best = Some((penalty, task_start, prop));
                        }
                    }
                    s += step;
                }
            }
        }
        best.map(|(_, _, p)| p)
    }

    // ------------------------------------------------------------------
    // Internos
    // ------------------------------------------------------------------

    fn allowed_region_contains(&self, iv: Interval) -> bool {
        let d0 = ms_to_day(iv.start);
        let d1 = ms_to_day(iv.end - 1);
        let mut d = d0;
        while d <= d1 {
            let (dd0, dd1) = day_bounds(d);
            let clip = Interval { start: iv.start.max(dd0), end: iv.end.min(dd1) };
            if clip.start < clip.end {
                let base = self.base_region_on(d);
                if !base.iter().any(|a| a.contains_interval(&clip)) {
                    return false;
                }
            }
            d += chrono::Duration::days(1);
        }
        true
    }

    fn overlaps_in_interval(&self, iv: Interval) -> Vec<Block> {
        let mut out: Vec<Block> = Vec::new();
        for b in &self.commitments {
            if b.interval.overlaps(iv) {
                out.push(Block {
                    interval: Interval { start: iv.start.max(b.interval.start), end: iv.end.min(b.interval.end) },
                    label: b.label.clone(),
                    severity: Severity::Hard,
                });
            }
        }
        for b in &self.blocks {
            if b.interval.overlaps(iv) {
                out.push(Block {
                    interval: Interval { start: iv.start.max(b.interval.start), end: iv.end.min(b.interval.end) },
                    label: b.label.clone(),
                    severity: Severity::Hard,
                });
            }
        }
        for b in self.sleep_blocks_in(iv) {
            out.push(b);
        }
        out
    }

    /// Región base permitida de un día: disponibilidad ∩ horario laboral
    /// (o cada uno por separado si el otro no existe; 24h si ninguno).
    fn base_region_on(&self, day: NaiveDate) -> Vec<Interval> {
        let (d0, d1) = day_bounds(day);
        let availability: Vec<Interval> = if !self.availability.is_empty() {
            self.availability
                .iter()
                .filter(|a| a.start < d1 && a.end > d0)
                .map(|a| Interval { start: a.start.max(d0), end: a.end.min(d1) })
                .collect()
        } else {
            Vec::new()
        };
        let working: Vec<Interval> = match self.working_hours {
            Some(w) => vec![Interval {
                start: d0 + w.start_min as i64 * MIN_MS,
                end: d0 + w.end_min as i64 * MIN_MS,
            }],
            None => Vec::new(),
        };
        if availability.is_empty() {
            if working.is_empty() {
                vec![Interval { start: d0, end: d1 }]
            } else {
                working
            }
        } else if working.is_empty() {
            availability
        } else {
            let mut out = Vec::new();
            for a in &availability {
                for w in &working {
                    if let Some(iv) = intersect_pair(*a, *w) {
                        out.push(iv);
                    }
                }
            }
            out
        }
    }

    /// Región permitida de un día: región base menos sueño, compromisos y
    /// bloques.
    pub fn allowed_on(&self, day: NaiveDate) -> Vec<Interval> {
        let base = self.base_region_on(day);
        let (d0, d1) = day_bounds(day);
        let mut hard: Vec<Interval> = self
            .sleep_blocks_on(day)
            .into_iter()
            .map(|b| b.interval)
            .collect();
        for b in &self.commitments {
            if b.interval.start < d1 && b.interval.end > d0 {
                hard.push(Interval { start: b.interval.start.max(d0), end: b.interval.end.min(d1) });
            }
        }
        for b in &self.blocks {
            if b.interval.start < d1 && b.interval.end > d0 {
                hard.push(Interval { start: b.interval.start.max(d0), end: b.interval.end.min(d1) });
            }
        }
        let hard = merge(hard);
        let mut out = Vec::new();
        for piece in base {
            out.extend(subtract(&piece, &hard));
        }
        out
    }

    /// Bloques de sueño que caen dentro de un intervalo (recortados a él).
    fn sleep_blocks_in(&self, iv: Interval) -> Vec<Block> {
        let mut out = Vec::new();
        let d0 = ms_to_day(iv.start);
        let d1 = ms_to_day(iv.end - 1);
        let mut d = d0;
        while d <= d1 {
            for b in self.sleep_blocks_on(d) {
                if b.interval.overlaps(iv) {
                    out.push(Block {
                        interval: Interval { start: iv.start.max(b.interval.start), end: iv.end.min(b.interval.end) },
                        label: b.label.clone(),
                        severity: Severity::Hard,
                    });
                }
            }
            d += chrono::Duration::days(1);
        }
        out
    }

    /// Bloques de sueño de un día, recortados al día. `end_min <= start_min`
    /// → cruce de medianoche ([start..24:00) de hoy + [00:00..end) de mañana).
    fn sleep_blocks_on(&self, day: NaiveDate) -> Vec<Block> {
        let Some(n) = self.sleep else { return Vec::new() };
        let (d0, d1) = day_bounds(day);
        let mk = |start_min: u32, end_min: u32, offset: i64| -> Option<Block> {
            let s = (d0 + offset + start_min as i64 * MIN_MS).max(d0);
            let e = (d0 + offset + end_min as i64 * MIN_MS).min(d1);
            if s >= e {
                None
            } else {
                Some(Block {
                    interval: Interval { start: s, end: e },
                    label: "sueño".into(),
                    severity: Severity::Hard,
                })
            }
        };
        let mut out = Vec::new();
        if n.end_min <= n.start_min {
            if let Some(b) = mk(n.start_min, 24 * 60, 0) {
                out.push(b);
            }
            if let Some(b) = mk(0, n.end_min, DAY_MS) {
                out.push(b);
            }
        } else if let Some(b) = mk(n.start_min, n.end_min, 0) {
            out.push(b);
        }
        out
    }
}

/// Intersección de dos intervalos (None si no se solapan).
fn intersect_pair(a: Interval, b: Interval) -> Option<Interval> {
    let s = a.start.max(b.start);
    let e = a.end.min(b.end);
    if s < e {
        Some(Interval { start: s, end: e })
    } else {
        None
    }
}

fn parse_hhmm(v: &str) -> Option<u32> {
    let parts: Vec<&str> = v.split(':').collect();
    let h: u32 = parts.first()?.parse().ok()?;
    let m: u32 = match parts.get(1) {
        Some(s) => s.parse().ok()?,
        None => 0,
    };
    if h <= 23 && m <= 59 {
        Some(h * 60 + m)
    } else {
        None
    }
}

fn day_bounds(day: NaiveDate) -> (i64, i64) {
    let start = local_ms(day.and_hms_opt(0, 0, 0).unwrap());
    (start, start + DAY_MS)
}

fn ms_to_day(ms: i64) -> NaiveDate {
    Local.timestamp_millis_opt(ms).earliest().map(|d| d.date_naive()).unwrap_or_else(|| Local::now().date_naive())
}

fn grid_ceil(ms: i64, step: i64) -> i64 {
    let rem = ms.rem_euclid(step);
    if rem == 0 {
        ms
    } else {
        ms + step - rem
    }
}

/// Recorta un intervalo de HOY a partir de "ahora": el planificador nunca
/// agenda en horas ya pasadas del día actual. El resto de días no se toca.
pub(crate) fn clamp_today(iv: Interval, day: NaiveDate) -> Interval {
    if day != Local::now().date_naive() {
        return iv;
    }
    let now = Local::now().timestamp_millis();
    Interval { start: iv.start.max(now), end: iv.end }
}

/// Minutos desde medianoche local (hora del día) de un instante.
fn time_of_day_min(ms: i64) -> u32 {
    Local
        .timestamp_millis_opt(ms)
        .earliest()
        .map(|d| d.hour() * 60 + d.minute())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    fn dt((y, mo, d): (i32, u32, u32), h: u32, m: u32) -> i64 {
        local_ms(chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, m, 0).unwrap())
    }

    fn day(offset_days: i64) -> (i32, u32, u32) {
        let t = Local::now().date_naive() + chrono::Duration::days(offset_days);
        (t.year(), t.month(), t.day())
    }

    /// Motor base: horario 9-18, sin sueño, sin bloques.
    fn base() -> ConstraintEngine {
        ConstraintEngine::default()
    }

    /// Bloquea hoy por completo para que los slots propuestos caigan en
    /// `day(1)` y el test no dependa de la hora del día.
    fn no_today(mut e: ConstraintEngine) -> ConstraintEngine {
        let t = Local::now().date_naive();
        let (t0, _) = day_bounds(t);
        e.blocks.push(hard(iv(t0, t0 + DAY_MS), "hoy"));
        e
    }

    fn iv(start_ms: i64, end_ms: i64) -> Interval {
        Interval { start: start_ms, end: end_ms }
    }

    fn hard(iv: Interval, label: &str) -> Block {
        Block { interval: iv, label: label.into(), severity: Severity::Hard }
    }

    // ------------------------------------------------------------------
    // Semántica de intervalos
    // ------------------------------------------------------------------

    #[test]
    fn interval_semantics_half_open() {
        let a = iv(dt(day(1), 9, 0), dt(day(1), 10, 0));
        let b = iv(dt(day(1), 10, 0), dt(day(1), 11, 0));
        assert!(!a.overlaps(b), "adyacentes no se solapan");
        assert!(a.contains_ms(dt(day(1), 9, 30)));
        assert!(!a.contains_ms(dt(day(1), 10, 0)), "end es exclusivo");
        assert_eq!(a.minutes(), 60);
        assert!(a.overlaps(iv(dt(day(1), 9, 30), dt(day(1), 9, 45))));
        assert!(iv(dt(day(1), 8, 0), dt(day(1), 12, 0)).contains_interval(&a));
    }

    #[test]
    fn merge_joins_overlap_not_adjacency() {
        let m = merge(vec![
            iv(dt(day(1), 10, 0), dt(day(1), 12, 0)),
            iv(dt(day(1), 9, 0), dt(day(1), 11, 0)),
            iv(dt(day(1), 12, 0), dt(day(1), 13, 0)),
        ]);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0], iv(dt(day(1), 9, 0), dt(day(1), 12, 0)));
        assert_eq!(m[1], iv(dt(day(1), 12, 0), dt(day(1), 13, 0)));
    }

    #[test]
    fn subtract_splits_allowed_region() {
        let allowed = iv(dt(day(1), 9, 0), dt(day(1), 18, 0));
        let blocks = merge(vec![
            iv(dt(day(1), 10, 0), dt(day(1), 11, 0)),
            iv(dt(day(1), 12, 0), dt(day(1), 13, 0)),
        ]);
        let free = subtract(&allowed, &blocks);
        assert_eq!(free.len(), 3);
        assert_eq!(free[0], iv(dt(day(1), 9, 0), dt(day(1), 10, 0)));
        assert_eq!(free[1], iv(dt(day(1), 11, 0), dt(day(1), 12, 0)));
        assert_eq!(free[2], iv(dt(day(1), 13, 0), dt(day(1), 18, 0)));
    }

    // ------------------------------------------------------------------
    // Eventos solapados y adyacentes
    // ------------------------------------------------------------------

    #[test]
    fn overlapping_events_block_union() {
        let mut e = base();
        e.commitments = vec![
            hard(iv(dt(day(1), 9, 0), dt(day(1), 11, 0)), "A"),
            hard(iv(dt(day(1), 10, 0), dt(day(1), 12, 0)), "B"),
        ];
        let b = e.is_available(dt(day(1), 9, 30), dt(day(1), 10, 30));
        assert_eq!(b.len(), 2, "choca con ambos: {b:?}");
        let free = e.free_intervals_on(Local::now().date_naive() + chrono::Duration::days(1));
        assert_eq!(free[0], iv(dt(day(1), 6, 0), dt(day(1), 9, 0)), "mañana libre");
        assert_eq!(free[1], iv(dt(day(1), 12, 0), dt(day(1), 22, 0)), "unión [9,12) bloqueada");
        assert_eq!(e.available_minutes(dt(day(1), 9, 0), dt(day(1), 18, 0)), 360);
    }

    #[test]
    fn adjacent_events_no_false_overlap() {
        let mut e = base();
        e.commitments = vec![
            hard(iv(dt(day(1), 9, 0), dt(day(1), 10, 0)), "A"),
            hard(iv(dt(day(1), 10, 0), dt(day(1), 11, 0)), "B"),
        ];
        assert!(!e.is_available(dt(day(1), 9, 0), dt(day(1), 10, 0)).is_empty(), "A ocupado");
        assert!(!e.is_available(dt(day(1), 10, 0), dt(day(1), 11, 0)).is_empty(), "B ocupado");
        let free = e.free_intervals_on(Local::now().date_naive() + chrono::Duration::days(1));
        assert_eq!(free.len(), 2);
        assert_eq!(free[0], iv(dt(day(1), 6, 0), dt(day(1), 9, 0)));
        assert_eq!(free[1], iv(dt(day(1), 11, 0), dt(day(1), 22, 0)));
        assert_eq!(e.available_minutes(dt(day(1), 9, 0), dt(day(1), 18, 0)), 7 * 60);
    }

    // ------------------------------------------------------------------
    // Tiempo bloqueado explícito
    // ------------------------------------------------------------------

    #[test]
    fn explicit_blocked_time() {
        let mut e = base();
        e.blocks.push(hard(iv(dt(day(1), 12, 0), dt(day(1), 13, 0)), "almuerzo"));
        let b = e.is_available(dt(day(1), 12, 30), dt(day(1), 13, 0));
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].label, "almuerzo");
        assert!(e.is_available(dt(day(1), 13, 0), dt(day(1), 14, 0)).is_empty());
        assert_eq!(e.available_minutes(dt(day(1), 9, 0), dt(day(1), 18, 0)), 8 * 60);
    }

    // ------------------------------------------------------------------
    // Vencimientos
    // ------------------------------------------------------------------

    #[test]
    fn deadline_respected() {
        let mut e = base();
        e.deadlines.push(Deadline { at_ms: dt(day(1), 15, 0), label: "informe".into() });
        let v = e.violations(dt(day(1), 9, 0), dt(day(1), 16, 0), None);
        assert!(v.iter().any(|x| x.severity == Severity::Hard && x.rule.contains("vencimiento")));
        let v2 = e.violations(dt(day(1), 9, 0), dt(day(1), 15, 0), None);
        assert!(v2.is_empty(), "termina justo en el vencimiento → ok");
        let slot = e.suggest_slot(240, 0, Some(dt(day(1), 15, 0)), None).unwrap();
        assert!(slot.task_end_ms <= dt(day(1), 15, 0));
    }

    #[test]
    fn deadline_across_days() {
        let mut e = no_today(base());
        e.deadlines.push(Deadline { at_ms: dt(day(3), 10, 0), label: "entrega".into() });
        let slot = e.suggest_slot(300, 0, None, None).unwrap();
        assert!(slot.task_end_ms <= dt(day(3), 10, 0), "5h antes del vencimiento: {slot:?}");
        assert_eq!(slot.task_start_ms, dt(day(1), 6, 0), "primer slot hábil");
    }

    // ------------------------------------------------------------------
    // Duración mínima
    // ------------------------------------------------------------------

    #[test]
    fn minimum_duration_enforced() {
        let mut e = no_today(base());
        e.min_duration_min = Some(120);
        e.lookahead_days = 2;
        assert!(e.suggest_slot(60, 0, None, None).is_none(), "60 < 120 → rechazado");
        e.blocks.push(hard(iv(dt(day(1), 8, 0), dt(day(1), 22, 0)), "bloque"));
        let s60 = e.suggest_slot(120, 0, None, None).unwrap();
        assert_eq!(s60.task_start_ms, dt(day(1), 6, 0), "solo 06:00-08:00 libre");
        assert!(e.suggest_slot(240, 0, None, None).is_none(), "solo quedan 120 min");
    }

    // ------------------------------------------------------------------
    // Ventanas de disponibilidad
    // ------------------------------------------------------------------

    #[test]
    fn availability_window_restricts() {
        let mut e = no_today(base());
        e.availability = vec![iv(dt(day(1), 8, 0), dt(day(1), 11, 0))];
        let s = e.suggest_slot(90, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 8, 0), "intersección con horario laboral");
        assert_eq!(s.task_end_ms, dt(day(1), 9, 30));
        let b = e.is_available(dt(day(1), 5, 0), dt(day(1), 6, 0));
        assert!(!b.is_empty(), "antes del horario laboral → bloqueado");
        assert!(e.is_available(dt(day(1), 14, 0), dt(day(1), 15, 0)).len() == 1);
    }

    #[test]
    fn availability_only_region() {
        let mut e = no_today(base());
        e.availability = vec![iv(dt(day(1), 14, 0), dt(day(1), 16, 0))];
        let s = e.suggest_slot(60, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 14, 0));
        assert!(e.is_available(dt(day(1), 10, 0), dt(day(1), 11, 0)).len() == 1);
    }

    // ------------------------------------------------------------------
    // Múltiples restricciones combinadas
    // ------------------------------------------------------------------

    #[test]
    fn multiple_constraints_combined() {
        let mut e = no_today(base());
        e.commitments.push(hard(iv(dt(day(1), 10, 0), dt(day(1), 11, 0)), "reunión"));
        e.blocks.push(hard(iv(dt(day(1), 12, 0), dt(day(1), 13, 0)), "almuerzo"));
        e.blocks.push(hard(iv(dt(day(1), 6, 0), dt(day(1), 10, 0)), "madrugada"));
        e.deadlines.push(Deadline { at_ms: dt(day(1), 15, 0), label: "informe".into() });
        e.lookahead_days = 2;

        let b = e.is_available(dt(day(1), 10, 30), dt(day(1), 12, 30));
        let labels: Vec<&str> = b.iter().map(|x| x.label.as_str()).collect();
        assert!(labels.contains(&"reunión"), "{labels:?}");
        assert!(labels.contains(&"almuerzo"), "{labels:?}");

        let v = e.violations(dt(day(1), 13, 0), dt(day(1), 16, 0), None);
        assert!(v.iter().any(|x| x.rule.contains("informe")));

        // 4h contiguas terminando antes de las 15:00 (reunión 10-11,
        // almuerzo 12-13) → imposible; 2h → 13:00-15:00
        assert!(e.suggest_slot(240, 0, Some(dt(day(1), 15, 0)), None).is_none());
        let s = e.suggest_slot(120, 0, Some(dt(day(1), 15, 0)), None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 13, 0), "primer hueco contiguo de 2h");
    }

    #[test]
    fn conflicting_constraints_infeasible() {
        // horario 6-22 pero vencimiento a las 08:00 con 6h de tarea → imposible
        let e = no_today(base());
        let slot = e.suggest_slot(360, 0, Some(dt(day(1), 8, 0)), None);
        assert!(slot.is_none(), "no cabe 6h antes de las 8");
        let slot2 = e.suggest_slot(120, 0, Some(dt(day(1), 8, 0)), None);
        assert_eq!(slot2.unwrap().task_end_ms, dt(day(1), 8, 0), "120 min caben 06:00-08:00");
    }

    #[test]
    fn sleep_shrinks_horizon() {
        let mut e = base();
        e.working_hours = Some(DayWindow { start_min: 7 * 60, end_min: 18 * 60 });
        e.sleep = Some(Night { start_min: 23 * 60, end_min: 7 * 60 });
        let free = e.free_intervals_on(Local::now().date_naive() + chrono::Duration::days(1));
        assert_eq!(free[0], iv(dt(day(1), 7, 0), dt(day(1), 18, 0)), "sueño hasta las 7, luego hábil");
        assert!(!e.is_available(dt(day(1), 22, 30), dt(day(1), 23, 30)).is_empty(), "22:30-23:00 libre, 23:00-23:30 sueño");
        assert!(!e.is_available(dt(day(2), 6, 30), dt(day(2), 7, 0)).is_empty(), "cruce de medianoche bloqueado");
        assert!(e.is_available(dt(day(2), 7, 0), dt(day(2), 8, 0)).is_empty(), "07:00 ya es hábil");
    }

    // ------------------------------------------------------------------
    // Restricciones soft
    // ------------------------------------------------------------------

    #[test]
    fn soft_preference_start_after() {
        let mut e = no_today(base());
        e.preferences.push(SoftPreference::StartAfter { minute: 16 * 60 });
        let s = e.suggest_slot(90, 0, None, Some(16 * 60)).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 16, 0), "respeta la preferencia si es posible");
        let v = e.violations(dt(day(1), 9, 0), dt(day(1), 10, 0), None);
        assert!(v.iter().any(|x| x.severity == Severity::Soft));
    }

    #[test]
    fn soft_preference_yields_when_infeasible() {
        let mut e = no_today(base());
        e.preferences.push(SoftPreference::StartAfter { minute: 17 * 60 });
        e.commitments.push(hard(iv(dt(day(1), 17, 0), dt(day(1), 22, 0)), "fijo"));
        let s = e.suggest_slot(60, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 6, 0), "cede y usa el primer hueco");
    }

    #[test]
    fn soft_constraints_listed() {
        let mut e = base();
        e.preferences = vec![
            SoftPreference::StartAfter { minute: 16 * 60 },
            SoftPreference::Order { first: "Estudiar".into(), second: "Entrenar".into() },
        ];
        let all = e.all_constraints();
        assert!(all.len() >= 2);
        assert!(all.iter().filter(|(_, s)| *s == Severity::Soft).count() == 2);
        let hard_count = all.iter().filter(|(_, s)| *s == Severity::Hard).count();
        assert!(hard_count >= 1, "horario laboral está hard");
    }

    // ------------------------------------------------------------------
    // Preparación previa
    // ------------------------------------------------------------------

    #[test]
    fn preparation_occupies_contiguous_block_before() {
        let e = no_today(base());
        let s = e.suggest_slot(120, 60, None, None).unwrap();
        assert_eq!(s.prep_start_ms, Some(dt(day(1), 6, 0)));
        assert_eq!(s.task_start_ms, dt(day(1), 7, 0));
        assert_eq!(s.task_end_ms, dt(day(1), 9, 0));
        let span = Interval { start: s.prep_start_ms.unwrap(), end: s.task_end_ms };
        assert!(e.is_available(span.start, span.end).is_empty(), "prep+tarea ocupan un bloque libre");
    }

    #[test]
    fn preparation_plus_deadline() {
        let e = no_today(base());
        let s = e.suggest_slot(120, 60, Some(dt(day(1), 12, 30)), None).unwrap();
        assert_eq!(s.task_end_ms, dt(day(1), 9, 0), "prep y tarea terminan antes del vencimiento");
    }

    // ------------------------------------------------------------------
    // Consultas de reporte
    // ------------------------------------------------------------------

    #[test]
    fn blocked_intervals_report() {
        let mut e = base();
        e.commitments.push(hard(iv(dt(day(1), 10, 0), dt(day(1), 11, 0)), "reunión"));
        e.blocks.push(hard(iv(dt(day(1), 12, 0), dt(day(1), 13, 0)), "almuerzo"));
        e.sleep = Some(Night { start_min: 23 * 60, end_min: 7 * 60 });
        let blocks = e.blocked_intervals_on(Local::now().date_naive() + chrono::Duration::days(1));
        let labels: Vec<&str> = blocks.iter().map(|b| b.label.as_str()).collect();
        assert!(labels.contains(&"reunión"));
        assert!(labels.contains(&"almuerzo"));
        assert!(labels.contains(&"sueño"));
        let sorted = blocks.windows(2).all(|w| w[0].interval.start <= w[1].interval.start);
        assert!(sorted, "reporte ordenado por inicio");
    }

    #[test]
    fn invalid_interval_reported() {
        let e = base();
        let b = e.is_available(dt(day(1), 12, 0), dt(day(1), 11, 0));
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].label, "intervalo inválido");
        let v = e.violations(dt(day(1), 12, 0), dt(day(1), 11, 0), None);
        assert!(!v.is_empty());
    }

    #[test]
    fn available_minutes_query() {
        let mut e = base();
        e.commitments.push(hard(iv(dt(day(1), 10, 0), dt(day(1), 11, 0)), "reunión"));
        assert_eq!(e.available_minutes(dt(day(1), 9, 0), dt(day(1), 12, 0)), 120);
        assert_eq!(e.available_minutes(dt(day(1), 9, 0), dt(day(1), 18, 0)), 8 * 60);
        assert_eq!(e.available_minutes(dt(day(2), 9, 0), dt(day(2), 18, 0)), 9 * 60);
    }

    #[test]
    fn free_slot_is_reported_free() {
        let e = base();
        assert!(e.is_available(dt(day(1), 9, 0), dt(day(1), 10, 0)).is_empty());
        assert_eq!(e.violations(dt(day(1), 9, 0), dt(day(1), 10, 0), None).len(), 0);
    }

    #[test]
    fn working_hours_enforced_when_present() {
        let e = base();
        assert!(!e.is_available(dt(day(1), 23, 0), dt(day(2), 0, 0)).is_empty(), "fuera del horario 06-22");
        assert!(e.is_available(dt(day(1), 17, 0), dt(day(1), 18, 0)).is_empty());
        let free = e.free_intervals_on(Local::now().date_naive() + chrono::Duration::days(1));
        assert_eq!(free, vec![iv(dt(day(1), 6, 0), dt(day(1), 22, 0))]);
    }

    #[test]
    fn no_working_hours_means_24h() {
        let mut e = no_today(base());
        e.working_hours = None;
        assert!(e.is_available(dt(day(1), 3, 0), dt(day(1), 4, 0)).is_empty());
        let s = e.suggest_slot(60, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 0, 0), "primer slot del día");
    }

    #[test]
    fn suggest_prefers_earliest_free_slot() {
        let mut e = no_today(base());
        e.commitments.push(hard(iv(dt(day(1), 10, 0), dt(day(1), 12, 0)), "bloque"));
        let s = e.suggest_slot(60, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 6, 0), "6-7 primero");
    }

    #[test]
    fn grid_alignment_step() {
        let mut e = no_today(base());
        e.blocks.push(hard(iv(dt(day(1), 6, 0), dt(day(1), 6, 07)), "corto"));
        e.step_min = 15;
        let s = e.suggest_slot(60, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 6, 15), "alinea al grid de 15'");
    }

    // ------------------------------------------------------------------
    // Determinismo y horizonte
    // ------------------------------------------------------------------

    #[test]
    fn deterministic_repeated_queries() {
        let e = base();
        let a = e.suggest_slot(90, 30, Some(dt(day(2), 12, 0)), Some(15 * 60));
        let b = e.suggest_slot(90, 30, Some(dt(day(2), 12, 0)), Some(15 * 60));
        assert_eq!(a, b);
        assert_eq!(e.is_available(dt(day(1), 9, 0), dt(day(1), 10, 0)), e.is_available(dt(day(1), 9, 0), dt(day(1), 10, 0)));
    }

    #[test]
    fn no_slot_within_lookahead_returns_none() {
        let mut e = no_today(base());
        e.lookahead_days = 1;
        e.blocks.push(hard(iv(dt(day(1), 0, 0), dt(day(2), 0, 0)), "todo el día"));
        assert!(e.suggest_slot(30, 0, None, None).is_none());
    }

    #[test]
    fn suggest_never_schedules_in_past_on_today() {
        // sin bloquear hoy, el primer intervalo del día empieza a las 06:00
        // (antes de "ahora") → el slot propuesto nunca cae en horas pasadas.
        let e = base();
        let now = Local::now().timestamp_millis();
        let s = e.suggest_slot(30, 0, None, None).unwrap();
        assert!(s.task_start_ms >= now, "slot en el pasado: {s:?} (ahora {now})");
    }

    // ------------------------------------------------------------------
    // Puente desde intents (fase 3)
    // ------------------------------------------------------------------

    fn intent_event(title: &str, start: i64, end: i64) -> Intent {
        Intent {
            intent_type: IntentType::Event,
            title: title.into(),
            description: String::new(),
            category_id: "uni".into(),
            priority: crate::ai::intent::Priority::Media,
            window: crate::ai::intent::TimeWindow { start: Some(start), end: Some(end), all_day: false },
            duration: None,
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

    fn intent_cap(value: &str) -> Intent {
        Intent {
            intent_type: IntentType::Constraint,
            title: "no programar antes".into(),
            description: String::new(),
            category_id: "otr".into(),
            priority: crate::ai::intent::Priority::Alta,
            window: crate::ai::intent::TimeWindow { start: None, end: None, all_day: false },
            duration: None,
            deadline: None,
            preparation: None,
            recurrence: None,
            reminders: Vec::new(),
            constraints: vec![crate::ai::intent::Constraint {
                kind: ConstraintKind::DailyCap,
                target: None,
                value: Some(value.into()),
            }],
            confidence: 0.8,
            reason: "test".into(),
            source: "local".into(),
        }
    }

    #[test]
    fn from_intents_multiday_allday_event_blocks_external_days() {
        let start = dt(day(1), 0, 0);
        let end = dt(day(4), 0, 0); // "lunes al jueves" (sin hora de cierre)
        let mut i = intent_event("Proyecto", start, end);
        i.window.all_day = true;
        let e = ConstraintEngine::from_intents(&[i]);
        let hour = HOUR_MS;
        // día inicial: bloqueado; días medios: libres
        assert_eq!(e.available_minutes(start + 9 * hour, start + 10 * hour), 0);
        assert_eq!(e.available_minutes(dt(day(2), 9, 0), dt(day(2), 10, 0)), 60);
        // día de fin: libre con fecha límite 22:00
        assert_eq!(e.available_minutes(dt(day(4), 9, 0), dt(day(4), 10, 0)), 60);
        let dl = e.deadlines.iter().find(|d| d.label == "Proyecto").expect("deadline");
        assert_eq!(dl.at_ms, dt(day(4), 22, 0));

        // con hora de cierre: bloquea las 2 h previas
        let mut i = intent_event("Proyecto", start, dt(day(4), 22, 0));
        i.window.all_day = true;
        let e = ConstraintEngine::from_intents(&[i]);
        assert_eq!(e.available_minutes(dt(day(4), 20, 0), dt(day(4), 22, 0)), 0);
        assert_eq!(e.available_minutes(dt(day(4), 14, 0), dt(day(4), 15, 0)), 60);
    }

    #[test]
    fn from_intents_maps_events_caps_deadlines_availability() {
        let intents = vec![
            intent_event("Clase de álgebra", dt(day(1), 14, 0), dt(day(1), 16, 0)),
            intent_cap("06:00"),
            Intent {
                intent_type: IntentType::Deadline,
                title: "Proyecto".into(),
                description: String::new(),
                category_id: "uni".into(),
                priority: crate::ai::intent::Priority::Alta,
                window: crate::ai::intent::TimeWindow { start: None, end: None, all_day: false },
                duration: None,
                deadline: Some(dt(day(3), 23, 59)),
                preparation: None,
                recurrence: None,
                reminders: Vec::new(),
                constraints: Vec::new(),
                confidence: 0.9,
                reason: "test".into(),
                source: "local".into(),
            },
            Intent {
                intent_type: IntentType::Availability,
                title: "Disponibilidad".into(),
                description: String::new(),
                category_id: "otr".into(),
                priority: crate::ai::intent::Priority::Media,
                window: crate::ai::intent::TimeWindow {
                    start: Some(dt(day(1), 8, 0)),
                    end: Some(dt(day(1), 20, 0)),
                    all_day: true,
                },
                duration: None,
                deadline: None,
                preparation: None,
                recurrence: None,
                reminders: Vec::new(),
                constraints: Vec::new(),
                confidence: 0.85,
                reason: "test".into(),
                source: "local".into(),
            },
        ];
        let e = ConstraintEngine::from_intents(&intents);
        assert_eq!(e.commitments.len(), 1);
        assert_eq!(e.commitments[0].label, "Clase de álgebra");
        assert_eq!(e.commitments[0].interval, iv(dt(day(1), 14, 0), dt(day(1), 16, 0)));
        assert_eq!(e.working_hours.unwrap().start_min, 6 * 60, "daily_cap 06:00 coincide con el default");
        assert_eq!(e.deadlines.len(), 1);
        assert_eq!(e.deadlines[0].label, "Proyecto");
        assert_eq!(e.availability.len(), 1);
        assert_eq!(e.availability[0], iv(dt(day(1), 8, 0), dt(day(1), 20, 0)));

        // el evento mapeado bloquea de verdad
        assert!(!e.is_available(dt(day(1), 15, 0), dt(day(1), 15, 30)).is_empty());
        assert!(e.is_available(dt(day(1), 17, 0), dt(day(1), 18, 0)).is_empty());
    }

    #[test]
    fn from_intents_cap_replaces_or_raises() {
        let intents = vec![intent_cap("10:00")];
        let e2 = ConstraintEngine::from_intents(&intents);
        assert_eq!(e2.working_hours.unwrap().start_min, 10 * 60, "eleva el inicio por defecto");
        let intents3 = vec![intent_cap("05:00")];
        let e3 = ConstraintEngine::from_intents(&intents3);
        assert_eq!(e3.working_hours.unwrap().start_min, 5 * 60, "cap temprano reemplaza el inicio por defecto");
    }

    #[test]
    fn example_dont_schedule_before_6am() {
        // ejemplo del enunciado: "Don't schedule anything before 6 AM"
        let intents = vec![intent_cap("06:00")];
        let e = ConstraintEngine::from_intents(&intents);
        let w = e.working_hours.unwrap();
        assert_eq!(w.start_min, 6 * 60);
        assert!(!e.is_available(dt(day(1), 5, 0), dt(day(1), 6, 0)).is_empty(), "antes de 6AM bloqueado");
        assert!(e.is_available(dt(day(1), 6, 0), dt(day(1), 7, 0)).is_empty(), "6AM en adelante hábil");
        let s = no_today(e).suggest_slot(60, 0, None, None).unwrap();
        assert_eq!(s.task_start_ms, dt(day(1), 6, 0));
    }

    #[test]
    fn example_blocked_class() {
        // ejemplo del enunciado: "I have class from 2 PM to 4 PM" → bloqueo
        let mut e = base();
        e.blocks.push(hard(iv(dt(day(1), 14, 0), dt(day(1), 16, 0)), "clase"));
        assert!(!e.is_available(dt(day(1), 15, 0), dt(day(1), 15, 30)).is_empty());
        let b = e.is_available(dt(day(1), 15, 0), dt(day(1), 15, 30));
        assert_eq!(b[0].label, "clase");
        assert_eq!(e.available_minutes(dt(day(1), 9, 0), dt(day(1), 18, 0)), 7 * 60);
    }
}
