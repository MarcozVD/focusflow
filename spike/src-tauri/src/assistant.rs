//! Asistente de FocusFlow (fase 9): ayuda con la gestión del tiempo.
//!
//! NO es un chatbot genérico: entiende el dominio (tareas, eventos,
//! vencimientos, disponibilidad, preferencias) y opera con el pipeline de la
//! app:
//!
//! ```text
//! pregunta ──► decisión (LLM) ──► contexto mínimo (solo lectura)
//!                 ├─ answer  ──► texto con contexto
//!                 ├─ plan    ──► intent_parser ─► planning::plan_from_text ─► propuesta
//!                 └─ action  ──► propuesta de acción (pendiente)
//!                                   └► aprobación del usuario ─► store (servicios existentes)
//! ```
//!
//! El asistente JAMÁS muta la base de datos directamente: toda mutación se
//! persiste como propuesta `pending` y se aplica al aprobar, pasando por los
//! servicios existentes del store ([Db::set_completed], [Db::move_to],
//! [Db::create], [planning::reject_plan]).

use chrono::{NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};

use crate::ai::intent::Intent;
use crate::ai::intent_parser::parse_intent;
use crate::ai::validation::category_id_from_name;
use crate::ai::{AiError, AiProvider, AiResult};
use crate::planning;
use crate::store::Db;

/// Forma del JSON que DEBE devolver la decisión del asistente.
pub const ASSISTANT_DECISION_SCHEMA: &str = r#"{
  "mode": "answer|plan|action",
  "action": {
    "kind": "complete|reschedule|create_event|cancel_proposal",
    "task_title": "string|null — título exacto de una tarea existente",
    "title": "string|null — título de la tarea a crear",
    "category": "Universidad|Trabajo|Personal|Finanzas|Salud|Otro|null",
    "priority": "alta|media|baja|null",
    "start_date": "YYYY-MM-DD|null — fecha absoluta",
    "start_time": "HH:MM|null",
    "duration_minutes": 60 | null
  } | null,
  "note": "string|null — explicación breve de lo que se hará",
  "answer": "string|null — si mode=answer, la respuesta COMPLETA aquí (2-4 frases)"
}"#;

const DECISION_SYSTEM_PROMPT: &str = r#"Eres el asistente de FocusFlow, un planificador de estudio y tiempo.
Clasificas la petición del usuario y devuelves EXCLUSIVAMENTE un JSON con el esquema dado.

REGLAS:
1. mode = "answer" cuando la petición SOLO pide información o análisis (tiempo disponible, qué hacer hoy, qué es urgente, si vas atrasado, dudas generales). Nunca muta nada. En este caso escribe la respuesta COMPLETA en el campo "answer" (2-4 frases, español, concreta, usando SOLO los datos del contexto: para tiempo libre calcula con las horas libres; para "qué hago hoy" prioriza vencimientos; no inventes datos ni propongas mutaciones — si hace falta actuar, dile al usuario que lo pida, ej. "dime 'marca X como hecha'").
2. mode = "plan" cuando la petición pide planificar/organizar tiempo o añadir estudio ("organiza mi semana", "planifica 4 horas de cálculo", "necesito preparar el examen"). El planificador se encarga: no rellenes action.
3. mode = "action" SOLO para acciones concretas sobre elementos EXISTENTES o nuevas citas concretas:
   - complete: "marca como hecha/completa X" → task_title.
   - reschedule: "mueve/reprograma X" → task_title + start_date/start_time + duration_minutes.
   - create_event: nueva cita/evento con fecha concreta ("reunión con Juan el viernes a las 10") → title + fecha.
   - cancel_proposal: "cancela la propuesta" → cancela la propuesta de plan pendiente (no rellenes nada más).
4. Fechas relativas → ABSOLUTAS (YYYY-MM-DD) respecto al día de HOY que se te da. Hora 24h. Si el usuario no da hora, start_time: null.
5. duration_minutes: la duración si se menciona; si no, 60 para eventos.
6. task_title: usa el título EXACTO de una tarea del contexto. Si ninguna tarea del contexto coincide, NO uses mode action: usa mode answer y pregunta a cuál te refieres.
7. DESCONOCIDO = null. Jamás inventes tareas, fechas ni duraciones.
8. context: usa el contexto dado (tareas pendientes, horas libres, hoy). Para cálculos de tiempo libre, responde con números del contexto.
9. note: breve resumen en español de lo que se hará o por qué (máx 2 frases).
10. Todo con mayúscula o en español: los títulos conservan el idioma del usuario.
"#;

const ANSWER_SYSTEM_PROMPT: &str = r#"Eres el asistente de FocusFlow, un planificador de estudio y tiempo.
Responde a la pregunta del usuario en español, breve y concreta (máximo 4 frases), usando SOLO los datos del contexto.

REGLAS:
1. Para "¿tengo tiempo para X?" → calcula con las horas libres del contexto y responde cuántas horas hay y dónde.
2. Para "¿qué hago hoy?" → prioriza vencimientos y tareas de hoy; si nada urge, sugiere la tarea más antigua sin fecha.
3. Para "¿qué es urgente?" → tareas con vencimiento próximo o prioridad alta.
4. Para "voy atrasado/a con X" → di cuánto se ha hecho y qué falta; sugiere planificarlo (el usuario puede pedir "planifica X").
5. No inventes datos que no estén en el contexto. No propongas mutaciones: si se necesita actuar, dile al usuario que lo pida ("dime 'marca X como hecha'").
6. Devuelve EXCLUSIVAMENTE un JSON: {"text": "tu respuesta"}.
"#;

/// Un turno del historial (rol "user" | "assistant").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMsg {
    pub role: String,
    pub text: String,
}

/// Acción propuesta (payload persistido en `assistant_actions`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantAction {
    pub kind: String,
    pub task_title: String,
    pub title: String,
    pub category_id: String,
    pub priority: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub all_day: bool,
    pub summary: String,
    /// Tarea objetivo resuelta (complete/reschedule). Se revalida al aceptar.
    pub task_id: Option<i64>,
}

/// Acción + id de propuesta, lo que ve el frontend.
#[derive(Debug, Clone, Serialize)]
pub struct AssistantActionView {
    pub proposal_id: i64,
    pub kind: String,
    pub task_title: String,
    pub title: String,
    pub category_id: String,
    pub priority: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub all_day: bool,
    pub summary: String,
}

/// Resultado de un turno del asistente.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum AssistantTurnView {
    /// Respuesta informativa (solo lectura).
    Answer { text: String, #[serde(default)] tasks: Vec<TaskRefView> },
    /// Propuesta de planificación (flujo de la fase 7, nada mutado aún).
    Plan { proposal: planning::PlanProposalView, note: String },
    /// Propuesta de acción concreta, pendiente de aprobación.
    Action { action: AssistantActionView },
    /// El asistente no puede/debe hacer nada con la petición.
    Nothing { text: String },
}

/// Referencia estructurada a una tarea real, con nivel de urgencia calculado
/// de forma DETERMINISTA (sin LLM): el modelo nunca inventa tareas ni fechas.
#[derive(Debug, Clone, Serialize)]
pub struct TaskRefView {
    pub id: i64,
    pub title: String,
    pub cat: String,
    pub priority: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    /// "URGENT" | "IMPORTANT" | "NORMAL"
    pub level: String,
}

const MAX_TASKS_IN_CONTEXT: usize = 40;

/// Contexto mínimo (solo lectura) para un turno: tareas pendientes compactas,
/// horas libres de los próximos 7 días y datos de preferencia. Nada del cuerpo
/// de descripciones/notas: solo lo necesario.
pub fn context_snapshot(db: &Db) -> String {
    let now = crate::email::now_ms();
    let today = chrono::Local::now().date_naive();

    let mut tasks = serde_json::json!([]);
    let mut total = 0usize;
    if let Ok(rows) = db.list() {
        let mut pending: Vec<_> = rows
            .into_iter()
            .filter(|t| t.status != "completada")
            .collect();
        pending.sort_by_key(|t| t.id);
        // Deduplica copias idénticas (mismo título normalizado y mismo día de
        // vencimiento): altas repetidas no deben inflar el contexto de la IA.
        let mut seen: std::collections::HashSet<(String, i64)> = std::collections::HashSet::new();
        pending.retain(|t| {
            let day = chrono::Local
                .timestamp_millis_opt(t.end_at)
                .earliest()
                .map(|d| d.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
                .unwrap_or(0);
            seen.insert((t.title.trim().to_lowercase(), day))
        });
        pending.sort_by_key(|t| t.start_at);
        total = pending.len();
        let view: Vec<serde_json::Value> = pending
            .iter()
            .take(MAX_TASKS_IN_CONTEXT)
            .map(|t| {
                let start_day = chrono::Local
                    .timestamp_millis_opt(t.start_at)
                    .earliest()
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                let end_day = chrono::Local
                    .timestamp_millis_opt(t.end_at)
                    .earliest()
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                serde_json::json!({
                    "id": t.id,
                    "title": t.title,
                    "cat": t.category_id,
                    "priority": t.priority,
                    "start_day": start_day,
                    "end_day": end_day,
                    "all_day": t.all_day,
                    "done": t.status == "completada",
                })
            })
            .collect();
        tasks = serde_json::Value::Array(view);
    }

    let engine = planning::engine_with_calendar(db);
    let mut free_days = serde_json::json!({});
    // Ventanas libres CONCRETAS (deterministas, del motor real): el LLM puede
    // decir "el martes 15h tienes 2 horas libres" con datos, no con agregados.
    let mut free_windows = serde_json::json!({});
    for d in 0..7 {
        let day = today + chrono::Duration::days(d);
        let start = crate::engine::local_ms(day.and_hms_opt(0, 0, 0).unwrap());
        let end = start + 24 * 3_600_000;
        let free_min = engine.available_minutes(start, end);
        free_days.as_object_mut().unwrap().insert(
            day.format("%Y-%m-%d").to_string(),
            serde_json::json!(free_min / 60),
        );
        let windows: Vec<serde_json::Value> = engine
            .free_intervals_on(day)
            .iter()
            .filter(|iv| iv.end - iv.start >= 30 * crate::engine::MIN_MS)
            .take(8)
            .map(|iv| {
                let s = chrono::Local.timestamp_millis_opt(iv.start).earliest().unwrap();
                let e = chrono::Local.timestamp_millis_opt(iv.end).earliest().unwrap();
                serde_json::json!({
                    "from": s.format("%H:%M").to_string(),
                    "to": e.format("%H:%M").to_string(),
                })
            })
            .collect();
        free_windows
            .as_object_mut()
            .unwrap()
            .insert(day.format("%Y-%m-%d").to_string(), serde_json::Value::Array(windows));
    }

    // Horario laboral y preferencia REALES del motor (no constantes).
    let working_hours = match engine.working_hours {
        Some(w) => format!(
            "{:02}:{:02}–{:02}:{:02}",
            w.start_min / 60,
            w.start_min % 60,
            w.end_min / 60,
            w.end_min % 60
        ),
        None => "24 horas".into(),
    };
    let preferred_start = engine.preferences.iter().find_map(|p| match p {
        crate::engine::SoftPreference::StartAfter { minute } => {
            Some(format!("{:02}:{:02}", minute / 60, minute % 60))
        }
        _ => None,
    });

    let overdue = db
        .list()
        .ok()
        .map(|r| {
            r.iter()
                .filter(|t| t.status != "completada" && !t.all_day && t.end_at < now)
                .count()
        })
        .unwrap_or(0);

    serde_json::json!({
        "today": today.format("%Y-%m-%d").to_string(),
        "now_local": chrono::Local::now().format("%H:%M").to_string(),
        "working_hours": working_hours,
        "preferred_start": preferred_start,
        "pending_tasks": tasks,
        "pending_total": total,
        "overdue": overdue,
        "free_hours_next_days": free_days,
        "free_windows_next_days": free_windows,
    })
    .to_string()
}

/// Clasifica las tareas pendientes reales en URGENT/IMPORTANT/NORMAL con
/// reglas deterministas (fechas y prioridad de la BD — la IA no inventa nada):
///
/// - URGENT: vencida (end < ahora) o vence hoy (mismo día de calendario).
/// - IMPORTANT: prioridad alta o vence en los próximos 7 días.
/// - NORMAL: el resto.
///
/// Orden estable: URGENT primero (por vencimiento), luego IMPORTANT, luego
/// NORMAL, cada grupo ordenado por end_at. Tope de 15 para no inflar la
/// respuesta. Solo tareas no completadas.
pub fn task_refs(db: &Db, now: i64) -> Vec<TaskRefView> {
    let today = chrono::Local::now().date_naive();
    const MAX: usize = 15;

    let mut refs: Vec<TaskRefView> = db
        .list()
        .ok()
        .into_iter()
        .flatten()
        .filter(|t| t.status != "completada")
        .map(|t| {
            let end_day = chrono::Local
                .timestamp_millis_opt(t.end_at)
                .earliest()
                .map(|d| d.date_naive())
                .unwrap_or(today);
            let level = if t.end_at < now || end_day == today {
                "URGENT"
            } else if t.priority == "alta" || (end_day - today).num_days() <= 7 {
                "IMPORTANT"
            } else {
                "NORMAL"
            };
            TaskRefView {
                id: t.id,
                title: t.title.clone(),
                cat: t.category_id.clone(),
                priority: t.priority.clone(),
                start_ms: t.start_at,
                end_ms: t.end_at,
                all_day: t.all_day,
                level: level.into(),
            }
        })
        .collect();

    // Deduplica copias idénticas (mismo título normalizado y mismo día de
    // vencimiento): puede haber duplicados activos por altas repetidas; el
    // asistente solo debe ver una. Se conserva la de id menor.
    let mut seen: std::collections::HashMap<(String, chrono::NaiveDate), i64> =
        std::collections::HashMap::new();
    refs.retain(|r| {
        let day = chrono::Local
            .timestamp_millis_opt(r.end_ms)
            .earliest()
            .map(|d| d.date_naive())
            .unwrap_or(today);
        let key = (r.title.trim().to_lowercase(), day);
        match seen.get(&key) {
            Some(prev) if *prev < r.id => false,
            Some(_) => {
                seen.insert(key, r.id);
                true
            }
            None => {
                seen.insert(key, r.id);
                true
            }
        }
    });

    let rank = |l: &str| match l {
        "URGENT" => 0,
        "IMPORTANT" => 1,
        _ => 2,
    };
    refs.sort_by(|a, b| {
        rank(&a.level)
            .cmp(&rank(&b.level))
            .then(a.end_ms.cmp(&b.end_ms))
    });
    refs.truncate(MAX);
    refs
}

fn fmt_history(history: &[HistoryMsg]) -> String {
    if history.is_empty() {
        return "(sin historial)".into();
    }
    history
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|m| format!("{}: {}", m.role, m.text))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_local(date: &str, time: Option<&str>) -> Option<i64> {
    let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    match time {
        Some(t) => {
            let hm: Vec<u32> = t.split(':').filter_map(|x| x.parse().ok()).collect();
            let (h, m) = (hm.first().copied()?, hm.get(1).copied()?);
            Some(crate::engine::local_ms(d.and_hms_opt(h, m, 0)?))
        }
        None => Some(crate::engine::local_ms(d.and_hms_opt(0, 0, 0)?)),
    }
}

/// Resuelve una tarea existente por título normalizado. Devuelve Err con la
/// lista de candidatos si hay ambigüedad.
fn resolve_task(db: &Db, title: &str) -> Result<Option<(i64, String)>, String> {
    let norm = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let target = norm(title);
    if target.is_empty() {
        return Ok(None);
    }
    let rows = db.list().map_err(|e| e.to_string())?;
    let pending: Vec<_> = rows.iter().filter(|t| t.status != "completada").collect();
    // 1) igualdad exacta normalizada
    if let Some(t) = pending.iter().find(|t| norm(&t.title) == target) {
        return Ok(Some((t.id, t.title.clone())));
    }
    // 2) contención (el título del usuario contiene el real o viceversa)
    let fuzzy: Vec<&crate::store::TaskRow> = pending
        .iter()
        .copied()
        .filter(|t| {
            let nt = norm(&t.title);
            nt.contains(&target) || target.contains(&nt)
        })
        .collect();
    match fuzzy.len() {
        0 => Ok(None),
        1 => Ok(Some((fuzzy[0].id, fuzzy[0].title.clone()))),
        n => Err(format!(
            "Me refiero a varias tareas ({}): {}. Sé más específico.",
            n,
            fuzzy
                .iter()
                .map(|t| t.title.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Resuelve la categoría aceptando id o nombre; default "otr".
fn resolve_category(cat: Option<&str>) -> String {
    match cat {
        Some(c) if c.trim().is_empty() => "otr".into(),
        Some(c) => {
            let id = category_id_from_name(c);
            if id == "otr" && !matches!(c, "otr" | "otra" | "otras" | "otros") {
                "otr".into()
            } else {
                id
            }
        }
        None => "otr".into(),
    }
}

/// Construye la acción propuesta desde el JSON de la decisión.
fn build_action(
    db: &Db,
    obj: &serde_json::Value,
    note: &str,
) -> Result<AssistantAction, String> {
    let kind = obj.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let task_title = obj.get("task_title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let date = obj.get("start_date").and_then(|v| v.as_str());
    let time = obj.get("start_time").and_then(|v| v.as_str());
    let duration_min = obj
        .get("duration_minutes")
        .and_then(|v| v.as_u64())
        .unwrap_or(60)
        .clamp(15, 480);
    let priority = obj.get("priority").and_then(|v| v.as_str()).unwrap_or("media").to_string();
    let category_id = resolve_category(obj.get("category").and_then(|v| v.as_str()));
    let summary = if note.trim().is_empty() {
        format!("{}: {}", kind, if !title.is_empty() { &title } else { &task_title })
    } else {
        note.to_string()
    };

    match kind.as_str() {
        "complete" => {
            let (id, real_title) = resolve_task(db, &task_title)?.ok_or_else(|| {
                "No encontré ninguna tarea pendiente con ese nombre. Sé más específico o escríbela tú.".to_string()
            })?;
            Ok(AssistantAction {
                kind,
                task_title: real_title,
                title: String::new(),
                category_id,
                priority,
                start_ms: None,
                end_ms: None,
                all_day: false,
                summary,
                task_id: Some(id),
            })
        }
        "reschedule" => {
            let (id, real_title) = resolve_task(db, &task_title)?.ok_or_else(|| {
                "No encontré ninguna tarea pendiente con ese nombre. Sé más específico.".to_string()
            })?;
            let date = date.ok_or_else(|| "reschedule sin fecha: pide la fecha al usuario".to_string())?;
            let has_time = time.map(|t| !t.is_empty()).unwrap_or(false);
            let start = parse_local(date, if has_time { time } else { None }).ok_or_else(|| "fecha inválida".to_string())?;
            let (end, all_day) = if has_time {
                (start + duration_min as i64 * 60_000, false)
            } else {
                (start + 24 * 3_600_000, true)
            };
            Ok(AssistantAction {
                kind,
                task_title: real_title,
                title: String::new(),
                category_id,
                priority,
                start_ms: Some(start),
                end_ms: Some(end),
                all_day,
                summary,
                task_id: Some(id),
            })
        }
        "create_event" => {
            if title.is_empty() {
                return Err("create_event sin título: no se puede crear".to_string());
            }
            let (start, end, all_day) = match date {
                Some(d) => {
                    let s = parse_local(d, time.filter(|t| !t.is_empty()))
                        .ok_or_else(|| "fecha inválida".to_string())?;
                    if time.map(|t| !t.is_empty()).unwrap_or(false) {
                        (Some(s), Some(s + duration_min as i64 * 60_000), false)
                    } else {
                        (Some(s), Some(s + 24 * 3_600_000), true)
                    }
                }
                None => (None, None, true),
            };
            Ok(AssistantAction {
                kind,
                task_title: String::new(),
                title,
                category_id,
                priority,
                start_ms: start,
                end_ms: end,
                all_day,
                summary,
                task_id: None,
            })
        }
        "cancel_proposal" => Ok(AssistantAction {
            kind,
            task_title: String::new(),
            title: String::new(),
            category_id,
            priority,
            start_ms: None,
            end_ms: None,
            all_day: false,
            summary: "Cancelar la propuesta de plan pendiente".into(),
            task_id: None,
        }),
        other => Err(format!("acción desconocida: {other}")),
    }
}

/// Construye el prompt del usuario a partir del contexto ya obtenido.
/// No toca la base de datos (contexto se pasa como string).
pub fn build_user_prompt(ctx: &str, text: &str, history: &[HistoryMsg]) -> String {
    format!(
        "Hoy es {}.\n\nHistorial reciente:\n{}\n\nContexto del calendario:\n{}\n\nPetición del usuario:\n{}",
        chrono::Local::now().format("%Y-%m-%d %A"),
        fmt_history(history),
        ctx,
        text
    )
}

/// Error distinguible de límite de peticiones: `ia_429 [retry_after] [detalle]`.
/// El detalle (p. ej. FreeUsageLimitError) es técnico: va al log, no al frontend.
pub fn rate_limited_err(retry_after: Option<u64>, detail: String) -> String {
    let wait = retry_after
        .map(|s| format!(" {s}"))
        .unwrap_or_default();
    format!("ia_429{wait} {detail}")
}

/// Primera decisión del asistente: clasifica la petición (solo red).
pub fn request_decision(
    provider: &dyn AiProvider,
    user: &str,
) -> Result<serde_json::Value, String> {
    let decision: AiResult<serde_json::Value> =
        provider.chat_json(DECISION_SYSTEM_PROMPT, user, ASSISTANT_DECISION_SCHEMA);
    match decision {
        Ok(v) => Ok(v),
        Err(AiError::RateLimited { retry_after, detail }) => Err(rate_limited_err(retry_after, detail)),
        Err(AiError::Http(e)) | Err(AiError::NotConfigured(e)) | Err(AiError::BadResponse(e)) => {
            Err(format!("ia_fail {e}"))
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn note_from_decision(decision: &serde_json::Value) -> String {
    decision.get("note").and_then(|n| n.as_str()).unwrap_or("").to_string()
}

/// Modo "answer": respuesta conversacional con contexto (solo red).
/// El usuario envía el MISMO prompt que la decisión (tiene el contexto).
/// `tasks` son referencias estructuradas reales que el frontend resalta.
pub fn answer_text(
    provider: &dyn AiProvider,
    user: &str,
    tasks: Vec<TaskRefView>,
) -> Result<AssistantTurnView, String> {
    let a: AiResult<serde_json::Value> =
        provider.chat_json(ANSWER_SYSTEM_PROMPT, user, r#"{"text":"string"}"#);
    match a {
        Ok(v) => {
            let t = v.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
            if t.trim().is_empty() {
                Ok(AssistantTurnView::Nothing {
                    text: "No pude formular una respuesta. Intenta reformular la pregunta.".into(),
                })
            } else {
                Ok(AssistantTurnView::Answer { text: t, tasks })
            }
        }
        Err(AiError::RateLimited { retry_after, detail }) => Err(rate_limited_err(retry_after, detail)),
        Err(e) => Err(format!("ia_fail {e}")),
    }
}

/// Modo "plan": parsea la intención (red) y genera la propuesta (db).
/// Combinado — para tests. El comando de Tauri separa las fases.
pub fn plan_mode(
    db: &Db,
    text: &str,
    provider: &dyn AiProvider,
    note: String,
) -> Result<AssistantTurnView, String> {
    let batch = parse_intent(text, Some(provider), true).map_err(|e| e.to_string())?;
    plan_from_intents(db, text, &batch.intents, note)
}

/// Genera la propuesta de plan desde intents ya parseados (solo db).
pub fn plan_from_intents(
    db: &Db,
    text: &str,
    intents: &[Intent],
    note: String,
) -> Result<AssistantTurnView, String> {
    let proposal = planning::plan_from_text(db, text, intents, "assistant")?;
    Ok(AssistantTurnView::Plan {
        proposal,
        note: if note.is_empty() {
            "Propuesta de planificación: nada cambia hasta que la aceptes.".into()
        } else {
            note
        },
    })
}

/// Modo "action": resuelve la acción contra la db y la guarda como propuesta.
pub fn action_mode(
    db: &Db,
    decision: &serde_json::Value,
    note: &str,
) -> Result<AssistantTurnView, String> {
    let action_obj = decision.get("action").cloned().unwrap_or(serde_json::json!({}));
    // no se pudo resolver (tarea desconocida/ambigua, datos incompletos)
    // → responder de forma conversacional, sin crear propuestas basura
    let action = match build_action(db, &action_obj, note) {
        Ok(a) => a,
        Err(clarify) => return Ok(AssistantTurnView::Answer { text: clarify, tasks: Vec::new() }),
    };
    let payload = serde_json::to_string(&action).map_err(|e| e.to_string())?;
    let proposal_id = db.insert_assistant_action(&action.kind, &payload).map_err(|e| e.to_string())?;
    Ok(AssistantTurnView::Action {
        action: AssistantActionView {
            proposal_id,
            kind: action.kind,
            task_title: action.task_title,
            title: action.title,
            category_id: action.category_id,
            priority: action.priority,
            start_ms: action.start_ms,
            end_ms: action.end_ms,
            all_day: action.all_day,
            summary: action.summary,
        },
    })
}

/// Un turno del asistente. `configured` = IA real configurada (no local).
/// Sin IA: responde con un mensaje informativo (no crea propuestas basura).
///
/// Nota: combina red + db (el comando de Tauri usa las fases por separado
/// para no retener el lock durante la red; esta función existe para tests).
pub fn assistant_turn(
    db: &Db,
    text: &str,
    history: &[HistoryMsg],
    provider: Option<&dyn AiProvider>,
    configured: bool,
) -> Result<AssistantTurnView, String> {
    if !configured {
        return Ok(AssistantTurnView::Nothing {
            text: "Sin IA configurada no puedo analizar tu calendario ni responder preguntas. Configura la IA en Ajustes → IA, o usa la barra rápida para añadir tareas.".into(),
        });
    }
    let provider = provider.ok_or_else(|| "sin proveedor de IA".to_string())?;
    let ctx = context_snapshot(db);
    let user = build_user_prompt(&ctx, text, history);
    let decision = request_decision(provider, &user)?;
    let note = note_from_decision(&decision);

    let mode = decision.get("mode").and_then(|m| m.as_str()).unwrap_or("answer");
    match mode {
        "plan" => plan_mode(db, text, provider, note),
        "action" => action_mode(db, &decision, &note),
        _ => answer_text(provider, &user, task_refs(db, crate::email::now_ms())),
    }
}

/// Acción aprobada → aplicar SOLO vía los servicios existentes del store.
/// Devuelve un resumen en texto del efecto.
pub fn apply_action(db: &Db, action: &AssistantAction) -> Result<String, String> {
    match action.kind.as_str() {
        "complete" => {
            let id = action.task_id.ok_or_else(|| "tarea no resuelta".to_string())?;
            let t = db.get_task(id).map_err(|e| e.to_string())?.ok_or_else(|| "tarea ya no existe".to_string())?;
            if t.status == "completada" {
                return Ok(format!("'{}' ya estaba completada.", t.title));
            }
            db.set_completed(id, true).map_err(|e| e.to_string())?;
            Ok(format!("Marcada como completada: {}", t.title))
        }
        "reschedule" => {
            let id = action.task_id.ok_or_else(|| "tarea no resuelta".to_string())?;
            let (start, end) = (
                action.start_ms.ok_or_else(|| "reschedule sin fecha".to_string())?,
                action.end_ms.ok_or_else(|| "reschedule sin fin".to_string())?,
            );
            let t = db.get_task(id).map_err(|e| e.to_string())?.ok_or_else(|| "tarea ya no existe".to_string())?;
            if let Some((_, other)) = db.find_overlap(id, start, end).map_err(|e| e.to_string())? {
                return Err(format!("'{}' se solapa con '{}' en ese horario.", t.title, other));
            }
            db.move_to(id, start, end, Some(action.all_day)).map_err(|e| e.to_string())?;
            Ok(format!(
                "Reagendada '{}': {}",
                t.title,
                crate::engine::planner::fmt_session(start, end).0
            ))
        }
        "create_event" => {
            let title = if action.title.is_empty() { "Nuevo evento".to_string() } else { action.title.clone() };
            let (start, end) = (
                action.start_ms.unwrap_or_else(crate::email::now_ms),
                action.end_ms.unwrap_or_else(crate::email::now_ms),
            );
            let t = db
                .create(&title, &action.category_id, &action.priority, start, end, action.all_day)
                .map_err(|e| e.to_string())?;
            Ok(format!("Creado '{}' en el calendario.", t.title))
        }
        "cancel_proposal" => {
            let pending = db.list_plan_proposals(true).map_err(|e| e.to_string())?;
            let p = pending.into_iter().next().ok_or_else(|| "no hay ninguna propuesta pendiente que cancelar".to_string())?;
            planning::reject_plan(db, p.id)?;
            Ok("Propuesta de plan cancelada.".into())
        }
        other => Err(format!("acción desconocida: {other}")),
    }
}

/// Carga una acción guardada desde la BD.
pub fn get_action(db: &Db, id: i64) -> Result<Option<(i64, AssistantAction)>, String> {
    let Some(row) = db.get_assistant_action(id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let action: AssistantAction = serde_json::from_str(&row.payload).map_err(|e| e.to_string())?;
    Ok(Some((row.id, action)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    struct QueueProvider(Mutex<Vec<serde_json::Value>>);
    impl QueueProvider {
        fn new(v: Vec<serde_json::Value>) -> Self {
            QueueProvider(Mutex::new(v))
        }
    }
    impl AiProvider for QueueProvider {
        fn id(&self) -> &str {
            "queue"
        }
        fn chat_json(&self, _s: &str, _u: &str, _schema: &str) -> AiResult<serde_json::Value> {
            let mut q = self.0.lock().unwrap();
            if q.len() == 1 {
                return Ok(q[0].clone());
            }
            Ok(q.remove(0))
        }
    }

    fn db() -> Db {
        Db::open_memory_pub().unwrap()
    }

    fn action(kind: &str, task_id: Option<i64>, task_title: &str, start: Option<i64>) -> AssistantAction {
        AssistantAction {
            kind: kind.into(),
            task_title: task_title.into(),
            title: String::new(),
            category_id: "uni".into(),
            priority: "media".into(),
            start_ms: start,
            end_ms: start.map(|s| s + 3_600_000),
            all_day: false,
            summary: "test".into(),
            task_id,
        }
    }

    #[test]
    fn no_ai_returns_nothing_without_writing() {
        let d = db();
        let before = d.list_plan_proposals(false).unwrap().len();
        let t = assistant_turn(&d, "organiza mi semana", &[], None, false).unwrap();
        match t {
            AssistantTurnView::Nothing { text } => assert!(text.contains("Sin IA configurada")),
            other => panic!("esperado Nothing, got {other:?}"),
        }
        assert_eq!(d.list_plan_proposals(false).unwrap().len(), before, "no escribe nada");
    }

    #[test]
    fn answer_mode_uses_second_call_with_text() {
        let p = QueueProvider::new(vec![
            json!({"mode": "answer", "note": null}),
            json!({"text": "Hoy tienes 2 horas libres por la mañana."}),
        ]);
        let t = assistant_turn(&db(), "¿tengo tiempo hoy?", &[], Some(&p), true).unwrap();
        match t {
            AssistantTurnView::Answer { text, .. } => assert!(text.contains("2 horas"), "{text}"),
            other => panic!("esperado Answer, got {other:?}"),
        }
    }

    #[test]
    fn plan_mode_reuses_planning_pipeline() {
        let p = QueueProvider::new(vec![
            json!({"mode": "plan", "note": "te planifico el estudio"}),
            json!({"intents": [{"intent_type": "task", "title": "Estudiar cálculo", "duration_minutes": 120, "confidence": 0.9}]}),
        ]);
        let d = db();
        let t = assistant_turn(&d, "planifica 2 horas de cálculo", &[], Some(&p), true).unwrap();
        match t {
            AssistantTurnView::Plan { proposal, note } => {
                assert_eq!(proposal.status, "pending");
                assert!(note.contains("nada cambia") || note.contains("te planifico"), "{note}");
            }
            other => panic!("esperado Plan, got {other:?}"),
        }
    }

    #[test]
    fn action_complete_requires_approval_then_applies() {
        let d = db();
        let t = d.create("Informe final", "tra", "alta", crate::email::now_ms(), crate::email::now_ms() + 3_600_000, false).unwrap();
        let p = QueueProvider::new(vec![json!({
            "mode": "action",
            "note": "marcaré el informe como hecho",
            "action": {"kind": "complete", "task_title": "Informe final"}
        })]);
        let turn = assistant_turn(&d, "marca el informe como hecho", &[], Some(&p), true).unwrap();
        let proposal_id = match turn {
            AssistantTurnView::Action { action } => action.proposal_id,
            other => panic!("esperado Action, got {other:?}"),
        };
        // nada ha cambiado todavía
        let task = d.get_task(t.id).unwrap().unwrap();
        assert_ne!(task.status, "completada", "sin aprobar no muta");
        let row = d.get_assistant_action(proposal_id).unwrap().unwrap();
        assert_eq!(row.status, "pending");
        let (_, a) = get_action(&d, proposal_id).unwrap().unwrap();
        assert_eq!(a.task_id, Some(t.id));
        let summary = apply_action(&d, &a).unwrap();
        assert!(summary.contains("Informe final"), "{summary}");
        assert_eq!(d.get_task(t.id).unwrap().unwrap().status, "completada", "aprobada aplica");
    }

    #[test]
    fn unknown_task_title_returns_clarifying_nothing() {
        let d = db();
        d.create("Estudiar física", "uni", "media", crate::email::now_ms(), crate::email::now_ms() + 3_600_000, false).unwrap();
        let p = QueueProvider::new(vec![json!({
            "mode": "action",
            "note": null,
            "action": {"kind": "complete", "task_title": "No existe tal tarea"}
        })]);
        let turn = assistant_turn(&d, "marca eso como hecho", &[], Some(&p), true).unwrap();
        match turn {
            AssistantTurnView::Answer { text, .. } | AssistantTurnView::Nothing { text } => {
                assert!(text.contains("No encontré"), "{text}")
            }
            other => panic!("esperado aclaración, got {other:?}"),
        }
    }

    #[test]
    fn ambiguous_title_lists_candidates() {
        let d = db();
        d.create("Informe A", "tra", "media", crate::email::now_ms(), crate::email::now_ms() + 3_600_000, false).unwrap();
        d.create("Informe B", "tra", "media", crate::email::now_ms(), crate::email::now_ms() + 3_600_000, false).unwrap();
        let p = QueueProvider::new(vec![json!({
            "mode": "action",
            "note": null,
            "action": {"kind": "complete", "task_title": "Informe"}
        })]);
        let turn = assistant_turn(&d, "marca informe como hecho", &[], Some(&p), true).unwrap();
        match turn {
            AssistantTurnView::Answer { text, .. } | AssistantTurnView::Nothing { text } => {
                assert!(text.contains("varias tareas"), "{text}");
                assert!(text.contains("Informe A") && text.contains("Informe B"), "{text}");
            }
            other => panic!("esperado aclaración, got {other:?}"),
        }
    }

    #[test]
    fn apply_reschedule_moves_via_service() {
        let d = db();
        let t = d.create("Estudiar", "uni", "media", crate::email::now_ms(), crate::email::now_ms() + 3_600_000, false).unwrap();
        let tomorrow = crate::engine::local_ms(
            (chrono::Local::now().date_naive() + chrono::Duration::days(1)).and_hms_opt(9, 0, 0).unwrap(),
        );
        let a = action("reschedule", Some(t.id), "Estudiar", Some(tomorrow));
        let summary = apply_action(&d, &a).unwrap();
        assert!(summary.contains("Estudiar"), "{summary}");
        let moved = d.get_task(t.id).unwrap().unwrap();
        assert_eq!(moved.start_at, tomorrow);
    }

    #[test]
    fn apply_create_event_creates_all_day_without_time() {
        let d = db();
        let a = AssistantAction {
            kind: "create_event".into(),
            title: "Reunión con Juan".into(),
            category_id: "tra".into(),
            priority: "alta".into(),
            start_ms: Some(crate::engine::local_ms(
                (chrono::Local::now().date_naive() + chrono::Duration::days(2)).and_hms_opt(0, 0, 0).unwrap(),
            )),
            end_ms: Some(crate::engine::local_ms(
                (chrono::Local::now().date_naive() + chrono::Duration::days(3)).and_hms_opt(0, 0, 0).unwrap(),
            )),
            all_day: true,
            ..action("create_event", None, "", None)
        };
        let summary = apply_action(&d, &a).unwrap();
        assert!(summary.contains("Reunión con Juan"), "{summary}");
        let rows = d.list().unwrap();
        let t = rows.iter().find(|t| t.title == "Reunión con Juan").unwrap();
        assert!(t.all_day, "todo el día");
    }

    #[test]
    fn apply_cancel_proposal_rejects_latest_pending() {
        let d = db();
        let intents = vec![crate::ai::intent::Intent {
            intent_type: crate::ai::intent::IntentType::Task,
            title: "Estudiar".into(),
            description: String::new(),
            category_id: "uni".into(),
            priority: crate::ai::intent::Priority::Media,
            window: crate::ai::intent::TimeWindow { start: None, end: None, all_day: false },
            duration: Some(crate::ai::intent::Duration { minutes: 60 }),
            deadline: None,
            preparation: None,
            recurrence: None,
            reminders: Vec::new(),
            constraints: Vec::new(),
            confidence: 0.9,
            reason: "test".into(),
            source: "local".into(),
        }];
        let plan = crate::planning::plan_from_text(&d, "estudiar", &intents, "assistant").unwrap();
        let a = action("cancel_proposal", None, "", None);
        let summary = apply_action(&d, &a).unwrap();
        assert!(summary.contains("cancelada"), "{summary}");
        let row = d.get_plan_proposal(plan.id).unwrap().unwrap();
        assert_eq!(row.status, "rejected");
    }

    #[test]
    fn context_snapshot_is_minimal_no_descriptions() {
        let d = db();
        let t = d.create("Estudiar cálculo", "uni", "alta", crate::email::now_ms(), crate::email::now_ms() + 3_600_000, false).unwrap();
        d.update_task_full(t.id, "Estudiar cálculo", "uni", "alta", t.start_at, t.end_at,
            "descripción secreta que no debe filtrarse", "[]", "", "", None, Some(false)).unwrap();
        let ctx = context_snapshot(&d);
        assert!(ctx.contains("Estudiar cálculo"), "título sí va");
        assert!(!ctx.contains("secreta"), "descripción NO va");
    }

    #[test]
    fn context_snapshot_has_real_windows_and_working_hours() {
        // La tarea del test anterior es un compromiso del motor real → las
        // ventanas libres de hoy se derivan de ella; el horario laboral sale
        // del motor (default 06:00–22:00), no de una constante.
        let d = Db::open_memory_clean_pub().unwrap();
        d.create("Clase 10:00", "uni", "media", day_start(0) + 10 * 3_600_000, day_start(0) + 12 * 3_600_000, false).unwrap();
        let ctx = context_snapshot(&d);
        assert!(ctx.contains("free_windows_next_days"), "ventanas libres concretas");
        assert!(ctx.contains("free_hours_next_days"), "agregado por día sigue");
        assert!(ctx.contains("06:00–22:00"), "working_hours real del motor");
        assert!(ctx.contains("\"preferred_start\":null") || ctx.contains("preferred_start"), "campo esté presente");
    }

    fn day_start(days_from_today: i64) -> i64 {
        crate::engine::local_ms(
            (chrono::Local::now().date_naive() + chrono::Duration::days(days_from_today))
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        )
    }

    #[test]
    fn task_refs_classify_urgent_important_normal() {
        let d = Db::open_memory_clean_pub().unwrap();
        // vencida → URGENT
        d.create("Entrega pasada", "uni", "media", day_start(-2), day_start(-1), false).unwrap();
        // vence hoy → URGENT
        d.create("Entrega hoy", "uni", "media", day_start(0), day_start(0) + 3_600_000, false).unwrap();
        // prioridad alta futura → IMPORTANT
        d.create("Parcial", "uni", "alta", day_start(5), day_start(5) + 3_600_000, false).unwrap();
        // vence en 7 días → IMPORTANT
        d.create("Informe 7d", "tra", "media", day_start(7), day_start(7) + 3_600_000, false).unwrap();
        // lejana y baja → NORMAL
        d.create("Leer libro", "per", "baja", day_start(20), day_start(20) + 3_600_000, false).unwrap();
        // completada → excluida
        let done = d.create("Hecha", "tra", "alta", day_start(-1), day_start(-1) + 3_600_000, false).unwrap();
        d.set_completed(done.id, true).unwrap();

        let refs = task_refs(&d, crate::email::now_ms());
        let levels: Vec<&str> = refs.iter().map(|r| r.level.as_str()).collect();
        assert_eq!(levels[..3], ["URGENT", "URGENT", "IMPORTANT"], "{levels:?}");
        assert_eq!(levels[3], "IMPORTANT", "{levels:?}");
        assert_eq!(levels[4], "NORMAL", "{levels:?}");
        assert_eq!(refs.len(), 5, "la completada no aparece: {levels:?}");
        assert!(!refs.iter().any(|r| r.title == "Hecha"));
    }

    #[test]
    fn task_refs_never_invent_and_stay_under_cap() {
        let d = Db::open_memory_clean_pub().unwrap();
        for i in 0..40 {
            d.create(
                &format!("Tarea {i}"),
                "uni",
                "media",
                day_start((i % 30) as i64 + 1),
                day_start((i % 30) as i64 + 1) + 3_600_000,
                false,
            )
            .unwrap();
        }
        let refs = task_refs(&d, crate::email::now_ms());
        assert!(refs.len() <= 15, "topado a 15: {}", refs.len());
        for r in &refs {
            let t = d.get_task(r.id).unwrap().unwrap();
            assert_eq!(r.title, t.title, "referencia real, no inventada");
        }
    }

    #[test]
    fn task_refs_dedupes_identical_copies() {
        let d = Db::open_memory_clean_pub().unwrap();
        // 5 copias idénticas (mismo título y mismo día de vencimiento), como
        // las que quedaron por altas repetidas: el asistente solo ve una.
        for _ in 0..5 {
            d.create("Reunion", "uni", "media", day_start(3), day_start(3) + 3_600_000, false)
                .unwrap();
        }
        // mismo título pero otro día: NO es duplicado
        d.create("Reunion", "uni", "media", day_start(5), day_start(5) + 3_600_000, false)
            .unwrap();
        // distinto título, mismo día: NO es duplicado
        d.create("Otra reunion", "uni", "media", day_start(3), day_start(3) + 7_200_000, false)
            .unwrap();

        let refs = task_refs(&d, crate::email::now_ms());
        assert_eq!(refs.len(), 3, "una copia por duplicado: {}", refs.len());
        let first = refs.iter().find(|r| r.title == "Reunion" && r.end_ms == day_start(3) + 3_600_000).unwrap();
        assert_eq!(first.id, 1, "se conserva la copia de id menor");
    }
}
