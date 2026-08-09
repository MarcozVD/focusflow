//! Pruebas E2E (fase 13): viajes reales del usuario a nivel de proceso/datos.
//! Usan una base de datos REAL en disco (la misma `Db::open` de producción):
//! lo que se escribe persiste entre "sesiones" — crear, cerrar, reabrir y
//! verificar es el equivalente headless de cerrar y relanzar la app.

use focusflow_spike_lib::ai::intent::IntentType;
use focusflow_spike_lib::ai::intent_parser::parse_batch_json;
use focusflow_spike_lib::ai::rule_based::analyze_to_json;
use focusflow_spike_lib::planning::{accept_plan, plan_from_text};
use focusflow_spike_lib::store::Db;
use focusflow_spike_lib::sync::accept_suggestion;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_data_dir() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ff-e2e-{}", std::process::id()))
        .join(stamp.to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open(dir: &PathBuf) -> Db {
    Db::open(dir).unwrap()
}

/// Abre y limpia las tareas de demostración del primer arranque (siembra
/// intencional de producción): los E2E trabajan solo con sus propios datos.
fn open_clean(dir: &PathBuf) -> Db {
    let d = open(dir);
    d.wipe_data().unwrap();
    d
}

fn interpret(text: &str) -> Vec<focusflow_spike_lib::ai::intent::Intent> {
    let json = analyze_to_json(text).expect("análisis local");
    parse_batch_json(&json).expect("intents válidos").intents
}

// ---------------------------------------------------------------------------
// Escenario 1: crear tarea → cerrar app → reabrir → la tarea sigue
// ---------------------------------------------------------------------------

#[test]
fn s1_task_survives_app_restart() {
    let dir = temp_data_dir();
    let now = chrono::Local::now().timestamp_millis();

    // sesión 1: crear
    {
        let d = open_clean(&dir);
        let t = d.create("Entregar informe de prácticas", "uni", "alta", now + 86_400_000, now + 86_400_000 + 3_600_000, false).unwrap();
        assert!(t.id > 0);
    } // cierra (drop)

    // sesión 2: reabrir y verificar
    {
        let d = open(&dir);
        let tasks = d.list().unwrap();
        assert_eq!(tasks.len(), 1, "una sola tarea, sin demo re-sembrada");
        assert_eq!(tasks[0].title, "Entregar informe de prácticas");
        assert_eq!(tasks[0].status, "pendiente");
        let t = d.get_task(tasks[0].id).unwrap().unwrap();
        assert_eq!(t.priority, "alta");
        assert_eq!(t.start_at, now + 86_400_000);
    }

    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Escenario 2: lenguaje natural → plan → aceptar → calendario (persistente)
// ---------------------------------------------------------------------------

#[test]
fn s2_nl_plan_accept_persists_to_calendar() {
    let dir = temp_data_dir();
    let d = open_clean(&dir);
    let text = "Examen de cálculo el viernes y necesito 4 horas para preparar";
    let intents = interpret(text);
    let view = plan_from_text(&d, text, &intents, "local").unwrap();
    let tasks = accept_plan(&d, view.id, &Default::default()).expect("acepta el plan");
    assert!(tasks.len() >= 2, "evento + sesiones de preparación");
    drop(d);

    // "relanzar": la sesión persistida sigue en el calendario, y el motor
    // (que es lo que pinta la vista) la descuenta del tiempo libre
    let d = open(&dir);
    let reopened = d.list().unwrap();
    assert_eq!(reopened.len(), tasks.len(), "todo persistió");
    let ev = reopened
        .iter()
        .find(|t| t.metadata.contains(r#""plan_kind":"event""#))
        .expect("el evento del examen está");
    let engine = focusflow_spike_lib::planning::engine_with_calendar(&d);
    let free = engine.available_minutes(ev.start_at - 3_600_000, ev.start_at + 3_600_000);
    assert!(free < 120, "el examen ocupa su hueco tras el reinicio (libre={free})");
    let row = d.get_plan_proposal(view.id).unwrap().unwrap();
    assert_eq!(row.status, "accepted", "el estado de la propuesta persistió");
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Escenario 3: correo → compromiso → sugerencia → aceptar → calendario
// ---------------------------------------------------------------------------

struct MockEmailAi(serde_json::Value);
impl focusflow_spike_lib::ai::AiProvider for MockEmailAi {
    fn id(&self) -> &str {
        "mock"
    }
    fn chat_json(&self, _s: &str, _u: &str, _schema: &str) -> focusflow_spike_lib::ai::AiResult<serde_json::Value> {
        Ok(self.0.clone())
    }
}

#[test]
fn s3_email_suggestion_accept_persists() {
    let dir = temp_data_dir();
    let d = open_clean(&dir);
    let manana = (chrono::Local::now().date_naive() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let fixture = serde_json::json!({"intents": [
        {"intent_type": "event", "title": "Tutoría de tesis", "category": "Universidad",
         "priority": "alta", "start_date": manana, "start_time": "09:00",
         "duration_minutes": 90, "confidence": 0.95, "reason": "hora explícita"}
    ]});
    let raw = focusflow_spike_lib::email::RawEmail {
        mailbox: "INBOX".into(),
        uid: 3,
        message_id: "e2e-msg-3".into(),
        subject: "Tutoría de tesis mañana".into(),
        sender: "director@unab.edu.co".into(),
        date: "2026-08-08".into(),
        body: "Nos vemos mañana a las 9 para la tutoría de tesis.".into(),
    };
    let batch = focusflow_spike_lib::ai::email_intent::parse_email_intent(&raw, &MockEmailAi(fixture), true).unwrap();
    let it = &batch.intents[0];
    assert_eq!(it.intent_type, IntentType::Event);
    let start = it.window.start.unwrap();
    let end = it.window.end.unwrap_or(start);

    // sugerencia pendiente → aceptar → tarea
    let sid = d
        .insert_suggestion(
            "email", Some(&raw.message_id), Some(&raw.sender), &raw.subject, "event",
            &it.title, &it.description, &it.category_id, "alta",
            Some(start), Some(end), None, 0, "", "[]", it.confidence, &it.reason,
            None, "", "pending",
        )
        .unwrap();
    let task = accept_suggestion(&d, sid).unwrap();
    assert_eq!(task.start_at, start, "la tarea usa la hora del correo");
    drop(d);

    // relanzar: tarea y estado de la sugerencia persisten
    let d = open(&dir);
    let rows = d.list_range(start - 1, end + 1).unwrap();
    assert!(rows.iter().any(|r| r.id == task.id), "la tutoría está en el calendario");
    assert_eq!(d.get_suggestion(sid).unwrap().unwrap().status, "accepted");
    assert_eq!(d.suggestion_count_for_email(&raw.message_id).unwrap(), 1, "dedupe por correo");
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Escenario 4: conflicto → detectar → alternativa
// ---------------------------------------------------------------------------

#[test]
fn s4_conflict_blocked_then_alternative_lands() {
    let dir = temp_data_dir();
    let d = open_clean(&dir);
    let text = "Estudiar química 1 hora";
    let intents = interpret(text);
    let view = plan_from_text(&d, text, &intents, "local").unwrap();
    let slot = view.items[0].sessions[0].start_ms;

    // el compromiso que llega después choca → aceptación bloqueada
    d.create("Reunión urgente", "tra", "alta", slot, slot + 3_600_000, false).unwrap();
    let err = accept_plan(&d, view.id, &Default::default()).expect_err("conflicto");
    assert!(err.contains("se solapa"), "{err}");

    // alternativa: mover el compromiso → el plan se acepta y persiste
    let clash_id = d.list().unwrap().iter().find(|t| t.title == "Reunión urgente").unwrap().id;
    d.move_to(clash_id, slot + 48 * 3_600_000, slot + 49 * 3_600_000, None).unwrap();
    let tasks = accept_plan(&d, view.id, &Default::default()).expect("alternativa aceptada");
    drop(d);

    let d = open(&dir);
    let mut all = d.list().unwrap();
    assert_eq!(all.len(), 2, "reunión + estudio persistieron");
    all.sort_by_key(|t| t.start_at);
    for w in all.windows(2) {
        assert!(w[1].start_at >= w[0].end_at, "sin solapamientos tras reinicio");
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Escenario 5: onboarding — primer arranque incompleto, completar persiste,
// wipe devuelve al primer arranque
// ---------------------------------------------------------------------------

#[test]
fn s5_onboarding_flag_lifecycle() {
    let dir = temp_data_dir();

    // primer arranque: sin flag (onboarding pendiente)
    {
        let d = open(&dir);
        assert!(
            d.settings_get("onboarding.completed").unwrap().is_none(),
            "primer arranque muestra onboarding"
        );
    }

    // el usuario completa el onboarding
    {
        let d = open(&dir);
        d.settings_set("onboarding.completed", "1").unwrap();
    }

    // relanzar: no vuelve a aparecer
    {
        let d = open(&dir);
        assert_eq!(
            d.settings_get("onboarding.completed").unwrap().as_deref(),
            Some("1"),
            "completado nunca reaparece"
        );
    }

    // wipe = primer arranque de nuevo
    {
        let d = open(&dir);
        d.wipe_data().unwrap();
        assert!(
            d.settings_get("onboarding.completed").unwrap().is_none(),
            "wipe reinicia el onboarding"
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}
