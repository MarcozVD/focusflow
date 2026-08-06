use serde::{Deserialize, Serialize};

use super::AiError;

pub fn extract_json(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    let cleaned = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if cleaned.starts_with('{') {
        return serde_json::from_str(cleaned).ok();
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if end > start {
                return serde_json::from_str(&trimmed[start..=end]).ok();
            }
        }
    }
    None
}

pub fn category_id_from_name(name: &str) -> String {
    let n = name.trim().to_lowercase();
    if n.contains("universidad") || n.contains("uni") || n.contains("estudio") || n.contains("examen") {
        "uni".into()
    } else if n.contains("trabajo") || n.contains("trab") || n.contains("reunión") || n.contains("reunion") {
        "trab".into()
    } else if n.contains("personal") {
        "per".into()
    } else if n.contains("finan") || n.contains("pagar") || n.contains("factura") {
        "fin".into()
    } else if n.contains("salud") || n.contains("médico") || n.contains("medico") || n.contains("gimnasio") {
        "sal".into()
    } else {
        "otr".into()
    }
}

pub fn priority_from_name(p: &str) -> String {
    let n = p.trim().to_lowercase();
    if n.contains("alta") || n.contains("urgente") || n.contains("high") {
        "alta".into()
    } else if n.contains("baja") || n.contains("low") {
        "baja".into()
    } else {
        "media".into()
    }
}

/// Resultado validado del Módulo 1 (tarea desde lenguaje natural).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTask {
    pub title: String,
    pub description: String,
    pub category_id: String,
    pub priority: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    pub location: String,
    pub tags: Vec<String>,
    pub reminders: Vec<String>,
}

/// Evento extraído de un correo (Módulo 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedEvent {
    pub title: String,
    pub description: String,
    pub category_id: String,
    pub priority: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    pub location: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailParseResult {
    pub is_relevant: bool,
    pub confidence: f64,
    pub reason: String,
    pub events: Vec<ParsedEvent>,
}

fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn parse_time(s: &str) -> Option<chrono::NaiveTime> {
    let s = s.trim();
    for f in ["%H:%M", "%H:%M:%S", "%I:%M %p", "%I %p", "%I%p"] {
        if let Ok(t) = chrono::NaiveTime::parse_from_str(s, f) {
            return Some(t);
        }
    }
    let lower = s.to_lowercase();
    for (end, marker) in [("a. m.", "am"), ("p. m.", "pm"), ("am", "am"), ("pm", "pm")] {
        if lower.ends_with(end) {
            if let Ok(h) = lower.trim_end_matches(end).trim().parse::<u32>() {
                let mut h = h;
                if marker == "pm" && h < 12 {
                    h += 12;
                }
                if marker == "am" && h == 12 {
                    h = 0;
                }
                return chrono::NaiveTime::from_hms_opt(h.min(23), 0, 0);
            }
        }
    }
    if let Ok(h) = s.parse::<u32>() {
        if (0..=23).contains(&h) {
            return chrono::NaiveTime::from_hms_opt(h, 0, 0);
        }
    }
    None
}

pub fn naive_to_ms(date: chrono::NaiveDate, time: chrono::NaiveTime) -> i64 {
    let dt = date.and_time(time);
    dt.and_utc().timestamp_millis()
}

pub fn parse_date_ms(date: &str, time: &str) -> Option<i64> {
    let d = parse_date(date)?;
    let t = match parse_time(time) {
        Some(t) => t,
        None => chrono::NaiveTime::from_hms_opt(9, 0, 0)?,
    };
    Some(naive_to_ms(d, t))
}

const HOUR: i64 = 3_600_000;

/// Construye una ParsedTask a partir del JSON devuelto por la IA.
/// Aplica todos los defaults y reglas de la aplicación. Nunca confía en el proveedor.
pub fn validate_task_json(v: &serde_json::Value) -> Result<ParsedTask, AiError> {
    let obj = v
        .as_object()
        .ok_or_else(|| AiError::InvalidJson("la raíz debe ser un objeto".into()))?;
    let title = obj
        .get("title")
        .and_then(|t| t.as_str())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| AiError::InvalidJson("falta campo 'title'".into()))?;

    let description = obj.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
    let category_id = obj
        .get("category")
        .and_then(|c| c.as_str())
        .map(category_id_from_name)
        .unwrap_or_else(|| "otr".into());
    let priority = obj
        .get("priority")
        .and_then(|p| p.as_str())
        .map(priority_from_name)
        .unwrap_or_else(|| "media".into());
    let location = obj.get("location").and_then(|l| l.as_str()).unwrap_or("").to_string();

    let tags: Vec<String> = obj
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let reminders: Vec<String> = obj
        .get("reminders")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let now = chrono::Utc::now().date_naive();
    let start_date = obj
        .get("start_date")
        .and_then(|s| s.as_str())
        .and_then(parse_date)
        .unwrap_or(now);
    let end_date = obj
        .get("end_date")
        .and_then(|s| s.as_str())
        .and_then(parse_date)
        .unwrap_or(start_date);

    let start_time_raw = obj.get("start_time").and_then(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let end_time_raw = obj.get("end_time").and_then(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let all_day = start_time_raw.is_none() || end_time_raw.is_none();

    let start_time = match &start_time_raw {
        Some(s) => parse_time(s).unwrap_or(chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
        None => chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    };
    let end_time = match &end_time_raw {
        Some(s) => parse_time(s).unwrap_or(start_time),
        None => start_time,
    };

    let mut start_ms = naive_to_ms(start_date, start_time);
    let mut end_ms = naive_to_ms(end_date, end_time);
    if end_ms < start_ms {
        end_ms = start_ms + HOUR;
    }

    let start_now = now.and_time(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let start_day = start_date;
    let _ = start_now;
    if start_day < now {
        // la IA devolvió una fecha pasada → la subimos a hoy
        let delta = (now - start_day).num_days();
        start_ms += delta * 24 * HOUR;
        end_ms += delta * 24 * HOUR;
    }

    Ok(ParsedTask {
        title,
        description,
        category_id,
        priority,
        start_ms,
        end_ms,
        all_day,
        location,
        tags,
        reminders,
    })
}

pub fn validate_email_json(v: &serde_json::Value) -> Result<EmailParseResult, AiError> {
    let obj = v
        .as_object()
        .ok_or_else(|| AiError::InvalidJson("la raíz debe ser un objeto".into()))?;
    let is_relevant = obj.get("is_relevant").and_then(|b| b.as_bool()).unwrap_or(false);
    let confidence = obj
        .get("confidence")
        .and_then(|c| c.as_f64())
        .map(|c| c.clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let reason = obj.get("reason").and_then(|r| r.as_str()).unwrap_or("").to_string();
    let mut events = Vec::new();
    if let Some(arr) = obj.get("events").and_then(|e| e.as_array()) {
        for ev in arr {
            let task = validate_task_json(ev)?;
            events.push(ParsedEvent {
                title: task.title,
                description: task.description,
                category_id: task.category_id,
                priority: task.priority,
                start_ms: task.start_ms,
                end_ms: task.end_ms,
                all_day: task.all_day,
                location: task.location,
                tags: task.tags,
            });
        }
    }
    Ok(EmailParseResult {
        is_relevant,
        confidence,
        reason,
        events,
    })
}

pub const TASK_SCHEMA: &str = r#"{
  "title": "Título corto de la tarea (sin fecha ni hora)",
  "description": "Descripción opcional",
  "category": "Universidad | Trabajo | Personal | Finanzas | Salud | Otros",
  "priority": "Alta | Media | Baja",
  "start_date": "YYYY-MM-DD",
  "end_date": "YYYY-MM-DD",
  "start_time": "HH:MM (formato 24h). SOLO si hay hora explícita en el texto; si no la hay, dejarlo vacío y la tarea será de Todo el día",
  "end_time": "HH:MM (formato 24h). SOLO si hay hora final explícita; si no, vacío",
  "tags": ["tag1"],
  "location": "",
  "reminders": ["1d", "1h"]
}"#;

pub const EMAIL_SCHEMA: &str = r#"{
  "is_relevant": true,
  "confidence": 0.0-1.0,
  "reason": "por qué el correo es (o no) relevante",
  "events": [
    {
      "title": "Título corto",
      "description": "Detalle",
      "category": "Universidad | Trabajo | Personal | Finanzas | Salud | Otros",
      "start_date": "YYYY-MM-DD",
      "end_date": "YYYY-MM-DD",
      "start_time": "HH:MM",
      "end_time": "HH:MM",
      "priority": "Alta | Media | Baja",
      "location": "",
      "tags": []
    }
  ]
}"#;
