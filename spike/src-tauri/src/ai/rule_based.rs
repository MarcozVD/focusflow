//! Proveedor determinista local (fase 4): texto → JSON del esquema de
//! intents, SIN red ni clave. Cumple el requisito de fallback: el sistema
//! funciona completo sin proveedor externo.
//!
//! Todo lo que esta función emite pasa por el MISMO validador que la salida
//! de un LLM ([super::intent_validator]), garantizando invariantes únicas.

use chrono::{TimeZone, Timelike};
use serde_json::json;

use super::nl;
use super::provider::AiResult;

const HOUR_MS: i64 = 3_600_000;
const MIN_MS: i64 = 60_000;

/// Intención intermedia antes de convertirse al JSON del esquema.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleIntent {
    pub intent_type: &'static str,
    pub title: String,
    pub category_id: String,
    pub priority: &'static str,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub all_day: bool,
    pub duration_min: Option<u32>,
    pub deadline_ms: Option<i64>,
    pub prep_min: Option<u32>,
    pub prep_note: String,
    pub recurrence_freq: Option<&'static str>,
    pub recurrence_by_day: Vec<u8>,
    pub reminders_min: Option<u32>,
    pub constraints: Vec<(String, Option<String>, Option<String>)>,
    pub confidence: f64,
    pub reason: String,
}

fn now() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn ms_to_date(ms: i64) -> chrono::NaiveDate {
    chrono::Local.timestamp_millis_opt(ms).earliest().map(|d| d.date_naive()).unwrap_or_else(now)
}

fn next_weekday(by_day: &[u8]) -> chrono::NaiveDate {
    let today = now();
    let today_wd = nl::weekday_num(today);
    if by_day.contains(&today_wd) && chrono::Local::now().time().hour() < 21 {
        return today;
    }
    for d in 1..=7 {
        let c = today + chrono::Duration::days(d);
        if by_day.contains(&nl::weekday_num(c)) {
            return c;
        }
    }
    today
}

// ---------------------------------------------------------------------------
// Detecciones específicas
// ---------------------------------------------------------------------------

fn detect_recurrence(lower: &str) -> Option<(&'static str, Vec<u8>)> {
    if lower.contains("entre semana")
        || lower.contains("lunes a viernes")
        || lower.contains("weekdays")
        || lower.contains("weekday")
        || lower.contains("días hábiles")
        || lower.contains("dias habiles")
        || lower.contains("mon-fri")
    {
        return Some(("weekly", vec![1, 2, 3, 4, 5]));
    }
    if lower.contains("daily")
        || lower.contains("every day")
        || lower.contains("todos los días")
        || lower.contains("todos los dias")
        || lower.contains("cada día")
        || lower.contains("cada dia")
        || lower.contains("diario")
    {
        return Some(("daily", vec![]));
    }
    // "every monday", "cada lunes", "todos los martes", "each wednesday"
    let re = regex::Regex::new(
        r"(?:every|each|cada|todos los|todas las)\s+(lunes|martes|mi[eé]rcoles|jueves|viernes|s[aá]bado|domingo|monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tue|wed|thu|fri|sat|sun)",
    )
    .ok()?;
    if let Some(caps) = re.captures(lower) {
        if let Some(wd) = nl::weekday_from_name(caps.get(1)?.as_str()) {
            return Some(("weekly", vec![wd.number_from_monday() as u8]));
        }
    }
    if lower.contains("weekly") || lower.contains("every week") || lower.contains("semanal") || lower.contains("cada semana") {
        return Some(("weekly", vec![]));
    }
    None
}

fn detect_prep_min(lower: &str) -> Option<(u32, String)> {
    // "need four hours to finish" / "necesito 4 horas para terminar"
    let re = regex::Regex::new(
        r"(?:need|necesito|requiero|requerir)\s+(?:at least|al menos|m[íi]nimo)?\s*(\d+(?:\.\d+)?)\s*(?:hours?|horas?)",
    )
    .ok()?;
    if let Some(caps) = re.captures(lower) {
        if let Ok(v) = caps.get(1)?.as_str().parse::<f64>() {
            let mins = (v * 60.0).round() as u32;
            return Some((mins, caps.get(0)?.as_str().to_string()));
        }
    }
    // "four hours to finish the project" — duración + verbo de preparación
    if let Some(dur) = nl::duration_from_text(lower) {
        if lower.contains("to finish")
            || lower.contains("to prepare")
            || lower.contains("para terminar")
            || lower.contains("para preparar")
            || lower.contains("para prepararme")
        {
            return Some((dur as u32, "preparación requerida".into()));
        }
    }
    None
}

/// "6 PM", "18:00", "6", "8 am" → minutos desde medianoche (solo con am/pm
/// o dos puntos se acepta el dígito suelto).
fn parse_clock(s: &str) -> Option<u32> {
    let re = regex::Regex::new(r"^\s*(\d{1,2})(?::(\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?\s*$").ok()?;
    let caps = re.captures(s)?;
    let mut h: u32 = caps.get(1)?.as_str().parse().ok()?;
    let mm: u32 = caps.get(2).map_or(Ok(0), |m| m.as_str().parse()).ok()?;
    let ampm = caps.get(3).map(|m| m.as_str()).unwrap_or("");
    let has_suffix = !ampm.is_empty() || s.contains(':');
    if !has_suffix {
        return None; // dígito suelto sin am/pm ni ":" → no hora
    }
    if ampm.contains("pm") && h < 12 {
        h += 12;
    }
    if ampm.contains("am") && h == 12 {
        h = 0;
    }
    if h <= 23 && mm <= 59 {
        Some(h * 60 + mm)
    } else {
        None
    }
}

/// "de 3 pm a 6 pm" (nl), "from 6 PM to 9 PM", "entre 8 y 12".
fn detect_time_only(lower: &str) -> Option<(u32, u32)> {
    if let Some((s, e)) = nl::time_range(lower) {
        return Some((s, e));
    }
    let re = regex::Regex::new(
        r"\b(?:from|entre)\s+(\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?)\s+(?:to|until|hasta|y)\s+(\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?)\b",
    )
    .ok()?;
    if let Some(caps) = re.captures(lower) {
        let s = parse_clock(caps.get(1)?.as_str())?;
        let mut e = parse_clock(caps.get(2)?.as_str())?;
        if e <= s {
            e += 12 * 60;
        }
        return Some((s, e));
    }
    let s = nl::hour_from_text(lower)?;
    let dur = nl::duration_from_text(lower).unwrap_or(60);
    Some((s, (s as i64 + dur) as u32))
}

/// "del 5 al 23 de agosto", "5 de agosto al 23 de agosto",
/// "August 5 through August 23", "August 5 through 23".
fn detect_date_range(lower: &str) -> Option<(i64, i64)> {
    let (y0, _m0, _) = nl::ymd(now());
    let months_pat = "(enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre|january|february|march|april|may|june|july|august|september|october|november|december)";

    let re1 = regex::Regex::new(&format!(r"del (\d{{1,2}}) (?:al|a|hasta) (?:el\s+)?(\d{{1,2}}) de {months_pat}")).ok()?;
    let re2 = regex::Regex::new(&format!(r"(\d{{1,2}}) de {months_pat} (?:al|a|hasta) (?:el\s+|d[íi]a\s+)?(\d{{1,2}}) de {months_pat}")).ok()?;
    let re3 = regex::Regex::new(&format!(r"{months_pat} (\d{{1,2}})(?:st|nd|rd|th)? (?:through|to|until|thru) {months_pat} (\d{{1,2}})(?:st|nd|rd|th)?")).ok()?;
    let re4 = regex::Regex::new(&format!(r"({months_pat}) (\d{{1,2}})(?:st|nd|rd|th)? (?:through|to|until|thru) (\d{{1,2}})(?:st|nd|rd|th)?")).ok()?;

    let mk = |month: &str, day: u32, year: i32| -> Option<i64> {
        let m = nl::month_number(month)?;
        let mut y = year;
        let (_, cur_m, _) = nl::ymd(now());
        if m < cur_m {
            y += 1;
        }
        let date = chrono::NaiveDate::from_ymd_opt(y, m, day)?;
        Some(nl::local_ms(date.and_hms_opt(0, 0, 0).unwrap()))
    };
    // la ventana puede estar en curso (inicio en el pasado): solo se exige
    // que el FINAL sea futuro.
    let in_future = |ms: i64| ms >= nl::local_ms(now().and_hms_opt(0, 0, 0).unwrap());

    if let Some(caps) = re1.captures(lower) {
        let d1: u32 = caps.get(1)?.as_str().parse().ok()?;
        let d2: u32 = caps.get(2)?.as_str().parse().ok()?;
        let month = caps.get(3)?.as_str();
        let s = mk(month, d1, y0)?;
        let e = mk(month, d2, y0)?;
        if s < e && in_future(e) {
            return Some((s, e));
        }
    }
    if let Some(caps) = re2.captures(lower) {
        let d1: u32 = caps.get(1)?.as_str().parse().ok()?;
        let m1 = caps.get(2)?.as_str();
        let d2: u32 = caps.get(3)?.as_str().parse().ok()?;
        let m2 = caps.get(4)?.as_str();
        let s = mk(m1, d1, y0)?;
        let mut e = mk(m2, d2, y0)?;
        if nl::month_number(m2)? < nl::month_number(m1)? {
            e += 365 * 24 * HOUR_MS; // cruce de año ("del 25 de diciembre al 2 de enero")
        }
        if s < e && in_future(e) {
            return Some((s, e));
        }
    }
    if let Some(caps) = re3.captures(lower) {
        let d1: u32 = caps.get(2)?.as_str().parse().ok()?;
        let d2: u32 = caps.get(4)?.as_str().parse().ok()?;
        let s = mk(caps.get(1)?.as_str(), d1, y0)?;
        let mut e = mk(caps.get(3)?.as_str(), d2, y0)?;
        if nl::month_number(caps.get(3)?.as_str())? < nl::month_number(caps.get(1)?.as_str())? {
            e += 365 * 24 * HOUR_MS;
        }
        if s < e && in_future(e) {
            return Some((s, e));
        }
    }
    if let Some(caps) = re4.captures(lower) {
        let d1: u32 = caps.get(2)?.as_str().parse().ok()?;
        let d2: u32 = caps.get(3)?.as_str().parse().ok()?;
        let s = mk(caps.get(1)?.as_str(), d1, y0)?;
        let e = mk(caps.get(1)?.as_str(), d2, y0)?;
        if s < e && in_future(e) {
            return Some((s, e));
        }
    }
    // rangos por día de la semana: "del lunes al viernes", "lunes a viernes",
    // "monday to friday" (próximas ocurrencias, como parse_day)
    let wd_pat = "(lunes|martes|mi[eé]rcoles|jueves|viernes|s[aá]bado|domingo|monday|tuesday|wednesday|thursday|friday|saturday|sunday)";
    let wd_ms = |caps: &regex::Captures| -> Option<(i64, i64)> {
        let w1 = nl::weekday_from_name(caps.get(1)?.as_str())?;
        let w2 = nl::weekday_from_name(caps.get(2)?.as_str())?;
        let today = chrono::Local::now().date_naive();
        let start = today + chrono::Duration::days(nl::weekday_delta(w1));
        let mut delta = (w2.number_from_monday() as i64 - w1.number_from_monday() as i64 + 7) % 7;
        if delta == 0 {
            delta = 7; // "del lunes al lunes" = semana completa
        }
        let end = start + chrono::Duration::days(delta);
        if in_future(nl::local_ms(end.and_hms_opt(0, 0, 0).unwrap())) {
            Some((
                nl::local_ms(start.and_hms_opt(0, 0, 0).unwrap()),
                nl::local_ms(end.and_hms_opt(0, 0, 0).unwrap()),
            ))
        } else {
            None
        }
    };
    let re_wd1 = regex::Regex::new(&format!(r"del\s+{wd_pat}\s+(?:al|a|hasta)\s+(?:el\s+|d[íi]a\s+)?{wd_pat}")).ok()?;
    if let Some(caps) = re_wd1.captures(lower) {
        if let Some(pair) = wd_ms(&caps) {
            return Some(pair);
        }
    }
    let re_wd2 = regex::Regex::new(&format!(r"\b{wd_pat}\s+(?:al|a|hasta)\s+(?:el\s+|d[íi]a\s+)?{wd_pat}")).ok()?;
    if let Some(caps) = re_wd2.captures(lower) {
        if let Some(pair) = wd_ms(&caps) {
            return Some(pair);
        }
    }
    let re_wd3 = regex::Regex::new(&format!(r"(?:from\s+)?{wd_pat}\s+(?:to|through|until)\s+(?:next\s+|this\s+)?{wd_pat}")).ok()?;
    if let Some(caps) = re_wd3.captures(lower) {
        if let Some(pair) = wd_ms(&caps) {
            return Some(pair);
        }
    }
    None
}

/// Restricción dura: "Don't schedule anything before 6 AM".
fn detect_constraint(lower: &str) -> Option<(String, String, String)> {
    let re = regex::Regex::new(
        r"(?:don'?t|do not|never|no)\s+(?:schedule|program(?:ar|es)?|plan(?:ificar|ificas)?|agendar|agendes|work|estudiar)\b[^.]*?(before|antes de|after|despu[eé]s de)\s+(?:las?\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?",
    )
    .ok()?;
    let caps = re.captures(lower)?;
    let dir = caps.get(1)?.as_str();
    let is_after = dir.contains("after") || dir.contains("despu");
    let h = caps.get(2)?.as_str();
    let mm = caps.get(3).map_or("", |m| m.as_str());
    let ampm = caps.get(4).map_or("", |m| m.as_str());
    let raw = if mm.is_empty() {
        format!("{h} {ampm}")
    } else {
        format!("{h}:{mm} {ampm}")
    };
    let minutes = parse_clock(&raw)?;
    let value = format!("{:02}:{:02}", minutes / 60, minutes % 60);
    let title = if is_after {
        format!("No programar después de las {value}")
    } else {
        format!("No programar antes de las {value}")
    };
    Some(("daily_cap".to_string(), title, value))
}

/// Título limpio: elimina frases de preparación/disponibilidad/restricción/
/// rango de fechas y artículos iniciales, antes del limpiador temporal del
/// módulo 1. Vacío si el texto es solo señales temporales.
fn clean_title(text: &str) -> String {
    let mut t = text.to_string();
    for p in [
        r"(?i)\b(?:need|necesito|requiero)\s+(?:at least|al menos|m[íi]nimo)?\s*(?:\d+(?:\.\d+)?|\w+)\s*(?:hours?|horas?)\b",
        r"(?i)\b(?:to finish|to prepare|para terminar|para preparar(?:me)?)\b",
        r"(?i)\b(?:available|disponible|is available)\b",
        r"(?i)\b(?:every weekday|weekdays?|entre semana|lunes a viernes|todos los d[íi]as|cada d[íi]a|daily|every day|every week|weekly|semanal|cada semana)\b",
        r"(?i)\b(?:don'?t|do not|no)\s+(?:schedule|program(?:ar|es)?|plan|agendar|agendes|work|estudiar)\b[^.]*?(?:before|antes de|after|despu[eé]s de)\s+(?:las?\s+)?\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?",
        r"(?i)\b(?:due|deadline|vence|entrega|to submit|submit|before|antes de|para el|para la)\s+(?:el\s+|d[íi]a\s+)?(?:[a-záéíóúñ]+\s+)?[a-záéíóúñ]+(?:,\s*)?\b",
        r"(?i)\b(?:through|until|thru)\s+(?:[a-záéíóúñ]+\s+)?\d{1,2}(?:st|nd|rd|th)?\b",
        r"(?i)\b(?:al|a|hasta)\s+(?:el\s+|d[íi]a\s+)?\d{1,2}\s+de\s+[a-záéíóúñ]+\b",
        r"(?i)\bdel\s+\d{1,2}\b",
        r"(?i)\b(?:a|at|hasta)\s+(?:las?\s+)?\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?\b",
        r"(?i)\b[a-záéíóúñ]+\s+\d{1,2}(?:st|nd|rd|th)?\b",
        r"(?i)\b(?:from|de)\s+\d{1,2}(?::\d{2})?\s*(?:am|pm)?\s*(?:to|a|hasta)\s+\d{1,2}(?::\d{2})?\s*(?:am|pm)?\b",
        r"(?i)\b(is|es|the)\b",
        r"(?i)\b(?:del\s+)?(?:lunes|martes|mi[eé]rcoles|jueves|viernes|s[aá]bado|domingo|monday|tuesday|wednesday|thursday|friday|saturday|sunday)\s+(?:al|a|hasta|to|through|until)\s+(?:el\s+|next\s+|this\s+|d[íi]a\s+)?(?:lunes|martes|mi[eé]rcoles|jueves|viernes|s[aá]bado|domingo|monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
    ] {
        if let Ok(re) = regex::Regex::new(p) {
            t = re.replace_all(&t, " ").to_string();
        }
    }
    nl::build_title(&t)
}

fn title_or(intent_type: &str, t: String) -> String {
    if t.is_empty() || t == "Tarea" {
        match intent_type {
            "availability" => "Disponibilidad".into(),
            "constraint" => "Restricción de horario".into(),
            "preparation" => "Preparación".into(),
            "reminder" => "Recordatorio".into(),
            "deadline" => "Vencimiento".into(),
            _ => "Tarea".into(),
        }
    } else {
        t
    }
}

// ---------------------------------------------------------------------------
// Análisis por cláusula
// ---------------------------------------------------------------------------

fn analyze_clause(clause: &str) -> RuleIntent {
    let lower = clause.to_lowercase();

    // 1. restricción dura (manda sobre todo lo demás)
    if let Some((kind, _constraint_title, value)) = detect_constraint(&lower) {
        return RuleIntent {
            intent_type: "constraint",
            title: title_or("constraint", clean_title(clause)),
            category_id: "otr".into(),
            priority: "alta",
            start_ms: None,
            end_ms: None,
            all_day: false,
            duration_min: None,
            deadline_ms: None,
            prep_min: None,
            prep_note: String::new(),
            recurrence_freq: None,
            recurrence_by_day: Vec::new(),
            reminders_min: None,
            constraints: vec![(kind, None, Some(value))],
            confidence: 0.8,
            reason: "restricción explícita (ventana dura)".into(),
        };
    }

    let is_availability = lower.contains("available")
        || lower.contains("disponible")
        || lower.contains("ventana")
        || lower.contains("window");

    // 2. rango de fechas → evento multi-día, o disponibilidad si el texto
    // declara disponibilidad explícita ("disponible del 5 al 23").
    let date_range = detect_date_range(&lower);
    if let Some((s, e)) = date_range {
        let intent_type = if is_availability { "availability" } else { "event" };
        let reason = if is_availability {
            "ventana de disponibilidad con ambos extremos"
        } else {
            "evento multi-día con rango de fechas"
        };
        return RuleIntent {
            intent_type,
            title: title_or(intent_type, clean_title(clause)),
            category_id: nl::detect_category(&lower).into(),
            priority: if lower.contains("urgente") || lower.contains("urgent") { "alta" } else { "media" },
            start_ms: Some(s),
            end_ms: Some(e),
            all_day: true,
            duration_min: None,
            deadline_ms: None,
            prep_min: None,
            prep_note: String::new(),
            recurrence_freq: None,
            recurrence_by_day: Vec::new(),
            reminders_min: None,
            constraints: Vec::new(),
            confidence: 0.85,
            reason: reason.into(),
        };
    }

    // 3. disponibilidad recurrente con horario ("every weekday from 6 PM to 9 PM")
    let recurrence = detect_recurrence(&lower);
    let time_only = detect_time_only(&lower);
    if is_availability && recurrence.is_some() && time_only.is_some() {
        let (freq, by_day) = recurrence.clone().unwrap();
        let (s_min, e_min) = time_only.unwrap();
        let day = next_weekday(&by_day);
        let day_ms = nl::local_ms(day.and_hms_opt(0, 0, 0).unwrap());
        return RuleIntent {
            intent_type: "availability",
            title: title_or("availability", clean_title(clause)),
            category_id: nl::detect_category(&lower).into(),
            priority: "media",
            start_ms: Some(day_ms + s_min as i64 * MIN_MS),
            end_ms: Some(day_ms + e_min as i64 * MIN_MS),
            all_day: false,
            duration_min: None,
            deadline_ms: None,
            prep_min: None,
            prep_note: String::new(),
            recurrence_freq: Some(freq),
            recurrence_by_day: by_day,
            reminders_min: None,
            constraints: Vec::new(),
            confidence: 0.85,
            reason: "disponibilidad recurrente con horario".into(),
        };
    }

    let day_ms = nl::parse_day(&lower);
    let deadline_kw = lower.contains("due")
        || lower.contains("deadline")
        || lower.contains("vence")
        || lower.contains("entrega")
        || lower.contains("submit")
        || lower.contains("before")
        || lower.contains("antes de")
        || lower.contains("para el")
        || lower.contains("para la");
    let is_reminder = lower.contains("remind")
        || lower.contains("recordarme")
        || lower.contains("recuérdame")
        || lower.contains("recuerdame")
        || lower.contains("avisar")
        || lower.contains("avísame")
        || lower.contains("avisame");

    // 4. preparación ("need four hours to finish...")
    let prep = detect_prep_min(&lower);
    let prep_min = prep.as_ref().map(|(m, _)| *m);
    let prep_note = prep.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();

    // 5. vencimiento ("Project due Monday", "before Monday")
    let deadline_ms = if deadline_kw {
        day_ms.map(|d| d + 23 * HOUR_MS + 59 * MIN_MS)
    } else {
        None
    };

    let duration_min = if prep_min.is_some() {
        None // duración que es preparación no es la duración del evento
    } else {
        nl::duration_from_text(&lower).map(|m| m as u32)
    };

    let ambiguous_time = if time_only.is_some() && day_ms.is_none() {
        true // hora sin fecha → ambigua
    } else {
        match time_only {
            Some((s, _)) if s < 12 * 60 => {
                let has_suffix = regex::Regex::new(r"(?:am|pm|a\.m\.|p\.m\.)").unwrap().is_match(&lower);
                let has_part = lower.contains("tarde") || lower.contains("noche") || lower.contains("mañana ") || lower.contains("morning") || lower.contains("evening");
                !has_suffix && !has_part
            }
            _ => false,
        }
    };

    let base: f64 = if deadline_kw && deadline_ms.is_some() {
        0.65
    } else if prep_min.is_some() && day_ms.is_none() && !deadline_kw {
        0.6
    } else {
        0.5
    };
    let mut confidence = base
        + if day_ms.is_some() { 0.2 } else { 0.0 }
        + if time_only.is_some() { 0.15 } else { 0.0 }
        + if duration_min.is_some() || prep_min.is_some() { 0.05 } else { 0.0 }
        + if recurrence.is_some() { 0.1 } else { 0.0 }
        - if ambiguous_time { 0.3 } else { 0.0 };
    confidence = confidence.clamp(0.0, 0.95);

    // tipo de intención
    let (intent_type, reminders_min): (&'static str, Option<u32>) = if prep_min.is_some() && day_ms.is_none() && deadline_ms.is_none() {
        ("preparation", None)
    } else if is_reminder && day_ms.is_none() && time_only.is_none() {
        ("reminder", Some(60))
    } else if deadline_ms.is_some() {
        ("deadline", if is_reminder { Some(60) } else { None })
    } else if day_ms.is_some() || time_only.is_some() {
        ("event", if is_reminder { Some(60) } else { None })
    } else {
        ("task", if is_reminder { Some(60) } else { None })
    };

    let (start_ms, end_ms, all_day) = match (day_ms, time_only) {
        (Some(d), Some((s, e))) => (Some(d + s as i64 * MIN_MS), Some(d + e as i64 * MIN_MS), false),
        (Some(d), None) => (Some(d), None, true),
        // hora sin fecha: se conserva la hora, sin inventar el día
        (None, Some(_)) => (None, None, false),
        (None, None) => (None, None, true),
    };

    let reason = if ambiguous_time {
        "hora sin fecha o sin am/pm: interpretación ambigua".into()
    } else if day_ms.is_some() {
        "fecha explícita o relativa".into()
    } else if prep_min.is_some() {
        "requisito de preparación".into()
    } else {
        "sin señales temporales: tarea de backlog".into()
    };

    RuleIntent {
        intent_type,
        title: title_or(intent_type, clean_title(clause)),
        category_id: nl::detect_category(&lower).into(),
        priority: if lower.contains("urgente") || lower.contains("urgent") || lower.contains("high") { "alta" } else if lower.contains("baja") || lower.contains("low") { "baja" } else { "media" },
        start_ms,
        end_ms,
        all_day,
        duration_min,
        deadline_ms,
        prep_min,
        prep_note,
        recurrence_freq: None,
        recurrence_by_day: Vec::new(),
        reminders_min,
        constraints: Vec::new(),
        confidence,
        reason,
    }
}

/// Separa compromisos múltiples ("y"/", "). Solo divide cuando ambos lados
/// tienen al menos una señal temporal o de intención.
fn split_clauses(text: &str) -> Vec<String> {
    let parts: Vec<&str> = text
        .split(|c| c == ',' || c == ';')
        .flat_map(|s| s.split(" y "))
        .flat_map(|s| s.split(" and "))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() < 2 {
        return vec![text.trim().to_string()];
    }
    let signal = |s: &str| {
        let l = s.to_lowercase();
        nl::parse_day(&l).is_some()
            || nl::hour_from_text(&l).is_some()
            || nl::time_range(&l).is_some()
            || nl::duration_from_text(&l).is_some()
            || detect_prep_min(&l).is_some()
            || l.contains("due")
            || l.contains("before")
            || l.contains("antes de")
            || detect_constraint(&l).is_some()
            || detect_date_range(&l).is_some()
    };
    let all_signaled = parts.iter().all(|p| signal(p));
    if all_signaled {
        parts.into_iter().map(|s| s.to_string()).collect()
    } else {
        vec![text.trim().to_string()]
    }
}

/// Prepara una intención de preparación suelta y la adjunta al compromiso
/// con fecha si existe (regla de compuestos).
fn merge_preparation(mut intents: Vec<RuleIntent>) -> Vec<RuleIntent> {
    let prep_idx = intents.iter().position(|i| i.intent_type == "preparation");
    let Some(prep_idx) = prep_idx else { return intents };
    let prep = intents.remove(prep_idx);
    let Some((m, n)) = prep.prep_min.map(|m| (m, prep.prep_note.clone())) else { return intents };
    if let Some(target) = intents.iter_mut().find(|i| i.start_ms.is_some() || i.deadline_ms.is_some()) {
        target.prep_min = Some(m);
        if !n.is_empty() {
            target.prep_note = n;
        }
        return intents;
    }
    // sin compromiso con fecha: queda como Preparation
    let mut prep = prep;
    prep.prep_min = Some(m);
    intents.push(prep);
    intents
}

// ---------------------------------------------------------------------------
// Salida al esquema
// ---------------------------------------------------------------------------

fn to_schema_json(i: &RuleIntent) -> serde_json::Value {
    let (sd, st) = match (i.start_ms, i.all_day) {
        (Some(ms), false) => {
            let dt = chrono::Local.timestamp_millis_opt(ms).earliest();
            match dt {
                Some(dt) => (Some(dt.format("%Y-%m-%d").to_string()), Some(dt.format("%H:%M").to_string())),
                None => (None, None),
            }
        }
        (Some(ms), true) => (Some(ms_to_date(ms).format("%Y-%m-%d").to_string()), None),
        _ => (None, None),
    };
    let (ed, et) = match (i.end_ms, i.all_day) {
        (Some(ms), false) => {
            let dt = chrono::Local.timestamp_millis_opt(ms).earliest();
            match dt {
                Some(dt) => (Some(dt.format("%Y-%m-%d").to_string()), Some(dt.format("%H:%M").to_string())),
                None => (None, None),
            }
        }
        (Some(ms), true) => (Some(ms_to_date(ms).format("%Y-%m-%d").to_string()), None),
        _ => (None, None),
    };

    let recurrence = i.recurrence_freq.map(|f| {
        json!({
            "frequency": f,
            "interval": 1,
            "by_day": i.recurrence_by_day,
            "count": null,
            "until": null,
        })
    });

    json!({
        "intent_type": i.intent_type,
        "title": i.title,
        "category": category_name(&i.category_id),
        "priority": i.priority,
        "start_date": sd,
        "start_time": st,
        "end_date": ed,
        "end_time": et,
        "duration_minutes": i.duration_min,
        "deadline_date": i.deadline_ms.map(|d| ms_to_date(d).format("%Y-%m-%d").to_string()),
        "deadline_time": i.deadline_ms.map(|_| "23:59".to_string()),
        "preparation_minutes": i.prep_min,
        "preparation_note": if i.prep_note.is_empty() { None } else { Some(i.prep_note.clone()) },
        "recurrence": recurrence,
        "reminders": i.reminders_min.map(|m| json!([{"minutes_before": m, "at": null}])).unwrap_or_else(|| json!([])),
        "constraints": i.constraints.iter().map(|(k, t, v)| json!({"kind": k, "target": t, "value": v})).collect::<Vec<_>>(),
        "confidence": i.confidence,
        "reason": i.reason,
    })
}

fn category_name(id: &str) -> &'static str {
    match id {
        "uni" => "Universidad",
        "trab" => "Trabajo",
        "per" => "Personal",
        "fin" => "Finanzas",
        "sal" => "Salud",
        _ => "Otro",
    }
}

/// Análisis determinista completo → JSON del esquema.
pub fn analyze_to_json(text: &str) -> AiResult<serde_json::Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(json!({"intents": []}));
    }
    let mut intents: Vec<RuleIntent> = split_clauses(trimmed)
        .iter()
        .map(|c| analyze_clause(c))
        .collect();
    intents = merge_preparation(intents);
    Ok(json!({ "intents": intents.iter().map(to_schema_json).collect::<Vec<_>>() }))
}

/// Análisis determinista → intents intermedios (para tests).
pub fn analyze(text: &str) -> Vec<RuleIntent> {
    let intents: Vec<RuleIntent> =
        split_clauses(text.trim()).iter().map(|c| analyze_clause(c)).collect();
    merge_preparation(intents)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::intent::{Intent, IntentType};
    use crate::ai::intent_validator::{parse_intent_json, validate_intent};

    const HOUR: i64 = 3_600_000;

    fn intent_of(text: &str, idx: usize) -> Intent {
        let v = analyze_to_json(text).expect("json del esquema");
        let item = v["intents"][idx].clone();
        let i = parse_intent_json(&item).expect("parseo tolerante del esquema");
        validate_intent(&i).expect("invariantes");
        i
    }

    fn count(text: &str) -> usize {
        analyze_to_json(text).unwrap()["intents"].as_array().unwrap().len()
    }

    fn midnight(days: i64) -> i64 {
        let t = chrono::Local::now().date_naive() + chrono::Duration::days(days);
        nl::local_ms(t.and_hms_opt(0, 0, 0).unwrap())
    }

    #[test]
    fn dates_relative_and_absolute() {
        // "Study calculus tomorrow at 4" — fase 4, ejemplo 1
        let i = intent_of("Study calculus tomorrow at 4", 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert_eq!(i.window.start, Some(midnight(1) + 4 * HOUR));
        assert_eq!(i.window.end, Some(midnight(1) + 5 * HOUR));
        assert_eq!(i.title, "Study calculus");
        assert_eq!(i.category_id, "uni");
        // "at 4" sin am/pm → ambigüedad penalizada
        assert!(i.confidence < 0.6, "ambigua → confirmación: {}", i.confidence);
        assert!(i.reason.contains("ambigua"));

        // "Exam Friday at 8 AM" — ejemplo 2
        let i = intent_of("Exam Friday at 8 AM", 0);
        let fri = midnight(crate::ai::nl::weekday_delta(chrono::Weekday::Fri) as i64);
        assert_eq!(i.window.start, Some(fri + 8 * HOUR));
        assert_eq!(i.window.end, Some(fri + 9 * HOUR));
        assert!(!i.window.all_day);
        assert_eq!(i.title, "Exam");
        assert!(i.confidence >= 0.8, "explícita: {}", i.confidence);

        // fecha absoluta con mes (misma regla del parser: si el día ya pasó
        // este mes, rota al próximo día 15)
        let i = intent_of("el 15 de agosto a las 9 presentar informe", 0);
        let today = chrono::Local::now().date_naive();
        let (y, m, _) = nl::ymd(today);
        let year = if 8 < m { y + 1 } else { y };
        let date = chrono::NaiveDate::from_ymd_opt(year, 8, 15).unwrap();
        let expected = if date >= today {
            date
        } else {
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            chrono::NaiveDate::from_ymd_opt(ny, nm, 15).unwrap()
        };
        let dms = nl::local_ms(expected.and_hms_opt(0, 0, 0).unwrap());
        assert_eq!(i.window.start, Some(dms + 9 * HOUR));
        assert_eq!(i.title, "Presentar informe");
    }

    #[test]
    fn durations() {
        // duración con hora de inicio → fin = inicio + duración, duración declarada
        let i = intent_of("Estudiar cálculo mañana a las 10 durante 3 horas", 0);
        assert_eq!(i.window.start, Some(midnight(1) + 10 * HOUR));
        assert_eq!(i.window.end, Some(midnight(1) + 13 * HOUR));
        assert_eq!(i.duration, Some(crate::ai::intent::Duration { minutes: 180 }));

        // rango explícito sin fecha → no inventa día, confianza baja (ambiguo)
        let i = intent_of("estudiar cálculo de 3pm a 5pm", 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert!(i.window.start.is_none(), "no inventa la fecha");
        assert!(i.window.end.is_none());
        assert!(i.confidence < 0.6, "ambiguo: {}", i.confidence);

        // rango en inglés ("from 6 PM to 9 PM")
        let i = intent_of("Available every weekday from 6 PM to 9 PM", 0);
        let dt = chrono::Local
            .timestamp_millis_opt(i.window.start.unwrap())
            .earliest()
            .unwrap();
        assert_eq!(dt.hour(), 18);
        let e = chrono::Local
            .timestamp_millis_opt(i.window.end.unwrap())
            .earliest()
            .unwrap();
        assert_eq!(e.hour(), 21);
    }

    #[test]
    fn deadlines() {
        // "Project due Monday" — ejemplo 3
        let i = intent_of("Project due Monday", 0);
        assert_eq!(i.intent_type, IntentType::Deadline);
        let mon = midnight(crate::ai::nl::weekday_delta(chrono::Weekday::Mon) as i64);
        assert_eq!(i.deadline, Some(mon + 23 * HOUR + 59 * 60_000));
        assert_eq!(i.title, "Project");

        // "Need four hours to finish the project before Monday" — ejemplo 4
        let i = intent_of("Need four hours to finish the project before Monday", 0);
        assert_eq!(i.intent_type, IntentType::Deadline);
        assert_eq!(i.deadline, Some(mon + 23 * HOUR + 59 * 60_000));
        let prep = i.preparation.expect("preparación adjunta");
        assert_eq!(prep.minutes, 240);
        assert_eq!(i.title, "Project");
        assert!(i.confidence >= 0.7);
    }

    #[test]
    fn availability_ranges() {
        // "Diagnostic Test is available August 5 through August 23" — ejemplo 7
        let i = intent_of("Diagnostic Test is available August 5 through August 23", 0);
        assert_eq!(i.intent_type, IntentType::Availability);
        assert!(i.window.all_day);
        let today = chrono::Local::now().date_naive();
        let (y0, m0, _) = nl::ymd(today);
        let year = if 8 < m0 { y0 + 1 } else { y0 };
        let s = nl::local_ms(chrono::NaiveDate::from_ymd_opt(year, 8, 5).unwrap().and_hms_opt(0, 0, 0).unwrap());
        let e = nl::local_ms(chrono::NaiveDate::from_ymd_opt(year, 8, 23).unwrap().and_hms_opt(0, 0, 0).unwrap());
        assert_eq!(i.window.start, Some(s));
        assert_eq!(i.window.end, Some(e));
        assert_eq!(i.title, "Diagnostic Test");

        // rango en español
        let i = intent_of("Disponible del 5 al 23 de agosto", 0);
        assert_eq!(i.intent_type, IntentType::Availability);
        assert!(i.window.start.unwrap() < i.window.end.unwrap());

        // disponibilidad recurrente con horario — ejemplo 5
        let i = intent_of("Available every weekday from 6 PM to 9 PM", 0);
        assert_eq!(i.intent_type, IntentType::Availability);
        let r = i.recurrence.expect("recurrencia");
        assert_eq!(r.frequency, crate::ai::intent::Frequency::Weekly);
        assert_eq!(r.by_day, vec![1, 2, 3, 4, 5]);
        assert!(!i.window.all_day);
        let s = i.window.start.unwrap();
        let e = i.window.end.unwrap();
        assert!(s < e);
        let sd = chrono::Local.timestamp_millis_opt(s).earliest().unwrap();
        let wd = crate::ai::nl::weekday_num(sd.date_naive());
        assert!((1..=5).contains(&wd), "cae en día hábil, fue {}", wd);
        assert_eq!(sd.hour(), 18);
    }

    #[test]
    fn weekday_range_is_multiday_event() {
        // "proyecto del lunes al viernes" → evento multi-día, no disponibilidad
        let i = intent_of("proyecto del lunes al viernes", 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert!(i.window.all_day);
        let s = i.window.start.unwrap();
        let e = i.window.end.unwrap();
        let s_date = chrono::Local.timestamp_millis_opt(s).earliest().unwrap().date_naive();
        assert_eq!(crate::ai::nl::weekday_num(s_date), 1, "empieza lunes, fue {s_date}");
        assert_eq!((e - s) / 86_400_000, 4, "lunes a viernes = 4 días");
        assert_eq!(i.title, "Proyecto");

        // rango de dígitos con actividad también es evento multi-día
        let (_, m0, _) = nl::ymd(chrono::Local::now().date_naive());
        let nm = if m0 == 12 { 1 } else { m0 + 1 };
        let month_name = ["enero","febrero","marzo","abril","mayo","junio","julio","agosto","septiembre","octubre","noviembre","diciembre"][(nm - 1) as usize];
        let i = intent_of(&format!("proyecto del 10 al 15 de {month_name}"), 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert!(i.window.start.unwrap() < i.window.end.unwrap());

        // disponibilidad explícita sigue siendo availability
        let i = intent_of("Disponible del lunes al viernes", 0);
        assert_eq!(i.intent_type, IntentType::Availability);

        // inglés
        let i = intent_of("work on the report monday to friday", 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert!(i.title.to_lowercase().contains("report"), "título: {}", i.title);
    }

    #[test]
    fn constraints() {
        // "Don't schedule anything before 6 AM" — ejemplo 6
        let i = intent_of("Don't schedule anything before 6 AM", 0);
        assert_eq!(i.intent_type, IntentType::Constraint);
        assert_eq!(i.constraints.len(), 1);
        assert_eq!(i.constraints[0].kind, crate::ai::intent::ConstraintKind::DailyCap);
        assert_eq!(i.constraints[0].value.as_deref(), Some("06:00"));

        // "no programar nada después de las 9 pm"
        let i = intent_of("no programes nada después de las 9 pm", 0);
        assert_eq!(i.intent_type, IntentType::Constraint);
        assert_eq!(i.constraints[0].value.as_deref(), Some("21:00"));
    }

    #[test]
    fn missing_information_needs_confirmation() {
        // sin fecha, sin hora → backlog, confianza baja
        let i = intent_of("Estudiar cálculo", 0);
        assert_eq!(i.intent_type, IntentType::Task);
        assert!(i.window.is_empty());
        assert_eq!(i.confidence, 0.5);
        assert!(i.confidence < 0.6, "debe requerir confirmación");

        // hora sin fecha → ambigua, confianza baja, no inventa día
        let i = intent_of("Reunión a las 5", 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert!(i.window.start.is_none(), "no inventa la fecha");
        assert!(i.confidence < 0.6, "debe requerir confirmación");

        // recordatorio sin fecha
        let i = intent_of("Remind me to call mom", 0);
        assert_eq!(i.intent_type, IntentType::Reminder);
        assert_eq!(i.reminders.len(), 1);
        assert!(i.confidence < 0.6);
    }

    #[test]
    fn ambiguity_penalized() {
        let i = intent_of("Study calculus tomorrow at 4", 0);
        assert!(i.confidence < 0.6, "sin am/pm → {} (debe requerir confirmación)", i.confidence);
        let i = intent_of("Study calculus tomorrow at 4 PM", 0);
        assert!(i.confidence >= 0.8, "con am/pm → {}", i.confidence);
    }

    #[test]
    fn multiple_commitments() {
        let text = "Estudiar cálculo mañana y programación el viernes";
        assert_eq!(count(text), 2);
        let a = intent_of(text, 0);
        let b = intent_of(text, 1);
        assert_eq!(a.intent_type, IntentType::Event);
        assert_eq!(b.intent_type, IntentType::Event);
        assert!(a.window.start.unwrap() < b.window.start.unwrap());

        // compromiso compuesto con preparación → se fusiona (regla de compuestos)
        let text = "Examen el viernes y necesito 4 horas para preparar";
        assert_eq!(count(text), 1, "preparación adjunta al evento");
        let i = intent_of(text, 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert_eq!(i.preparation.expect("prep").minutes, 240);
    }

    #[test]
    fn reminders_attach_to_events() {
        let i = intent_of("Recuérdame comprar pan el sábado", 0);
        assert_eq!(i.intent_type, IntentType::Event);
        assert_eq!(i.reminders.len(), 1);
        assert_eq!(i.reminders[0].minutes_before, Some(60));
        assert_eq!(i.title, "Comprar pan");
    }

    #[test]
    fn priorities_and_categories() {
        let i = intent_of("URGENTE pagar factura mañana", 0);
        assert_eq!(i.priority, crate::ai::intent::Priority::Alta);
        assert_eq!(i.category_id, "fin");
        let i = intent_of("cita médico el próximo lunes", 0);
        assert_eq!(i.category_id, "sal");
        let i = intent_of("reunión con el equipo mañana", 0);
        assert_eq!(i.category_id, "trab");
    }

    #[test]
    fn empty_text_yields_no_intents() {
        assert_eq!(count("   "), 0);
        let v = analyze_to_json("   ").unwrap();
        assert_eq!(v["intents"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_matches_schema_and_validates() {
        // toda salida del proveedor local pasa por el mismo validador que la IA
        for text in [
            "Study calculus tomorrow at 4",
            "Exam Friday at 8 AM",
            "Project due Monday",
            "Need four hours to finish the project before Monday",
            "Available every weekday from 6 PM to 9 PM",
            "Don't schedule anything before 6 AM",
            "Diagnostic Test is available August 5 through August 23",
        ] {
            let v = analyze_to_json(text).expect("json");
            let items = v["intents"].as_array().unwrap().clone();
            assert!(!items.is_empty(), "{text}");
            for item in items {
                let i = parse_intent_json(&item).expect("parsea");
                validate_intent(&i).expect("invariantes");
            }
        }
    }
}
