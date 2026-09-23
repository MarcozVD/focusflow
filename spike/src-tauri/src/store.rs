use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[derive(Serialize, Clone, Debug)]
pub struct TaskRow {
    pub id: i64,
    pub title: String,
    pub category_id: String,
    pub priority: String,
    pub status: String,
    pub start_at: i64,
    pub end_at: i64,
    pub all_day: bool,
    pub progress: i64,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub description: String,
    pub tags: String,
    pub notes: String,
    pub links: String,
    pub reminder_minutes: Option<i64>,
    pub reminder_fired_at: Option<i64>,
    pub metadata: String,
}

fn task_from_row(r: &rusqlite::Row) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: r.get("id")?,
        title: r.get("title")?,
        category_id: r.get("category_id")?,
        priority: r.get("priority")?,
        status: r.get("status")?,
        start_at: r.get("start_at")?,
        end_at: r.get("end_at")?,
        all_day: r.get::<_, i64>("all_day")? != 0,
        progress: r.get("progress")?,
        completed_at: r.get("completed_at")?,
        created_at: r.get("created_at")?,
        description: r.get("description")?,
        tags: r.get("tags")?,
        notes: r.get("notes")?,
        links: r.get("links")?,
        reminder_minutes: r.get("reminder_minutes")?,
        reminder_fired_at: r.get("reminder_fired_at")?,
        metadata: r.get("metadata")?,
    })
}

/// Prefijos de settings que SÍ se exportan con "mis datos". Todo lo demás
/// (config de correo, endpoint/modelo/clave de IA, flags internos) queda
/// fuera: es configuración sensible aunque no contenga secretos.
const EXPORTABLE_SETTINGS: &[&str] = &[
    "email.enabled",
    "email.interval",
    "email.max_age",
    "email.suggestion",
    "general.",
    "ui.",
    "notif.",
    "onboarding.",
];

/// Categorías válidas de la app. Cualquier otro id se normaliza a "otr":
/// evita que datos inválidos (IPC arbitrario, import sucio) rompan el
/// frontend o los filtros por categoría.
const VALID_CATEGORIES: &[&str] = &["uni", "trab", "per", "fin", "sal", "otr"];

fn sanitize_category(id: &str) -> &str {
    if VALID_CATEGORIES.contains(&id) {
        id
    } else {
        "otr"
    }
}

/// Prioridad válida whitelist (mismo criterio que el CHECK de la tabla).
/// Una prioridad inválida desde IPC/IA cae a "media" en vez de provocar un
/// error de restricción opaco para el usuario (bug L7).
fn sanitize_priority(p: &str) -> &str {
    match p {
        "alta" | "media" | "baja" => p,
        _ => "media",
    }
}

/// Normaliza el span de un marcador TODO EL DÍA: un all-day sin duración
/// cubre [inicio, inicio + 24 h). Si se guarda con end <= start, `coversDay`
/// del frontend exige end > dayStart y la tarea queda invisible en
/// mes/día/semana (bug M1, mismo síntoma que la regresión all-day de b0f894b).
///
/// IMPORTANTE: con hora (all_day=false) end == start es un modelo de datos
/// INTENCIONAL — "tarea flexible sin horario fijo" que el backlog de
/// "organiza mi semana/día" consume (`flexible_backlog` filtra
/// end_at <= start_at). NO se toca ese caso aquí ni en `create`.
pub(crate) fn normalize_task_span(start: i64, end: i64, all_day: bool) -> (i64, i64) {
    if all_day && end <= start {
        (start, start + crate::engine::DAY_MS)
    } else {
        (start, end)
    }
}

/// Recordatorio máximo: 4 semanas en minutos. Clampa entradas de IPC/IA para
/// evitar valores negativos (disparo tras el inicio) u overflow en
/// `start_at - reminder_minutes * 60000` (auditoría 17, hallazgo #13).
const MAX_REMINDER_MINUTES: i64 = 4 * 7 * 24 * 60;

fn sanitize_reminder(minutes: Option<i64>) -> Option<i64> {
    minutes.map(|m| m.clamp(0, MAX_REMINDER_MINUTES))
}

pub struct Db {
    conn: Connection,
}

/// Lock con recuperación de poison: un pánico previo envenena el mutex y, con
/// `panic = "abort"` en release, cada `.lock().unwrap()` posterior abortaría
/// el proceso entero (auditoría 17, hallazgo #6). El estado interno sigue
/// siendo usable en la práctica; abortar la app es peor que continuar.
pub fn lock_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Recordatorio por disparar (ventana vencida y sin marca de disparo).
#[derive(Debug, Clone)]
pub struct DueReminder {
    pub task_id: i64,
    pub title: String,
    pub start_at: i64,
    pub end_at: i64,
    pub all_day: bool,
    pub reminder_minutes: i64,
}

/// Sesión de Google OAuth persistida en `auth_sessions` (CAMBIO 2).
#[derive(Debug, Clone, Default)]
pub struct AuthSession {
    pub user_id: String,
    pub email: String,
    pub name: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

/// Clase del horario semanal: bloque de tiempo fijo con vigencia
/// (independiente de las tareas: sin estado, sin completado, sin estadísticas).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ClassRow {
    pub id: i64,
    pub title: String,
    /// 0 = lunes … 6 = domingo.
    pub day_of_week: i64,
    /// Minutos desde medianoche local.
    pub start_min: i64,
    pub end_min: i64,
    /// Día ÉPOCA (medianoche local) de inicio de la vigencia, inclusivo.
    pub start_date: i64,
    /// Día ÉPOCA (medianoche local) de fin de la vigencia, inclusivo.
    pub end_date: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

fn class_from_row(r: &rusqlite::Row) -> rusqlite::Result<ClassRow> {
    Ok(ClassRow {
        id: r.get("id")?,
        title: r.get("title")?,
        day_of_week: r.get("day_of_week")?,
        start_min: r.get("start_min")?,
        end_min: r.get("end_min")?,
        start_date: r.get("start_date")?,
        end_date: r.get("end_date")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

/// Sesión de estudio: bloque de tiempo RESERVADO (planificado y movible) para
/// estudiar o trabajar sobre un tema/tarea. Independiente de tareas y clases:
/// no tiene estado, no es pendiente, no cuenta en estadísticas. `task_id` es
/// una relación OPCIONAL (la tarea sigue viviendo por su cuenta).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct StudyRow {
    pub id: i64,
    pub title: String,
    /// Inicio del bloque (ms ÉPOCA local).
    pub start_at: i64,
    /// Fin del bloque (ms ÉPOCA local), estrictamente posterior al inicio.
    pub end_at: i64,
    /// Tarea relacionada (opcional): `None` = sesión libre de un tema.
    pub task_id: Option<i64>,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn study_from_row(r: &rusqlite::Row) -> rusqlite::Result<StudyRow> {
    Ok(StudyRow {
        id: r.get("id")?,
        title: r.get("title")?,
        start_at: r.get("start_at")?,
        end_at: r.get("end_at")?,
        task_id: r.get("task_id")?,
        notes: r.get("notes")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

/// Normaliza y valida una sesión de estudio antes de tocar la BD. Devuelve
/// `Err(texto)` con el motivo legible (título obligatorio, fecha válida y
/// duración > 0 — es decir, fin posterior al inicio).
pub(crate) fn validate_study(title: &str, start_at: i64, end_at: i64) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("El título de la sesión es obligatorio".into());
    }
    if start_at <= 0 || end_at <= 0 {
        return Err("La fecha de la sesión no es válida".into());
    }
    if end_at <= start_at {
        return Err("La hora de finalización debe ser posterior a la de inicio".into());
    }
    Ok(())
}

/// Envuelve un mensaje legible como error de rusqlite para que viaje intacto
/// hasta el frontend (mismo patrón que las clases).
fn study_err(msg: String) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(msg)))
}

pub(crate) fn validate_class(
    title: &str,
    day_of_week: i64,
    start_min: i64,
    end_min: i64,
    start_date: i64,
    end_date: i64,
) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("El nombre de la clase es obligatorio".into());
    }
    if !(0..=6).contains(&day_of_week) {
        return Err("Día de la semana inválido".into());
    }
    if end_min <= start_min {
        return Err("La hora de finalización debe ser posterior a la de inicio".into());
    }
    if !(0..1440).contains(&start_min) || end_min > 1440 {
        return Err("Horario fuera del rango del día".into());
    }
    if end_date < start_date {
        return Err("La fecha de finalización debe ser igual o posterior a la de inicio".into());
    }
    Ok(())
}

/// Resultado devuelto al frontend tras una importación: cuántos registros
/// se insertaron en cada tabla (los que ya existían se ignoran por id).
#[derive(serde::Serialize, Default)]
pub struct ImportSummary {
    pub tasks: usize,
    pub suggestions: usize,
    pub classes: usize,
    pub study_sessions: usize,
    pub trusted_senders: usize,
    pub settings: usize,
}

impl Db {
    pub fn open(data_dir: &PathBuf) -> rusqlite::Result<Self> {
        Self::open_with(data_dir, true)
    }

    /// Como `open` pero SIN copia de seguridad: el CLI `ff` abre la BD en
    /// cada comando y rotaría los `.bak` en segundos, perdiendo la copia
    /// buena del último arranque de la app.
    pub fn open_no_backup(data_dir: &PathBuf) -> rusqlite::Result<Self> {
        Self::open_with(data_dir, false)
    }

    fn open_with(data_dir: &PathBuf, backup: bool) -> rusqlite::Result<Self> {
        std::fs::create_dir_all(data_dir).ok();
        let conn = Connection::open(data_dir.join("focusflow.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 2000)?;
        let db = Db { conn };
        db.migrate()?;
        if backup {
            db.backup_rotating(data_dir);
        }
        // datos de demostración SOLO en builds de desarrollo: una app real
        // nunca mezcla datos falsos con los del usuario
        #[cfg(debug_assertions)]
        db.seed_if_empty()?;
        Ok(db)
    }

    /// Copia de seguridad rotativa al arrancar: mantiene dos generaciones
    /// (`focusflow.db.bak`, `.bak.1`) junto a la DB. Sin esto, un SQLite
    /// corrupto = pérdida total (auditoría 17, hallazgo #2). Nunca impide
    /// el arranque: un error de backup se ignora silenciosamente.
    /// `VACUUM INTO` falla si el destino existe: antes el `.bak` quedaba
    /// congelado en la primera copia. Ahora se vuelca a un temporal y, solo
    /// si salió bien, se rota `.bak` → `.bak.1` y el temporal pasa a `.bak`.
    fn backup_rotating(&self, data_dir: &PathBuf) {
        if !data_dir.join("focusflow.db").exists() {
            return;
        }
        let bak = data_dir.join("focusflow.db.bak");
        let bak1 = data_dir.join("focusflow.db.bak.1");
        let tmp = data_dir.join("focusflow.db.bak.tmp");
        let _ = std::fs::remove_file(&tmp);
        // VACUUM INTO: copia consistente aunque haya WAL sin checkpoint.
        let path = tmp.display().to_string().replace('\'', "''");
        if self
            .conn
            .execute_batch(&format!("VACUUM INTO '{path}'"))
            .is_err()
        {
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        if bak.exists() {
            let _ = std::fs::remove_file(&bak1);
            let _ = std::fs::rename(&bak, &bak1);
        }
        if std::fs::rename(&tmp, &bak).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }

    #[cfg(test)]
    fn open_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "busy_timeout", 2000)?;
        let db = Db { conn };
        db.migrate()?;
        db.seed_if_empty()?;
        Ok(db)
    }

    /// Base de datos efímera para pruebas de integración (tests/).
    #[doc(hidden)]
    pub fn open_memory_pub() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "busy_timeout", 2000)?;
        let db = Db { conn };
        db.migrate()?;
        db.seed_if_empty()?;
        Ok(db)
    }

    /// Base efímera SIN datos de demostración (para tests deterministas).
    #[doc(hidden)]
    pub fn open_memory_clean_pub() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "busy_timeout", 2000)?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Versión del esquema = nº de migraciones. `PRAGMA user_version` ancla
    /// qué migraciones ya corrieron: los guards por columna no distinguen
    /// "migración aplicada" de "columna preexistente", y una futura migración
    /// con transformación de datos necesita ese punto de anclaje
    /// (auditoría 17, hallazgo #7).
    const SCHEMA_VERSION: i64 = 14;

    fn migrate(&self) -> rusqlite::Result<()> {
        let v: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if v < 1 {
            self.migrate_0001()?;
        }
        if v < 2 {
            self.migrate_0002()?;
        }
        if v < 3 {
            self.migrate_0003()?;
        }
        if v < 4 {
            self.migrate_0004()?;
        }
        if v < 5 {
            self.migrate_0005()?;
        }
        if v < 6 {
            self.migrate_0006()?;
        }
        if v < 7 {
            self.migrate_0007()?;
        }
        if v < 8 {
            self.migrate_0008()?;
        }
        if v < 9 {
            self.migrate_0009()?;
        }
        if v < 10 {
            self.migrate_0010()?;
        }
        if v < 11 {
            self.migrate_0011()?;
        }
        if v < 12 {
            self.migrate_0012()?;
        }
        if v < 13 {
            self.migrate_0013()?;
        }
        if v < 14 {
            self.migrate_0014()?;
        }
        self.conn
            .pragma_update(None, "user_version", Self::SCHEMA_VERSION)?;
        Ok(())
    }

    /// Trazabilidad sugerencia → TODAS las tareas creadas: aceptar una
    /// sugerencia de rango multi-día crea varios bloques (inicio +
    /// "(entrega)") pero `result_task_id` solo apunta al primero; revertir
    /// o borrar dejaba los demás huérfanos en el calendario (bug A1).
    /// Se rellena con el vínculo único existente para datos viejos.
    fn migrate_0014(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS suggestion_tasks (
                suggestion_id INTEGER NOT NULL,
                task_id       INTEGER NOT NULL,
                PRIMARY KEY (suggestion_id, task_id)
            );
            INSERT OR IGNORE INTO suggestion_tasks (suggestion_id, task_id)
                SELECT id, result_task_id FROM suggested_events
                WHERE result_task_id IS NOT NULL AND result_task_id != 0;",
        )?;
        Ok(())
    }

    /// Sesiones de estudio (bloques de tiempo RESERVADO para estudiar/trabajar
    /// en un tema o tarea). Entidad propia e independiente de `tasks` y de
    /// `classes`: no tiene estado de completado, no cuenta como pendiente y no
    /// entra en contadores ni estadísticas. `task_id` es la relación OPCIONAL
    /// con una tarea (la tarea sigue existiendo por sí sola).
    fn migrate_0013(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS study_sessions (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                title      TEXT NOT NULL CHECK (length(trim(title)) > 0),
                start_at   INTEGER NOT NULL,
                end_at     INTEGER NOT NULL CHECK (end_at > start_at),
                task_id    INTEGER,
                notes      TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_study_start ON study_sessions(start_at);
            CREATE INDEX IF NOT EXISTS idx_study_task ON study_sessions(task_id);",
        )?;
        Ok(())
    }

    /// Clases del horario (bloques recurrentes con vigencia, independientes
    /// de las tareas): día de la semana + franja horaria + periodo académico.
    /// `day_of_week`: 0 = lunes … 6 = domingo (mismo criterio que el
    /// frontend: `(getDay() + 6) % 7`). `start_min`/`end_min`: minutos desde
    /// medianoche local. `start_date`/`end_date`: días ÉPOCA (medianoche
    /// local) que delimitan la vigencia (ambos inclusivos).
    fn migrate_0012(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS classes (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL CHECK (length(trim(title)) > 0),
                day_of_week INTEGER NOT NULL CHECK (day_of_week BETWEEN 0 AND 6),
                start_min   INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
                end_min     INTEGER NOT NULL CHECK (end_min BETWEEN 1 AND 1440 AND end_min > start_min),
                start_date  INTEGER NOT NULL,
                end_date    INTEGER NOT NULL CHECK (end_date >= start_date),
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_classes_day ON classes(day_of_week);
            CREATE INDEX IF NOT EXISTS idx_classes_dates ON classes(start_date, end_date);",
        )?;
        Ok(())
    }

    /// Registro de correos ya revisados (fase 8, dedupe por correo): una fila
    /// por Message-ID procesado. A diferencia del conteo de sugerencias vivas
    /// (el método anterior), esta marca NO desaparece cuando el usuario
    /// rechaza/borra sugerencias o cuando la retención archiva la fila: el
    /// correo no vuelve a enviarse a la IA en cada sync.
    fn migrate_0011(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS email_seen (
                message_id TEXT PRIMARY KEY,
                seen_at    INTEGER NOT NULL
            )",
        )?;
        // backfill de instalación existente: todo Message-ID que ya generó
        // sugerencias cuenta como revisado (las sugerencias no vencidas se
        // mantienen; las archivadas no se re-analizan nunca más).
        self.email_seen_backfill()?;
        self.conn
            .execute_batch("CREATE INDEX IF NOT EXISTS idx_email_seen_at ON email_seen(seen_at)")?;
        Ok(())
    }

    /// Sesión de Google OAuth (CAMBIO 2): tokens de acceso/refresco y perfil
    /// del usuario. Se guardan en la DB local (SQLite del usuario), NO en
    /// Credential Manager. Una sola fila por instalación (id = 1).
    fn migrate_0010(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS auth_sessions (
                id            INTEGER PRIMARY KEY CHECK (id = 1),
                user_id       TEXT NOT NULL DEFAULT '',
                email         TEXT NOT NULL DEFAULT '',
                name          TEXT NOT NULL DEFAULT '',
                access_token  TEXT NOT NULL DEFAULT '',
                refresh_token TEXT NOT NULL DEFAULT '',
                expires_at    INTEGER NOT NULL DEFAULT 0,
                created_at    INTEGER NOT NULL,
                updated_at    INTEGER NOT NULL
            )",
        )?;
        Ok(())
    }

    /// Notificaciones contextuales (fase 11): registro de disparos para
    /// deduplicar (sin spam), respetar cadencia diaria y recordar las
    /// decisiones del usuario (dismiss/más tarde).
    fn migrate_0009(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notification_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                task_id INTEGER NOT NULL,
                fired_at INTEGER NOT NULL,
                payload TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'shown'
            )",
        )?;
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_notif_log_lookup ON notification_log(kind, task_id)",
        )?;
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_notif_log_fired ON notification_log(fired_at)",
        )?;
        Ok(())
    }

    /// Inteligencia de correo (fase 8): tipo de compromiso por sugerencia
    /// (event | deadline | availability | task), vencimiento, preparación y
    /// asunto de origen para deduplicación entre correos.
    fn migrate_0007(&self) -> rusqlite::Result<()> {
        let cols: Vec<String> = self
            .conn
            .prepare("SELECT name FROM pragma_table_info('suggested_events')")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let add = |name: &str, ddl: &str| -> rusqlite::Result<()> {
            if !cols.iter().any(|c| c == name) {
                self.conn
                    .execute_batch(&format!("ALTER TABLE suggested_events ADD COLUMN {ddl}"))?;
            }
            Ok(())
        };
        add("kind", "kind TEXT NOT NULL DEFAULT 'event' CHECK (kind IN ('event','deadline','availability','task'))")?;
        add("deadline_at", "deadline_at INTEGER")?;
        add("prep_min", "prep_min INTEGER NOT NULL DEFAULT 0")?;
        add("source_subject", "source_subject TEXT NOT NULL DEFAULT ''")?;
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_suggestions_kind ON suggested_events(kind)",
        )?;
        Ok(())
    }

    /// Asistente (fase 9): propuestas de acción pendientes de aprobación.
    /// El asistente jamás muta el calendario directamente: crea una propuesta
    /// aquí y el usuario la aprueba (o la rechaza).
    fn migrate_0008(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS assistant_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
        )?;
        Ok(())
    }

    /// Propuestas de planificación (fase 7): texto → intents → plan. El
    /// payload guarda la propuesta completa en JSON; el estado va de
    /// `pending` → `accepted` | `rejected`. Aceptar crea las tareas reales.
    fn migrate_0006(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS plan_proposals (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                text       TEXT NOT NULL,
                status     TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','accepted','rejected')),
                payload    TEXT NOT NULL DEFAULT '{}',
                source     TEXT NOT NULL DEFAULT 'local',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_plan_proposals_status ON plan_proposals(status);
            ",
        )
    }

    /// Marca de disparo del recordatorio (null = sin disparar; al cambiar el
    /// recordatorio se rearma automáticamente).
    fn migrate_0005(&self) -> rusqlite::Result<()> {
        let cols: Vec<String> = self
            .conn
            .prepare("SELECT name FROM pragma_table_info('tasks')")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        if !cols.iter().any(|c| c == "reminder_fired_at") {
            self.conn
                .execute_batch("ALTER TABLE tasks ADD COLUMN reminder_fired_at INTEGER")?;
        }
        Ok(())
    }

    /// result_task_id: tarea creada al aceptar una sugerencia (para revertir/editar enlazado)
    fn migrate_0003(&self) -> rusqlite::Result<()> {
        let has: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('suggested_events') WHERE name = 'result_task_id'",
            [],
            |r| r.get(0),
        )?;
        if has == 0 {
            self.conn
                .execute_batch("ALTER TABLE suggested_events ADD COLUMN result_task_id INTEGER")?;
        }
        Ok(())
    }

    /// Columnas de detalle de tareas: descripción, etiquetas, notas, enlaces, recordatorio.
    fn migrate_0004(&self) -> rusqlite::Result<()> {
        let cols: Vec<String> = self
            .conn
            .prepare("SELECT name FROM pragma_table_info('tasks')")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let add = |name: &str, ddl: &str| -> rusqlite::Result<()> {
            if !cols.iter().any(|c| c == name) {
                self.conn
                    .execute_batch(&format!("ALTER TABLE tasks ADD COLUMN {ddl}"))?;
            }
            Ok(())
        };
        add("description", "description TEXT NOT NULL DEFAULT ''")?;
        add("tags", "tags TEXT NOT NULL DEFAULT '[]'")?;
        add("notes", "notes TEXT NOT NULL DEFAULT ''")?;
        add("links", "links TEXT NOT NULL DEFAULT ''")?;
        add("reminder_minutes", "reminder_minutes INTEGER")?;
        Ok(())
    }

    fn migrate_0001(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL CHECK (length(trim(title)) > 0),
                category_id TEXT NOT NULL DEFAULT 'otr',
                priority    TEXT NOT NULL DEFAULT 'media' CHECK (priority IN ('alta','media','baja')),
                status      TEXT NOT NULL DEFAULT 'pendiente' CHECK (status IN ('pendiente','en-curso','completada')),
                start_at    INTEGER NOT NULL,
                end_at      INTEGER NOT NULL,
                all_day     INTEGER NOT NULL DEFAULT 0,
                progress    INTEGER NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
                completed_at INTEGER,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                deleted_at  INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_start ON tasks(start_at) WHERE deleted_at IS NULL;
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status) WHERE deleted_at IS NULL;
            ",
        )
    }

    fn migrate_0002(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS suggested_events (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                source          TEXT NOT NULL DEFAULT 'email',
                source_email_id TEXT,
                source_sender   TEXT,
                title           TEXT NOT NULL,
                description     TEXT NOT NULL DEFAULT '',
                category_id     TEXT NOT NULL DEFAULT 'otr',
                priority        TEXT NOT NULL DEFAULT 'media' CHECK (priority IN ('alta','media','baja')),
                start_at        INTEGER,
                end_at          INTEGER,
                location        TEXT NOT NULL DEFAULT '',
                tags            TEXT NOT NULL DEFAULT '[]',
                confidence      REAL NOT NULL DEFAULT 0,
                reason          TEXT NOT NULL DEFAULT '',
                status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','accepted','rejected','merged','auto_approved')),
                dedupe_task_id  INTEGER,
                dedupe_note     TEXT NOT NULL DEFAULT '',
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_suggestions_status ON suggested_events(status);
            CREATE TABLE IF NOT EXISTS trusted_senders (
                sender   TEXT PRIMARY KEY,
                added_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sync_state (
                id             INTEGER PRIMARY KEY AUTOINCREMENT,
                source         TEXT NOT NULL UNIQUE,
                checkpoint     TEXT NOT NULL DEFAULT '{}',
                last_result    TEXT NOT NULL DEFAULT 'never',
                last_error     TEXT NOT NULL DEFAULT '',
                last_run_at    INTEGER,
                created_at     INTEGER NOT NULL,
                updated_at     INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sync_history (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                source          TEXT NOT NULL DEFAULT 'email',
                started_at      INTEGER NOT NULL,
                finished_at     INTEGER,
                result          TEXT NOT NULL DEFAULT 'running',
                items_found     INTEGER NOT NULL DEFAULT 0,
                items_processed INTEGER NOT NULL DEFAULT 0,
                error           TEXT NOT NULL DEFAULT '',
                note            TEXT NOT NULL DEFAULT ''
            );
            ",
        )?;
        let has_metadata: bool = {
            let mut stmt = self.conn.prepare(
                "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'metadata'",
            )?;
            let n: i64 = stmt.query_row([], |r| r.get(0))?;
            n > 0
        };
        if !has_metadata {
            self.conn.execute_batch(
                "ALTER TABLE tasks ADD COLUMN metadata TEXT NOT NULL DEFAULT '{}'",
            )?;
        }
        Ok(())
    }

    pub fn settings_get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()
    }

    pub fn settings_set(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    pub fn settings_default(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// Exporta los datos del usuario como JSON (privacy: "mis datos"). NUNCA
    /// incluye secretos: no hay claves ni contraseñas (viven en el Credential
    /// Manager del SO), y las settings sensibles (config de correo) se
    /// exportan sin la contraseña (nunca estuvo en DB) y sin el rescan flag.
    pub fn export_data(&self) -> rusqlite::Result<serde_json::Value> {
        fn dump_all(conn: &Connection, table: &str) -> rusqlite::Result<Vec<serde_json::Value>> {
            use rusqlite::types::Value;
            let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY id"))?;
            let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let rows = stmt.query_map([], |r| {
                let mut obj = serde_json::Map::new();
                for (i, n) in names.iter().enumerate() {
                    let v: Option<Value> = r.get(i).ok();
                    let j = match v {
                        Some(Value::Null) | None => serde_json::Value::Null,
                        Some(Value::Integer(n)) => serde_json::json!(n),
                        Some(Value::Real(f)) => serde_json::json!(f),
                        Some(Value::Text(s)) => serde_json::json!(s),
                        Some(Value::Blob(b)) => serde_json::json!(b),
                    };
                    obj.insert(n.clone(), j);
                }
                Ok(serde_json::Value::Object(obj))
            })?;
            rows.collect()
        }
        let tasks = dump_all(&self.conn, "tasks")?;
        let suggestions = dump_all(&self.conn, "suggested_events")?;
        let classes = dump_all(&self.conn, "classes")?;
        let study_sessions = dump_all(&self.conn, "study_sessions")?;

        let trusted: Vec<String> = self
            .conn
            .prepare("SELECT sender FROM trusted_senders ORDER BY sender")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let settings: Vec<(String, String)> = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key")?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?
            // lista blanca: NUNCA se exporta configuración sensible (config de
            // correo, endpoint/modelo/clave de IA) ni flags internos
            .into_iter()
            .filter(|(k, _)| EXPORTABLE_SETTINGS.iter().any(|p| k.starts_with(p)))
            .collect();

        Ok(serde_json::json!({
            "app": "focusflow",
            "exported_at": now_ms(),
            "tasks": tasks,
            "suggestions": suggestions,
            "classes": classes,
            "study_sessions": study_sessions,
            "trusted_senders": trusted,
            "settings": settings,
        }))
    }

    /// Importa datos desde un JSON previamente exportado por `export_data`.
    /// Estrategia: INSERT OR IGNORE por id en todas las tablas (no borra ni
    /// sobreescribe datos existentes; idempotente si se importa dos veces).
    /// Las settings se restauran solo si su clave está en la whitelist
    /// `EXPORTABLE_SETTINGS`. Nunca toca config de correo, tokens ni claves.
    pub fn import_data(&self, json: &str) -> Result<ImportSummary, String> {
        let v: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("JSON inválido: {e}"))?;
        if v.get("app").and_then(|a| a.as_str()) != Some("focusflow") {
            return Err(
                "El archivo no es un export de FocusFlow (falta «app: focusflow»)".into(),
            );
        }

        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;

        // Helpers para extraer valores del JSON con fallback seguro
        fn str_val<'a>(obj: &'a serde_json::Value, key: &str) -> &'a str {
            obj.get(key).and_then(|v| v.as_str()).unwrap_or("")
        }
        fn i64_val(obj: &serde_json::Value, key: &str) -> i64 {
            obj.get(key).and_then(|v| v.as_i64()).unwrap_or(0)
        }
        fn opt_i64(obj: &serde_json::Value, key: &str) -> Option<i64> {
            obj.get(key).and_then(|v| v.as_i64())
        }
        fn f64_val(obj: &serde_json::Value, key: &str) -> f64 {
            obj.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
        }

        /// Resuelve el id con el que importar una fila (bug: INSERT OR IGNORE
        /// por id descartaba EN SILENCIO filas distintas que colisionaban con
        /// un id local, p. ej. al importar el export de otro equipo):
        /// - id libre → `Some(Some(id))` (se conserva);
        /// - misma fila (mismo título y created_at: re-importar el mismo
        ///   export) → `None` (se omite, idempotente);
        /// - fila distinta → `Some(None)`: se inserta con id nuevo.
        fn import_id(
            conn: &Connection,
            table: &str,
            id: Option<i64>,
            title: &str,
            created_at: i64,
        ) -> Result<Option<Option<i64>>, String> {
            let Some(id) = id else {
                return Ok(Some(None));
            };
            let row: Option<(String, i64)> = conn
                .query_row(
                    &format!("SELECT title, created_at FROM {table} WHERE id = ?1"),
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(|e| format!("{table} lookup: {e}"))?;
            Ok(match row {
                None => Some(Some(id)),
                Some((t, c)) if t == title && c == created_at => None,
                // colisión con OTRA fila: si la misma fila ya se importó antes
                // con id nuevo, no duplicarla (re-importar debe ser idempotente)
                Some(_) if existing_id(conn, table, title, created_at)?.is_some() => None,
                Some(_) => Some(None),
            })
        }

        /// Fila local con el mismo contenido clave (título + created_at).
        fn existing_id(
            conn: &Connection,
            table: &str,
            title: &str,
            created_at: i64,
        ) -> Result<Option<i64>, String> {
            conn.query_row(
                &format!("SELECT id FROM {table} WHERE title = ?1 AND created_at = ?2 LIMIT 1"),
                rusqlite::params![title, created_at],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("{table} lookup: {e}"))
        }

        let mut summary = ImportSummary::default();
        // id de tarea en el JSON → id real en esta DB (cambia si colisionó):
        // las referencias (sesiones de estudio, dedupe de sugerencias) se
        // remapean con esto
        let mut task_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        let remap = |m: &std::collections::HashMap<i64, i64>, v: Option<i64>| {
            v.map(|old| m.get(&old).copied().unwrap_or(old))
        };

        // ── Tareas ──────────────────────────────────────────────────────────
        if let Some(tasks) = v.get("tasks").and_then(|t| t.as_array()) {
            for t in tasks {
                let title = str_val(t, "title");
                if title.trim().is_empty() {
                    continue;
                }
                let cat = sanitize_category(str_val(t, "category_id"));
                let prio = sanitize_priority(str_val(t, "priority"));
                let status = match str_val(t, "status") {
                    "completada" => "completada",
                    "en-curso" => "en-curso",
                    _ => "pendiente",
                };
                let json_id = opt_i64(t, "id");
                let task_id =
                    match import_id(&tx, "tasks", json_id, title, i64_val(t, "created_at"))? {
                        Some(v) => v,
                        None => {
                            // ya importada (misma fila): las referencias del
                            // JSON deben apuntar a la fila local existente
                            if let (Some(old), Some(ex)) = (
                                json_id,
                                existing_id(&tx, "tasks", title, i64_val(t, "created_at"))?,
                            ) {
                                task_map.insert(old, ex);
                            }
                            continue;
                        }
                    };
                let n = tx
                    .execute(
                        "INSERT OR IGNORE INTO tasks
                         (id, title, category_id, priority, status,
                          start_at, end_at, all_day, progress,
                          completed_at, created_at, updated_at, deleted_at,
                          description, notes, links, tags, metadata,
                          reminder_minutes, reminder_fired_at)
                         VALUES
                         (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,
                          ?14,?15,?16,?17,?18,?19,?20)",
                        rusqlite::params![
                            task_id,
                            title,
                            cat,
                            prio,
                            status,
                            i64_val(t, "start_at"),
                            i64_val(t, "end_at"),
                            i64_val(t, "all_day"),
                            i64_val(t, "progress").clamp(0, 100),
                            opt_i64(t, "completed_at"),
                            i64_val(t, "created_at"),
                            i64_val(t, "updated_at"),
                            opt_i64(t, "deleted_at"),
                            str_val(t, "description"),
                            str_val(t, "notes"),
                            str_val(t, "links"),
                            str_val(t, "tags"),
                            str_val(t, "metadata"),
                            opt_i64(t, "reminder_minutes")
                                .map(|m| m.clamp(0, MAX_REMINDER_MINUTES)),
                            opt_i64(t, "reminder_fired_at"),
                        ],
                    )
                    .map_err(|e| format!("task insert: {e}"))?;
                if let (Some(old), true) = (json_id, n > 0) {
                    task_map.insert(old, tx.last_insert_rowid());
                }
                summary.tasks += n;
            }
        }

        // ── Sugerencias ──────────────────────────────────────────────────────
        if let Some(sug) = v.get("suggestions").and_then(|s| s.as_array()) {
            for s in sug {
                let title = str_val(s, "title");
                if title.trim().is_empty() {
                    continue;
                }
                let prio = sanitize_priority(str_val(s, "priority"));
                let cat = sanitize_category(str_val(s, "category_id"));
                let status = match str_val(s, "status") {
                    "accepted" | "rejected" | "merged" | "auto_approved" => str_val(s, "status"),
                    _ => "pending",
                };
                let sug_id = match import_id(
                    &tx,
                    "suggested_events",
                    opt_i64(s, "id"),
                    title,
                    i64_val(s, "created_at"),
                )? {
                    Some(v) => v,
                    None => continue,
                };
                let n = tx
                    .execute(
                        "INSERT OR IGNORE INTO suggested_events
                         (id, source, source_email_id, source_sender,
                          title, description, category_id, priority,
                          start_at, end_at, location, tags, confidence,
                          reason, status, dedupe_task_id, dedupe_note,
                          created_at, updated_at)
                         VALUES
                         (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,
                          ?14,?15,?16,?17,?18,?19)",
                        rusqlite::params![
                            sug_id,
                            str_val(s, "source"),
                            s.get("source_email_id").and_then(|v| v.as_str()),
                            s.get("source_sender").and_then(|v| v.as_str()),
                            title,
                            str_val(s, "description"),
                            cat,
                            prio,
                            opt_i64(s, "start_at"),
                            opt_i64(s, "end_at"),
                            str_val(s, "location"),
                            str_val(s, "tags"),
                            f64_val(s, "confidence"),
                            str_val(s, "reason"),
                            status,
                            remap(&task_map, opt_i64(s, "dedupe_task_id")),
                            str_val(s, "dedupe_note"),
                            i64_val(s, "created_at"),
                            i64_val(s, "updated_at"),
                        ],
                    )
                    .map_err(|e| format!("suggestion insert: {e}"))?;
                summary.suggestions += n;
            }
        }

        // ── Clases ───────────────────────────────────────────────────────────
        if let Some(classes) = v.get("classes").and_then(|c| c.as_array()) {
            for c in classes {
                let title = str_val(c, "title");
                if title.trim().is_empty() {
                    continue;
                }
                // una clase inválida (fin <= inicio, vigencia invertida) violaba
                // los CHECK de la tabla y abortaba TODO el import: se omite,
                // igual que las sesiones de estudio inválidas
                let (dow, smin, emin) = (
                    i64_val(c, "day_of_week"),
                    i64_val(c, "start_min"),
                    i64_val(c, "end_min"),
                );
                let (sdate, edate) = (i64_val(c, "start_date"), i64_val(c, "end_date"));
                if validate_class(title, dow, smin, emin, sdate, edate).is_err() {
                    continue;
                }
                let class_id =
                    match import_id(&tx, "classes", opt_i64(c, "id"), title, i64_val(c, "created_at"))? {
                        Some(v) => v,
                        None => continue,
                    };
                let n = tx
                    .execute(
                        "INSERT OR IGNORE INTO classes
                         (id, title, day_of_week, start_min, end_min,
                          start_date, end_date, created_at, updated_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                        rusqlite::params![
                            class_id,
                            title,
                            dow,
                            smin,
                            emin,
                            sdate,
                            edate,
                            i64_val(c, "created_at"),
                            i64_val(c, "updated_at"),
                        ],
                    )
                    .map_err(|e| format!("class insert: {e}"))?;
                summary.classes += n;
            }
        }

        // ── Sesiones de estudio ───────────────────────────────────────────────
        if let Some(sessions) = v.get("study_sessions").and_then(|s| s.as_array()) {
            for s in sessions {
                let title = str_val(s, "title");
                if title.trim().is_empty() {
                    continue;
                }
                let start_at = i64_val(s, "start_at");
                let end_at = i64_val(s, "end_at");
                if end_at <= start_at {
                    continue; // sesión inválida: fin <= inicio
                }
                let study_id = match import_id(
                    &tx,
                    "study_sessions",
                    opt_i64(s, "id"),
                    title,
                    i64_val(s, "created_at"),
                )? {
                    Some(v) => v,
                    None => continue,
                };
                let n = tx
                    .execute(
                        "INSERT OR IGNORE INTO study_sessions
                         (id, title, start_at, end_at, task_id, notes,
                          created_at, updated_at)
                         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                        rusqlite::params![
                            study_id,
                            title,
                            start_at,
                            end_at,
                            remap(&task_map, opt_i64(s, "task_id")),
                            str_val(s, "notes"),
                            i64_val(s, "created_at"),
                            i64_val(s, "updated_at"),
                        ],
                    )
                    .map_err(|e| format!("study_session insert: {e}"))?;
                summary.study_sessions += n;
            }
        }

        // ── Remitentes de confianza ───────────────────────────────────────────
        if let Some(senders) = v.get("trusted_senders").and_then(|s| s.as_array()) {
            for s in senders {
                let sender = s.as_str().unwrap_or("").trim().to_string();
                if sender.is_empty() {
                    continue;
                }
                let n = tx
                    .execute(
                        "INSERT OR IGNORE INTO trusted_senders (sender, added_at)
                         VALUES (?1, ?2)",
                        rusqlite::params![sender, now_ms()],
                    )
                    .map_err(|e| format!("trusted_sender insert: {e}"))?;
                summary.trusted_senders += n;
            }
        }

        // ── Settings (solo whitelist) ─────────────────────────────────────────
        if let Some(settings) = v.get("settings").and_then(|s| s.as_array()) {
            for entry in settings {
                // El export guarda settings como [[key, value], ...]
                let (key, value) = if let Some(arr) = entry.as_array() {
                    let k = arr.first().and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let val = arr.get(1).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    (k, val)
                } else if let Some(obj) = entry.as_object() {
                    let k = obj.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let val = obj.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    (k, val)
                } else {
                    continue;
                };
                if key.is_empty() {
                    continue;
                }
                // Solo restauramos claves de la whitelist
                if !EXPORTABLE_SETTINGS.iter().any(|p| key.starts_with(p)) {
                    continue;
                }
                let n = tx
                    .execute(
                        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                        rusqlite::params![key, value],
                    )
                    .map_err(|e| format!("setting insert: {e}"))?;
                summary.settings += n;
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(summary)
    }

    /// Borra TODOS los datos del usuario: tareas, sugerencias, propuestas,
    /// notificaciones, historial de sync, remitentes de confianza y ajustes
    /// (los defaults se re-siembran en el próximo open). Los secretos del
    /// Credential Manager NO tocan aquí: los gestiona el comando `data_wipe`.
    pub fn wipe_data(&self) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for t in [
            "notification_log",
            "assistant_actions",
            "plan_proposals",
            "suggested_events",
            // vínculos sugerencia→tareas (migración 0014): sin esto quedaban
            // filas huérfanas que apuntaban a ids de tareas futuras
            "suggestion_tasks",
            "email_seen",
            "sync_state",
            "sync_history",
            "trusted_senders",
        ] {
            tx.execute_batch(&format!("DELETE FROM {t};"))?;
        }
        tx.execute("DELETE FROM tasks", [])?;
        tx.execute("DELETE FROM classes", [])?;
        tx.execute("DELETE FROM study_sessions", [])?;
        tx.execute("DELETE FROM settings", [])?;
        tx.commit()?;
        Ok(())
    }

    // en release solo la usan los tests; en debug también la app (seed demo)
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    fn seed_if_empty(&self) -> rusqlite::Result<()> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }
        let now = now_ms();
        let day = 86_400_000;
        let t = |title: &str,
                 cat: &str,
                 prio: &str,
                 start: i64,
                 end: i64|
         -> rusqlite::Result<()> {
            self.conn.execute(
                "INSERT INTO tasks (title, category_id, priority, status, start_at, end_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'pendiente', ?4, ?5, ?6, ?6)",
                rusqlite::params![title, cat, prio, start, end, now],
            )?;
            Ok(())
        };
        t(
            "Estudiar cálculo — derivadas e integrales",
            "uni",
            "alta",
            now + 9 * 3_600_000,
            now + 11 * 3_600_000,
        )?;
        t(
            "Entregar proyecto de redes",
            "uni",
            "alta",
            now + day + 14 * 3_600_000,
            now + day + 14 * 3_600_000 + 1_800_000,
        )?;
        t(
            "Pagar internet",
            "fin",
            "media",
            now + day * 2 + 9 * 3_600_000,
            now + day * 2 + 9 * 3_600_000,
        )?;
        t(
            "Examen de física — parcial 2",
            "uni",
            "alta",
            now + day * 3 + 8 * 3_600_000,
            now + day * 3 + 10 * 3_600_000,
        )?;
        t(
            "Cita médico — revisión anual",
            "sal",
            "baja",
            now + day * 4 + 12 * 3_600_000,
            now + day * 4 + 12 * 3_600_000 + 45 * 60_000,
        )?;
        Ok(())
    }

    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
    }

    pub fn list(&self) -> rusqlite::Result<Vec<TaskRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM tasks WHERE deleted_at IS NULL ORDER BY start_at")?;
        let rows = stmt.query_map([], task_from_row)?;
        rows.collect()
    }

    /// Tareas que se cruzan con [start, end] (inclusive, para multi-día).
    pub fn list_range(&self, start: i64, end: i64) -> rusqlite::Result<Vec<TaskRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM tasks WHERE deleted_at IS NULL AND start_at <= ?2 AND end_at >= ?1 ORDER BY start_at",
        )?;
        let rows = stmt.query_map([start, end], task_from_row)?;
        rows.collect()
    }

    /// Devuelve otra tarea activa que se solapa con [start, end].
    pub fn find_overlap(
        &self,
        exclude_id: i64,
        start: i64,
        end: i64,
    ) -> rusqlite::Result<Option<(i64, String)>> {
        // No cuentan como solape: tareas de todo el día ni spans multi-día
        // (>= 24 h). Son marcadores/rangos inicio→fin cuyos días quedan
        // libres para añadir otras tareas. Se excluye por DURACIÓN (no solo
        // por `all_day`) para cubrir también datos viejos guardados con
        // all_day=0 antes de la normalización.
        self.conn
            .query_row(
                "SELECT id, title FROM tasks
                 WHERE deleted_at IS NULL AND id != ?1 AND status != 'completada'
                   AND (end_at - start_at) < 86400000
                   AND start_at < ?3 AND end_at > ?2
                 ORDER BY start_at LIMIT 1",
                rusqlite::params![exclude_id, start, end],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
    }

    /// Igual que `find_overlap` pero excluyendo varias tareas (p. ej. los
    /// eventos recién creados por la propia propuesta al aceptar un plan).
    pub fn find_overlap_excluding(
        &self,
        exclude_ids: &[i64],
        start: i64,
        end: i64,
    ) -> rusqlite::Result<Option<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title FROM tasks
             WHERE deleted_at IS NULL AND status != 'completada'
               AND (end_at - start_at) < 86400000
               AND start_at < ?2 AND end_at > ?1
             ORDER BY start_at LIMIT 16",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![start, end], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().find(|(id, _)| !exclude_ids.contains(id)))
    }

    // ------------------------------------------------------------------
    // Clases del horario (bloques fijos con vigencia, NO son tareas)
    // ------------------------------------------------------------------

    pub fn class_list(&self) -> rusqlite::Result<Vec<ClassRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM classes ORDER BY day_of_week, start_min")?;
        let rows = stmt.query_map([], class_from_row)?;
        rows.collect()
    }

    pub fn class_get(&self, id: i64) -> rusqlite::Result<Option<ClassRow>> {
        self.conn
            .query_row("SELECT * FROM classes WHERE id = ?1", [id], class_from_row)
            .optional()
    }

    pub fn class_create(
        &self,
        title: &str,
        day_of_week: i64,
        start_min: i64,
        end_min: i64,
        start_date: i64,
        end_date: i64,
    ) -> rusqlite::Result<ClassRow> {
        #[allow(clippy::too_many_arguments)]
        fn inner(
            conn: &rusqlite::Connection,
            title: &str,
            day_of_week: i64,
            start_min: i64,
            end_min: i64,
            start_date: i64,
            end_date: i64,
        ) -> rusqlite::Result<ClassRow> {
            validate_class(title, day_of_week, start_min, end_min, start_date, end_date).map_err(
                |e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e))),
            )?;
            let now = now_ms();
            conn.execute(
                "INSERT INTO classes (title, day_of_week, start_min, end_min, start_date, end_date, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                rusqlite::params![
                    title.trim(),
                    day_of_week,
                    start_min,
                    end_min,
                    start_date,
                    end_date,
                    now
                ],
            )?;
            let id = conn.last_insert_rowid();
            let mut stmt = conn.prepare("SELECT * FROM classes WHERE id = ?1")?;
            stmt.query_row([id], class_from_row)
        }
        inner(
            &self.conn,
            title,
            day_of_week,
            start_min,
            end_min,
            start_date,
            end_date,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn class_update(
        &self,
        id: i64,
        title: &str,
        day_of_week: i64,
        start_min: i64,
        end_min: i64,
        start_date: i64,
        end_date: i64,
    ) -> rusqlite::Result<ClassRow> {
        validate_class(title, day_of_week, start_min, end_min, start_date, end_date).map_err(
            |e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(e))),
        )?;
        let n = self.conn.execute(
            "UPDATE classes SET title = ?2, day_of_week = ?3, start_min = ?4, end_min = ?5,
               start_date = ?6, end_date = ?7, updated_at = ?8
             WHERE id = ?1",
            rusqlite::params![
                id,
                title.trim(),
                day_of_week,
                start_min,
                end_min,
                start_date,
                end_date,
                now_ms()
            ],
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(self.class_get(id)?.expect("clase existente"))
    }

    // ------------------------------------------------------------------
    // Sesiones de estudio (tiempo RESERVADO, movible y editable).
    // NO son tareas: sin estado, sin pendientes, sin contadores.
    // ------------------------------------------------------------------

    /// Sesiones que tocan el rango [start, end) — solape real semiabierto.
    pub fn study_list_range(&self, start: i64, end: i64) -> rusqlite::Result<Vec<StudyRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM study_sessions
             WHERE start_at < ?2 AND end_at > ?1
             ORDER BY start_at, id",
        )?;
        let rows = stmt.query_map(rusqlite::params![start, end], study_from_row)?;
        rows.collect()
    }

    /// Todas las sesiones (orden cronológico). Igual que `class_list`: la usa
    /// el planificador y el export.
    pub fn study_list(&self) -> rusqlite::Result<Vec<StudyRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM study_sessions ORDER BY start_at, id")?;
        let rows = stmt.query_map([], study_from_row)?;
        rows.collect()
    }

    pub fn study_get(&self, id: i64) -> rusqlite::Result<Option<StudyRow>> {
        self.conn
            .query_row(
                "SELECT * FROM study_sessions WHERE id = ?1",
                [id],
                study_from_row,
            )
            .optional()
    }

    /// Crea una sesión. `task_id` es opcional, pero si viene debe apuntar a una
    /// tarea existente (relación válida, nunca una tarea disfrazada).
    pub fn study_create(
        &self,
        title: &str,
        start_at: i64,
        end_at: i64,
        task_id: Option<i64>,
        notes: &str,
    ) -> rusqlite::Result<StudyRow> {
        validate_study(title, start_at, end_at).map_err(study_err)?;
        if let Some(id) = task_id {
            if self.get_task(id)?.is_none() {
                return Err(study_err("La tarea relacionada no existe".into()));
            }
        }
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO study_sessions (title, start_at, end_at, task_id, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![title.trim(), start_at, end_at, task_id, notes.trim(), now],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(self.study_get(id)?.expect("sesión recién creada"))
    }

    pub fn study_update(
        &self,
        id: i64,
        title: &str,
        start_at: i64,
        end_at: i64,
        task_id: Option<i64>,
        notes: &str,
    ) -> rusqlite::Result<StudyRow> {
        validate_study(title, start_at, end_at).map_err(study_err)?;
        let mut task_id = task_id;
        if let Some(tid) = task_id {
            if self.get_task(tid)?.is_none() {
                // datos previos al fix de `delete`: la sesión conserva el id de
                // una tarea ya borrada; reenviar ESE mismo id se trata como
                // "sin tarea" en vez de bloquear la edición
                let current = self.study_get(id)?.and_then(|s| s.task_id);
                if current != Some(tid) {
                    return Err(study_err("La tarea relacionada no existe".into()));
                }
                task_id = None;
            }
        }
        let n = self.conn.execute(
            "UPDATE study_sessions SET title = ?2, start_at = ?3, end_at = ?4,
               task_id = ?5, notes = ?6, updated_at = ?7
             WHERE id = ?1",
            rusqlite::params![
                id,
                title.trim(),
                start_at,
                end_at,
                task_id,
                notes.trim(),
                now_ms()
            ],
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        Ok(self.study_get(id)?.expect("sesión existente"))
    }

    /// Mueve/redimensiona una sesión (drag & drop del calendario): solo cambia
    /// la ventana temporal, conservando título, tarea y notas.
    pub fn study_move(&self, id: i64, start_at: i64, end_at: i64) -> rusqlite::Result<StudyRow> {
        let cur = self
            .study_get(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        validate_study(&cur.title, start_at, end_at).map_err(study_err)?;
        self.conn.execute(
            "UPDATE study_sessions SET start_at = ?2, end_at = ?3, updated_at = ?4 WHERE id = ?1",
            rusqlite::params![id, start_at, end_at, now_ms()],
        )?;
        Ok(self.study_get(id)?.expect("sesión existente"))
    }

    pub fn study_delete(&self, id: i64) -> rusqlite::Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM study_sessions WHERE id = ?1", [id])?;
        Ok(n > 0)
    }

    pub fn class_delete(&self, id: i64) -> rusqlite::Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM classes WHERE id = ?1", [id])?;
        Ok(n > 0)
    }

    /// Texto legible de un error de BD para el frontend: los errores de
    /// validación de clase viajan como mensaje directo (sin prefijo técnico).
    /// `pub`: también lo usa el CLI (`ff clases` / `ff sesiones`).
    pub fn rusqlite_error_text(e: rusqlite::Error) -> String {
        if let rusqlite::Error::ToSqlConversionFailure(f) = &e {
            return f.to_string();
        }
        if let rusqlite::Error::SqliteFailure(_, Some(msg)) = &e {
            // CHECK constraints: mensaje técnico → regla legible
            if msg.contains("end_min > start_min")
                || msg.contains("end_date >= start_date")
                || msg.contains("La hora de finalización")
            {
                return "La hora de finalización debe ser posterior a la de inicio".into();
            }
            if msg.contains("CHECK") || msg.contains("constraint") {
                return "Datos de la clase inválidos".into();
            }
        }
        e.to_string()
    }

    pub fn create(
        &self,
        title: &str,
        category_id: &str,
        priority: &str,
        start_at: i64,
        end_at: i64,
        all_day: bool,
    ) -> rusqlite::Result<TaskRow> {
        let now = now_ms();
        let priority = sanitize_priority(priority);
        let (start_at, end_at) = normalize_task_span(start_at, end_at, all_day);
        // span invertido (end < start) con hora: se aplana a un instante —
        // la duracion cero con all_day=false es el modelo legitimo de "sin
        // horario fijo" que consume el backlog flexible, pero un rango
        // negativo nunca debe guardarse
        let end_at = end_at.max(start_at);
        self.conn.execute(
            "INSERT INTO tasks (title, category_id, priority, status, start_at, end_at, all_day, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'pendiente', ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![title, sanitize_category(category_id), priority, start_at, end_at, all_day as i64, now],
        )?;
        let id = self.conn.last_insert_rowid();
        self.conn
            .query_row("SELECT * FROM tasks WHERE id = ?1", [id], task_from_row)
    }

    /// Actualiza solo la descripción (contexto) de una tarea.
    pub fn set_description(&self, id: i64, description: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET description = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, description, now_ms()],
        )?;
        Ok(())
    }

    pub fn set_completed(&self, id: i64, done: bool) -> rusqlite::Result<()> {
        let now = now_ms();
        self.conn.execute(
            "UPDATE tasks SET status = CASE WHEN ?2 THEN 'completada' ELSE 'pendiente' END,
                    completed_at = CASE WHEN ?2 THEN ?3 ELSE NULL END,
                    progress = CASE WHEN ?2 THEN 100 ELSE 0 END,
                    updated_at = ?3
             WHERE id = ?1",
            rusqlite::params![id, done, now],
        )?;
        Ok(())
    }

    /// Cambia el estado de una tarea a uno válido (pendiente | en-curso |
    /// completada). Acciones rápidas del widget usan este servicio.
    pub fn set_task_status(&self, id: i64, status: &str) -> rusqlite::Result<()> {
        debug_assert!(matches!(status, "pendiente" | "en-curso" | "completada"));
        let now = now_ms();
        self.conn.execute(
            "UPDATE tasks SET status = ?2, updated_at = ?3
             WHERE id = ?1",
            rusqlite::params![id, status, now],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![id, now_ms()],
        )?;
        // la relación opcional sesión→tarea se suelta: si no, editar la
        // sesión fallaba con "La tarea relacionada no existe"
        self.conn.execute(
            "UPDATE study_sessions SET task_id = NULL WHERE task_id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub fn move_to(
        &self,
        id: i64,
        start_at: i64,
        end_at: i64,
        all_day: Option<bool>,
    ) -> rusqlite::Result<()> {
        // mismas reglas de span que create/update_task_full (bug M1 vía drag):
        // all-day cubre 24 h mínimo; un rango invertido se aplana
        let stored_all_day: Option<bool> = self
            .conn
            .query_row(
                "SELECT all_day FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |r| Ok(r.get::<_, i64>(0)? != 0),
            )
            .optional()?;
        let effective_all_day = all_day.or(stored_all_day).unwrap_or(false);
        let (start_at, end_at) = normalize_task_span(start_at, end_at, effective_all_day);
        let end_at = end_at.max(start_at);
        // mover a otra hora rearma el recordatorio (si no, un evento
        // re-programado ya avisado nunca vuelve a sonar); mover sin cambiar
        // start_at (solo el fin) no re-dispara.
        self.conn.execute(
            "UPDATE tasks SET
                    reminder_fired_at = CASE WHEN start_at != ?2 THEN NULL ELSE reminder_fired_at END,
                    start_at = ?2, end_at = ?3,
                    all_day = CASE WHEN ?5 IS NULL THEN all_day ELSE ?5 END,
                    updated_at = ?4 WHERE id = ?1",
            rusqlite::params![id, start_at, end_at, now_ms(), all_day],
        )?;
        Ok(())
    }

    pub fn update_task_full(
        &self,
        id: i64,
        title: &str,
        category_id: &str,
        priority: &str,
        start_at: i64,
        end_at: i64,
        description: &str,
        tags: &str,
        notes: &str,
        links: &str,
        reminder_minutes: Option<i64>,
        all_day: Option<bool>,
    ) -> rusqlite::Result<()> {
        let reminder_minutes = sanitize_reminder(reminder_minutes);
        let priority = sanitize_priority(priority);
        let prev: Option<(Option<i64>, bool, i64)> = self
            .conn
            .query_row(
                "SELECT reminder_minutes, all_day, start_at FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?,
                        r.get::<_, i64>(1)? != 0,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        let prev_reminder = prev.as_ref().and_then(|p| p.0);
        let prev_start = prev.as_ref().map(|p| p.2);
        // all_day=None conserva el valor actual de la tarea; la normalización
        // del span debe usar ESE all_day efectivo (bug M1: editar/fusionar una
        // sugerencia con inicio==fin guardaba un marcador all-day de duración
        // cero, invisible en mes/día/semana).
        let effective_all_day = all_day.unwrap_or_else(|| prev.map(|p| p.1).unwrap_or(false));
        let (start_at, end_at) = normalize_task_span(start_at, end_at, effective_all_day);
        let end_at = end_at.max(start_at);
        self.conn.execute(
            "UPDATE tasks SET title = ?2, category_id = ?3, priority = ?4,
                    start_at = ?5, end_at = ?6, description = ?7, tags = ?8, notes = ?9,
                    links = ?10, reminder_minutes = ?11,
                    all_day = CASE WHEN ?13 IS NULL THEN all_day ELSE ?13 END,
                    updated_at = ?12
             WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![
                id,
                title,
                sanitize_category(category_id),
                priority,
                start_at,
                end_at,
                description,
                tags,
                notes,
                links,
                reminder_minutes,
                now_ms(),
                all_day
            ],
        )?;
        // FR-30: cambiar el recordatorio O la hora de inicio rearma el disparo
        // (no refirar si nada de eso cambió)
        if prev_reminder != reminder_minutes || prev_start != Some(start_at) {
            self.conn.execute(
                "UPDATE tasks SET reminder_fired_at = NULL WHERE id = ?1 AND reminder_fired_at IS NOT NULL",
                [id],
            )?;
        }
        Ok(())
    }

    pub fn get_task(&self, id: i64) -> rusqlite::Result<Option<TaskRow>> {
        self.conn
            .query_row(
                "SELECT * FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                task_from_row,
            )
            .optional()
    }

    /// Recordatorio con la ventana de disparo vencida y sin marcar.
    pub fn due_reminders(&self, now: i64) -> rusqlite::Result<Vec<DueReminder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, start_at, end_at, all_day, reminder_minutes FROM tasks
             WHERE deleted_at IS NULL
               AND status != 'completada'
               AND reminder_minutes IS NOT NULL
               AND reminder_fired_at IS NULL
               AND start_at - reminder_minutes * 60000 <= ?1",
        )?;
        let rows = stmt.query_map([now], |r| {
            Ok(DueReminder {
                task_id: r.get(0)?,
                title: r.get(1)?,
                start_at: r.get(2)?,
                end_at: r.get(3)?,
                all_day: r.get::<_, i64>(4)? != 0,
                reminder_minutes: r.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn mark_reminder_fired(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET reminder_fired_at = ?2, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![id, now_ms()],
        )?;
        Ok(())
    }

    pub fn set_task_reminder(&self, id: i64, minutes: i64) -> rusqlite::Result<()> {
        let minutes = sanitize_reminder(Some(minutes)).unwrap_or(0);
        self.conn.execute(
            "UPDATE tasks SET reminder_minutes = ?2, reminder_fired_at = NULL, updated_at = ?3
             WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id, minutes, now_ms()],
        )?;
        Ok(())
    }

    /// Sesión de Google OAuth: una fila única (id = 1). Uso local de la DB
    /// (refresh_token incluido); el acceso a esta tabla queda en la app.
    pub fn auth_save(&self, s: &AuthSession) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO auth_sessions (id, user_id, email, name, access_token, refresh_token, expires_at, created_at, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(id) DO UPDATE SET
               user_id = excluded.user_id,
               email = excluded.email,
               name = excluded.name,
               access_token = excluded.access_token,
               refresh_token = excluded.refresh_token,
               expires_at = excluded.expires_at,
               updated_at = excluded.updated_at",
            rusqlite::params![
                s.user_id,
                s.email,
                s.name,
                s.access_token,
                s.refresh_token,
                s.expires_at,
                now_ms()
            ],
        )?;
        Ok(())
    }

    pub fn auth_load(&self) -> rusqlite::Result<Option<AuthSession>> {
        let mut stmt = self
            .conn
            .prepare("SELECT user_id, email, name, access_token, refresh_token, expires_at FROM auth_sessions WHERE id = 1")?;
        let mut rows = stmt.query_map([], |r| {
            Ok(AuthSession {
                user_id: r.get(0)?,
                email: r.get(1)?,
                name: r.get(2)?,
                access_token: r.get(3)?,
                refresh_token: r.get(4)?,
                expires_at: r.get(5)?,
            })
        })?;
        rows.next().transpose()
    }

    pub fn auth_clear(&self) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM auth_sessions WHERE id = 1", [])?;
        Ok(())
    }

    /// Guarda un access_token refrescado SOLO si la sesión sigue siendo la
    /// misma (mismo usuario y refresh_token). El refresco ocurre fuera del
    /// lock: si entretanto el usuario cerró sesión o cambió de cuenta, un
    /// `auth_save` a ciegas resucitaba la sesión vieja. Devuelve filas
    /// afectadas (0 = la sesión cambió; el token se descarta).
    pub fn auth_update_access(
        &self,
        user_id: &str,
        refresh_token: &str,
        access_token: &str,
        expires_at: i64,
    ) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE auth_sessions SET access_token = ?3, expires_at = ?4, updated_at = ?5
             WHERE id = 1 AND user_id = ?1 AND refresh_token = ?2",
            rusqlite::params![user_id, refresh_token, access_token, expires_at, now_ms()],
        )
    }

    /// Refresh token revocado/caducado (`invalid_grant`): vacía los tokens de
    /// ESA sesión (conserva email/nombre para la UI) → `gmail_connected`
    /// pasa a false y la UI pide volver a iniciar sesión.
    pub fn auth_drop_tokens(&self, user_id: &str, refresh_token: &str) -> rusqlite::Result<usize> {
        self.conn.execute(
            "UPDATE auth_sessions SET access_token = '', refresh_token = '', expires_at = 0,
                    updated_at = ?3
             WHERE id = 1 AND user_id = ?1 AND refresh_token = ?2",
            rusqlite::params![user_id, refresh_token, now_ms()],
        )
    }

    /// Metadata JSON en `tasks.metadata` (ej: enlace a la propuesta de plan
    /// que creó la tarea).
    pub fn set_task_metadata(&self, id: i64, metadata: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET metadata = ?2, updated_at = ?3 WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id, metadata, now_ms()],
        )?;
        Ok(())
    }

    // ---------------- registro de notificaciones (fase 11) ----------------

    /// Registra un disparo de notificación contextual y devuelve su id.
    pub fn log_notification(
        &self,
        kind: &str,
        task_id: i64,
        payload: &str,
    ) -> rusqlite::Result<i64> {
        self.conn.execute(
            "INSERT INTO notification_log (kind, task_id, fired_at, payload) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![kind, task_id, now_ms(), payload],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn set_notif_status(&self, id: i64, status: &str) -> rusqlite::Result<()> {
        // whitelist: la UI solo envía estos tres estados
        if !matches!(status, "planned" | "later" | "dismissed") {
            return Ok(());
        }
        self.conn.execute(
            "UPDATE notification_log SET status = ?2 WHERE id = ?1",
            rusqlite::params![id, status],
        )?;
        Ok(())
    }

    /// ¿El usuario descartó este (kind, tarea) alguna vez? → no volver a insistir.
    pub fn notif_dismissed(&self, kind: &str, task_id: i64) -> rusqlite::Result<bool> {
        self.conn
            .query_row(
                "SELECT 1 FROM notification_log WHERE kind = ?1 AND task_id = ?2 AND status = 'dismissed' LIMIT 1",
                rusqlite::params![kind, task_id],
                |_| Ok(()),
            )
            .optional()
            .map(|v| v.is_some())
    }

    /// ¿Disparado recientemente (ventana de cadencia) para (kind, tarea)?
    pub fn notif_fired_recently(
        &self,
        kind: &str,
        task_id: i64,
        since_ms: i64,
    ) -> rusqlite::Result<bool> {
        self.conn
            .query_row(
                "SELECT 1 FROM notification_log
                 WHERE kind = ?1 AND task_id = ?2 AND status != 'dismissed' AND fired_at >= ?3 LIMIT 1",
                rusqlite::params![kind, task_id, since_ms],
                |_| Ok(()),
            )
            .optional()
            .map(|v| v.is_some())
    }

    /// Disparos de hoy (tope diario anti-spam).
    pub fn notif_fired_today(&self, day_start_ms: i64) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM notification_log WHERE fired_at >= ?1 AND status != 'dismissed'",
            [day_start_ms],
            |r| r.get(0),
        )
    }

    // ---------------- propuestas de planificación (fase 7) ----------------

    pub fn insert_plan_proposal(
        &self,
        text: &str,
        payload: &str,
        source: &str,
    ) -> rusqlite::Result<i64> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO plan_proposals (text, status, payload, source, created_at, updated_at)
             VALUES (?1, 'pending', ?2, ?3, ?4, ?4)",
            rusqlite::params![text, payload, source, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_plan_proposal(&self, id: i64) -> rusqlite::Result<Option<PlanProposalRow>> {
        self.conn
            .query_row(
                "SELECT id, text, status, payload, source, created_at, updated_at FROM plan_proposals WHERE id = ?1",
                [id],
                plan_proposal_from_row,
            )
            .optional()
    }

    pub fn list_plan_proposals(
        &self,
        only_pending: bool,
    ) -> rusqlite::Result<Vec<PlanProposalRow>> {
        let mut stmt = self.conn.prepare(if only_pending {
            "SELECT id, text, status, payload, source, created_at, updated_at FROM plan_proposals
             WHERE status = 'pending' ORDER BY created_at DESC"
        } else {
            "SELECT id, text, status, payload, source, created_at, updated_at FROM plan_proposals
             ORDER BY created_at DESC LIMIT 50"
        })?;
        let rows = stmt.query_map([], plan_proposal_from_row)?;
        rows.collect()
    }

    pub fn set_plan_proposal_status(&self, id: i64, status: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE plan_proposals SET status = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, status, now_ms()],
        )?;
        Ok(())
    }

    // ---------------- acciones del asistente (fase 9) ----------------

    pub fn insert_assistant_action(&self, kind: &str, payload: &str) -> rusqlite::Result<i64> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO assistant_actions (kind, payload, status, created_at, updated_at)
             VALUES (?1, ?2, 'pending', ?3, ?3)",
            rusqlite::params![kind, payload, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_assistant_action(&self, id: i64) -> rusqlite::Result<Option<AssistantActionRow>> {
        self.conn
            .query_row(
                "SELECT id, kind, payload, status, created_at, updated_at FROM assistant_actions WHERE id = ?1",
                [id],
                assistant_action_from_row,
            )
            .optional()
    }

    pub fn list_assistant_actions(
        &self,
        only_pending: bool,
    ) -> rusqlite::Result<Vec<AssistantActionRow>> {
        let mut stmt = self.conn.prepare(if only_pending {
            "SELECT id, kind, payload, status, created_at, updated_at FROM assistant_actions
             WHERE status = 'pending' ORDER BY created_at DESC"
        } else {
            "SELECT id, kind, payload, status, created_at, updated_at FROM assistant_actions
             ORDER BY created_at DESC LIMIT 50"
        })?;
        let rows = stmt.query_map([], assistant_action_from_row)?;
        rows.collect()
    }

    pub fn set_assistant_action_status(&self, id: i64, status: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE assistant_actions SET status = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, status, now_ms()],
        )?;
        Ok(())
    }

    pub fn duplicate(&self, id: i64) -> rusqlite::Result<Option<TaskRow>> {
        let Some(t) = self.get_task(id)? else {
            return Ok(None);
        };
        let now = now_ms();
        let shift = (t.end_at - t.start_at).max(3_600_000);
        self.conn.execute(
            "INSERT INTO tasks (title, category_id, priority, status, start_at, end_at, all_day, progress,
                                description, tags, notes, links, reminder_minutes, created_at, updated_at)
             VALUES (?1,?2,?3,'pendiente',?4,?5,?6,0,?7,?8,?9,?10,?11,?12,?12)",
            rusqlite::params![
                format!("{} (copia)", t.title), t.category_id, t.priority,
                t.start_at + shift, t.end_at + shift, t.all_day as i64,
                t.description, t.tags, t.notes, t.links, t.reminder_minutes, now
            ],
        )?;
        let nid = self.conn.last_insert_rowid();
        self.conn
            .query_row("SELECT * FROM tasks WHERE id = ?1", [nid], task_from_row)
            .map(Some)
    }

    pub fn get_task_title(&self, id: i64) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT title FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn find_similar_task(
        &self,
        title: &str,
        start_at: i64,
        sender: &str,
    ) -> rusqlite::Result<Option<(i64, String)>> {
        let window_start = start_at - 48 * 3_600_000;
        let window_end = start_at + 48 * 3_600_000;
        let mut stmt = self.conn.prepare(
            "SELECT id, title FROM tasks
             WHERE deleted_at IS NULL AND start_at BETWEEN ?1 AND ?2
             ORDER BY ABS(start_at - ?3)",
        )?;
        let candidates: Vec<(i64, String)> = stmt
            .query_map(rusqlite::params![window_start, window_end, start_at], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<Result<_, _>>()?;
        for (id, t) in candidates {
            if title_similar(title, &t) {
                return Ok(Some((id, t)));
            }
        }
        let _ = sender;
        Ok(None)
    }

    // ---- suggested_events ----
    #[allow(clippy::too_many_arguments)]
    pub fn insert_suggestion(
        &self,
        source: &str,
        email_id: Option<&str>,
        sender: Option<&str>,
        subject: &str,
        kind: &str,
        title: &str,
        description: &str,
        category_id: &str,
        priority: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
        deadline_at: Option<i64>,
        prep_min: u32,
        location: &str,
        tags: &str,
        confidence: f64,
        reason: &str,
        dedupe_task_id: Option<i64>,
        dedupe_note: &str,
        status: &str,
    ) -> rusqlite::Result<i64> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO suggested_events
             (source, source_email_id, source_sender, source_subject, kind, title, description,
              category_id, priority, start_at, end_at, deadline_at, prep_min, location, tags,
              confidence, reason, status, dedupe_task_id, dedupe_note, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?21)",
            rusqlite::params![
                source, email_id, sender, subject, kind, title, description,
                sanitize_category(category_id), priority, start_at, end_at, deadline_at, prep_min as i64,
                location, tags, confidence, reason, status, dedupe_task_id, dedupe_note, now
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// ¿Ya existe una sugerencia pendiente (o auto-aprobada) equivalente de
    /// OTRO correo? Mismo compromiso en varios correos → una sola sugerencia
    /// (fase 8). `exclude_email_id` evita chocar con la del propio correo.
    /// Sin fecha (intent "task"): ventana completa, decide `title_similar`.
    pub fn find_similar_suggestion(
        &self,
        title: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
        exclude_email_id: Option<&str>,
    ) -> rusqlite::Result<Option<(i64, String)>> {
        let window_start = start_at.unwrap_or(i64::MIN).saturating_sub(48 * 3_600_000);
        let window_end = end_at.unwrap_or(i64::MAX).saturating_add(48 * 3_600_000);
        // `start_at IS NULL` (compromiso sin fecha, p. ej. intent "task"
        // "llamar a Juan") NUNCA pasa un BETWEEN — caía del dedupe y el mismo
        // compromiso en varios correos generaba una sugerencia por correo
        // (bug A2). Solo cuando la BUSQUEDA es sin fecha participan las
        // sugerencias sin fecha: una nueva CON fecha no debe fusionarse con
        // una vaga de otro correo (perdería su hora concreta).
        let null_clause = if start_at.is_some() {
            ""
        } else {
            "start_at IS NULL OR "
        };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id, title FROM suggested_events
             WHERE status IN ('pending','auto_approved')
               AND ({null_clause} start_at BETWEEN ?1 AND ?2)
               AND (?4 IS NULL OR source_email_id != ?4)
             ORDER BY created_at ASC",
        ))?;
        let candidates: Vec<(i64, String)> = stmt
            .query_map(
                rusqlite::params![window_start, window_end, window_end, exclude_email_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .collect::<Result<_, _>>()?;
        for (id, t) in candidates {
            if title_similar(title, &t) {
                return Ok(Some((id, t)));
            }
        }
        Ok(None)
    }

    /// Elimina la sugerencia por completo (control del usuario: "borrar").
    /// Si tenía tareas creadas (una o varias — rango multi-día), también se
    /// borran (bug A1: antes solo se borraba result_task_id).
    pub fn delete_suggestion(&self, id: i64) -> rusqlite::Result<()> {
        self.unlink_and_delete_suggestion_tasks(id)?;
        self.conn
            .execute("DELETE FROM suggested_events WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn list_suggestions(
        &self,
        only_pending: bool,
        retention_ms: i64,
    ) -> rusqlite::Result<Vec<SuggestionRow>> {
        let sql = if only_pending {
            "SELECT * FROM suggested_events WHERE status = 'pending' ORDER BY created_at DESC"
        } else {
            // las resueltas (aceptada/rechazada/fusionada) solo se muestran durante la retención
            "SELECT * FROM suggested_events
             WHERE status = 'pending'
                OR (status IN ('accepted','rejected','merged','auto_approved') AND updated_at >= ?1)
             ORDER BY created_at DESC LIMIT 300"
        };
        let mut stmt = self.conn.prepare(sql)?;
        if only_pending {
            let rows = stmt.query_map([], suggestion_from_row)?;
            rows.collect()
        } else {
            let cutoff = now_ms() - retention_ms;
            let rows = stmt.query_map([cutoff], suggestion_from_row)?;
            rows.collect()
        }
    }

    pub fn set_suggestion_status(&self, id: i64, status: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE suggested_events SET status = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, status, now_ms()],
        )?;
        Ok(())
    }

    /// Transacción sobre la conexión compartida: BEGIN IMMEDIATE/COMMIT/
    /// ROLLBACK manual porque los métodos de `Db` toman `&self` y una
    /// `rusqlite::Transaction` necesitaría `&mut` (auditoría 17, hallazgo #5).
    pub fn tx_begin(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")
    }

    pub fn tx_commit(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("COMMIT")
    }

    pub fn tx_rollback(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch("ROLLBACK")
    }

    /// ¿Este correo ya fue revisado? (registro persistente, migración 0011)
    pub fn email_seen(&self, message_id: &str) -> rusqlite::Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM email_seen WHERE message_id = ?1",
            [message_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Marca un correo como revisado (idempotente).
    pub fn email_mark_seen(&self, message_id: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO email_seen (message_id, seen_at) VALUES (?1, ?2)",
            rusqlite::params![message_id, now_ms()],
        )?;
        Ok(())
    }

    /// Cuántos Message-ID distintos hay registrados como revisados.
    #[cfg(test)]
    pub fn email_seen_count(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM email_seen", [], |r| r.get(0))
    }

    /// Backfill del registro de vistos desde sugerencias existentes (lo usa
    /// la migración 0011 sobre una BD vieja; público para poder testearlo).
    #[doc(hidden)]
    pub fn email_seen_backfill(&self) -> rusqlite::Result<usize> {
        self.conn.execute(
            "INSERT OR IGNORE INTO email_seen (message_id, seen_at)
             SELECT source_email_id, created_at
             FROM suggested_events
             WHERE source_email_id IS NOT NULL AND source_email_id != ''",
            [],
        )
    }

    /// Auto-archiva (borra) sugerencias resueltas más antiguas que el corte.
    pub fn prune_suggestions(&self, cutoff_ms: i64) -> rusqlite::Result<usize> {
        let n = self.conn.execute(
            "DELETE FROM suggested_events WHERE status != 'pending' AND updated_at < ?1",
            [cutoff_ms],
        )?;
        Ok(n)
    }

    pub fn set_suggestion_result_task(&self, id: i64, task_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE suggested_events SET result_task_id = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, task_id, now_ms()],
        )?;
        Ok(())
    }

    /// Registra UNA de las tareas creadas por una sugerencia (puede ser más
    /// de una: rangos multi-día generan bloque de inicio + "(entrega)").
    pub fn link_suggestion_task(&self, suggestion_id: i64, task_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO suggestion_tasks (suggestion_id, task_id) VALUES (?1, ?2)",
            rusqlite::params![suggestion_id, task_id],
        )?;
        Ok(())
    }

    /// Todas las tareas creadas por la sugerencia (orden de creación).
    pub fn suggestion_task_ids(&self, suggestion_id: i64) -> rusqlite::Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id FROM suggestion_tasks WHERE suggestion_id = ?1 ORDER BY rowid",
        )?;
        let rows = stmt.query_map([suggestion_id], |r| r.get(0))?;
        let ids: Vec<i64> = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        // compatibilidad: sugerencias aceptadas antes de la migración 0014
        // solo tenían result_task_id
        if ids.is_empty() {
            let main: Option<i64> = self
                .conn
                .query_row(
                    "SELECT result_task_id FROM suggested_events
                     WHERE id = ?1 AND result_task_id IS NOT NULL AND result_task_id != 0",
                    [suggestion_id],
                    |r| r.get(0),
                )
                .optional()?;
            return Ok(main.into_iter().collect());
        }
        Ok(ids)
    }

    /// Desvincula y elimina TODAS las tareas creadas por la sugerencia.
    /// Devuelve cuántas tareas existían todavía (las ya borradas no cuentan).
    pub fn unlink_and_delete_suggestion_tasks(
        &self,
        suggestion_id: i64,
    ) -> rusqlite::Result<usize> {
        let ids = self.suggestion_task_ids(suggestion_id)?;
        let mut alive = 0usize;
        for tid in &ids {
            if self.get_task(*tid)?.is_some() {
                self.delete(*tid)?;
                alive += 1;
            }
        }
        self.conn.execute(
            "DELETE FROM suggestion_tasks WHERE suggestion_id = ?1",
            [suggestion_id],
        )?;
        Ok(alive)
    }

    pub fn update_suggestion_data(
        &self,
        id: i64,
        title: &str,
        category_id: &str,
        priority: &str,
        start_at: i64,
        end_at: i64,
        description: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE suggested_events SET title = ?2, category_id = ?3, priority = ?4,
                    start_at = ?5, end_at = ?6, description = ?7, updated_at = ?8,
                    deadline_at = CASE WHEN deadline_at IS NOT NULL THEN ?6 ELSE deadline_at END
             WHERE id = ?1",
            rusqlite::params![
                id,
                title,
                sanitize_category(category_id),
                sanitize_priority(priority),
                start_at,
                end_at,
                description,
                now_ms()
            ],
        )?;
        Ok(())
    }

    pub fn get_suggestion(&self, id: i64) -> rusqlite::Result<Option<SuggestionRow>> {
        self.conn
            .query_row(
                "SELECT * FROM suggested_events WHERE id = ?1",
                [id],
                suggestion_from_row,
            )
            .optional()
    }

    pub fn suggestion_count_for_email(&self, email_id: &str) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM suggested_events WHERE source_email_id = ?1 AND status != 'rejected'",
            [email_id],
            |r| r.get(0),
        )
    }

    /// Compromiso ya sugerido por OTRO correo del mismo hilo (las
    /// referencias `In-Reply-To`/`References` permiten vincular conversación).
    /// La corrección dentro de un hilo (título similar) NO debe duplicar la
    /// sugerencia original: devuelve la sugerencia predecesora.
    pub fn find_similar_suggestion_in_thread(
        &self,
        title: &str,
        thread_ids: &[String],
        exclude_email_id: Option<&str>,
    ) -> rusqlite::Result<Option<(i64, String)>> {
        if thread_ids.is_empty() {
            return Ok(None);
        }
        let placeholders = vec!["?"; thread_ids.len()].join(",");
        let sql = format!(
            "SELECT id, title FROM suggested_events
             WHERE status IN ('pending','auto_approved')
               AND source_email_id IS NOT NULL
               AND source_email_id IN ({placeholders})
               AND (? IS NULL OR source_email_id != ?)
             ORDER BY created_at ASC"
        );
        let mut params: Vec<&dyn rusqlite::ToSql> = thread_ids
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();
        params.push(&exclude_email_id);
        params.push(&exclude_email_id);
        let mut stmt = self.conn.prepare(&sql)?;
        let candidates: Vec<(i64, String)> = stmt
            .query_map(rusqlite::params_from_iter(params), |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<Result<_, _>>()?;
        for (id, t) in candidates {
            if crate::store::title_similar(title, &t) {
                return Ok(Some((id, t)));
            }
        }
        Ok(None)
    }

    // ---- trusted senders ----
    pub fn trusted_add(&self, sender: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO trusted_senders (sender, added_at) VALUES (?1, ?2)",
            rusqlite::params![sender, now_ms()],
        )?;
        Ok(())
    }

    pub fn trusted_remove(&self, sender: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM trusted_senders WHERE sender = ?1", [sender])?;
        Ok(())
    }

    pub fn trusted_list(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT sender FROM trusted_senders ORDER BY sender")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }

    pub fn is_trusted(&self, sender: &str) -> rusqlite::Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM trusted_senders WHERE sender = ?1",
            [sender],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    // ---- sync state / history ----
    pub fn sync_state_get(&self, source: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT checkpoint FROM sync_state WHERE source = ?1",
                [source],
                |r| r.get(0),
            )
            .optional()
    }

    /// Resetea todos los checkpoints (rescan de la ventana reciente).
    pub fn sync_state_clear_all(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM sync_state", [])?;
        Ok(())
    }

    pub fn sync_state_set(
        &self,
        source: &str,
        checkpoint: &str,
        result: &str,
        error: &str,
    ) -> rusqlite::Result<()> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO sync_state (source, checkpoint, last_result, last_error, last_run_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)
             ON CONFLICT(source) DO UPDATE SET checkpoint = excluded.checkpoint,
                last_result = excluded.last_result, last_error = excluded.last_error,
                last_run_at = excluded.last_run_at, updated_at = excluded.updated_at",
            rusqlite::params![source, checkpoint, result, error, now],
        )?;
        Ok(())
    }

    pub fn sync_state_all(&self) -> rusqlite::Result<Vec<SyncStateRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, checkpoint, last_result, last_error, last_run_at FROM sync_state ORDER BY source",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SyncStateRow {
                source: r.get(0)?,
                checkpoint: r.get(1)?,
                last_result: r.get(2)?,
                last_error: r.get(3)?,
                last_run_at: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn sync_history_add(
        &self,
        source: &str,
        started_at: i64,
        result: &str,
        items_found: i64,
        items_processed: i64,
        error: &str,
        note: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO sync_history (source, started_at, finished_at, result, items_found, items_processed, error, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![source, started_at, now_ms(), result, items_found, items_processed, error, note],
        )?;
        Ok(())
    }

    pub fn sync_history_last(&self, limit: i64) -> rusqlite::Result<Vec<SyncHistoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, started_at, finished_at, result, items_found, items_processed, error, note
             FROM sync_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(SyncHistoryRow {
                id: r.get(0)?,
                source: r.get(1)?,
                started_at: r.get(2)?,
                finished_at: r.get(3)?,
                result: r.get(4)?,
                items_found: r.get(5)?,
                items_processed: r.get(6)?,
                error: r.get(7)?,
                note: r.get(8)?,
            })
        })?;
        rows.collect()
    }

    pub fn sync_history_today(
        &self,
        start_of_day_ms: i64,
    ) -> rusqlite::Result<Vec<SyncHistoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source, started_at, finished_at, result, items_found, items_processed, error, note
             FROM sync_history WHERE started_at >= ?1 ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([start_of_day_ms], |r| {
            Ok(SyncHistoryRow {
                id: r.get(0)?,
                source: r.get(1)?,
                started_at: r.get(2)?,
                finished_at: r.get(3)?,
                result: r.get(4)?,
                items_found: r.get(5)?,
                items_processed: r.get(6)?,
                error: r.get(7)?,
                note: r.get(8)?,
            })
        })?;
        rows.collect()
    }
}

fn suggestion_from_row(r: &rusqlite::Row) -> rusqlite::Result<SuggestionRow> {
    Ok(SuggestionRow {
        id: r.get("id")?,
        source: r.get("source")?,
        source_email_id: r.get("source_email_id")?,
        source_sender: r.get("source_sender")?,
        source_subject: r.get("source_subject")?,
        kind: r.get("kind")?,
        title: r.get("title")?,
        description: r.get("description")?,
        category_id: r.get("category_id")?,
        priority: r.get("priority")?,
        start_at: r.get("start_at")?,
        end_at: r.get("end_at")?,
        deadline_at: r.get("deadline_at")?,
        prep_min: r.get("prep_min")?,
        location: r.get("location")?,
        tags: r.get("tags")?,
        confidence: r.get("confidence")?,
        reason: r.get("reason")?,
        status: r.get("status")?,
        dedupe_task_id: r.get("dedupe_task_id")?,
        dedupe_note: r.get("dedupe_note")?,
        result_task_id: r.get("result_task_id")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

#[derive(Serialize, Clone)]
pub struct SuggestionRow {
    pub id: i64,
    pub source: String,
    pub source_email_id: Option<String>,
    pub source_sender: Option<String>,
    pub source_subject: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub category_id: String,
    pub priority: String,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub deadline_at: Option<i64>,
    pub prep_min: u32,
    pub location: String,
    pub tags: String,
    pub confidence: f64,
    pub reason: String,
    pub status: String,
    pub dedupe_task_id: Option<i64>,
    pub dedupe_note: String,
    pub result_task_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Clone)]
pub struct PlanProposalRow {
    pub id: i64,
    pub text: String,
    pub status: String,
    pub payload: String,
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn plan_proposal_from_row(r: &rusqlite::Row) -> rusqlite::Result<PlanProposalRow> {
    Ok(PlanProposalRow {
        id: r.get(0)?,
        text: r.get(1)?,
        status: r.get(2)?,
        payload: r.get(3)?,
        source: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

fn assistant_action_from_row(r: &rusqlite::Row) -> rusqlite::Result<AssistantActionRow> {
    Ok(AssistantActionRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        payload: r.get(2)?,
        status: r.get(3)?,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

#[derive(Serialize, Clone)]
pub struct AssistantActionRow {
    pub id: i64,
    pub kind: String,
    pub payload: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Clone)]
pub struct SyncStateRow {
    pub source: String,
    pub checkpoint: String,
    pub last_result: String,
    pub last_error: String,
    pub last_run_at: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct SyncHistoryRow {
    pub id: i64,
    pub source: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub result: String,
    pub items_found: i64,
    pub items_processed: i64,
    pub error: String,
    pub note: String,
}

fn title_similar(a: &str, b: &str) -> bool {
    let norm = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|w| w.to_string())
            .collect()
    };
    let ta = norm(a);
    let tb = norm(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    let common = ta.iter().filter(|w| tb.contains(w)).count();
    let union = ta.len() + tb.len() - common;
    common as f64 / union as f64 >= 0.6
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        Db::open_memory().unwrap()
    }

    #[test]
    fn migration_0005_adds_fired_column() {
        let db = db();
        let cols: Vec<String> = db
            .conn
            .prepare("SELECT name FROM pragma_table_info('tasks')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(cols.iter().any(|c| c == "reminder_fired_at"));
    }

    #[test]
    fn backup_rotating_creates_copy_on_reopen() {
        let dir = std::env::temp_dir().join(format!("ff-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let db = Db::open(&dir).unwrap();
            db.create("x", "uni", "alta", 1, 2, false).unwrap();
        }
        // segundo arranque: rota y genera focusflow.db.bak
        let db = Db::open(&dir).unwrap();
        assert!(dir.join("focusflow.db.bak").exists());
        db.create("nueva-tras-backup", "uni", "alta", 1, 2, false).unwrap();
        drop(db);
        // tercer arranque: el .bak debe reflejar los datos NUEVOS (antes
        // VACUUM INTO fallaba con destino existente y quedaba congelado)
        drop(Db::open(&dir).unwrap());
        let bak = Connection::open(dir.join("focusflow.db.bak")).unwrap();
        let n: i64 = bak
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE title = 'nueva-tras-backup'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, ".bak actualizado");
        assert!(dir.join("focusflow.db.bak.1").exists(), "generación anterior");
        assert!(!dir.join("focusflow.db.bak.tmp").exists());
        drop(bak);
        // open_no_backup no rota
        let before = std::fs::metadata(dir.join("focusflow.db.bak")).unwrap().modified().unwrap();
        drop(Db::open_no_backup(&dir).unwrap());
        let after = std::fs::metadata(dir.join("focusflow.db.bak")).unwrap().modified().unwrap();
        assert_eq!(before, after);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reminder_minutes_are_clamped() {
        let db = db();
        let now = now_ms();
        let t = db
            .create(
                "clamp",
                "uni",
                "media",
                now + 3_600_000,
                now + 7_200_000,
                false,
            )
            .unwrap();
        // negativo → 0 (dispara al inicio, nunca después); gigante → 4 semanas
        db.set_task_reminder(t.id, -30).unwrap();
        assert_eq!(
            db.get_task(t.id).unwrap().unwrap().reminder_minutes,
            Some(0)
        );
        db.set_task_reminder(t.id, i64::MAX).unwrap();
        assert_eq!(
            db.get_task(t.id).unwrap().unwrap().reminder_minutes,
            Some(MAX_REMINDER_MINUTES)
        );
    }

    #[test]
    fn due_reminders_fires_overdue_and_marks_once() {
        let db = db();
        let now = now_ms();
        let t = db
            .create(
                "parcial",
                "uni",
                "alta",
                now - 2 * 3_600_000,
                now - 1_800_000,
                false,
            )
            .unwrap();
        db.set_task_reminder(t.id, 60).unwrap();

        let due = db.due_reminders(now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].task_id, t.id);
        assert_eq!(due[0].reminder_minutes, 60);

        db.mark_reminder_fired(t.id).unwrap();
        assert!(db.due_reminders(now).unwrap().is_empty());
    }

    #[test]
    fn due_reminders_ignores_future_and_completed() {
        let db = db();
        let now = now_ms();

        let futura = db
            .create(
                "futura",
                "uni",
                "media",
                now + 3_600_000 * 2,
                now + 5_400_000,
                false,
            )
            .unwrap();
        db.set_task_reminder(futura.id, 60).unwrap();
        assert!(db.due_reminders(now).unwrap().is_empty());

        let pasada = db
            .create(
                "pasada",
                "uni",
                "media",
                now - 3_600_000,
                now - 1_800_000,
                false,
            )
            .unwrap();
        db.set_task_reminder(pasada.id, 60).unwrap();
        db.set_completed(pasada.id, true).unwrap();
        assert!(db.due_reminders(now).unwrap().is_empty());
    }

    #[test]
    fn reminder_rearms_only_when_changed() {
        let db = db();
        let now = now_ms();
        let t = db
            .create(
                "tarea",
                "uni",
                "media",
                now - 3_600_000,
                now - 1_800_000,
                false,
            )
            .unwrap();
        db.set_task_reminder(t.id, 60).unwrap();
        db.mark_reminder_fired(t.id).unwrap();
        assert!(db.due_reminders(now).unwrap().is_empty());

        // cambiar el recordatorio → rearma
        db.update_task_full(
            t.id,
            "tarea",
            "uni",
            "media",
            now - 3_600_000,
            now - 1_800_000,
            "",
            "[]",
            "",
            "",
            Some(30),
            Some(false),
        )
        .unwrap();
        assert_eq!(db.due_reminders(now).unwrap().len(), 1);

        // mismo valor → no refira
        db.mark_reminder_fired(t.id).unwrap();
        db.update_task_full(
            t.id,
            "tarea",
            "uni",
            "media",
            now - 3_600_000,
            now - 1_800_000,
            "",
            "[]",
            "",
            "",
            Some(30),
            Some(false),
        )
        .unwrap();
        assert!(db.due_reminders(now).unwrap().is_empty());
    }

    #[test]
    fn move_to_rearms_reminder_only_when_start_changes() {
        // Corregido (auditoría lote A #12): el test anterior
        // (`move_to_does_not_refire`) codificaba el bug — mover un evento ya
        // avisado a otra hora NO volvía a avisar nunca.
        let db = db();
        let now = now_ms();
        let t = db
            .create(
                "tarea",
                "uni",
                "media",
                now - 3_600_000,
                now - 1_800_000,
                false,
            )
            .unwrap();
        db.set_task_reminder(t.id, 60).unwrap();
        db.mark_reminder_fired(t.id).unwrap();
        // misma hora de inicio (solo cambia el fin) → no re-dispara
        db.move_to(t.id, now - 3_600_000, now - 600_000, None)
            .unwrap();
        assert!(db.due_reminders(now).unwrap().is_empty());
        // nueva hora de inicio → rearmado (ventana 60 min ya vencida: vence ya)
        db.move_to(t.id, now + 3_600_000, now + 5_400_000, None)
            .unwrap();
        assert_eq!(db.due_reminders(now).unwrap().len(), 1);
        // antes de la nueva ventana, no vence
        assert!(db.due_reminders(now - 60_000).unwrap().is_empty());
    }

    #[test]
    fn update_task_full_rearms_reminder_when_start_changes() {
        let db = db();
        let now = now_ms();
        let t = db
            .create("tarea", "uni", "media", now - 3_600_000, now - 1_800_000, false)
            .unwrap();
        db.set_task_reminder(t.id, 60).unwrap();
        db.mark_reminder_fired(t.id).unwrap();
        // mismo recordatorio, NUEVA fecha → vuelve a avisar
        db.update_task_full(
            t.id,
            "tarea",
            "uni",
            "media",
            now + 1_800_000,
            now + 3_600_000,
            "",
            "[]",
            "",
            "",
            Some(60),
            Some(false),
        )
        .unwrap();
        assert_eq!(db.due_reminders(now).unwrap().len(), 1);
    }

    #[test]
    fn auth_update_access_only_touches_same_session() {
        let db = db();
        let s = AuthSession {
            user_id: "u1".into(),
            email: "a@x.com".into(),
            name: "A".into(),
            access_token: "old".into(),
            refresh_token: "r1".into(),
            expires_at: 1,
        };
        db.auth_save(&s).unwrap();
        assert_eq!(db.auth_update_access("u1", "r1", "new", 99).unwrap(), 1);
        let got = db.auth_load().unwrap().unwrap();
        assert_eq!(got.access_token, "new");
        assert_eq!(got.expires_at, 99);
        // la sesión se cerró mientras se refrescaba → NO resucita
        db.auth_clear().unwrap();
        assert_eq!(db.auth_update_access("u1", "r1", "zombie", 5).unwrap(), 0);
        assert!(db.auth_load().unwrap().is_none());
        // otra cuenta entró entretanto → no se pisa su token
        db.auth_save(&AuthSession {
            user_id: "u2".into(),
            refresh_token: "r2".into(),
            access_token: "b".into(),
            ..s.clone()
        })
        .unwrap();
        assert_eq!(db.auth_update_access("u1", "r1", "zombie", 5).unwrap(), 0);
        assert_eq!(db.auth_load().unwrap().unwrap().access_token, "b");
        // invalid_grant: vaciar tokens de ESA sesión
        assert_eq!(db.auth_drop_tokens("u2", "r2").unwrap(), 1);
        let got = db.auth_load().unwrap().unwrap();
        assert!(got.access_token.is_empty() && got.refresh_token.is_empty());
        assert_eq!(got.email, "a@x.com");
    }

    fn ins_suggestion(
        db: &Db,
        email_id: &str,
        subject: &str,
        kind: &str,
        title: &str,
        start: i64,
    ) -> i64 {
        db.insert_suggestion(
            "email",
            Some(email_id),
            Some("a@b.c"),
            subject,
            kind,
            title,
            "",
            "otr",
            "media",
            Some(start),
            Some(start + 3_600_000),
            None,
            0,
            "",
            "[]",
            0.9,
            "test",
            None,
            "",
            "pending",
        )
        .unwrap()
    }

    #[test]
    fn migration_0007_adds_email_intel_columns() {
        let db = db();
        let cols: Vec<String> = db
            .conn
            .prepare("SELECT name FROM pragma_table_info('suggested_events')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for c in ["kind", "deadline_at", "prep_min", "source_subject"] {
            assert!(cols.iter().any(|x| x == c), "falta columna {c}");
        }
    }

    #[test]
    fn insert_suggestion_roundtrips_new_fields() {
        let db = db();
        let id = db
            .insert_suggestion(
                "email",
                Some("m1"),
                Some("jefe@x.com"),
                "Asunto",
                "deadline",
                "Informe",
                "desc",
                "tra",
                "alta",
                Some(5),
                Some(5),
                Some(5),
                240,
                "",
                "[]",
                0.9,
                "entrega",
                None,
                "",
                "pending",
            )
            .unwrap();
        let s = db.get_suggestion(id).unwrap().unwrap();
        assert_eq!(s.kind, "deadline");
        assert_eq!(s.source_subject, "Asunto");
        assert_eq!(s.deadline_at, Some(5));
        assert_eq!(s.prep_min, 240);
    }

    #[test]
    fn find_similar_suggestion_dedupes_across_emails() {
        let db = db();
        let start = now_ms() + 86_400_000;
        let first = ins_suggestion(
            &db,
            "email-1",
            "Re: Informe",
            "event",
            "Informe del proyecto",
            start,
        );
        // mismo compromiso desde OTRO correo → detectado
        let (id, _) = db
            .find_similar_suggestion(
                "Informe del proyecto",
                Some(start),
                Some(start + 3_600_000),
                Some("email-2"),
            )
            .unwrap()
            .expect("duplicado detectado");
        assert_eq!(id, first);
        // el mismo correo que la creó no debe chocar consigo mismo
        assert!(
            db.find_similar_suggestion(
                "Informe del proyecto",
                Some(start),
                Some(start + 3_600_000),
                Some("email-1")
            )
            .unwrap()
            .is_none(),
            "excluye su propio correo"
        );
        // título distinto → no duplicado
        assert!(db
            .find_similar_suggestion(
                "Cena familiar",
                Some(start),
                Some(start + 3_600_000),
                Some("email-2")
            )
            .unwrap()
            .is_none());
        // ventana muy lejana → no duplicado
        assert!(db
            .find_similar_suggestion(
                "Informe del proyecto",
                Some(start + 30 * 86_400_000),
                Some(start + 30 * 86_400_000 + 3_600_000),
                Some("email-2")
            )
            .unwrap()
            .is_none());
    }

    #[test]
    fn find_similar_suggestion_without_date_does_not_overflow() {
        let db = db();
        let start = now_ms() + 86_400_000;
        ins_suggestion(
            &db,
            "email-1",
            "Re: Tareas",
            "task",
            "Enviar informe semanal",
            start,
        );
        // intents "task" sin fecha (None) → antes paniqueaba con overflow (i64::MIN - 48h)
        let r = db
            .find_similar_suggestion("Enviar informe semanal", None, None, Some("email-2"))
            .unwrap();
        assert!(r.is_some(), "dedupe por título sin fecha");
        let none = db
            .find_similar_suggestion("Otra cosa distinta", None, None, Some("email-2"))
            .unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn delete_suggestion_removes_row_and_created_task() {
        let db = db();
        let now = now_ms();
        let id = ins_suggestion(&db, "email-1", "Asunto", "event", "Reunión", now);
        db.set_suggestion_status(id, "accepted").unwrap();
        let t = db
            .create("Reunión", "tra", "media", now, now + 3_600_000, false)
            .unwrap();
        db.set_suggestion_result_task(id, t.id).unwrap();

        db.delete_suggestion(id).unwrap();
        assert!(db.get_suggestion(id).unwrap().is_none());
        assert!(
            db.get_task(t.id).unwrap().is_none(),
            "tarea creada también borrada"
        );
    }

    #[test]
    fn find_similar_suggestion_in_thread_links_reply_to_parent() {
        let db = db();
        let start = now_ms() + 86_400_000;
        // email-1 anuncia el compromiso ("entrega viernes")
        ins_suggestion(
            &db,
            "email-1",
            "Entrega proyecto",
            "deadline",
            "Entregar proyecto",
            start,
        );
        // email-2 es respuesta del hilo (References incluye email-1) y corrige
        let thread = vec!["email-1".to_string()];
        let hit = db
            .find_similar_suggestion_in_thread("Entregar proyecto", &thread, Some("email-2"))
            .unwrap();
        assert!(hit.is_some(), "corrección del hilo → no duplicar");
        // sin referencias → no hay vínculo de hilo
        let none = db
            .find_similar_suggestion_in_thread("Entregar proyecto", &[], Some("email-2"))
            .unwrap();
        assert!(none.is_none());
        // título distinto en el hilo → compromiso nuevo
        let other = db
            .find_similar_suggestion_in_thread("Cena de cumpleaños", &thread, Some("email-2"))
            .unwrap();
        assert!(other.is_none());
    }

    #[test]
    fn assistant_actions_roundtrip() {
        let db = db();
        let id = db
            .insert_assistant_action("complete", r#"{"kind":"complete"}"#)
            .unwrap();
        let row = db.get_assistant_action(id).unwrap().unwrap();
        assert_eq!(row.kind, "complete");
        assert_eq!(row.status, "pending");
        assert_eq!(db.list_assistant_actions(true).unwrap().len(), 1);
        db.set_assistant_action_status(id, "accepted").unwrap();
        assert!(db.list_assistant_actions(true).unwrap().is_empty());
        assert_eq!(db.list_assistant_actions(false).unwrap().len(), 1);
    }

    #[test]
    fn set_task_status_marks_en_curso() {
        let db = db();
        let now = now_ms();
        let t = db
            .create("Estudiar", "uni", "media", now, now + 3_600_000, false)
            .unwrap();
        assert_eq!(db.get_task(t.id).unwrap().unwrap().status, "pendiente");
        db.set_task_status(t.id, "en-curso").unwrap();
        assert_eq!(db.get_task(t.id).unwrap().unwrap().status, "en-curso");
        db.set_task_status(t.id, "completada").unwrap();
        assert_eq!(db.get_task(t.id).unwrap().unwrap().status, "completada");
    }

    #[test]
    fn find_similar_task_dedupes_against_existing_tasks() {
        let db = db();
        let start = now_ms() + 86_400_000;
        let t = db
            .create(
                "Informe del proyecto",
                "uni",
                "alta",
                start,
                start + 3_600_000,
                false,
            )
            .unwrap();
        // mismo título cerca de la fecha → detectado (dedupe de sugerencias)
        let (id, title) = db
            .find_similar_task("Informe del proyecto", start + 3_600_000, "remitente@x.com")
            .unwrap()
            .expect("duplicado de tarea detectado");
        assert_eq!(id, t.id);
        assert_eq!(title, "Informe del proyecto");
        // mismo remitente con título idéntico pero lejos en el tiempo → no
        assert!(db
            .find_similar_task(
                "Informe del proyecto",
                start + 40 * 86_400_000,
                "remitente@x.com"
            )
            .unwrap()
            .is_none());
        // título distinto → no
        assert!(db
            .find_similar_task("Cena familiar", start, "remitente@x.com")
            .unwrap()
            .is_none());
    }

    #[test]
    fn export_contains_data_but_never_secrets() {
        let db = db();
        let now = now_ms();
        db.create("Tarea A", "uni", "media", now, now + 3_600_000, false)
            .unwrap();
        db.insert_suggestion(
            "email",
            Some("m1"),
            Some("x@y.com"),
            "Asunto",
            "task",
            "Compromiso X",
            "",
            "uni",
            "media",
            None,
            None,
            None,
            0,
            "",
            "[]",
            0.9,
            "razón",
            None,
            "",
            "pending",
        )
        .unwrap();
        db.settings_set("ai.endpoint", "http://x").unwrap();
        db.settings_set("ai.model", "m").unwrap();
        db.settings_set("ai.provider", "openai").unwrap();
        db.settings_set(
            "email.config",
            r#"{"host":"imap.gmail.com","user":"x@y.com"}"#,
        )
        .unwrap();
        db.settings_set("email.rescan_pending", "1").unwrap();
        let v = db.export_data().unwrap();
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("Tarea A"), "tareas exportadas");
        assert!(s.contains("Compromiso X"), "sugerencias exportadas");
        assert!(!s.contains("rescan_pending"), "settings internos excluidos");
        assert!(!s.contains("ai.endpoint"), "config IA excluida");
        assert!(!s.contains("ai.model"), "config IA excluida");
        assert!(!s.contains("ai.provider"), "config IA excluida");
        assert!(!s.contains("imap.gmail.com"), "config correo excluida");
    }

    #[test]
    fn wipe_clears_user_data_and_settings() {
        let db = db();
        let now = now_ms();
        let t = db
            .create("Tarea A", "uni", "media", now, now + 3_600_000, false)
            .unwrap();
        db.settings_set("ui.theme", "dark").unwrap();
        db.wipe_data().unwrap();
        assert!(db.list().unwrap().is_empty(), "tareas borradas");
        assert!(
            db.settings_get("ui.theme").unwrap().is_none(),
            "settings borradas"
        );
        let _ = t;
    }

    #[test]
    fn import_data_roundtrip_and_idempotent() {
        let db1 = Db::open_memory_clean_pub().unwrap();
        let now = now_ms();
        db1.create("Tarea Importable", "uni", "alta", now, now + 3_600_000, false)
            .unwrap();
        db1.settings_set("ui.theme", "dark").unwrap();
        let exported = db1.export_data().unwrap();
        let json_str = serde_json::to_string(&exported).unwrap();

        // Importar en una BD vacía
        let db2 = Db::open_memory_clean_pub().unwrap();
        let summary = db2.import_data(&json_str).unwrap();
        assert_eq!(summary.tasks, 1);
        assert_eq!(summary.settings, 1);
        let tasks = db2.list().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Tarea Importable");
        assert_eq!(db2.settings_get("ui.theme").unwrap().as_deref(), Some("dark"));

        // Importar de nuevo: debe ser idempotente (0 tareas nuevas insertadas)
        let summary2 = db2.import_data(&json_str).unwrap();
        assert_eq!(summary2.tasks, 0);
        assert_eq!(db2.list().unwrap().len(), 1);
    }

    #[test]
    fn import_collision_keeps_distinct_rows_and_remaps_refs() {
        // Lote B #9: un id del JSON que ya existe localmente con OTRA fila no
        // debe descartarse en silencio: se inserta con id nuevo y las
        // referencias (study_sessions.task_id) se remapean.
        let now = now_ms();
        let db1 = Db::open_memory_clean_pub().unwrap();
        let a = db1
            .create("Tarea remota", "uni", "alta", now, now + 3_600_000, false)
            .unwrap();
        db1.study_create("Repaso", now + 7_200_000, now + 10_800_000, Some(a.id), "")
            .unwrap();
        let json = serde_json::to_string(&db1.export_data().unwrap()).unwrap();

        let db2 = Db::open_memory_clean_pub().unwrap();
        let local = db2
            .create("Tarea local", "per", "baja", now, now + 3_600_000, false)
            .unwrap();
        assert_eq!(local.id, a.id, "mismo id en ambas DBs: colisión");
        let summary = db2.import_data(&json).unwrap();
        assert_eq!(summary.tasks, 1, "la tarea distinta se importa");
        let tasks = db2.list().unwrap();
        assert_eq!(tasks.len(), 2);
        let remote = tasks.iter().find(|t| t.title == "Tarea remota").unwrap();
        assert_ne!(remote.id, local.id);
        let sessions = db2.study_list().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].task_id, Some(remote.id), "referencia remapeada");
        // re-importar el mismo export sigue siendo idempotente
        let again = db2.import_data(&json).unwrap();
        assert_eq!(again.tasks, 0);
        assert_eq!(again.study_sessions, 0);
        assert_eq!(db2.list().unwrap().len(), 2);
    }

    #[test]
    fn import_skips_invalid_class_without_aborting() {
        // Lote B #10: una clase inválida abortaba todo el import.
        let json = serde_json::json!({
            "app": "focusflow",
            "tasks": [{"id": 1, "title": "T", "start_at": 1, "end_at": 2, "created_at": 5}],
            "classes": [
                {"id": 1, "title": "Mala", "day_of_week": 0, "start_min": 600,
                 "end_min": 540, "start_date": 0, "end_date": 10},
                {"id": 2, "title": "Buena", "day_of_week": 1, "start_min": 480,
                 "end_min": 540, "start_date": 0, "end_date": 10}
            ]
        })
        .to_string();
        let db = Db::open_memory_clean_pub().unwrap();
        let summary = db.import_data(&json).unwrap();
        assert_eq!(summary.classes, 1);
        assert_eq!(summary.tasks, 1);
        let classes = db.class_list().unwrap();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].title, "Buena");
    }

    #[test]
    fn wipe_clears_suggestion_task_links() {
        // Lote B #12: wipe_data no borraba suggestion_tasks
        let db = db();
        db.link_suggestion_task(7, 9).unwrap();
        db.wipe_data().unwrap();
        assert!(db.suggestion_task_ids(7).unwrap().is_empty());
    }

    #[test]
    fn deleting_task_unlinks_study_sessions() {
        // Lote B #6: borrar la tarea dejaba study_sessions.task_id colgando y
        // editar la sesión fallaba con "La tarea relacionada no existe"
        let db = db();
        let now = now_ms();
        let t = db
            .create("Tarea", "uni", "media", now, now + 3_600_000, false)
            .unwrap();
        let s = db
            .study_create("Sesión", now + 7_200_000, now + 10_800_000, Some(t.id), "")
            .unwrap();
        db.delete(t.id).unwrap();
        assert_eq!(db.study_get(s.id).unwrap().unwrap().task_id, None);
        // datos viejos: sesión que aún apunta a la tarea borrada → editar con
        // ese mismo id no falla (se guarda sin tarea)
        db.conn
            .execute(
                "UPDATE study_sessions SET task_id = ?1 WHERE id = ?2",
                [t.id, s.id],
            )
            .unwrap();
        let upd = db
            .study_update(s.id, "Sesión 2", now + 7_200_000, now + 10_800_000, Some(t.id), "")
            .unwrap();
        assert_eq!(upd.task_id, None);
        // un id inexistente DISTINTO sigue siendo error
        assert!(db
            .study_update(s.id, "Sesión 3", now + 7_200_000, now + 10_800_000, Some(99_999), "")
            .is_err());
    }
}

