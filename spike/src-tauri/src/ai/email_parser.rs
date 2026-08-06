use super::validation::{self, EmailParseResult, EMAIL_SCHEMA};
use super::{AiProvider, AiResult};

const EMAIL_SYSTEM_PROMPT: &str = r#"Eres el asistente de calendario de FocusFlow.
Analizas correos electrónicos del usuario y detectas información de calendario:
tareas, reuniones, exámenes, entregas, fechas límite, actividades, eventos, recordatorios y cambios de horario.
Hoy es {fecha}.

Reglas:
- is_relevant: true solo si el correo contiene información de calendario concreta.
- Extrae fecha y hora exactas (si el correo dice "el viernes" o "next week", calcula la fecha concreta).
- Si no hay hora → 09:00. Duración por defecto 1 hora.
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
pub fn html_to_text(html: &str) -> String {
    let re_tags = regex::Regex::new(r"<(script|style)[^>]*>[\s\S]*?</(script|style)>").unwrap();
    let without_scripts = re_tags.replace_all(html, " ").to_string();
    let re_tag = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tag.replace_all(&without_scripts, " ").to_string();
    let re_space = regex::Regex::new(r"\s+").unwrap();
    re_space.replace_all(&text, " ").trim().to_string()
}
