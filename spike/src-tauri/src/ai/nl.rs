use super::validation::ParsedTask;

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

fn weekday_num(date: chrono::NaiveDate) -> u8 {
    date.format("%u").to_string().parse().unwrap_or(0)
}

fn ymd(date: chrono::NaiveDate) -> (i32, u32, u32) {
    let parts: Vec<u32> = date.format("%Y-%m-%d").to_string().split('-').map(|s| s.parse().unwrap()).collect();
    (parts[0] as i32, parts[1], parts[2])
}

fn weekday_delta(target: chrono::Weekday) -> i64 {
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

fn weekday_from_name(s: &str) -> Option<chrono::Weekday> {
    let n = s.trim().to_lowercase();
    if n.starts_with("dom") {
        Some(chrono::Weekday::Sun)
    } else if n.starts_with("lun") {
        Some(chrono::Weekday::Mon)
    } else if n.starts_with("mar") {
        Some(chrono::Weekday::Tue)
    } else if n.starts_with("mié") || n.starts_with("mie") {
        Some(chrono::Weekday::Wed)
    } else if n.starts_with("jue") {
        Some(chrono::Weekday::Thu)
    } else if n.starts_with("vie") {
        Some(chrono::Weekday::Fri)
    } else if n.starts_with("sáb") || n.starts_with("sab") {
        Some(chrono::Weekday::Sat)
    } else {
        None
    }
}

/// Hora desde texto: "3 pm", "15:00", "a las 10 de la mañana", "8 AM".
fn hour_from_text(s: &str) -> Option<u32> {
    let lower = s.trim().to_lowercase();
    let m = regex_extract(&lower, r"(\d{1,2})(?:\s*(:|\.)?\s*(\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)").or_else(|| {
        regex_extract(&lower, r"(\d{1,2})(?::(\d{2}))?")
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

fn regex_extract<'a>(s: &'a str, pat: &str) -> Option<(&'a str, Option<&'a str>)> {
    let re = regex::Regex::new(pat).ok()?;
    let caps = re.captures(s)?;
    let g1 = caps.get(1)?.as_str();
    let g2 = caps.get(2).map(|m| m.as_str());
    Some((g1, g2))
}

/// Parsea "de 3 pm a 6 pm" o "de 3 a 5" y devuelve (hora_inicio_min, hora_fin_min).
fn time_range(text: &str) -> Option<(u32, u32)> {
    let re = regex::Regex::new(r"de (\d{1,2}(?::\d{2})?(?:\s*(?:am|pm|a\.m\.|p\.m\.))?)\s*(?:a|hasta)\s+(\d{1,2}(?::\d{2})?(?:\s*(?:am|pm|a\.m\.|p\.m\.))?)").ok()?;
    let caps = re.captures(text)?;
    let start = hour_from_text(caps.get(1)?.as_str())?;
    let mut end = hour_from_text(caps.get(2)?.as_str())?;
    if end <= start {
        end += 12 * 60;
    }
    Some((start, end))
}

fn parse_hora(text: &str) -> Option<(u32, u32)> {
    if let Some((s, e)) = time_range(text) {
        return Some((s, e));
    }
    let re = regex::Regex::new(r"(?:a las|a la|alrededor de las)\s+(\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.)?)").ok()?;
    if let Some(caps) = re.captures(text) {
        if let Some(h) = hour_from_text(caps.get(1)?.as_str()) {
            return Some((h, h + 60));
        }
    }
    let re2 = regex::Regex::new(r"\b(\d{1,2}(?::\d{2})?\s*(?:am|pm|a\.m\.|p\.m\.))\b").ok()?;
    if let Some(caps) = re2.captures(text) {
        if let Some(h) = hour_from_text(caps.get(1)?.as_str()) {
            return Some((h, h + 60));
        }
    }
    None
}

/// Calcula el día de inicio en ms.
fn parse_day(text: &str) -> Option<i64> {
    let now = chrono::Local::now();
    let today = now.date_naive();
    let lower = text.to_lowercase();

    if lower.contains("pasado mañana") || lower.contains("pasado manana") {
        return Some((today + chrono::Duration::days(2)).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
    }
    if lower.contains("mañana") || lower.contains("manana") {
        return Some((today + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
    }
    if lower.contains("hoy") {
        return Some(today.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
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
            return Some(candidate.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
        }
    }

    // día de la semana
    for t in lower.split_whitespace() {
        if let Some(wd) = weekday_from_name(t) {
            let delta = weekday_delta(wd);
            return Some((today + chrono::Duration::days(delta)).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis());
        }
    }
    None
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
        (today + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis()
    });

    let (start_min, end_min, all_day) = match parse_hora(&lower) {
        Some((s, e)) => (s, e, false),
        None => (0, 0, true),
    };

    let day_start = day_ms - (day_ms % DAY);
    let start_ms = day_start + start_min as i64 * 60_000;
    let end_ms = day_start + end_min as i64 * 60_000;

    let priority = if lower.contains("urgente") { "alta" } else { "media" };
    let category_id = if lower.contains("pagar") || lower.contains("factura") {
        "fin"
    } else if lower.contains("examen") || lower.contains("estudiar") || lower.contains("entregar") || lower.contains("proyecto") {
        "uni"
    } else if lower.contains("médico") || lower.contains("medico") || lower.contains("gimnasio") {
        "sal"
    } else if lower.contains("cita") {
        "per"
    } else if lower.contains("reunión") || lower.contains("reunion") {
        "trab"
    } else {
        "otr"
    };
    let category_id = category_id.to_string();

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
        reminders: Vec::new(),
    })
}

/// Título limpio: quita prefijos de comando y fragmentos de fecha/hora.
fn build_title(text: &str) -> String {
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
        r"(?i)\bpróximo sábado\b",
        r"(?i)\bpróximo domingo\b",
        r"(?i)\bproximo sabado\b",
        r"(?i)\bproximo domingo\b",
        r"(?i)\bpróxim[oa] (lunes|martes|miércoles|jueves|viernes)\b",
        r"(?i)\bproxim[oa] (lunes|martes|miercoles|jueves|viernes)\b",
        r"(?i)\b(el|los|la|las)? ?(lunes|martes|miércoles|jueves|viernes|sábado|domingo)\b",
        r"(?i)\b(el|los|la|las)? ?(lunes|martes|miercoles|jueves|viernes|sabado|domingo)\b",
        r"(?i)\bel día \d{1,2}\b",
        r"(?i)\bel \d{1,2}\b",
        r"(?i)\ba (las|la) \d{1,2}(?::\d{2})?\s*(am|pm|a\.m\.|p\.m\.)?\b",
        r"(?i)\b(am|pm|a\.m\.|p\.m\.)\b",
        r"(?i)\bde \d{1,2}(?::\d{2})?\s*(am|pm|a\.m\.|p\.m\.)?\s*(a|hasta)\s+\d{1,2}(?::\d{2})?\s*(am|pm|a\.m\.|p\.m\.)?\b",
        r"(?i)\b(mañana|manana|hoy)\b",
        r"(?i)\b(de las|alrededor de las|a la)\b",
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
