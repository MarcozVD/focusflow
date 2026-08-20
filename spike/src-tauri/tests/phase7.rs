//! Prueba de integración del bucle completo (fase 7):
//! Input → Intent → Planner → Propuesta → Aprobación → Calendario.
//!
//! Sin red: `RuleBasedProvider` interpreta el texto de forma determinista.

use focusflow_spike_lib::ai::intent::IntentType;
use focusflow_spike_lib::ai::intent_parser::parse_batch_json;
use focusflow_spike_lib::ai::rule_based::analyze_to_json;
use focusflow_spike_lib::planning::{accept_plan, plan_from_text, reject_plan};
use focusflow_spike_lib::store::Db;

fn db() -> Db {
    Db::open_memory_pub().unwrap()
}

/// DB sin seed: las pruebas con horarios fijos (ediciones) se rompen cuando
/// el seed demo (que cruza medianoche según la hora real) ocupa esos slots.
fn clean_db() -> Db {
    Db::open_memory_clean_pub().unwrap()
}

// "pasado mañana" (día+2) garantiza margen de preparación SIN depender del día
// de la semana: "el viernes" con hoy jueves dejaba <2 sesiones (flake fecha).
const EXAMPLE: &str = "Examen de cálculo pasado mañana y necesito 4 horas para preparar";

/// Input → Intent (proveedor local determinista, sin envoltorio de prompt).
fn interpret(text: &str) -> (Vec<focusflow_spike_lib::ai::intent::Intent>, String) {
    let json = analyze_to_json(text).expect("análisis local");
    let batch = parse_batch_json(&json).expect("intents válidos");
    (batch.intents, batch.source)
}

#[test]
fn core_loop_end_to_end() {
    let d = db();

    // 1. Input → Intent
    let (intents, source) = interpret(EXAMPLE);
    assert_eq!(intents.len(), 1, "examen + preparación se fusionan");
    assert_eq!(intents[0].intent_type, IntentType::Event);

    // 2. Intent → Planner → Propuesta (persistida como pending, sin tocar el calendario)
    let before = d.count().unwrap();
    let view = plan_from_text(&d, EXAMPLE, &intents, &source).expect("genera propuesta");
    assert_eq!(d.count().unwrap(), before, "generar propuesta no crea tareas");
    assert_eq!(view.status, "pending");
    assert_eq!(view.understanding.len(), 1);
    assert_eq!(view.understanding[0].prep_min, 240);

    let item = view.items.iter().find(|i| i.title.contains("Examen")).expect("ítem planificado");
    assert!(!item.sessions.is_empty(), "planifica sesiones de preparación");
    assert!(item.sessions.len() >= 2, "se divide en varias sesiones");
    assert!(item.sessions.len() <= 6, "fragmentación controlada");
    assert!(item.complete, "4 horas caben antes del viernes");
    // el límite del examen es su ventana (el proveedor local la pone en el
    // evento, no en `deadline`)
    let bound = view.understanding[0].window_start.expect("ventana del examen");
    for s in &item.sessions {
        assert!(s.is_prep, "sesión de preparación");
        assert!(s.end_ms <= bound, "termina antes del examen");
        assert!(s.end_ms > s.start_ms);
    }

    // 3. Propuesta → Aprobación → Calendario
    let tasks = accept_plan(&d, view.id, &Default::default()).expect("acepta");
    assert_eq!(d.count().unwrap(), before + tasks.len() as i64);
    let ev = tasks.iter().find(|t| t.metadata.contains(r#""plan_kind":"event""#)).expect("crea el evento del examen");
    assert_eq!(ev.start_at, view.understanding[0].window_start.expect("ventana del examen"));
    let sessions: Vec<_> = tasks.iter().filter(|t| t.metadata.contains(r#""plan_kind":"session""#)).collect();
    assert_eq!(sessions.len(), item.sessions.len(), "una tarea por sesión");
    for s in sessions {
        assert!(s.metadata.contains(&format!(r#""plan_proposal_id":{}"#, view.id)));
        assert!(s.end_at <= bound);
    }
    let row = d.get_plan_proposal(view.id).unwrap().unwrap();
    assert_eq!(row.status, "accepted");

    // 4. El calendario ahora es fuente de verdad: la re-planificación respeta
    // los compromisos aceptados (no vuelve a reservar los mismos huecos).
    let (intents2, source2) = interpret(EXAMPLE);
    let view2 = plan_from_text(&d, EXAMPLE, &intents2, &source2).expect("segunda propuesta");
    let item2 = view2.items.iter().find(|i| i.title.contains("Examen")).expect("ítem");
    assert!(item2.complete);
    for s in &item2.sessions {
        for t in &tasks {
            let overlap = s.start_ms < t.end_at && s.end_ms > t.start_at;
            assert!(!overlap, "'{}' se solapa con tarea existente '{}'", item2.title, t.title);
        }
    }
}

#[test]
fn rejected_plan_makes_no_changes() {
    let d = db();
    let (intents, source) = interpret("Estudiar álgebra 1 hora mañana a las 18");
    let before = d.count().unwrap();
    let view = plan_from_text(&d, "Estudiar álgebra 1 hora mañana a las 18", &intents, &source).unwrap();
    assert_eq!(d.count().unwrap(), before);

    reject_plan(&d, view.id).expect("rechaza");
    assert_eq!(d.count().unwrap(), before, "rechazar no modifica el calendario");
    let row = d.get_plan_proposal(view.id).unwrap().unwrap();
    assert_eq!(row.status, "rejected");
}

#[test]
fn edited_plan_reschedules_blocks() {
    let d = clean_db(); // slot fijo 20:00–22:00: el seed demo puede ocuparlo
    let (intents, source) = interpret("Estudiar biología 2 horas");
    let view = plan_from_text(&d, "Estudiar biología 2 horas", &intents, &source).unwrap();
    let item = &view.items[0];
    assert!(item.complete);

    // el usuario mueve el bloque: sábado 20:00–22:00
    let start = {
        let t = chrono::Local::now().date_naive() + chrono::Duration::days(2);
        focusflow_spike_lib::engine::local_ms(t.and_hms_opt(20, 0, 0).unwrap())
    };
    let edit = focusflow_spike_lib::planning::EditedPlan {
        items: vec![vec![focusflow_spike_lib::planning::EditedSession {
            start_ms: start,
            end_ms: start + 120 * 60_000,
        }]],
    };
    let tasks = accept_plan(&d, view.id, &edit).unwrap();
    assert_eq!(tasks.len(), 1, "una sesión según la edición");
    assert_eq!(tasks[0].start_at, start);
    assert_eq!(tasks[0].end_at, start + 120 * 60_000);
}

#[test]
fn conflicting_commitment_blocks_acceptance() {
    let d = db();
    let (intents, source) = interpret("Estudiar química 1 hora");
    let view = plan_from_text(&d, "Estudiar química 1 hora", &intents, &source).unwrap();

    // llega un compromiso nuevo que choca con el bloque propuesto
    let clash = view.items[0].sessions[0].start_ms;
    d.create("Reunión urgente", "trab", "alta", clash, clash + 3_600_000, false).unwrap();
    let err = accept_plan(&d, view.id, &Default::default()).unwrap_err();
    assert!(err.contains("se solapa"), "avisa del conflicto: {err}");
    let row = d.get_plan_proposal(view.id).unwrap().unwrap();
    assert_eq!(row.status, "pending", "la propuesta sigue pendiente");

    // el compromiso se resuelve (se mueve) → el plan puede aceptarse igual
    d.move_to(d.list().unwrap().iter().find(|t| t.title == "Reunión urgente").unwrap().id, clash + 3_600_000 * 24, clash + 3_600_000 * 25, None)
        .unwrap();
    let tasks = accept_plan(&d, view.id, &Default::default()).unwrap();
    assert_eq!(tasks.len(), 1, "se acepta al liberarse el hueco");
}

#[test]
fn no_available_time_is_reported() {
    let d = db();
    // horizonte completo ocupado con tareas reales
    for day in 0..15i64 {
        let t = chrono::Local::now().date_naive() + chrono::Duration::days(day);
        let s = focusflow_spike_lib::engine::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
        d.create("Bloque", "otr", "media", s, s + 24 * 3_600_000, true).unwrap();
    }
    let (intents, source) = interpret("Escribir informe 3 horas");
    let view = plan_from_text(&d, "Escribir informe 3 horas", &intents, &source).unwrap();
    let item = &view.items[0];
    assert!(!item.complete);
    assert_eq!(item.planned_min, 0, "sin tiempo disponible");
    assert!(!item.notes.is_empty(), "explica el motivo: {:?}", item.notes);
}

#[test]
fn insufficient_time_is_partial_and_explained() {
    let d = db();
    // solo 30 min libres por día → 10h requeridas no caben (5h máximas)
    for day in 1..15i64 {
        let t = chrono::Local::now().date_naive() + chrono::Duration::days(day);
        let s = focusflow_spike_lib::engine::local_ms(t.and_hms_opt(0, 0, 0).unwrap());
        d.create("Jornada", "trab", "media", s + 6 * 3_600_000, s + 21 * 3_600_000 + 30 * 60_000, false).unwrap();
    }
    let (intents, source) = interpret("Preparar presentación 10 horas");
    let view = plan_from_text(&d, "Preparar presentación 10 horas", &intents, &source).unwrap();
    let item = &view.items[0];
    assert!(!item.complete, "no alcanza");
    assert!(item.planned_min > 0, "planifica lo que cabe");
    assert!(item.planned_min < item.required_min);
    assert!(!item.notes.is_empty(), "explica la limitación");
}
