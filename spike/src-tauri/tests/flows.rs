//! Flujos de integración (fase 13): las cadenas completas que toca el usuario,
//! sin red. Cada flujo atraviesa los módulos reales (interpretación → planner
//! → store → motor de calendario).

use focusflow_spike_lib::ai::email_intent::parse_email_intent;
use focusflow_spike_lib::ai::intent::IntentType;
use focusflow_spike_lib::ai::intent_parser::parse_batch_json;
use focusflow_spike_lib::ai::rule_based::analyze_to_json;
use focusflow_spike_lib::ai::{AiError, AiProvider, AiResult};
use focusflow_spike_lib::email::RawEmail;
use focusflow_spike_lib::planning::{accept_plan, engine_with_calendar, plan_from_text};
use focusflow_spike_lib::store::Db;
use focusflow_spike_lib::sync::{accept_suggestion, revert_suggestion};

fn prio_str(p: &focusflow_spike_lib::ai::intent::Priority) -> &'static str {
    use focusflow_spike_lib::ai::intent::Priority::*;
    match p {
        Alta => "alta",
        Media => "media",
        Baja => "baja",
    }
}

fn db() -> Db {
    Db::open_memory_pub().unwrap()
}

fn interpret(text: &str) -> Vec<focusflow_spike_lib::ai::intent::Intent> {
    let json = analyze_to_json(text).expect("análisis local");
    parse_batch_json(&json).expect("intents válidos").intents
}

const HOUR: i64 = 3_600_000;

// ---------------------------------------------------------------------------
// Flujo 1: Quick Add → Intent → Calendario
// ---------------------------------------------------------------------------

#[test]
fn quickadd_text_becomes_calendar_block() {
    let d = db();
    // Quick Add = texto → intent → tarea directa (mismo camino que la UI)
    let intents = interpret("Reunión con el profe mañana a las 10 por 1 hora");
    let ev = intents.iter().find(|i| i.intent_type == IntentType::Event).expect("evento");
    let start = ev.window.start.expect("fecha+ hora");
    let end = ev.window.end.expect("duración aplicada");
    assert_eq!(end - start, 60 * 60_000, "1 hora");

    let t = d.create(&ev.title, &ev.category_id, prio_str(&ev.priority), start, end, false).unwrap();

    // El calendario (motor real) ve la tarea: las horas del bloque dejan de ser libres
    let engine = engine_with_calendar(&d);
    let before = engine.available_minutes(start - HOUR, start + 2 * HOUR);
    assert!(before < 3 * 60, "el bloque ocupado se descuenta del tiempo libre");
    let rows = d.list_range(start - HOUR, start + 2 * HOUR).unwrap();
    assert!(rows.iter().any(|r| r.id == t.id));
}

// ---------------------------------------------------------------------------
// Flujo 2: Email → Intent → Sugerencia → Calendario
// ---------------------------------------------------------------------------

/// Proveedor ficticio con fixture fijo (sin red, igual que la fase 8).
struct MockEmailAi(serde_json::Value);
impl AiProvider for MockEmailAi {
    fn id(&self) -> &str {
        "mock"
    }
    fn chat_json(&self, _s: &str, _u: &str, _schema: &str) -> AiResult<serde_json::Value> {
        Ok(self.0.clone())
    }
}

fn email_fixture() -> serde_json::Value {
    let manana = (chrono::Local::now().date_naive() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    serde_json::json!({"intents": [
        {"intent_type": "event", "title": "Reunión de equipo", "category": "Trabajo",
         "priority": "media", "start_date": manana, "start_time": "15:00",
         "duration_minutes": 60, "confidence": 0.9, "reason": "hora concreta"}
    ]})
}

#[test]
fn email_becomes_suggestion_then_calendar_task() {
    let d = db();
    let raw = RawEmail {
        mailbox: "INBOX".into(),
        uid: 7,
        message_id: "msg-flow-1".into(),
        subject: "Reunión de equipo mañana".into(),
        sender: "jefe@corp.com".into(),
        date: "2026-08-08".into(),
        body: "Hola, nos vemos mañana a las 15:00 para la reunión de equipo.".into(),
    };

    // Email → Intent (IA minimiza el correo; fixture determinista)
    let provider = MockEmailAi(email_fixture());
    let batch = parse_email_intent(&raw, &provider, true).expect("intents del correo");
    assert_eq!(batch.intents.len(), 1);
    assert_eq!(batch.intents[0].intent_type, IntentType::Event);

    // Intent → Sugerencia pendiente (sin remitente de confianza: pending)
    let it = &batch.intents[0];
    let start = it.window.start.expect("fecha");
    let end = it.window.end.unwrap_or(start);
    let sid = d
        .insert_suggestion(
            "email", Some(&raw.message_id), Some(&raw.sender), &raw.subject, "event",
            &it.title, &it.description, &it.category_id, prio_str(&it.priority),
            Some(start), Some(end), None, 0, "", "[]", it.confidence, &it.reason,
            None, "", "pending",
        )
        .unwrap();
    let s = d.get_suggestion(sid).unwrap().unwrap();
    assert_eq!(s.status, "pending", "sin remitente de confianza no se auto-aprueba");

    // Sugerencia → Aceptar → Tarea en el calendario
    let before = d.count().unwrap();
    let task = accept_suggestion(&d, sid).unwrap();
    assert_eq!(d.count().unwrap(), before + 1);
    assert_eq!(task.start_at, start);
    let rows = d.list_range(start - HOUR, end + HOUR).unwrap();
    assert!(rows.iter().any(|r| r.id == task.id), "la tarea ocupa su hueco");

    // Revertir → la tarea desaparece, la sugerencia vuelve a pending
    revert_suggestion(&d, sid).unwrap();
    assert_eq!(d.count().unwrap(), before, "revertir borra la tarea creada");
    assert_eq!(d.get_suggestion(sid).unwrap().unwrap().status, "pending");
}

#[test]
fn email_without_commitments_yields_nothing() {
    let d = db();
    let raw = RawEmail {
        mailbox: "INBOX".into(),
        uid: 8,
        message_id: "msg-flow-2".into(),
        subject: "Gracias".into(),
        sender: "x@y.com".into(),
        date: "2026-08-08".into(),
        body: "Espero que estés bien. Saludos.".into(),
    };
    let provider = MockEmailAi(serde_json::json!({"intents": []}));
    let batch = parse_email_intent(&raw, &provider, true).expect("sin intents es válido");
    assert!(batch.intents.is_empty());
    let _ = d;
}

// ---------------------------------------------------------------------------
// Flujo 3: Asistente → Propuesta → Aprobación → Calendario
// ---------------------------------------------------------------------------

#[test]
fn assistant_proposal_approved_lands_on_calendar() {
    let d = db();
    // el asistente interpreta el pedido y genera una propuesta persistida
    let text = "Necesito estudiar 3 horas para el examen del viernes";
    let intents = interpret(text);
    let view = plan_from_text(&d, text, &intents, "local").unwrap();
    assert_eq!(view.status, "pending");

    // aprobación con edición (correr la primera sesión 1h más tarde) → el
    // calendario refleja la edición
    let slot = view.items[0].sessions[0].start_ms + 60 * 60_000;
    let edit = focusflow_spike_lib::planning::EditedPlan {
        items: vec![vec![focusflow_spike_lib::planning::EditedSession {
            start_ms: slot,
            end_ms: slot + 60 * 60_000,
        }]],
    };
    let tasks = accept_plan(&d, view.id, &edit).expect("acepta con edición");
    assert!(!tasks.is_empty());
    for t in &tasks {
        let rows = d.list_range(t.start_at - 1, t.end_at + 1).unwrap();
        assert!(rows.iter().any(|r| r.id == t.id), "en el calendario");
    }
}

// ---------------------------------------------------------------------------
// Flujo 4: Conflicto → detección → propuesta alternativa
// ---------------------------------------------------------------------------

#[test]
fn conflict_detected_and_alternative_proposed() {
    let d = db();
    let day_start = {
        let today = chrono::Local::now().date_naive() + chrono::Duration::days(1);
        focusflow_spike_lib::engine::local_ms(today.and_hms_opt(0, 0, 0).unwrap())
    };

    // el usuario planifica estudiar 1 hora en un hueco libre
    let text = "Estudiar química 1 hora";
    let intents = interpret(text);
    let view = plan_from_text(&d, text, &intents, "local").unwrap();
    let slot = view.items[0].sessions[0].start_ms;

    // 1. Detección: llega un compromiso que choca con el bloque propuesto
    //    → la aceptación se rechaza con aviso y la propuesta sigue pending
    d.create("Reunión urgente", "tra", "alta", slot, slot + HOUR, false).unwrap();
    let err = accept_plan(&d, view.id, &Default::default()).expect_err("conflicto detectado");
    assert!(err.contains("se solapa"), "{err}");
    assert_eq!(d.get_plan_proposal(view.id).unwrap().unwrap().status, "pending");

    // 2. Alternativa: el compromiso se libera (se mueve a otro día) → el
    //    mismo plan se acepta y ocupa el calendario sin solaparse con nada
    let clash = d
        .list()
        .unwrap()
        .iter()
        .find(|t| t.title == "Reunión urgente")
        .unwrap()
        .id;
    d.move_to(clash, slot + 24 * HOUR, slot + 25 * HOUR, None).unwrap();
    let tasks = accept_plan(&d, view.id, &Default::default()).expect("se acepta al liberarse el hueco");
    assert_eq!(tasks.len(), 1, "una sesión de estudio");
    assert!(d.find_overlap(tasks[0].id, tasks[0].start_at, tasks[0].end_at).unwrap().is_none(),
        "sin solapamiento residual");
}

// ---------------------------------------------------------------------------
// Errores de IA: correo sin IA configurada no rompe el flujo
// ---------------------------------------------------------------------------

#[test]
fn email_without_ai_reports_not_configured() {
    let raw = RawEmail {
        mailbox: "INBOX".into(),
        uid: 9,
        message_id: "m9".into(),
        subject: "x".into(),
        sender: "a@b.c".into(),
        date: "2026-08-08".into(),
        body: "hola".into(),
    };
    let provider = MockEmailAi(serde_json::json!({"intents": []}));
    let err = parse_email_intent(&raw, &provider, false).expect_err("sin config");
    match err {
        AiError::NotConfigured(_) => {}
        other => panic!("esperado NotConfigured, got {other:?}"),
    }
}
