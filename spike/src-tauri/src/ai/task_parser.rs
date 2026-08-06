use super::validation::{self, ParsedTask, TASK_SCHEMA};
use super::{AiConfig, AiError, AiProvider, AiResult};

const TASK_SYSTEM_PROMPT: &str = r#"Eres el asistente de planificación personal de FocusFlow.
Interpretas frases en español en lenguaje natural y las conviertes en datos estructurados de tareas.
Hoy es {fecha} y son las {hora} (hora local del usuario).

Reglas:
- Extrae la fecha exacta (hoy, mañana, próximo viernes, el día 15, el 20 de junio…).
- Convierte horas de formato español a formato 24h (3 PM → 15:00, 10 de la mañana → 10:00).
- El título NO debe contener la fecha ni la hora.
- category siempre uno de: Universidad, Trabajo, Personal, Finanzas, Salud, Otros.
- priority: Alta solo si es urgente, examen o entrega cercana.
- Si no hay fecha → mañana. Si hay hora → tarea con hora. Si NO hay hora → NO inventar hora: dejar start_time y end_time vacíos (la tarea será de Todo el día).
- Duración: de la hora de inicio a la hora final si se indica; si solo hay hora de inicio, 1 hora.
- reminders: formato corto "1d", "1h", "30m", "1w". Vacío si no se pide recordatorio."#;

pub fn system_prompt_now() -> String {
    let now = chrono::Local::now();
    TASK_SYSTEM_PROMPT
        .replace("{fecha}", &now.format("%Y-%m-%d").to_string())
        .replace("{hora}", &now.format("%H:%M").to_string())
}

/// Módulo 1: texto natural → tarea validada. Usa IA si está configurada,
/// si no cae a la heurística local (nl).
pub fn parse_task_text(
    text: &str,
    provider: &dyn AiProvider,
    configured: bool,
) -> AiResult<(ParsedTask, String)> {
    if configured {
        match provider.chat_json(&system_prompt_now(), text, TASK_SCHEMA) {
            Ok(v) => match validation::validate_task_json(&v) {
                Ok(t) => return Ok((t, "ai".into())),
                Err(e) => {
                    // JSON inválido → no confiar, caer a heurística
                    return Err(e);
                }
            },
            Err(e) => return Err(e),
        }
    }
    super::nl::parse_task_nl(text)
        .map(|t| (t, "local".into()))
        .ok_or_else(|| AiError::InvalidJson("no se pudo interpretar el texto".into()))
}

/// ¿La configuración permite llamar a la IA?
pub fn provider_configured(cfg: &AiConfig) -> bool {
    !cfg.endpoint.is_empty()
        || !super::default_endpoint().is_empty() && !cfg.model.is_empty()
        || !super::default_model().is_empty() && super::get_ai_key().is_some()
}
