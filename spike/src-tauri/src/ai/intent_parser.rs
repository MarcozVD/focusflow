//! Parser de intenciones: convierte texto libre en `IntentBatch`.
//!
//! Independiente del proveedor: cualquier `AiProvider` (Gemini, OpenAI, Zen…)
//! sirve. Sin IA configurada, cae a la heurística del módulo 1
//! ([super::nl::parse_task_nl]) y produce un `Intent` de tipo Task.

use serde_json::Value;

use super::intent::{from_task, Intent, IntentBatch};
use super::intent_validator::{parse_intent_json, validate_intent};
use super::{AiError, AiProvider, AiResult};

/// Forma del JSON que DEBE devolver el proveedor. Todo campo opcional admite
/// `null` (información desconocida).
pub const INTENT_SCHEMA: &str = r#"{
  "intents": [
    {
      "intent_type": "event|task|deadline|preparation|availability|reminder|constraint",
      "title": "string obligatorio, en español, sin 'tarea' ni artículos",
      "description": "string|null",
      "category": "Universidad|Trabajo|Personal|Finanzas|Salud|Otro|null",
      "priority": "alta|media|baja|null",
      "start_date": "YYYY-MM-DD|null",
      "start_time": "HH:MM|null",
      "end_date": "YYYY-MM-DD|null",
      "end_time": "HH:MM|null",
      "duration_minutes": 120 | null,
      "deadline_date": "YYYY-MM-DD|null",
      "deadline_time": "HH:MM|null",
      "preparation_minutes": 240 | null,
      "preparation_note": "string|null",
      "recurrence": {
        "frequency": "daily|weekly|monthly|yearly",
        "interval": 1 | null,
        "by_day": [1..7] | null,
        "count": 5 | null,
        "until": "YYYY-MM-DD|null"
      } | null,
      "reminders": [ { "minutes_before": 60 | null, "at": "YYYY-MM-DD HH:MM|null" } ],
      "constraints": [
        { "kind": "blocked_by|cannot_overlap|must_finish_before|must_start_after|daily_cap|other",
          "target": "string|null", "value": "string|null" }
      ],
      "confidence": 0.9,
      "reason": "string|null"
    }
  ]
}"#;

const SYSTEM_PROMPT: &str = r#"Eres el analizador de intenciones de FocusFlow, un planificador de estudio personal.
Convierte el texto libre del usuario en una lista de intenciones estructuradas en JSON.

REGLAS:
1. Devuelve EXCLUSIVAMENTE un JSON con la forma del esquema dado. No texto fuera del JSON.
2. Tipos de intención:
   - event: actividad con fecha/hora concreta ("examen el viernes", "reunión el 14 a las 10").
   - task: actividad sin fecha concreta (backlog: "estudiar cálculo").
   - deadline: entrega/vencimiento con fecha límite ("el proyecto se entrega el lunes").
   - preparation: requisito de tiempo de preparación sin fecha ("necesito 4 horas para preparar").
   - availability: VENTANA de disponibilidad con inicio Y fin (RANGOS: "disponible desde el 5 hasta el 23", "del 10 al 20", "de 9 a 12"). Solo para rangos con ambos extremos.
   - reminder: solo aviso ("recuérdame el sábado", "avísame 1 hora antes").
   - constraint: restricción entre elementos ("no puedo el martes por la mañana porque trabajo", "no se superponga con X").
3. Desambiguación: si el input menciona UNA actividad con fecha, es event. Si menciona DOS fechas como rango, es availability.
4. COMPUESTOS: un input puede producir varios intents. "Examen el viernes y necesito 4 horas para preparar" → 1 intent event con preparation_minutes=240. "Estudiar el viernes y programación el sábado" → 2 intents event.
5. PREPARACIÓN ADJUNTA: cuando el usuario dice "necesito N horas para prepararme/estudiar para X" o "necesito al menos N horas", pon N*60 en preparation_minutes DENTRO del intent del evento/deadline. El planner reserva ese tiempo antes.
6. DESCONOCIDO = null. Jamás inventes fechas, horas ni duraciones. Si no se sabe la hora, start_time: null y all_day implícito. Si no se sabe el día, start_date: null.
7. Fechas relativas → absolutas: "mañana"/"hoy"/"el lunes"/"el 14" se resuelven a la fecha concreta (YYYY-MM-DD) según el día actual. Hoy es el día real del sistema.
8. Hora: formato 24h "HH:MM" o null. Duración en minutos enteros.
9. confidence: 0.0 (sin datos) a 1.0 (explícito). reason: breve justificación en español, o null.
10. category: una de Universidad|Trabajo|Personal|Finanzas|Salud|Otro; si no se puede inferir, null.
11. NO conviertas una disponibilidad en un evento: "disponible desde el 5 hasta el 23" es availability, no event.
12. Títulos en español, específicos, sin "Tarea:" ni prefijos.
13. Si el texto no contiene ninguna intención accionable, devuelve {"intents": []}.

Ejemplos:
Usuario: "I have a calculus exam Friday and need four hours to prepare."
→ {"intents":[{"intent_type":"event","title":"Examen de cálculo","category":"Universidad","priority":"alta","start_date":"2026-08-14","start_time":null,"end_date":null,"end_time":null,"duration_minutes":120,"preparation_minutes":240,"preparation_note":"necesito 4 horas para preparar","confidence":0.9,"reason":"fecha concreta"}]}

Usuario: "Tomorrow at 6 PM study programming for two hours."
→ {"intents":[{"intent_type":"event","title":"Estudiar programación","category":"Universidad","priority":"media","start_date":"<mañana>","start_time":"18:00","end_date":null,"end_time":null,"duration_minutes":120,"confidence":0.95,"reason":"hora y duración explícitas"}]}

Usuario: "The project is due Monday but I need at least six hours to finish it."
→ {"intents":[{"intent_type":"deadline","title":"Proyecto","category":"Universidad","priority":"alta","deadline_date":"<lunes>","deadline_time":null,"preparation_minutes":360,"preparation_note":"necesito al menos 6 horas","confidence":0.85,"reason":"entrega el lunes"}]}

Usuario: "Diagnostic Test is available from August 5 until August 23."
→ {"intents":[{"intent_type":"availability","title":"Diagnostic Test","category":"Universidad","priority":"alta","start_date":"2026-08-05","start_time":null,"end_date":"2026-08-23","end_time":null,"confidence":0.9,"reason":"ventana de disponibilidad"}]}
"#;

/// Parsea el JSON del proveedor en un lote validado de intents.
/// `{"intents": [...]}` o un array plano.
pub fn parse_batch_json(v: &Value) -> AiResult<IntentBatch> {
    let arr = match v {
        Value::Array(a) => a,
        Value::Object(o) => o
            .get("intents")
            .and_then(|x| x.as_array())
            .ok_or_else(|| AiError::InvalidJson("falta el array 'intents'".into()))?,
        _ => return Err(AiError::InvalidJson("la respuesta debe ser un objeto o array".into())),
    };
    // tope de tamaño: la IA (o un correo inyectado) no puede generar spam
    // de sugerencias ilimitado
    const MAX_INTENTS: usize = 12;
    if arr.len() > MAX_INTENTS {
        return Err(AiError::BadResponse(format!(
            "demasiadas intenciones en el lote: {} (máx. {MAX_INTENTS})",
            arr.len()
        )));
    }
    let mut intents: Vec<Intent> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        match parse_intent_json(item) {
            Ok(i) => match validate_intent(&i) {
                Ok(()) => intents.push(i),
                Err(e) => errors.push(format!("intent #{idx}: {e}")),
            },
            Err(AiError::InvalidJson(m)) => errors.push(format!("intent #{idx}: {m}")),
            Err(e) => return Err(e),
        }
    }
    if !errors.is_empty() {
        return Err(AiError::BadResponse(errors.join("; ")));
    }
    Ok(IntentBatch { intents, source: "ai".into() })
}

/// Punto de entrada único. `configured` + `provider` = IA; si no, heurística
/// local (módulo 1) como fallback de un solo intent de tipo Task.
pub fn parse_intent(
    text: &str,
    provider: Option<&dyn AiProvider>,
    configured: bool,
) -> AiResult<IntentBatch> {
    if configured {
        let provider = provider
            .ok_or_else(|| AiError::NotConfigured("sin proveedor configurado".into()))?;
        let user = format!(
            "Texto del usuario (hoy: {}):\n{text}",
            chrono::Local::now().format("%Y-%m-%d %A")
        );
        let v = provider.chat_json(SYSTEM_PROMPT, &user, INTENT_SCHEMA)?;
        return parse_batch_json(&v);
    }
    match super::nl::parse_task_nl(text) {
        Some(t) => Ok(IntentBatch { intents: vec![from_task(&t)], source: "local".into() }),
        None => Err(AiError::NotConfigured(
            "IA no configurada y la heurística local no entendió el texto".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::intent::IntentType;
    use serde_json::json;

    /// Proveedor ficticio que devuelve un fixture fijo, para probar que el
    /// pipeline es independiente del proveedor.
    struct DummyProvider(Value);
    impl AiProvider for DummyProvider {
        fn id(&self) -> &str {
            "dummy"
        }
        fn chat_json(&self, _s: &str, _u: &str, _schema: &str) -> AiResult<Value> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn parse_batch_json_handles_wrapper_and_errors() {
        let ok = json!({"intents": [
            {"intent_type": "task", "title": "Estudiar física", "confidence": 0.7}
        ]});
        let batch = parse_batch_json(&ok).expect("válido");
        assert_eq!(batch.intents.len(), 1);
        assert_eq!(batch.intents[0].intent_type, IntentType::Task);
        assert_eq!(batch.source, "ai");

        // end < start → error de validación reportado
        let bad = json!({"intents": [
            {"intent_type": "event", "title": "X", "start_date": "2026-08-23", "end_date": "2026-08-05", "confidence": 0.5}
        ]});
        let err = parse_batch_json(&bad).expect_err("debe fallar");
        match err {
            AiError::BadResponse(m) => assert!(m.contains("end antes de start"), "{m}"),
            other => panic!("esperado BadResponse, got {other:?}"),
        }
    }

    #[test]
    fn parse_batch_json_empty_ok() {
        let v = json!({"intents": []});
        let batch = parse_batch_json(&v).expect("vacío válido");
        assert!(batch.intents.is_empty());
    }

    #[test]
    fn oversized_batch_rejected() {
        let intents: Vec<_> = (0..13)
            .map(|i| json!({"intent_type": "task", "title": format!("T{i}"), "confidence": 0.5}))
            .collect();
        let v = json!({"intents": intents});
        let err = parse_batch_json(&v).expect_err("más de 12 intents se rechaza");
        match err {
            AiError::BadResponse(m) => assert!(m.contains("demasiadas"), "{m}"),
            other => panic!("esperado BadResponse, got {other:?}"),
        }
    }

    #[test]
    fn llm_text_fields_are_capped() {
        let v = json!({"intents": [
            {"intent_type": "task", "title": "A".repeat(500), "description": "B".repeat(700),
             "reason": "C".repeat(300), "preparation_minutes": 30,
             "preparation_note": "D".repeat(300), "confidence": 0.5}
        ]});
        let batch = parse_batch_json(&v).expect("válido con campos truncados");
        assert_eq!(batch.intents[0].title.chars().count(), 200, "título ≤ 200");
        assert_eq!(batch.intents[0].description.chars().count(), 600, "descripción ≤ 600");
        assert_eq!(batch.intents[0].reason.chars().count(), 200, "reason ≤ 200");
        assert_eq!(batch.intents[0].preparation.as_ref().unwrap().note.chars().count(), 200, "nota ≤ 200");
    }

    #[test]
    fn parse_intent_with_provider_uses_ai() {
        let fixture = json!({"intents": [
            {"intent_type": "event", "title": "Examen de cálculo", "category": "Universidad",
             "priority": "alta", "start_date": "2026-08-14", "preparation_minutes": 240,
             "confidence": 0.9, "reason": "fecha concreta"}
        ]});
        let p = DummyProvider(fixture);
        let batch = parse_intent("examen el viernes", Some(&p), true).expect("ai");
        assert_eq!(batch.source, "ai");
        assert_eq!(batch.intents.len(), 1);
        assert_eq!(batch.intents[0].intent_type, IntentType::Event);
        assert_eq!(batch.intents[0].source, "ai");
    }

    #[test]
    fn parse_intent_without_ai_falls_back_to_heuristic() {
        let batch = parse_intent("mañana a las 6 pm estudiar programación por 2 horas", None, false)
            .expect("local");
        assert_eq!(batch.source, "local");
        let i = &batch.intents[0];
        assert_eq!(i.intent_type, IntentType::Task);
        assert!(i.window.start.is_some(), "hora relativa resuelta");
        assert_eq!(i.source, "local");
    }

    #[test]
    fn parse_intent_without_ai_never_fails_on_non_empty_text() {
        // La heurística siempre produce una tarea (módulo 1), incluso con
        // texto sin señales: fallback de un intent de tipo Task.
        let batch = parse_intent("zqwxp", None, false).expect("fallback local");
        assert_eq!(batch.source, "local");
        assert_eq!(batch.intents[0].intent_type, IntentType::Task);
    }
}
