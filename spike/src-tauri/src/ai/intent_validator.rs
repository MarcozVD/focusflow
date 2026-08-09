//! Validación de intenciones: JSON del proveedor → `Intent`, y reglas de
//! invariantes. Principios:
//! - El JSON del LLM se parsea con tolerancia total: campos ausentes, `null`
//!   o vacíos son válidos ("unknown information").
//! - `validate_intent` rechaza solo lo que es *internamente inconsistente*
//!   (fin antes de inicio, vencimiento en el pasado, intervalos 0…).
//! - Nunca inventa datos: no hay defaults de hora ni de fecha.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use serde_json::Value;

use super::intent::{
    Constraint, ConstraintKind, Duration, Frequency, Intent, IntentType, Preparation, Priority,
    Recurrence, ReminderSpec, TimeWindow,
};
use super::validation::{category_id_from_name, naive_to_ms, priority_from_name};
use super::AiError;

const MAX_DURATION_MIN: u32 = 24 * 60;
const MAX_REMINDER_DAYS: u32 = 30;
const MAX_INTERVAL: u32 = 365;

/// Topes de tamaño sobre texto que proviene del LLM (los LLM son no
/// confiables: un correo inyectado podría pedir títulos enormes). Se trunca,
/// no se rechaza, para no romper flujos legítimos.
const MAX_TITLE_CHARS: usize = 200;
const MAX_DESCRIPTION_CHARS: usize = 600;
const MAX_REASON_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 200;
fn cap(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Error de validación: lista de reglas incumplidas (todas las que apliquen).
#[derive(Debug, Clone, PartialEq)]
pub struct IntentValidationError {
    pub errors: Vec<String>,
}

impl std::fmt::Display for IntentValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "intención inválida: {}", self.errors.join("; "))
    }
}

impl std::error::Error for IntentValidationError {}

fn err(errors: &mut Vec<String>, msg: impl Into<String>) {
    errors.push(msg.into());
}

// ---------------------------------------------------------------------------
// Parseo JSON (salida del proveedor) → Intent
// ---------------------------------------------------------------------------

fn parse_date(v: &serde_json::Value) -> Option<NaiveDate> {
    let s = v.as_str()?.trim();
    if s.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn parse_time(v: &serde_json::Value) -> Option<NaiveTime> {
    let s = v.as_str()?.trim();
    if s.is_empty() {
        return None;
    }
    for f in ["%H:%M", "%H:%M:%S", "%I:%M %p", "%I %p", "%I%p"] {
        if let Ok(t) = NaiveTime::parse_from_str(s, f) {
            return Some(t);
        }
    }
    None
}

/// Combina fecha+hora de un JSON a ms local. `time` ausente o `null` →
/// medianoche (día completo).
fn ms_of_date_time(date: &serde_json::Value, time: Option<&serde_json::Value>) -> Option<i64> {
    let d = parse_date(date)?;
    let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let t = time.and_then(parse_time).unwrap_or(midnight);
    Some(naive_to_ms(d, t))
}

fn is_null_or_missing(v: Option<&serde_json::Value>) -> bool {
    v.map(|x| x.is_null()).unwrap_or(true)
}

fn parse_intent_type(v: &serde_json::Value) -> Result<IntentType, AiError> {
    let s = v.as_str().unwrap_or_default();
    match s {
        "event" => Ok(IntentType::Event),
        "task" => Ok(IntentType::Task),
        "deadline" => Ok(IntentType::Deadline),
        "preparation" => Ok(IntentType::Preparation),
        "availability" => Ok(IntentType::Availability),
        "reminder" => Ok(IntentType::Reminder),
        "constraint" => Ok(IntentType::Constraint),
        _ => Err(AiError::InvalidJson(format!(
            "intent_type desconocido: '{s}' (event|task|deadline|preparation|availability|reminder|constraint)"
        ))),
    }
}

fn parse_frequency(v: &serde_json::Value) -> Result<Frequency, AiError> {
    let s = v.as_str().unwrap_or_default();
    match s {
        "daily" => Ok(Frequency::Daily),
        "weekly" => Ok(Frequency::Weekly),
        "monthly" => Ok(Frequency::Monthly),
        "yearly" => Ok(Frequency::Yearly),
        _ => Err(AiError::InvalidJson(format!(
            "frecuencia desconocida: '{s}' (daily|weekly|monthly|yearly)"
        ))),
    }
}

fn parse_constraint_kind(v: &serde_json::Value) -> Result<ConstraintKind, AiError> {
    let s = v.as_str().unwrap_or_default();
    match s {
        "blocked_by" => Ok(ConstraintKind::BlockedBy),
        "cannot_overlap" => Ok(ConstraintKind::CannotOverlap),
        "must_finish_before" => Ok(ConstraintKind::MustFinishBefore),
        "must_start_after" => Ok(ConstraintKind::MustStartAfter),
        "daily_cap" => Ok(ConstraintKind::DailyCap),
        "other" => Ok(ConstraintKind::Other),
        _ => Err(AiError::InvalidJson(format!(
            "tipo de restricción desconocido: '{s}'"
        ))),
    }
}

fn parse_u32(v: &serde_json::Value) -> Option<u32> {
    v.as_u64().map(|n| n as u32)
}

/// Convierte el JSON del proveedor en `Intent`, sin aplicar aún invariantes
/// (eso es `validate_intent`). `null`/ausente → `None`/vacío.
pub fn parse_intent_json(v: &serde_json::Value) -> Result<Intent, AiError> {
    let obj = v
        .as_object()
        .ok_or_else(|| AiError::InvalidJson("la raíz debe ser un objeto".into()))?;

    let intent_type = parse_intent_type(
        obj.get("intent_type")
            .ok_or_else(|| AiError::InvalidJson("falta campo 'intent_type'".into()))?,
    )?;

    let title = obj
        .get("title")
        .and_then(|t| t.as_str())
        .map(|t| cap(t, MAX_TITLE_CHARS))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AiError::InvalidJson("falta campo 'title'".into()))?;

    let description = obj
        .get("description")
        .and_then(|d| d.as_str())
        .map(|d| cap(d, MAX_DESCRIPTION_CHARS))
        .unwrap_or_default();
    let category_id = obj
        .get("category")
        .and_then(|c| c.as_str())
        .map(category_id_from_name)
        .unwrap_or_else(|| "otr".into());
    let priority = obj
        .get("priority")
        .and_then(|p| p.as_str())
        .map(|p| match priority_from_name(p).as_str() {
            "alta" => Priority::Alta,
            "baja" => Priority::Baja,
            _ => Priority::Media,
        })
        .unwrap_or(Priority::Media);

    // ventana temporal (todo opcional; null = desconocido)
    let start = obj.get("start_date").and_then(|d| ms_of_date_time(d, obj.get("start_time")));
    let end = obj.get("end_date").and_then(|d| ms_of_date_time(d, obj.get("end_time")));
    let all_day = is_null_or_missing(obj.get("start_time")) && is_null_or_missing(obj.get("end_time"));

    let duration = obj.get("duration_minutes").and_then(parse_u32).map(|m| Duration { minutes: m });
    let deadline = obj.get("deadline_date").and_then(|d| ms_of_date_time(d, obj.get("deadline_time")));

    let preparation = match (obj.get("preparation_minutes"), obj.get("preparation_note")) {
        (Some(m), _) if m.as_u64().map(|x| x > 0).unwrap_or(false) => Some(Preparation {
            minutes: m.as_u64().unwrap() as u32,
            note: obj
                .get("preparation_note")
                .and_then(|n| n.as_str())
                .map(|n| cap(n, MAX_NOTE_CHARS))
                .unwrap_or_default(),
        }),
        _ => None,
    };

    let recurrence = match obj.get("recurrence") {
        None | Some(Value::Null) => None,
        Some(r) => {
            let frequency = parse_frequency(r.get("frequency").ok_or_else(|| {
                AiError::InvalidJson("recurrencia sin campo 'frequency'".into())
            })?)?;
            let interval = r.get("interval").and_then(parse_u32).unwrap_or(1);
            let by_day = r
                .get("by_day")
                .and_then(|b| b.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_u64()).map(|n| n as u8).collect())
                .unwrap_or_default();
            let count = r.get("count").and_then(parse_u32);
            let until = r.get("until").and_then(|d| ms_of_date_time(d, None));
            Some(Recurrence { frequency, interval, by_day, count, until })
        }
    };

    let reminders: Vec<ReminderSpec> = obj
        .get("reminders")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    if r.is_null() {
                        return None;
                    }
                    let minutes_before = r.get("minutes_before").and_then(parse_u32);
                    let at = r
                        .get("at")
                        .and_then(|a| a.as_str())
                        .and_then(|s| {
                            if s.trim().is_empty() {
                                return None;
                            }
                            NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M").ok().map(
                                |dt| {
                                    chrono::Local
                                        .from_local_datetime(&dt)
                                        .earliest()
                                        .map(|d| d.timestamp_millis())
                                        .unwrap_or_else(|| dt.and_utc().timestamp_millis())
                                },
                            )
                        });
                    if minutes_before.is_none() && at.is_none() {
                        return None;
                    }
                    Some(ReminderSpec { minutes_before, at })
                })
                .collect()
        })
        .unwrap_or_default();

    let constraints: Vec<Constraint> = obj
        .get("constraints")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    if c.is_null() {
                        return None;
                    }
                    let kind = parse_constraint_kind(c.get("kind")?).ok()?;
                    let target = c.get("target").and_then(|t| t.as_str()).map(|s| s.to_string());
                    let value = c.get("value").and_then(|t| t.as_str()).map(|s| s.to_string());
                    Some(Constraint { kind, target, value })
                })
                .collect()
        })
        .unwrap_or_default();

    let confidence = obj
        .get("confidence")
        .and_then(|c| c.as_f64())
        .map(|c| c.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let reason = obj
        .get("reason")
        .and_then(|r| r.as_str())
        .map(|r| cap(r, MAX_REASON_CHARS))
        .unwrap_or_default();

    Ok(Intent {
        intent_type,
        title,
        description,
        category_id,
        priority,
        window: TimeWindow { start, end, all_day },
        duration,
        deadline,
        preparation,
        recurrence,
        reminders,
        constraints,
        confidence,
        reason,
        source: "ai".into(),
    })
}

// ---------------------------------------------------------------------------
// Reglas de invariantes sobre un `Intent` ya construido
// ---------------------------------------------------------------------------

fn valid_weekdays(by_day: &[u8]) -> bool {
    by_day.iter().all(|d| (1..=7).contains(d))
}

/// Verifica todas las invariantes del intent. Devuelve la lista completa de
/// errores; vacía = válido.
pub fn validate_intent(i: &Intent) -> Result<(), IntentValidationError> {
    let mut e: Vec<String> = Vec::new();
    let now = chrono::Local::now().timestamp_millis();

    if i.title.trim().is_empty() {
        err(&mut e, "título vacío");
    }
    if !(0.0..=1.0).contains(&i.confidence) {
        err(&mut e, format!("confidence fuera de [0,1]: {}", i.confidence));
    }

    // ventana temporal
    if let (Some(s), Some(en)) = (i.window.start, i.window.end) {
        if en < s {
            err(&mut e, "end antes de start en la ventana temporal");
        }
    }

    // duración
    if let Some(d) = i.duration {
        if d.minutes == 0 {
            err(&mut e, "duración 0");
        }
        if d.minutes > MAX_DURATION_MIN {
            err(&mut e, format!("duración > 24 h ({} min)", d.minutes));
        }
    }

    // vencimiento
    if let Some(d) = i.deadline {
        if d < now {
            err(&mut e, "deadline en el pasado");
        }
        if let Some(w) = i.window.start {
            if d < w {
                err(&mut e, "deadline antes del inicio de la ventana");
            }
        }
    }

    // preparación
    if let Some(p) = &i.preparation {
        if p.minutes == 0 {
            err(&mut e, "preparation con 0 minutos");
        }
        if p.minutes > MAX_DURATION_MIN {
            err(&mut e, format!("preparation > 24 h ({} min)", p.minutes));
        }
    }

    // recurrencia
    if let Some(r) = &i.recurrence {
        if r.interval == 0 {
            err(&mut e, "interval de recurrencia 0");
        }
        if r.interval > MAX_INTERVAL {
            err(&mut e, format!("interval de recurrencia > {} días", MAX_INTERVAL));
        }
        if !valid_weekdays(&r.by_day) {
            err(&mut e, "by_day fuera de 1..=7 (ISO: 1=Lunes..7=Domingo)");
        }
        if let Some(c) = r.count {
            if c == 0 {
                err(&mut e, "count de recurrencia 0");
            }
        }
        if let Some(u) = r.until {
            if let Some(s) = i.window.start {
                if u < s {
                    err(&mut e, "until antes del inicio de la ventana");
                }
            }
        }
    }

    // recordatorios
    for (idx, rem) in i.reminders.iter().enumerate() {
        if let Some(m) = rem.minutes_before {
            if m == 0 {
                err(&mut e, format!("recordatorio #{idx}: minutes_before 0"));
            }
            if m > MAX_REMINDER_DAYS * 24 * 60 {
                err(&mut e, format!("recordatorio #{idx}: más de 30 días antes"));
            }
        }
        if let Some(at) = rem.at {
            if at < now {
                err(&mut e, format!("recordatorio #{idx}: 'at' en el pasado"));
            }
        }
        if rem.minutes_before.is_none() && rem.at.is_none() {
            err(&mut e, format!("recordatorio #{idx}: sin minutes_before ni at"));
        }
    }

    // consistencia tipo ↔ contenido
    match i.intent_type {
        IntentType::Deadline if i.deadline.is_none() => {
            err(&mut e, "intent_type=deadline sin deadline");
        }
        IntentType::Preparation if i.preparation.is_none() => {
            err(&mut e, "intent_type=preparation sin preparation");
        }
        IntentType::Reminder if i.reminders.is_empty() => {
            err(&mut e, "intent_type=reminder sin reminders");
        }
        IntentType::Constraint if i.constraints.is_empty() => {
            err(&mut e, "intent_type=constraint sin constraints");
        }
        IntentType::Availability if i.window.start.is_none() => {
            err(&mut e, "intent_type=availability sin ventana de disponibilidad");
        }
        _ => {}
    }

    if e.is_empty() {
        Ok(())
    } else {
        Err(IntentValidationError { errors: e })
    }
}

/// Parseo + validación en un paso (el flujo normal).
pub fn parse_and_validate(v: &serde_json::Value) -> Result<Intent, AiError> {
    let intent = parse_intent_json(v)?;
    validate_intent(&intent)
        .map_err(|e| AiError::InvalidJson(e.to_string()))?;
    Ok(intent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const HOUR: i64 = 3_600_000;

    fn future_ms(days: i64) -> i64 {
        let now = chrono::Local::now();
        let day = now.date_naive() + chrono::Duration::days(days);
        naive_to_ms(day, NaiveTime::from_hms_opt(0, 0, 0).unwrap())
    }

    fn base_json() -> serde_json::Value {
        json!({
            "intent_type": "task",
            "title": "Estudiar cálculo",
            "description": null,
            "category": "Universidad",
            "priority": "media",
            "start_date": null,
            "start_time": null,
            "end_date": null,
            "end_time": null,
            "duration_minutes": null,
            "deadline_date": null,
            "deadline_time": null,
            "preparation_minutes": null,
            "preparation_note": null,
            "recurrence": null,
            "reminders": [],
            "constraints": [],
            "confidence": 0.8,
            "reason": "test"
        })
    }

    #[test]
    fn nulls_are_valid() {
        let i = parse_and_validate(&base_json()).expect("nulls válidos");
        assert_eq!(i.intent_type, IntentType::Task);
        assert_eq!(i.category_id, "uni");
        assert!(i.window.start.is_none());
        assert!(i.deadline.is_none());
        assert!(i.recurrence.is_none());
        assert_eq!(i.confidence, 0.8);
    }

    #[test]
    fn phase_example_event_with_preparation() {
        // "I have a calculus exam Friday and need four hours to prepare."
        let v = json!({
            "intent_type": "event",
            "title": "Examen de cálculo",
            "category": "Universidad",
            "priority": "alta",
            "start_date": "2026-08-14",
            "start_time": null,
            "end_date": "2026-08-14",
            "end_time": null,
            "duration_minutes": 120,
            "preparation_minutes": 240,
            "preparation_note": "necesito 4 horas para preparar",
            "confidence": 0.9,
            "reason": "fecha concreta"
        });
        let i = parse_and_validate(&v).expect("válido");
        assert_eq!(i.intent_type, IntentType::Event);
        assert!(i.window.all_day);
        assert_eq!(i.preparation, Some(Preparation { minutes: 240, note: "necesito 4 horas para preparar".into() }));
        assert_eq!(i.priority, Priority::Alta);
    }

    #[test]
    fn phase_example_timed_event_with_duration() {
        // "Tomorrow at 6 PM study programming for two hours."
        let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let v = json!({
            "intent_type": "event",
            "title": "Estudiar programación",
            "category": "Universidad",
            "priority": "media",
            "start_date": tomorrow,
            "start_time": "18:00",
            "end_date": null,
            "end_time": null,
            "duration_minutes": 120,
            "confidence": 0.95,
            "reason": null
        });
        let i = parse_and_validate(&v).expect("válido");
        assert_eq!(i.intent_type, IntentType::Event);
        let start = i.window.start.expect("hora conocida");
        assert_eq!(start, future_ms(1) + 18 * HOUR);
        // duración aplica cuando no hay end explícito: el planner la usa
        assert_eq!(i.duration, Some(Duration { minutes: 120 }));
        assert!(!i.window.all_day);
    }

    #[test]
    fn phase_example_deadline_with_preparation() {
        // "The project is due Monday but I need at least six hours to finish it."
        let v = json!({
            "intent_type": "deadline",
            "title": "Proyecto",
            "category": "Universidad",
            "priority": "alta",
            "deadline_date": "2026-08-10",
            "deadline_time": null,
            "preparation_minutes": 360,
            "preparation_note": "necesito al menos 6 horas",
            "confidence": 0.85,
            "reason": "entrega el lunes"
        });
        let i = parse_and_validate(&v).expect("válido");
        assert_eq!(i.intent_type, IntentType::Deadline);
        assert!(i.deadline.is_some());
        assert_eq!(i.preparation, Some(Preparation { minutes: 360, note: "necesito al menos 6 horas".into() }));
    }

    #[test]
    fn phase_example_availability_window() {
        // "Diagnostic Test is available from August 5 until August 23."
        let v = json!({
            "intent_type": "availability",
            "title": "Diagnostic Test",
            "category": "Universidad",
            "priority": "alta",
            "start_date": "2026-08-05",
            "start_time": null,
            "end_date": "2026-08-23",
            "end_time": null,
            "confidence": 0.9,
            "reason": "ventana de disponibilidad"
        });
        let i = parse_and_validate(&v).expect("válido");
        assert_eq!(i.intent_type, IntentType::Availability);
        assert!(i.window.all_day);
        assert!(i.window.start.unwrap() < i.window.end.unwrap());
    }

    #[test]
    fn end_before_start_rejected() {
        let mut v = base_json();
        v["start_date"] = json!("2026-08-23");
        v["end_date"] = json!("2026-08-05");
        let err = parse_and_validate(&v).expect_err("debe fallar");
        assert!(err.to_string().contains("end antes de start"), "{err}");
    }

    #[test]
    fn past_deadline_rejected() {
        let mut v = base_json();
        v["intent_type"] = json!("deadline");
        v["deadline_date"] = json!("2020-01-01");
        let err = parse_and_validate(&v).expect_err("debe fallar");
        assert!(err.to_string().contains("deadline"), "{err}");
    }

    #[test]
    fn duration_zero_and_over_24h_rejected() {
        let mut v = base_json();
        v["duration_minutes"] = json!(0);
        let err = parse_and_validate(&v).expect_err("duración 0");
        assert!(err.to_string().contains("duración 0"), "{err}");

        let mut v2 = base_json();
        v2["duration_minutes"] = json!(24 * 60 + 1);
        let err2 = parse_and_validate(&v2).expect_err("> 24 h");
        assert!(err2.to_string().contains("24 h"), "{err2}");
    }

    #[test]
    fn recurrence_invariants_rejected() {
        let mut v = base_json();
        v["recurrence"] = json!({"frequency": "weekly", "interval": 0, "by_day": [1], "count": null, "until": null});
        let err = parse_and_validate(&v).expect_err("interval 0");
        assert!(err.to_string().contains("interval"), "{err}");

        let mut v2 = base_json();
        v2["recurrence"] = json!({"frequency": "weekly", "interval": 1, "by_day": [0, 8], "count": null, "until": null});
        let err2 = parse_and_validate(&v2).expect_err("by_day inválido");
        assert!(err2.to_string().contains("by_day"), "{err2}");
    }

    #[test]
    fn reminder_invariants_rejected() {
        let mut v = base_json();
        v["intent_type"] = json!("reminder");
        v["reminders"] = json!([{"minutes_before": 0, "at": null}]);
        let err = parse_and_validate(&v).expect_err("minutes_before 0");
        assert!(err.to_string().contains("minutes_before 0"), "{err}");

        let mut v2 = base_json();
        v2["intent_type"] = json!("reminder");
        v2["reminders"] = json!([]);
        let err2 = parse_and_validate(&v2).expect_err("reminder sin reminders");
        assert!(err2.to_string().contains("sin reminders"), "{err2}");
    }

    #[test]
    fn type_consistency_rejected() {
        // preparation sin preparation_minutes
        let mut v = base_json();
        v["intent_type"] = json!("preparation");
        let err = parse_and_validate(&v).expect_err("preparation vacío");
        assert!(err.to_string().contains("sin preparation"), "{err}");

        // constraint sin constraints
        let mut v2 = base_json();
        v2["intent_type"] = json!("constraint");
        let err2 = parse_and_validate(&v2).expect_err("constraint vacío");
        assert!(err2.to_string().contains("sin constraints"), "{err2}");

        // availability sin ventana
        let mut v3 = base_json();
        v3["intent_type"] = json!("availability");
        let err3 = parse_and_validate(&v3).expect_err("availability vacío");
        assert!(err3.to_string().contains("sin ventana"), "{err3}");
    }

    #[test]
    fn unknown_intent_type_rejected() {
        let mut v = base_json();
        v["intent_type"] = json!("todo");
        let err = parse_and_validate(&v).expect_err("tipo desconocido");
        assert!(err.to_string().contains("intent_type desconocido"), "{err}");
    }

    #[test]
    fn unknown_frequency_rejected() {
        let mut v = base_json();
        v["recurrence"] = json!({"frequency": "semanal", "interval": 1, "by_day": [], "count": null, "until": null});
        let err = parse_and_validate(&v).expect_err("frecuencia desconocida");
        assert!(err.to_string().contains("frecuencia"), "{err}");
    }

    #[test]
    fn confidence_clamped_and_checked() {
        // parseo: fuera de rango se acota (tolerante al LLM)
        let mut v = base_json();
        v["confidence"] = json!(1.7);
        let i = parse_and_validate(&v).expect("acotada");
        assert_eq!(i.confidence, 1.0);

        // validación directa: fuera de rango es inválido
        let mut i2 = parse_and_validate(&base_json()).unwrap();
        i2.confidence = -0.1;
        let err = validate_intent(&i2).expect_err("confidence inválida");
        assert!(err.to_string().contains("confidence"), "{err}");
    }

    #[test]
    fn deadline_type_requires_deadline() {
        let mut v = base_json();
        v["intent_type"] = json!("deadline");
        let err = parse_and_validate(&v).expect_err("deadline sin deadline");
        assert!(err.to_string().contains("sin deadline"), "{err}");
    }

    #[test]
    fn windows_and_dates_use_local_time() {
        // "15:00" local no debe convertirse a UTC (regresión de timezone)
        let tomorrow = (chrono::Local::now().date_naive() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        let mut v = base_json();
        v["intent_type"] = json!("event");
        v["start_date"] = json!(tomorrow);
        v["start_time"] = json!("15:00");
        let i = parse_and_validate(&v).expect("válido");
        assert_eq!(i.window.start, Some(future_ms(1) + 15 * HOUR));
    }

    #[test]
    fn reminder_absolute_and_relative() {
        let mut v = base_json();
        v["intent_type"] = json!("reminder");
        v["reminders"] = json!([{"minutes_before": 60, "at": null}]);
        let i = parse_and_validate(&v).expect("válido");
        assert_eq!(i.reminders, vec![ReminderSpec { minutes_before: Some(60), at: None }]);
    }
}
