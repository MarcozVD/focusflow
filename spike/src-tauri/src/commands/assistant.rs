//! Dominio ASISTENTE (fase 9): turnos de conversación y acciones propuestas.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::store::{lock_recover, Db};
use crate::{ai, append_log, assistant};

/// Un turno del asistente: pregunta + historial → respuesta/propuesta.
/// El asistente nunca muta el calendario en este paso.
///
/// La base de datos NO se mantiene bloqueada durante las llamadas de red:
/// solo se toca para leer el contexto (breve) y para persistir propuestas.
#[tauri::command]
pub async fn assistant_turn(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    text: String,
    history: Vec<assistant::HistoryMsg>,
) -> Result<assistant::AssistantTurnView, String> {
    crate::ai_cooldown("assistant_turn")?;
    // fase 1: config + snapshot de contexto (lock breve, sin red)
    let (cfg, ctx) = crate::commands::with_db(&state, |db| {
        (
            crate::ai_config_from_db(db),
            assistant::context_snapshot(db),
        )
    });
    let configured =
        !cfg.endpoint.is_empty() && !cfg.model.is_empty() && cfg.provider_name() != "local";
    if !configured {
        append_log(&app, "assistant_turn mode=nothing");
        return Ok(assistant::AssistantTurnView::Nothing {
            text: "Sin IA configurada no puedo analizar tu calendario ni responder preguntas. Configura la IA en Ajustes → IA, o usa la barra rápida para añadir tareas.".into(),
        });
    }
    let log_app = app.clone();
    let text_ai = text.clone();
    // la IA es bloqueante: fuera del mutex
    let spawn_result = tauri::async_runtime::spawn_blocking(move || {
        let provider = match ai::provider_from_config(&cfg) {
            Ok(p) => p,
            Err(e) => return Err(format!("ia_fail {e}")),
        };
        // fase 2: red sin lock — decisión (y, en plan, el parseo de intención)
        let user = assistant::build_user_prompt(&ctx, &text_ai, &history);
        let decision = assistant::request_decision(provider.as_ref(), &user)?;
        let note = assistant::note_from_decision(&decision);
        let mode = decision
            .get("mode")
            .and_then(|m| m.as_str())
            .unwrap_or("answer");
        let app_ref: &AppHandle = &log_app;
        let state = app_ref.state::<Mutex<Db>>();
        match mode {
            "plan" => {
                let batch =
                    crate::ai::intent_parser::parse_intent(&text_ai, Some(provider.as_ref()), true)
                        .map_err(|e| e.to_string())?;
                // fase 3: persistir/leer con lock breve
                let db = lock_recover(&state);
                assistant::plan_from_intents(&db, &text_ai, &batch.intents, note)
            }
            "action" => {
                // fase 3: persistir/leer con lock breve
                let db = lock_recover(&state);
                assistant::action_mode(&db, &decision, &note)
            }
            _ => {
                // Respuesta inline en la decisión (1 sola llamada a la IA:
                // evita la segunda petición, que doblaba la exposición a 429
                // y dejaba al usuario esperando sin respuesta). Si el modelo
                // no la incluye, se recurre a la llamada dedicada de siempre.
                let inline = decision
                    .get("answer")
                    .and_then(|a| a.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                let db = lock_recover(&state);
                let refs = assistant::task_refs(&db, crate::email::now_ms());
                drop(db);
                match inline {
                    Some(t) => Ok(assistant::AssistantTurnView::Answer {
                        text: t,
                        tasks: refs,
                    }),
                    None => assistant::answer_text(provider.as_ref(), &user, refs),
                }
            }
        }
    })
    .await;
    let turn = match spawn_result {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            append_log(&app, &format!("assistant_turn_error: {e}"));
            return Err(e);
        }
        Err(e) => {
            append_log(&app, &format!("assistant_turn_join_error: {e}"));
            return Err(e.to_string());
        }
    };

    append_log(
        &app,
        &format!("assistant_turn mode={}", assistant_mode_name(&turn)),
    );
    let _ = app.emit("assistant:changed", ());
    Ok(turn)
}

fn assistant_mode_name(t: &assistant::AssistantTurnView) -> &'static str {
    match t {
        assistant::AssistantTurnView::Answer { .. } => "answer",
        assistant::AssistantTurnView::Plan { .. } => "plan",
        assistant::AssistantTurnView::Action { .. } => "action",
        assistant::AssistantTurnView::Nothing { .. } => "nothing",
    }
}

#[tauri::command]
pub fn assistant_actions_list(
    state: State<'_, Mutex<Db>>,
    only_pending: bool,
) -> Result<Vec<crate::store::AssistantActionRow>, String> {
    crate::commands::with_db(&state, |db| {
        db.list_assistant_actions(only_pending)
            .map_err(|e| e.to_string())
    })
}

/// Aprueba una acción propuesta: la aplica vía los servicios existentes del
/// store (nunca SQL directo del asistente).
#[tauri::command]
pub fn assistant_action_accept(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<String, String> {
    let summary = {
        let db = lock_recover(&state);
        let (_, action) =
            assistant::get_action(&db, id)?.ok_or_else(|| "acción no encontrada".to_string())?;
        let row = db
            .get_assistant_action(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "acción no encontrada".to_string())?;
        if row.status != "pending" {
            return Err(format!("acción ya procesada (estado: {})", row.status));
        }
        let summary = assistant::apply_action(&db, &action)?;
        db.set_assistant_action_status(id, "accepted")
            .map_err(|e| e.to_string())?;
        summary
    };
    append_log(
        &app,
        &format!("assistant_action_accepted id={id} -> {summary}"),
    );
    let _ = app.emit("tasks:changed", ());
    let _ = app.emit("assistant:changed", ());
    Ok(summary)
}

#[tauri::command]
pub fn assistant_action_reject(
    app: AppHandle,
    state: State<'_, Mutex<Db>>,
    id: i64,
) -> Result<(), String> {
    crate::commands::with_db(&state, |db| {
        db.set_assistant_action_status(id, "rejected")
            .map_err(|e| e.to_string())
    })?;
    append_log(&app, &format!("assistant_action_rejected id={id}"));
    let _ = app.emit("assistant:changed", ());
    Ok(())
}
