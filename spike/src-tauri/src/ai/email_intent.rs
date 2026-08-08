//! Inteligencia de correo (fase 8): correo → compromisos → sugerencias.
//!
//! Reutiliza el pipeline de intenciones ([super::intent_parser::parse_batch_json])
//! con un prompt específico de correo. Antes de llamar a la IA, el cuerpo se
//! minimiza ([`minimize_email`]) para respetar la privacidad: se eliminan las
//! citas/respondidos y se trunca el contenido.

use super::intent::{IntentBatch, IntentType};
use super::intent_parser::{parse_batch_json, INTENT_SCHEMA};
use super::{AiError, AiProvider, AiResult};
use crate::email::RawEmail;

/// Limites de la minimización: solo se manda a la IA una ventana del cuerpo.
const MAX_BODY_CHARS: usize = 900;

/// Prompt de sistema para correos. Igual esquema que el texto libre, con
/// reglas específicas de correo: compromisos, no a la información
/// sensible, ignorar fórmulas de cortesía y firmas.
pub const EMAIL_SYSTEM_PROMPT: &str = r#"Eres el analizador de compromisos de FocusFlow. Extraes compromisos de correos electrónicos.
Convierte CADA correo en una lista de intenciones estructuradas en JSON.

REGLAS:
1. Devuelve EXCLUSIVAMENTE un JSON con la forma del esquema dado. No texto fuera del JSON.
2. Tipos de intención:
   - event: actividad con fecha/hora concreta ("nos vemos el viernes a las 10", "reunión el 14").
   - deadline: entrega/vencimiento con fecha límite ("debes enviar el informe antes del lunes", "la matrícula cierra el 30").
   - availability: VENTANA de disponibilidad con inicio Y fin (RANGOS: "la encuesta está disponible del 5 al 23", "del 10 al 20"). Solo para rangos con ambos extremos.
   - task: compromiso sin fecha ("te envío el documento", "adjunto los apuntes").
3. Compromisos = acciones prometidas o solicitadas: fechas, plazos, reuniones, entregas, disponibilidades. Ignora fórmulas de cortesía, firmas, despedidas y metadatos ("espero que estés bien", "saludos", "adjunto: ...").
4. Fechas relativas → absolutas (YYYY-MM-DD) según el día actual. Hora: "HH:MM" 24h o null.
5. DESCONOCIDO = null. Jamás inventes fechas, horas ni duraciones.
6. PREPARACIÓN ADJUNTA: si el correo dice cuántas horas requiere el compromiso ("necesito 4 horas para preparar", "se necesitan 2 horas"), pon N*60 en preparation_minutes del intent.
7. confidence: 0.0 (ambiguo) a 1.0 (explícito). Si la fecha o la hora están implícitas o hay ambigüedad, baja la confianza (≤ 0.5) y explica en reason.
8. PRIVACIDAD: jamás incluyas información personal sensible (saludos, direcciones, teléfonos) en títulos ni descripciones. Solo el compromiso.
9. Títulos en español, específicos, sin artículos ni prefijos.
10. Un correo puede producir VARIOS intents (varias fechas/compromisos).
11. Si el correo NO contiene compromisos accionables, devuelve {"intents": []}.
12. reason: breve justificación en español, o null.

Ejemplos:
Correo: "Hola, te escribo para recordarte que el informe del proyecto se entrega el lunes 10 a las 23:59. También necesitamos verte el martes 11 a las 10:00. Un saludo."
→ {"intents":[{"intent_type":"deadline","title":"Informe del proyecto","category":"Trabajo","priority":"alta","deadline_date":"2026-08-10","deadline_time":"23:59","confidence":0.95,"reason":"entrega explícita"},{"intent_type":"event","title":"Reunión de seguimiento","category":"Trabajo","priority":"media","start_date":"2026-08-11","start_time":"10:00","duration_minutes":60,"confidence":0.9,"reason":"fecha y hora concretas"}]}

Correo: "La encuesta de satisfacción está disponible del 5 al 23 de agosto."
→ {"intents":[{"intent_type":"availability","title":"Encuesta de satisfacción","category":"Personal","priority":"baja","start_date":"2026-08-05","end_date":"2026-08-23","confidence":0.9,"reason":"ventana de disponibilidad"}]}
"#;

/// Minimiza el correo antes de mandarlo a la IA (privacidad, fase 8):
/// - elimina citas/respondidos (líneas con ">" y "On … wrote:")
/// - trunca el cuerpo a `MAX_BODY_CHARS` caracteres
/// El resultado sigue incluyendo remitente y asunto (necesarios para el
/// contexto), pero NUNCA el cuerpo completo.
pub fn minimize_email(raw: &RawEmail) -> String {
    let mut clean = String::with_capacity(raw.body.len().min(MAX_BODY_CHARS + 64));
    for line in raw.body.lines() {
        let t = line.trim_start();
        if t.starts_with('>') || t.starts_with("On ") && t.contains("wrote:") {
            continue;
        }
        clean.push_str(line);
        clean.push('\n');
        if clean.len() >= MAX_BODY_CHARS {
            break;
        }
    }
    if clean.chars().count() > MAX_BODY_CHARS {
        clean.truncate(MAX_BODY_CHARS);
        clean.push_str("\n[…]");
    }
    let snippet = clean.trim();
    format!(
        "De: {}\nAsunto: {}\nFecha: {}\n\nCuerpo:\n{}",
        raw.sender, raw.subject, raw.date, snippet
    )
}

/// Interpreta el correo (minimizado) con la IA y devuelve el lote de
/// compromisos. Sin IA configurada falla con `NotConfigured`: el correo no
/// tiene heurística local (a diferencia del texto libre).
pub fn parse_email_intent(
    raw: &RawEmail,
    provider: &dyn AiProvider,
    configured: bool,
) -> AiResult<IntentBatch> {
    if !configured {
        return Err(AiError::NotConfigured(
            "IA no configurada: no se pueden analizar correos".into(),
        ));
    }
    let user = format!(
        "Hoy es {}.\n\n{}",
        chrono::Local::now().format("%Y-%m-%d %A"),
        minimize_email(raw)
    );
    let v = provider.chat_json(EMAIL_SYSTEM_PROMPT, &user, INTENT_SCHEMA)?;
    parse_batch_json(&v)
}

/// Mapea el intent de un correo a la fila de sugerencia: el `kind` que se
/// persiste en `suggested_events.kind` (event | deadline | availability | task).
pub fn suggestion_kind(it: &IntentType) -> &'static str {
    match it {
        IntentType::Event => "event",
        IntentType::Task => "task",
        IntentType::Deadline => "deadline",
        IntentType::Availability => "availability",
        // no accionables desde correo; el llamador los filtra antes
        _ => "task",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn raw(body: &str) -> RawEmail {
        RawEmail {
            mailbox: "INBOX".into(),
            uid: 1,
            message_id: "msg-1".into(),
            subject: "Reunión".into(),
            sender: "jefe@corp.com".into(),
            date: "2026-08-08".into(),
            body: body.into(),
        }
    }

    struct DummyProvider(serde_json::Value);
    impl AiProvider for DummyProvider {
        fn id(&self) -> &str {
            "dummy"
        }
        fn chat_json(&self, _s: &str, _u: &str, _schema: &str) -> AiResult<serde_json::Value> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn minimize_strips_quotes_and_truncates() {
        let body = format!(
            "Te confirmo la reunión del viernes.\n\n{}\nFirma",
            "> On Tue, Aug 5 wrote:\n> > hola\n".repeat(50)
        );
        let r = raw(&body);
        let m = minimize_email(&r);
        assert!(!m.contains("wrote:"), "citas eliminadas");
        assert!(m.contains("reunión"), "cuerpo limpio conservado");
        assert!(m.len() < body.len() + 200, "truncado: {} vs {}", m.len(), body.len());
    }

    #[test]
    fn parse_deadline_and_event_from_email() {
        let fixture = json!({"intents": [
            {"intent_type":"deadline","title":"Informe del proyecto","category":"Trabajo",
             "priority":"alta","deadline_date":"2026-08-10","deadline_time":"23:59",
             "confidence":0.95,"reason":"entrega explícita"},
            {"intent_type":"event","title":"Reunión","category":"Trabajo",
             "start_date":"2026-08-11","start_time":"10:00","duration_minutes":60,
             "confidence":0.9,"reason":"fecha concreta"}
        ]});
        let p = DummyProvider(fixture);
        let batch = parse_email_intent(&raw("el informe se entrega el 10"), &p, true).expect("ai");
        assert_eq!(batch.intents.len(), 2);
        assert_eq!(batch.intents[0].intent_type, IntentType::Deadline);
        assert!(batch.intents[0].deadline.is_some());
        assert_eq!(batch.intents[1].intent_type, IntentType::Event);
        assert_eq!(suggestion_kind(&batch.intents[0].intent_type), "deadline");
        assert_eq!(suggestion_kind(&batch.intents[1].intent_type), "event");
    }

    #[test]
    fn parse_availability_range() {
        let fixture = json!({"intents": [
            {"intent_type":"availability","title":"Encuesta","category":"Personal",
             "start_date":"2026-08-05","end_date":"2026-08-23",
             "confidence":0.9,"reason":"ventana"}
        ]});
        let p = DummyProvider(fixture);
        let batch = parse_email_intent(&raw("disponible del 5 al 23"), &p, true).expect("ai");
        assert_eq!(batch.intents[0].intent_type, IntentType::Availability);
        assert_eq!(suggestion_kind(&batch.intents[0].intent_type), "availability");
        let w = batch.intents[0].window;
        assert!(w.start.is_some() && w.end.is_some() && w.start.unwrap() < w.end.unwrap());
    }

    #[test]
    fn parse_irrelevant_email_returns_empty() {
        let fixture = json!({"intents": []});
        let p = DummyProvider(fixture);
        let batch = parse_email_intent(&raw("espero que estés bien, saludos"), &p, true).expect("ai");
        assert!(batch.intents.is_empty());
    }

    #[test]
    fn parse_without_ai_fails() {
        let p = DummyProvider(json!({"intents": []}));
        let err = parse_email_intent(&raw("hola"), &p, false).expect_err("sin IA falla");
        match err {
            AiError::NotConfigured(_) => {}
            other => panic!("esperado NotConfigured, got {other:?}"),
        }
    }

    #[test]
    fn suggestion_kind_maps_all_actionable() {
        use IntentType::*;
        assert_eq!(suggestion_kind(&Event), "event");
        assert_eq!(suggestion_kind(&Task), "task");
        assert_eq!(suggestion_kind(&Deadline), "deadline");
        assert_eq!(suggestion_kind(&Availability), "availability");
    }
}
