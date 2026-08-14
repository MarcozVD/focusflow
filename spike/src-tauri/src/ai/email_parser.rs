use super::validation::{self, EmailParseResult, EMAIL_SCHEMA};
use super::{AiProvider, AiResult};

const EMAIL_SYSTEM_PROMPT: &str = r#"Eres el asistente de calendario de FocusFlow.
Analizas correos electrónicos del usuario y detectas información de calendario:
tareas, reuniones, exámenes, entregas, fechas límite, actividades, eventos, recordatorios y cambios de horario.
Hoy es {fecha}.

Reglas:
- is_relevant: true solo si el correo contiene información de calendario concreta.
- Extrae fecha y hora exactas (si el correo dice "el viernes" o "next week", calcula la fecha concreta).
- Si el correo NO indica hora → dejar start_time y end_time vacíos: el evento será de Todo el día. NUNCA inventar una hora (nada de 09:00 por defecto).
- Si solo hay hora de inicio → duración de 1 hora.
- category: Universidad, Trabajo, Personal, Finanzas, Salud u Otros.
- confidence: 0.0-1.0 según claridad de la información.
- events: vacío si no es relevante.
- Los cambios de horario (posponer, cambiar día/hora) también son eventos relevantes."#;

pub fn email_system_prompt() -> String {
    let now = chrono::Local::now();
    EMAIL_SYSTEM_PROMPT.replace("{fecha}", &now.format("%Y-%m-%d").to_string())
}

pub fn email_user_prompt(subject: &str, sender: &str, date: &str, body: &str) -> String {
    format!(
        "CORREO\nDe: {sender}\nFecha: {date}\nAsunto: {subject}\n\nCuerpo:\n{body}"
    )
}

/// Envía el correo a la IA y valida el resultado.
pub fn parse_email(
    subject: &str,
    sender: &str,
    date: &str,
    body: &str,
    provider: &dyn AiProvider,
) -> AiResult<EmailParseResult> {
    let user = email_user_prompt(subject, sender, date, body);
    let v = provider.chat_json(&email_system_prompt(), &user, EMAIL_SCHEMA)?;
    validation::validate_email_json(&v)
}

/// Extrae texto plano de un cuerpo HTML crudo (strip de tags).
///
/// - Elimina scripts/styles (ruido visual).
/// - Convierte `<a href="URL">texto</a>` → `texto (URL)` conservando los
///   enlaces relevantes; solo http/https, con límite de longitud por URL.
/// - Colapsa el resto de tags y espacios.
pub fn html_to_text(html: &str) -> String {
    let re_scripts = regex::Regex::new(r"<(script|style)[^>]*>[\s\S]*?</(script|style)>").unwrap();
    let without_scripts = re_scripts.replace_all(html, " ").to_string();

    // conservar hrefs de anchors antes del strip de tags
    let re_anchor = regex::Regex::new(
        r#"<a\b[^>]*href\s*=\s*["']([^"']+)["'][^>]*>([\s\S]*?)</a>"#,
    )
    .unwrap();
    let re_tag = regex::Regex::new(r"<[^>]+>").unwrap();
    let mut anchored = String::with_capacity(without_scripts.len());
    let mut last = 0;
    for cap in re_anchor.captures_iter(&without_scripts) {
        let m = cap.get(0).unwrap();
        anchored.push_str(&without_scripts[last..m.start()]);
        let href = cap.get(1).unwrap().as_str().trim();
        let inner = re_tag.replace_all(cap.get(2).unwrap().as_str(), " ").to_string();
        let inner = inner.split_whitespace().collect::<Vec<_>>().join(" ");
        let safe_href: String = if (href.starts_with("http://") || href.starts_with("https://"))
            && !href.contains(char::is_whitespace)
        {
            let truncated: String = href.chars().take(120).collect();
            if truncated.len() < href.chars().count() {
                format!("{truncated}…")
            } else {
                truncated
            }
        } else {
            String::new()
        };
        if inner.is_empty() {
            if !safe_href.is_empty() {
                anchored.push_str(&format!("({safe_href}) "));
            }
        } else if safe_href.is_empty() {
            anchored.push_str(&format!("{inner} "));
        } else {
            anchored.push_str(&format!("{inner} ({safe_href}) "));
        }
        last = m.end();
    }
    anchored.push_str(&without_scripts[last..]);

    let text = re_tag.replace_all(&anchored, " ").to_string();
    let re_space = regex::Regex::new(r"\s+").unwrap();
    re_space.replace_all(&text, " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_and_keeps_visible_text() {
        let html = "<html><head><style>.x{color:red}</style></head><body><script>var a=1;</script><p>Entrega <b>el lunes</b></p></body></html>";
        let t = html_to_text(html);
        assert!(t.contains("Entrega el lunes"), "{t}");
        assert!(!t.contains("color:red") && !t.contains("var a"), "{t}");
    }

    #[test]
    fn preserves_http_links() {
        let html = r#"<p>Instructivo: <a href="https://unab.edu.co/guia.pdf">guía de la materia</a>.</p>"#;
        let t = html_to_text(html);
        assert!(t.contains("guía de la materia (https://unab.edu.co/guia.pdf)"), "{t}");
    }

    #[test]
    fn drops_javascript_and_long_hrefs() {
        let long = format!("https://x.com/{}", "a".repeat(300));
        let html = format!(
            r#"<p>a <a href="javascript:alert(1)">malo</a> b <a href="{long}">largo</a></p>"#
        );
        let t = html_to_text(&html);
        assert!(!t.contains("javascript"), "{t}");
        assert!(!t.contains("malo (javascript"), "{t}");
        assert!(t.contains("largo (https://x.com/") && t.contains("…"), "{t}");
        assert!(t.len() < long.len() + 200, "url truncada: {}", t.len());
    }
}
