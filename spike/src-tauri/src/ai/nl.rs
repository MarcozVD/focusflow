use super::validation::ParsedTask;

use chrono::TimeZone;

pub(crate) fn weekday_num(date: chrono::NaiveDate) -> u8 {
    date.format("%u").to_string().parse().unwrap_or(0)
}

pub(crate) fn ymd(date: chrono::NaiveDate) -> (i32, u32, u32) {
    let parts: Vec<u32> = date.format("%Y-%m-%d").to_string().split('-').map(|s| s.parse().unwrap()).collect();
    (parts[0] as i32, parts[1], parts[2])
}

pub(crate) fn weekday_delta(target: chrono::Weekday) -> i64 {
    let now = chrono::Local::now().date_naive();
    let mut d = 1;
    loop {
        let wd = now + chrono::Duration::days(d);
        if weekday_num(wd) as u32 == target.number_from_monday() {
            return d;
        }
        d += 1;
    }
}

pub(crate) fn month_number(name: &str) -> Option<u32> {
    Some(match name {
        "enero" | "january" => 1,
        "febrero" | "february" => 2,
        "marzo" | "march" => 3,
        "abril" | "april" => 4,
        "mayo" | "may" => 5,
        "junio" | "june" => 6,
        "julio" | "july" => 7,
        "agosto" | "august" => 8,
        "septiembre" | "setiembre" | "september" => 9,
        "octubre" | "october" => 10,
        "noviembre" | "november" => 11,
        "diciembre" | "december" => 12,
        _ => return None,
    })
}

pub(crate) fn weekday_from_name(s: &str) -> Option<chrono::Weekday> {
    let n = s.trim().to_lowercase();
    if n.starts_with("dom") || n.starts_with("sun") {
        Some(chrono::Weekday::Sun)
    } else if n.starts_with("lun") || n.starts_with("mon") {
        Some(chrono::Weekday::Mon)
    } else if n.starts_with("mar") || n.starts_with("tue") || n.starts_with("tues") {
        Some(chrono::Weekday::Tue)
    } else if n.starts_with("mié") || n.starts_with("mie") || n.starts_with("wed") {
        Some(chrono::Weekday::Wed)
    } else if n.starts_with("jue") || n.starts_with("thu") || n.starts_with("thur") || n.starts_with("thurs") {
        Some(chrono::Weekday::Thu)
    } else if n.starts_with("vie") || n.starts_with("fri") {
        Some(chrono::Weekday::Fri)
    } else if n.starts_with("sáb") || n.starts_with("sab") || n.starts_with("sat") {
        Some(chrono::Weekday::Sat)
    } else {
        None
    }
}

/// Hora desde texto: "3 pm", "15:00", "a las 10 de la mañana", "at 4", "8 AM".
/// Los dígitos sueltos SOLO se aceptan con prefijo temporal ("a las", "at",
/// "de las") o con am/pm o con dos puntos: "el 15" o "2 horas" nunca son hora.
pub(crate) fn hour_from_text(s: &str) -> Option<u32> {
    let lower = s.trim().to_lowercase();
    let m = regex_extract(&lower, r"(\d{1,2})(?:\s*(:|\.)?\s*(\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)").or_else(|| {
        regex_extract(&lower, r"(?:a las|a la|at|alrededor de las|de las)\s+(\d{1,2})(?::(\d{2}))?")
    }).or_else(|| {
        regex_extract(&lower, r"\b(\d{1,2}):(\d{2})\b")
    });
    let (h, mm) = m?;
    let mut h: u32 = h.parse().ok()?;
    let mm: u32 = mm.unwrap_or("0").parse().unwrap_or(0);
    if h < 12 && lower.contains("pm") {
        h += 12;
    }
    if h == 12 && (lower.contains("am") || lower.contains("a. m.")) {
        h = 0;
    }
    Some(h * 60 + mm)
}

pub(crate) fn regex_extract<'a>(s: &'a str, pat: &str) -> Option<(&'a str, Option<&'a str>)> {
    let re = regex::Regex::new(pat).ok()?;
    let caps = re.captures(s)?;
    let g1 = caps.get(1)?.as_str();
    let g2 = caps.get(2).map(|m| m.as_str());
    Some((g1, g2))
}

/// Hora de un extremo de rango: acepta también dígitos sueltos ("de 3 a 5").
pub(crate) fn hour_in_range(s: &str) -> Option<u32> {
    hour_from_text(s).or_else(|| {
        let re = regex::Regex::new(r"(\d{1,2})(?::(\d{2}))?").ok()?;
        let caps = re.captures(s.trim())?;
        let h: u32 = caps.get(1)?.as_str().parse().ok()?;
        let mm: u32 = caps.get(2).map_or(Ok(0), |x| x.as_str().parse()).ok()?;
        if h <= 23 && mm <= 59 { Some(h * 60 + mm) } else { None }
    })
}

/// Parsea "de 3 pm a 6 pm" o "de 3 a 5" y devuelve (hora_inicio_min, hora_fin_min).
pub(crate) fn time_range(text: &str) -> Option<(u32, u32)> {
    let re = regex::Regex::new(r"de (\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?)\s*(?:a|hasta)\s+(\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?)").ok()?;
    let caps = re.captures(text)?;
    let start = hour_in_range(caps.get(1)?.as_str())?;
    let mut end = hour_in_range(caps.get(2)?.as_str())?;
    if end <= start {
        end += 12 * 60;
    }
    Some((start, end))
}

/// Fecha/hora naive LOCAL → ms (la hora del usuario, no UTC: `.and_utc()`
/// desplazaba las tareas según la zona horaria).
pub(crate) fn local_ms(dt: chrono::NaiveDateTime) -> i64 {
    chrono::Local
        .from_local_datetime(&dt)
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| dt.and_utc().timestamp_millis())
}

/// Calcula el día de inicio en ms.
pub(crate) fn parse_day(text: &str) -> Option<i64> {
    let now = chrono::Local::now();
    let today = now.date_naive();
    let lower = text.to_lowercase();

    if lower.contains("pasado mañana") || lower.contains("pasado manana") || lower.contains("day after tomorrow") {
        return Some(local_ms((today + chrono::Duration::days(2)).and_hms_opt(0, 0, 0).unwrap()));
    }
    if lower.contains("mañana") || lower.contains("manana") || lower.contains("tomorrow") {
        return Some(local_ms((today + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap()));
    }
    if lower.contains("hoy") || lower.contains("today") {
        return Some(local_ms(today.and_hms_opt(0, 0, 0).unwrap()));
    }

    // "el 15 de agosto" / "15 de marzo" → fecha absoluta (año actual o siguiente)
    let re_md = regex::Regex::new(
        r"\bde (enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre|january|february|march|april|may|june|july|august|september|october|november|december)\b",
    )
    .ok()?;
    if let Some(caps) = re_md.captures(&lower) {
        if let Some(m) = month_number(caps.get(1)?.as_str()) {
            let pos = caps.get(1)?.start();
            let before = &lower[..pos];
            if let Some(dcaps) = regex::Regex::new(r"(\d{1,2})\s*$").ok()?.captures(before) {
                if let Ok(d) = dcaps.get(1)?.as_str().parse::<u32>() {
                    if (1..=31).contains(&d) {
                        let (y, cur_m, _) = ymd(today);
                        let year = if m < cur_m { y + 1 } else { y };
                        if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, m, d) {
                            if date >= today {
                                return Some(local_ms(date.and_hms_opt(0, 0, 0).unwrap()));
                            }
                        }
                    }
                }
            }
        }
    }

    // "el 15" / "el día 15" → próximo día 15 (o mes siguiente si ya pasó)
    let re_day = regex::Regex::new(r"el (?:día |dia )?(\d{1,2})").ok()?;
    if let Some(caps) = re_day.captures(&lower) {
        if let Ok(d) = caps.get(1)?.as_str().parse::<u32>() {
            let (y, m, _) = ymd(today);
            let target = chrono::NaiveDate::from_ymd_opt(y, m, d)?;
            let candidate = if target >= today {
                target
            } else {
                let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                chrono::NaiveDate::from_ymd_opt(ny, nm, d)?
            };
            return Some(local_ms(candidate.and_hms_opt(0, 0, 0).unwrap()));
        }
    }

    // día de la semana
    for t in lower.split_whitespace() {
        if let Some(wd) = weekday_from_name(t) {
            let delta = weekday_delta(wd);
            return Some(local_ms((today + chrono::Duration::days(delta)).and_hms_opt(0, 0, 0).unwrap()));
        }
    }
    None
}

/// Día mencionado SOLO de forma relativa o por nombre de día de la semana
/// ("hoy", "mañana", "el viernes", "próximo lunes"). Devuelve la medianoche
/// LOCAL en ms. Devuelve None ante rangos ("del lunes al viernes", "del 5 al
/// 23"), fechas absolutas o varias menciones de día: ahí manda la IA.
pub(crate) fn relative_day_ms(text: &str) -> Option<i64> {
    let now = chrono::Local::now();
    let today = now.date_naive();
    let lower = text.to_lowercase();

    // Rango de fechas → no hay un único día que corregir.
    let range = regex::Regex::new(r"\b(del|desde|entre)\b.{0,60}\b(al|hasta)\b").ok()?;
    if range.is_match(&lower) {
        return None;
    }

    // "mañana" como fecha, no como parte del día ("de la mañana", "por la mañana").
    let manana_es_manana = lower.contains("de la mañana")
        || lower.contains("por la mañana")
        || lower.contains("en la mañana")
        || lower.contains("de la manana")
        || lower.contains("por la manana")
        || lower.contains("en la manana");

    // Rango con verbos de inicio/fin ("inicia hoy y finaliza el lunes…"): hay
    // DOS días distintos; corregir a uno solo destruiría el rango.
    let start_verb = ["inicia", "empieza", "comienza", "starts", "start"]
        .iter()
        .any(|v| lower.contains(v));
    let end_verb = ["finaliza", "termina", "acaba", "vence", "ends", "finishes", "finish"]
        .iter()
        .any(|v| lower.contains(v));
    if start_verb && end_verb {
        return None;
    }

    // Día relativo Y nombre de día de la semana a la vez ("hoy tengo clase y
    // el viernes examen"): hay varios días, manda la IA.
    let mentions_weekday = lower.split(|c: char| !c.is_alphabetic()).any(|tok| {
        matches!(
            tok,
            "lunes" | "martes" | "miércoles" | "miercoles" | "jueves" | "viernes"
                | "sábado" | "sabado" | "domingo" | "monday" | "tuesday" | "wednesday"
                | "thursday" | "friday" | "saturday" | "sunday"
        )
    });
    let mentions_relative = lower.contains("hoy")
        || lower.contains("today")
        || lower.contains("pasado mañana")
        || lower.contains("pasado manana")
        || lower.contains("day after tomorrow")
        || (!manana_es_manana && (lower.contains("mañana") || lower.contains("manana") || lower.contains("tomorrow")));
    if mentions_relative && mentions_weekday {
        return None;
    }

    if lower.contains("pasado mañana") || lower.contains("pasado manana") || lower.contains("day after tomorrow") {
        return Some(local_ms((today + chrono::Duration::days(2)).and_hms_opt(0, 0, 0).unwrap()));
    }
    if !manana_es_manana && (lower.contains("mañana") || lower.contains("manana") || lower.contains("tomorrow")) {
        return Some(local_ms((today + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap()));
    }
    if lower.contains("hoy") || lower.contains("today") {
        return Some(local_ms(today.and_hms_opt(0, 0, 0).unwrap()));
    }

    // Un único nombre de día de la semana (nombre COMPLETO: "marzo" no es
    // "martes"); si hay dos ("de lunes a viernes") es un rango y no se toca.
    let mut found: Option<chrono::Weekday> = None;
    for tok in lower.split(|c: char| !c.is_alphabetic()) {
        let wd = match tok {
            "lunes" | "monday" => Some(chrono::Weekday::Mon),
            "martes" | "tuesday" => Some(chrono::Weekday::Tue),
            "miércoles" | "miercoles" | "wednesday" => Some(chrono::Weekday::Wed),
            "jueves" | "thursday" => Some(chrono::Weekday::Thu),
            "viernes" | "friday" => Some(chrono::Weekday::Fri),
            "sábado" | "sabado" | "saturday" => Some(chrono::Weekday::Sat),
            "domingo" | "sunday" => Some(chrono::Weekday::Sun),
            _ => None,
        };
        if let Some(wd) = wd {
            if found.is_some() {
                return None;
            }
            found = Some(wd);
        }
    }
    let wd = found?;
    let delta = weekday_delta(wd);
    Some(local_ms((today + chrono::Duration::days(delta)).and_hms_opt(0, 0, 0).unwrap()))
}

/// Medianoche LOCAL (ms) del día al que pertenece un timestamp.
pub(crate) fn day_start_ms(ms: i64) -> Option<i64> {
    let dt = chrono::DateTime::from_timestamp_millis(ms)?.with_timezone(&chrono::Local);
    Some(local_ms(dt.date_naive().and_hms_opt(0, 0, 0)?))
}

/// Duración desde texto: "2 horas", "durante 3h", "media hora", "for two hours",
/// "1h30m", "half an hour". Devuelve minutos. Sin duración → None.
pub(crate) fn duration_from_text(text: &str) -> Option<i64> {
    let lower = text.to_lowercase();

    let words = [
        ("una", 60), ("media", 30), ("dos", 120), ("tres", 180), ("cuatro", 240),
        ("cinco", 300), ("seis", 360), ("siete", 420), ("ocho", 480), ("nueve", 540),
        ("diez", 600), ("an", 60), ("a", 60), ("one", 60), ("half", 30), ("two", 120),
        ("three", 180), ("four", 240), ("five", 300), ("six", 360), ("seven", 420),
        ("eight", 480), ("nine", 540), ("ten", 600),
    ];

    // "Xh Ym" combinado ("1h30m", "1 hora 30 minutos")
    let re_comb = regex::Regex::new(r"(\d+)\s*h(?:ora)?s?\s*(\d+)\s*m(?:inuto)?s?").ok()?;
    if let Some(caps) = re_comb.captures(&lower) {
        let h: i64 = caps.get(1)?.as_str().parse().ok()?;
        let m: i64 = caps.get(2)?.as_str().parse().ok()?;
        return Some(h * 60 + m);
    }

    if lower.contains("half an hour") {
        return Some(30);
    }

    // "media hora" / "half an hour" / "una hora"
    for (w, mins) in words {
        if lower.contains(&format!("{w} hora")) || lower.contains(&format!("{w} hour")) {
            return Some(mins);
        }
    }

    // "2 horas" / "2h" / "3 hours" / "for two hours"
    let re = regex::Regex::new(r"(?:for |durante |por )?(\d+(?:\.\d+)?)\s*(?:horas?|hrs?|hours?|h)\b").ok()?;
    if let Some(caps) = re.captures(&lower) {
        let v: f64 = caps.get(1)?.as_str().parse().ok()?;
        return Some((v * 60.0).round() as i64);
    }

    // palabras de duración en inglés ("two hours") sin dígito
    let re_en = regex::Regex::new(r"\b(?:one|two|three|four|five|six|seven|eight|nine|ten|half|an?)\s+hours?\b").ok()?;
    if let Some(caps) = re_en.captures(&lower) {
        let n = caps.get(1)?.as_str();
        for (w, mins) in words {
            if n == w {
                return Some(mins);
            }
        }
    }
    None
}

pub(crate) fn detect_category(lower: &str) -> &'static str {
    if lower.contains("pagar") || lower.contains("factura") || lower.contains("pay") || lower.contains("bill") {
        "fin"
    } else if lower.contains("examen") || lower.contains("estudiar") || lower.contains("entregar") || lower.contains("proyecto")
        || lower.contains("exam") || lower.contains("study") || lower.contains("submit") || lower.contains("assignment") || lower.contains("homework") {
        "uni"
    } else if lower.contains("médico") || lower.contains("medico") || lower.contains("gimnasio") || lower.contains("doctor") || lower.contains("gym") {
        "sal"
    } else if lower.contains("cita") || lower.contains("appointment") {
        "per"
    } else if lower.contains("reunión") || lower.contains("reunion") || lower.contains("meeting") {
        "trab"
    } else {
        "otr"
    }
}

/// Módulo 1 heurístico (sin IA): texto → ParsedTask.
pub fn parse_task_nl(text: &str) -> Option<ParsedTask> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();

    let day_ms = parse_day(&lower).unwrap_or_else(|| {
        // por defecto mañana
        let today = chrono::Local::now().date_naive();
        local_ms((today + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap())
    });

    // Horario: rango explícito ("de 3pm a 5pm") manda; si solo hay hora de
    // inicio y duración explícita ("a las 10 durante 3 horas") → fin = inicio +
    // duración; solo hora de inicio → 1 hora. Sin hora → Todo el día (nunca se
    // inventa una hora de inicio).
    let explicit_range = time_range(&lower);
    let dur_min = duration_from_text(&lower);
    let (start_min, end_min, all_day) = match explicit_range {
        Some((s, e)) => (s, e, false),
        None => match (hour_from_text(&lower), dur_min) {
            (Some(s), Some(d)) => (s, s + d as u32, false),
            (Some(s), None) => (s, s + 60, false),
            (None, _) => (0, 0, true),
        },
    };

    // day_ms ya es la medianoche LOCAL del día objetivo (parse_day usa local_ms):
    // NO se vuelve a alinear con % DAY — en zonas UTC±n la medianoche local no
    // es múltiplo de 86.400.000 ms y el floor desplazaba todas las horas (5 h
    // con UTC-5: "a las 4 PM" caía a las 11 AM).
    let start_ms = day_ms + start_min as i64 * 60_000;
    let end_ms = day_ms + end_min as i64 * 60_000;

    let priority = if lower.contains("urgente") || lower.contains("urgent") { "alta" } else { "media" };
    let category_id = detect_category(&lower).to_string();

    // "Recordarme X el [fecha]" (FR-08): crea tarea + recordatorio (1 hora antes).
    let mut reminders: Vec<String> = Vec::new();
    if lower.contains("record") || lower.contains("recuerd") {
        reminders.push("60m".into());
    }

    let title = build_title(trimmed);

    Some(ParsedTask {
        title,
        description: String::new(),
        category_id,
        priority: priority.to_string(),
        start_ms,
        end_ms,
        all_day,
        location: String::new(),
        tags: Vec::new(),
        reminders,
    })
}

/// Título limpio: quita prefijos de comando y fragmentos de fecha/hora.
pub(crate) fn build_title(text: &str) -> String {
    let mut t = text.trim().to_string();
    let lower = t.to_lowercase();
    for prefix in [
        "recordarme ",
        "recuérdame ",
        "acordarme de ",
        "tengo ",
        "tener ",
        "hay que ",
        "hacer ",
    ] {
        if lower.starts_with(prefix) {
            t = t[prefix.len()..].trim().to_string();
            break;
        }
    }

    // quitar fragmentos de fecha/hora
    let patterns = [
        r"(?i)\bpasado mañana\b",
        r"(?i)\bpasado manana\b",
        r"(?i)\bday after tomorrow\b",
        r"(?i)\bpróximo sábado\b",
        r"(?i)\bpróximo domingo\b",
        r"(?i)\bproximo sabado\b",
        r"(?i)\bproximo domingo\b",
        r"(?i)\bpróxim[oa] (lunes|martes|miércoles|jueves|viernes)\b",
        r"(?i)\bproxim[oa] (lunes|martes|miercoles|jueves|viernes)\b",
        r"(?i)\bnext\s+(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tue|wed|thu|fri|sat|sun)\b",
        r"(?i)\b(el|los|la|las)? ?(lunes|martes|miércoles|jueves|viernes|sábado|domingo)\b",
        r"(?i)\b(el|los|la|las)? ?(lunes|martes|miercoles|jueves|viernes|sabado|domingo)\b",
        r"(?i)\b(monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
        r"(?i)\b(mon|tue|wed|thu|fri|sat|sun)\b",
        r"(?i)\bel día \d{1,2}\b",
        r"(?i)\bel \d{1,2}\b",
        r"(?i)\bde (enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre|january|february|march|april|june|july|august|september|october|november|december)\b",
        r"(?i)\ba (las|la) \d{1,2}(?::\d{2})?\s*(am|pm|a\.m\.|p\.m\.)?\b",
        r"(?i)\bat \d{1,2}(?::\d{2})?\s*(am|pm)?\b",
        r"(?i)\b(am|pm|a\.m\.|p\.m\.)\b",
        r"(?i)\bde \d{1,2}(?::\d{2})?\s*(am|pm|a\.m\.|p\.m\.)?\s*(a|hasta)\s+\d{1,2}(?::\d{2})?\s*(am|pm|a\.m\.|p\.m\.)?\b",
        r"(?i)\b(mañana|manana|hoy|tomorrow|today)\b",
        r"(?i)\b(de las|alrededor de las|a la)\b",
        r"(?i)\bfor (?:an?|one|two|three|four|five|six|seven|eight|nine|ten|half|\d+(?:\.\d+)?)\s+hours?\b",
        r"(?i)\b(?:durante|por) (?:una|media|dos|tres|cuatro|cinco|seis|siete|ocho|nueve|diez|\d+(?:\.\d+)?)\s+horas?\b",
        r"(?i)\b\d+(?:\.\d+)?\s*(?:horas?|hours?|hrs?)\b",
    ];
    for p in patterns {
        if let Ok(re) = regex::Regex::new(p) {
            t = re.replace_all(&t, "").trim().to_string();
        }
    }

    // limpiar espacios y signos sobrantes
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_matches(|c| c == ' ' || c == ',' || c == ':' || c == '-' || c == '—').to_string();

    let mut chars = t.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Tarea".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    fn parse(s: &str) -> ParsedTask {
        parse_task_nl(s).expect("parseable")
    }

    fn midnight(days: i64) -> i64 {
        let t = chrono::Local::now().date_naive() + chrono::Duration::days(days);
        local_ms(t.and_hms_opt(0, 0, 0).unwrap())
    }

    const HOUR: i64 = 3_600_000;

    #[test]
    fn en_examples_land_on_correct_date_and_time() {
        // "Tomorrow at 4 PM study calculus"
        let t = parse("Tomorrow at 4 PM study calculus");
        assert_eq!(t.start_ms, midnight(1) + 16 * HOUR);
        assert_eq!(t.end_ms, midnight(1) + 17 * HOUR);
        assert!(!t.all_day);
        assert_eq!(t.title, "Study calculus");
        assert_eq!(t.category_id, "uni");

        // "Exam Friday at 8 AM"
        let t = parse("Exam Friday at 8 AM");
        let fri = midnight(weekday_delta(chrono::Weekday::Fri) as i64);
        assert_eq!(t.start_ms, fri + 8 * HOUR);
        assert_eq!(t.end_ms, fri + 9 * HOUR);
        assert_eq!(t.title, "Exam");
        assert_eq!(t.category_id, "uni");

        // "Submit project next Monday"
        let t = parse("Submit project next Monday");
        let mon = midnight(weekday_delta(chrono::Weekday::Mon) as i64);
        assert_eq!(t.start_ms, mon);
        assert!(t.all_day);
        assert_eq!(t.title, "Submit project");
        assert_eq!(t.category_id, "uni");

        // "Study calculus for two hours Thursday"
        let t = parse("Study calculus for two hours Thursday");
        let thu = midnight(weekday_delta(chrono::Weekday::Thu) as i64);
        assert_eq!(t.start_ms, thu);
        assert!(t.all_day, "duración sin hora de inicio → Todo el día (nunca inventar hora)");
        assert_eq!(t.title, "Study calculus");
    }

    #[test]
    fn duration_extends_end_time_only_with_start() {
        // ES: "a las 10 durante 3 horas" → 10:00–13:00
        let t = parse("reunión mañana a las 10 durante 3 horas");
        assert_eq!(t.start_ms, midnight(1) + 10 * HOUR);
        assert_eq!(t.end_ms, midnight(1) + 13 * HOUR);
        assert_eq!(t.title, "Reunión");
        assert_eq!(t.category_id, "trab");

        // EN: "at 10 for 2 hours" → 10:00–12:00
        let t = parse("meeting tomorrow at 10 for 2 hours");
        assert_eq!(t.start_ms, midnight(1) + 10 * HOUR);
        assert_eq!(t.end_ms, midnight(1) + 12 * HOUR);
        assert_eq!(t.title, "Meeting");

        // rango explícito manda sobre la duración
        let t = parse("estudiar cálculo de 3pm a 5pm");
        assert_eq!(t.start_ms, midnight(1) + 15 * HOUR);
        assert_eq!(t.end_ms, midnight(1) + 17 * HOUR);
        assert_eq!(t.title, "Estudiar cálculo");

        // sin hora de inicio → duración no inventa hora
        let t = parse("estudiar cálculo durante 2 horas el viernes");
        assert!(t.all_day);
        assert_eq!(t.title, "Estudiar cálculo");
    }

    #[test]
    fn recordarme_adds_reminder() {
        let t = parse("recordarme pagar internet el viernes");
        assert_eq!(t.reminders, vec!["60m"]);
        assert_eq!(t.title, "Pagar internet");
        assert_eq!(t.category_id, "fin");
    }

    #[test]
    fn absolute_date_with_month() {
        let t = parse("el 15 de agosto a las 9 presentar informe");
        let today = chrono::Local::now().date_naive();
        let (y, m, _) = ymd(today);
        // misma regla del parser: año actual o siguiente si el mes ya pasó…
        let year = if 8 < m { y + 1 } else { y };
        let date = chrono::NaiveDate::from_ymd_opt(year, 8, 15).unwrap();
        // …y si el día ya pasó este mes, cae al "próximo día 15"
        let expected = if date >= today {
            date
        } else {
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            chrono::NaiveDate::from_ymd_opt(ny, nm, 15).unwrap()
        };
        let day_ms = local_ms(expected.and_hms_opt(0, 0, 0).unwrap());
        assert_eq!(t.start_ms, day_ms + 9 * HOUR);
        assert_eq!(t.end_ms, day_ms + 10 * HOUR);
        assert_eq!(t.title, "Presentar informe");
    }

    #[test]
    fn es_keywords_unaffected() {
        let t = parse("estudiar cálculo mañana de 3pm a 5pm");
        assert_eq!(t.start_ms, midnight(1) + 15 * HOUR);
        assert_eq!(t.end_ms, midnight(1) + 17 * HOUR);
        assert_eq!(t.title, "Estudiar cálculo");
        assert_eq!(t.category_id, "uni");

        let t = parse("cita médico el próximo lunes");
        let mon = midnight(weekday_delta(chrono::Weekday::Mon) as i64);
        assert_eq!(t.start_ms, mon);
        assert!(t.all_day);
        assert_eq!(t.category_id, "sal");
    }

    #[test]
    fn relative_day_ms_rules() {
        let fri = midnight(weekday_delta(chrono::Weekday::Fri) as i64);
        assert_eq!(relative_day_ms("reunión de programación competitiva el viernes"), Some(fri));
        assert_eq!(
            relative_day_ms("quiz el martes a las 6pm"),
            Some(midnight(weekday_delta(chrono::Weekday::Tue) as i64))
        );
        assert_eq!(relative_day_ms("pagar internet mañana"), Some(midnight(1)));
        assert_eq!(relative_day_ms("cita pasado mañana"), Some(midnight(2)));
        assert_eq!(relative_day_ms("llamar hoy"), Some(midnight(0)));
        // "de la mañana" no es la fecha "mañana"
        assert_eq!(relative_day_ms("clase el viernes a las 10 de la mañana"), Some(fri));
        // rangos y fechas absolutas: sin corrección
        assert_eq!(relative_day_ms("disponible del 5 al 23 de agosto"), None);
        assert_eq!(relative_day_ms("de lunes a viernes estudiar"), None);
        assert_eq!(relative_day_ms("el 15 de agosto presentar informe"), None);
        // sin día → None
        assert_eq!(relative_day_ms("comprar leche"), None);
        // rango inicia/finaliza: dos días distintos, corregir a uno rompe el rango
        assert_eq!(
            relative_day_ms("proyecto de programacion, inicia hoy y finaliza el lunes del siguiente mes a las 4pm"),
            None
        );
        assert_eq!(relative_day_ms("empieza mañana y termina el viernes"), None);
        // día relativo + día de la semana: varios días, manda la IA
        assert_eq!(relative_day_ms("hoy tengo clase y el viernes examen"), None);
        // verbo de fin sin verbo de inicio: un solo día, se corrige igual
        assert_eq!(relative_day_ms("la clase termina a las 6pm el viernes"), Some(fri));
    }

    #[test]
    fn duration_variants() {
        assert_eq!(duration_from_text("durante 3 horas"), Some(180));
        assert_eq!(duration_from_text("por 2 horas"), Some(120));
        assert_eq!(duration_from_text("for two hours"), Some(120));
        assert_eq!(duration_from_text("for an hour"), Some(60));
        assert_eq!(duration_from_text("half an hour"), Some(30));
        assert_eq!(duration_from_text("media hora"), Some(30));
        assert_eq!(duration_from_text("1h30m"), Some(90));
        assert_eq!(duration_from_text("1.5 horas"), Some(90));
        assert_eq!(duration_from_text("sin duración aquí"), None);
    }
}
