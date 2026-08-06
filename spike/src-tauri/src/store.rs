use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[derive(Serialize, Clone)]
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
    })
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(data_dir: &PathBuf) -> rusqlite::Result<Self> {
        std::fs::create_dir_all(data_dir).ok();
        let conn = Connection::open(data_dir.join("focusflow.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 2000)?;
        let db = Db { conn };
        db.migrate()?;
        db.seed_if_empty()?;
        Ok(db)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.migrate_0001()?;
        self.migrate_0002()?;
        self.migrate_0003()?;
        self.migrate_0004()
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
                self.conn.execute_batch(&format!("ALTER TABLE tasks ADD COLUMN {ddl}"))?;
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
            let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name = 'metadata'")?;
            let n: i64 = stmt.query_row([], |r| r.get(0))?;
            n > 0
        };
        if !has_metadata {
            self.conn
                .execute_batch("ALTER TABLE tasks ADD COLUMN metadata TEXT NOT NULL DEFAULT '{}'")?;
        }
        Ok(())
    }

    pub fn settings_get(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
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

    fn seed_if_empty(&self) -> rusqlite::Result<()> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }
        let now = now_ms();
        let day = 86_400_000;
        let t = |title: &str, cat: &str, prio: &str, start: i64, end: i64| -> rusqlite::Result<()> {
            self.conn.execute(
                "INSERT INTO tasks (title, category_id, priority, status, start_at, end_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'pendiente', ?4, ?5, ?6, ?6)",
                rusqlite::params![title, cat, prio, start, end, now],
            )?;
            Ok(())
        };
        t("Estudiar cálculo — derivadas e integrales", "uni", "alta", now + day * 0 + 9 * 3_600_000, now + day * 0 + 11 * 3_600_000)?;
        t("Entregar proyecto de redes", "uni", "alta", now + day + 14 * 3_600_000, now + day + 14 * 3_600_000 + 1_800_000)?;
        t("Pagar internet", "fin", "media", now + day * 2 + 9 * 3_600_000, now + day * 2 + 9 * 3_600_000)?;
        t("Examen de física — parcial 2", "uni", "alta", now + day * 3 + 8 * 3_600_000, now + day * 3 + 10 * 3_600_000)?;
        t("Cita médico — revisión anual", "sal", "baja", now + day * 4 + 12 * 3_600_000, now + day * 4 + 12 * 3_600_000 + 45 * 60_000)?;
        Ok(())
    }

    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL", [], |r| r.get(0))
    }

    pub fn list(&self) -> rusqlite::Result<Vec<TaskRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM tasks WHERE deleted_at IS NULL ORDER BY start_at",
        )?;
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
    pub fn find_overlap(&self, exclude_id: i64, start: i64, end: i64) -> rusqlite::Result<Option<(i64, String)>> {
        self.conn
            .query_row(
                "SELECT id, title FROM tasks
                 WHERE deleted_at IS NULL AND id != ?1 AND status != 'completada'
                   AND start_at < ?3 AND end_at > ?2
                 ORDER BY start_at LIMIT 1",
                rusqlite::params![exclude_id, start, end],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
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
        self.conn.execute(
            "INSERT INTO tasks (title, category_id, priority, status, start_at, end_at, all_day, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'pendiente', ?4, ?5, ?6, ?7, ?7)",
            rusqlite::params![title, category_id, priority, start_at, end_at, all_day as i64, now],
        )?;
        let id = self.conn.last_insert_rowid();
        self.conn.query_row(
            "SELECT * FROM tasks WHERE id = ?1",
            [id],
            task_from_row,
        )
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

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![id, now_ms()],
        )?;
        Ok(())
    }

    pub fn move_to(&self, id: i64, start_at: i64, end_at: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET start_at = ?2, end_at = ?3, updated_at = ?4 WHERE id = ?1",
            rusqlite::params![id, start_at, end_at, now_ms()],
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
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tasks SET title = ?2, category_id = ?3, priority = ?4,
                    start_at = ?5, end_at = ?6, description = ?7, tags = ?8, notes = ?9,
                    links = ?10, reminder_minutes = ?11, updated_at = ?12
             WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![
                id, title, category_id, priority, start_at, end_at,
                description, tags, notes, links, reminder_minutes, now_ms()
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: i64) -> rusqlite::Result<Option<TaskRow>> {
        self.conn
            .query_row("SELECT * FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [id], task_from_row)
            .optional()
    }

    pub fn duplicate(&self, id: i64) -> rusqlite::Result<Option<TaskRow>> {
        let Some(t) = self.get_task(id)? else { return Ok(None) };
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

    pub fn find_similar_task(&self, title: &str, start_at: i64, sender: &str) -> rusqlite::Result<Option<(i64, String)>> {
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
    pub fn insert_suggestion(
        &self,
        source: &str,
        email_id: Option<&str>,
        sender: Option<&str>,
        title: &str,
        description: &str,
        category_id: &str,
        priority: &str,
        start_at: Option<i64>,
        end_at: Option<i64>,
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
             (source, source_email_id, source_sender, title, description, category_id, priority,
              start_at, end_at, location, tags, confidence, reason, status, dedupe_task_id, dedupe_note, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)",
            rusqlite::params![
                source, email_id, sender, title, description, category_id, priority,
                start_at, end_at, location, tags, confidence, reason, status,
                dedupe_task_id, dedupe_note, now
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_suggestions(&self, only_pending: bool, retention_ms: i64) -> rusqlite::Result<Vec<SuggestionRow>> {
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

    pub fn set_suggestion_result_task(&self, id: i64, task_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE suggested_events SET result_task_id = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, task_id, now_ms()],
        )?;
        Ok(())
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
                    start_at = ?5, end_at = ?6, description = ?7, updated_at = ?8
             WHERE id = ?1",
            rusqlite::params![id, title, category_id, priority, start_at, end_at, description, now_ms()],
        )?;
        Ok(())
    }

    pub fn get_suggestion(&self, id: i64) -> rusqlite::Result<Option<SuggestionRow>> {
        self.conn
            .query_row("SELECT * FROM suggested_events WHERE id = ?1", [id], suggestion_from_row)
            .optional()
    }

    pub fn suggestion_count_for_email(&self, email_id: &str) -> rusqlite::Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM suggested_events WHERE source_email_id = ?1 AND status != 'rejected'",
            [email_id],
            |r| r.get(0),
        )
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
        self.conn.execute("DELETE FROM trusted_senders WHERE sender = ?1", [sender])?;
        Ok(())
    }

    pub fn trusted_list(&self) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT sender FROM trusted_senders ORDER BY sender")?;
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

    pub fn sync_state_set(&self, source: &str, checkpoint: &str, result: &str, error: &str) -> rusqlite::Result<()> {
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

    pub fn sync_history_today(&self, start_of_day_ms: i64) -> rusqlite::Result<Vec<SyncHistoryRow>> {
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
        title: r.get("title")?,
        description: r.get("description")?,
        category_id: r.get("category_id")?,
        priority: r.get("priority")?,
        start_at: r.get("start_at")?,
        end_at: r.get("end_at")?,
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
    pub title: String,
    pub description: String,
    pub category_id: String,
    pub priority: String,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
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
