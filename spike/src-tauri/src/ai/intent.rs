//! Tipos del sistema de intenciones (fase 3).
//!
//! El flujo es: texto libre → [super::intent_parser] → `IntentBatch` de
//! `Intent`s → [super::intent_validator] (invariantes) → planner.
//!
//! Principios de diseño (spec/09):
//! - Todo campo temporal/débil es `Option`: la información desconocida se
//!   representa con `None`, jamás con un default inventado.
//! - Un solo input puede producir varios `Intent` (ej: "examen el viernes y
//!   necesito 4 horas" → Event con `preparation`).
//! - `confidence` expresa qué tan segura está la IA; el planner la usa para
//!   decidir qué confirmar con el usuario.

use serde::{Deserialize, Serialize};

use super::validation::ParsedTask;

/// Tipo de intención. El planner orquesta tipos; no existe un "generic task".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentType {
    Event,
    Task,
    Deadline,
    Preparation,
    Availability,
    Reminder,
    Constraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Alta,
    Media,
    Baja,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintKind {
    BlockedBy,
    CannotOverlap,
    MustFinishBefore,
    MustStartAfter,
    DailyCap,
    Other,
}

/// Ventana temporal en ms (epoch local, como el resto del modelo de datos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    pub start: Option<i64>,
    pub end: Option<i64>,
    /// true = día completo sin horas. `start` (y opcionalmente `end`) caen en
    /// medianoche local.
    pub all_day: bool,
}

impl TimeWindow {
    pub fn has_start(&self) -> bool {
        self.start.is_some()
    }
    pub fn has_end(&self) -> bool {
        self.end.is_some()
    }
    pub fn is_empty(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duration {
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preparation {
    pub minutes: u32,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recurrence {
    pub frequency: Frequency,
    pub interval: u32,
    /// Días ISO (1=Lunes..7=Domingo), válido con weekly.
    pub by_day: Vec<u8>,
    pub count: Option<u32>,
    pub until: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderSpec {
    pub minutes_before: Option<u32>,
    pub at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub target: Option<String>,
    pub value: Option<String>,
}

/// Intención estructurada. Todos los campos opcionales siguen la regla
/// "unknown = None" (ver spec/09, tabla de validación).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    pub intent_type: IntentType,
    pub title: String,
    pub description: String,
    pub category_id: String,
    pub priority: Priority,
    pub window: TimeWindow,
    pub duration: Option<Duration>,
    /// Vencimiento (deadline) en ms.
    pub deadline: Option<i64>,
    pub preparation: Option<Preparation>,
    pub recurrence: Option<Recurrence>,
    pub reminders: Vec<ReminderSpec>,
    pub constraints: Vec<Constraint>,
    /// 0.0 = incierto … 1.0 = seguro (del proveedor).
    pub confidence: f64,
    /// Justificación del análisis, para el modo "explica" y debugging.
    pub reason: String,
    /// "ai" = LLM, "local" = heurística del módulo 1 (fallback sin API).
    pub source: String,
}

impl Intent {
    /// Categoría efectiva si el análisis no devolvió una válida.
    pub fn resolved_category_id(&self) -> &str {
        &self.category_id
    }
}

/// Resultado de parsear un input libre: uno o más intents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentBatch {
    pub intents: Vec<Intent>,
    pub source: String,
}

/// Construye un `Intent` de tipo Task desde el análisis heurístico del
/// módulo 1 (fallback cuando no hay IA configurada).
pub fn from_task(t: &ParsedTask) -> Intent {
    let has_time = t.start_ms > 0;
    let start = if has_time { Some(t.start_ms) } else { None };
    let end = if has_time { Some(t.end_ms) } else { None };
    let reminders = t
        .reminders
        .iter()
        .filter_map(|s| crate::reminders::parse_reminder_minutes(s))
        .map(|m| ReminderSpec { minutes_before: Some(m as u32), at: None })
        .collect();

    Intent {
        intent_type: IntentType::Task,
        title: t.title.clone(),
        description: t.description.clone(),
        category_id: t.category_id.clone(),
        priority: match t.priority.as_str() {
            "alta" => Priority::Alta,
            "baja" => Priority::Baja,
            _ => Priority::Media,
        },
        window: TimeWindow { start, end, all_day: t.all_day },
        duration: None,
        deadline: None,
        preparation: None,
        recurrence: None,
        reminders,
        constraints: Vec::new(),
        confidence: 0.0,
        reason: "análisis heurístico local (módulo 1, sin IA)".into(),
        source: "local".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_task_maps_heuristic_fields() {
        let t = ParsedTask {
            title: "Estudiar programación".into(),
            description: String::new(),
            category_id: "uni".into(),
            priority: "media".into(),
            start_ms: 0,
            end_ms: 0,
            all_day: true,
            location: String::new(),
            tags: Vec::new(),
            reminders: vec!["60m".into()],
        };
        let i = from_task(&t);
        assert_eq!(i.intent_type, IntentType::Task);
        assert_eq!(i.title, "Estudiar programación");
        assert_eq!(i.category_id, "uni");
        assert!(i.window.all_day && i.window.start.is_none());
        assert_eq!(i.reminders, vec![ReminderSpec { minutes_before: Some(60), at: None }]);
        assert_eq!(i.confidence, 0.0);
        assert_eq!(i.source, "local");
    }

    #[test]
    fn from_task_preserves_explicit_window() {
        let t = ParsedTask {
            title: "tarea".into(),
            description: String::new(),
            category_id: "otr".into(),
            priority: "alta".into(),
            start_ms: 1_700_000_000_000,
            end_ms: 1_700_000_007_200,
            all_day: false,
            location: String::new(),
            tags: Vec::new(),
            reminders: vec![],
        };
        let i = from_task(&t);
        assert!(!i.window.all_day);
        assert!(i.window.start.unwrap() < i.window.end.unwrap());
        assert_eq!(i.priority, Priority::Alta);
    }
}
