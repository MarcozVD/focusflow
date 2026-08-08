//! Motor de notificaciones contextuales (fase 11).
//!
//! Convierte recordatorios tontos en sugerencias útiles:
//!   - vencimiento próximo (con preparación restante)
//!   - tarea atrasada (una sola vez)
//!   - conflicto de horario
//!   - tiempo libre disponible (oferta para preparar)
//!   - compromiso importante de hoy
//!   - sugerencia de reprogramación (empieza mientras estás en otra)
//!
//! Anti-spam:
//!   - `notif.enabled` → kill-switch global
//!   - `notif.quiet_start`/`notif.quiet_end` → ventana de silencio
//!   - `notif.daily_cap` → tope diario de disparos
//!   - `notif.cooldown_hours` → cadencia por (tipo, tarea)
//!   - dismiss → nunca más insistir en ese (tipo, tarea)
//!   - "atrasada" → una sola vez (no reaparece cada día)
//!
//! Nota de plataforma: el plugin de notificaciones de Tauri no soporta
//! botones de acción en Windows (solo Android). La interacción
//! [Plan]/[Más tarde]/[Descartar] se resuelve en la app: la notificación
//! nativa muestra el contexto, y al dispararse también se emite
//! `notif:contextual` que la ventana principal muestra como tarjeta con
//! los tres botones. Clic en la notificación nativa enfoca la app.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::store::{Db, TaskRow};

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

// ---------------- preferencias ----------------

#[derive(Debug, Clone)]
pub struct NotifPrefs {
    pub enabled: bool,
    /// minutos desde la medianoche (hora local)
    pub quiet_start_min: u32,
    pub quiet_end_min: u32,
    pub daily_cap: i64,
    pub free_minutes: i64,
    pub cooldown_ms: i64,
}

#[derive(Serialize)]
pub struct NotifPrefsView {
    pub enabled: bool,
    pub quiet_start: String,
    pub quiet_end: String,
    pub daily_cap: i64,
    pub free_minutes: i64,
}

fn parse_hhmm(s: &str) -> u32 {
    let (h, m) = s.split_once(':').unwrap_or((s, "0"));
    h.trim().parse::<u32>().unwrap_or(0) * 60 + m.trim().parse::<u32>().unwrap_or(0)
}

fn setting(db: &Db, key: &str, default: &str) -> String {
    db.settings_get(key).ok().flatten().unwrap_or_else(|| default.to_string())
}

pub fn prefs(db: &Db) -> NotifPrefs {
    let cooldown_h: i64 = setting(db, "notif.cooldown_hours", "24").trim().parse().unwrap_or(24);
    NotifPrefs {
        enabled: setting(db, "notif.enabled", "1") == "1",
        quiet_start_min: parse_hhmm(&setting(db, "notif.quiet_start", "22:00")),
        quiet_end_min: parse_hhmm(&setting(db, "notif.quiet_end", "08:00")),
        daily_cap: setting(db, "notif.daily_cap", "5").trim().parse().unwrap_or(5),
        free_minutes: setting(db, "notif.free_minutes", "120").trim().parse().unwrap_or(120),
        cooldown_ms: cooldown_h.saturating_mul(3_600_000),
    }
}

pub fn prefs_view(db: &Db) -> NotifPrefsView {
    NotifPrefsView {
        enabled: setting(db, "notif.enabled", "1") == "1",
        quiet_start: setting(db, "notif.quiet_start", "22:00"),
        quiet_end: setting(db, "notif.quiet_end", "08:00"),
        daily_cap: setting(db, "notif.daily_cap", "5").trim().parse().unwrap_or(5),
        free_minutes: setting(db, "notif.free_minutes", "120").trim().parse().unwrap_or(120),
    }
}

/// true si `now` (minutos desde la medianoche local) cae en la ventana de
/// silencio. Ventana vacía (inicio == fin) = sin horario de silencio.
pub fn in_quiet_hours(min: u32, p: &NotifPrefs) -> bool {
    if p.quiet_start_min == p.quiet_end_min {
        return false;
    }
    if p.quiet_start_min < p.quiet_end_min {
        min >= p.quiet_start_min && min < p.quiet_end_min
    } else {
        min >= p.quiet_start_min || min < p.quiet_end_min
    }
}

// ---------------- utilidades de tiempo ----------------

fn local(ms: i64) -> chrono::DateTime<Local> {
    Local.timestamp_millis_opt(ms).single().unwrap_or_else(Local::now)
}

fn day_start(ms: i64) -> i64 {
    let d = local(ms);
    Local
        .with_ymd_and_hms(d.year(), d.month(), d.day(), 0, 0, 0)
        .single()
        .map(|x| x.timestamp_millis())
        .unwrap_or(ms)
}

/// "hoy"/"mañana"/"dd/mm" según la fecha local de `ms` respecto a `now`.
fn day_label(ms: i64, now: i64) -> String {
    let d = local(ms).date_naive();
    let today = local(now).date_naive();
    if d == today {
        return "hoy".into();
    }
    if d == today + chrono::Days::new(1) {
        return "mañana".into();
    }
    format!("{}/{}", d.format("%d"), d.format("%m"))
}

fn fmt_hhmm(ms: i64) -> String {
    local(ms).format("%H:%M").to_string()
}

fn minutes_left(end: i64, now: i64, start: i64) -> i64 {
    (end - now.max(start)).max(0) / 60_000
}

fn fmt_duration(min: i64) -> String {
    if min >= 60 {
        format!("{}h {}min", min / 60, min % 60)
    } else {
        format!("{min}min")
    }
}

fn ago(ms: i64, now: i64) -> String {
    let min = ((now - ms).max(0) / 60_000) as i64;
    if min >= 60 {
        format!("{}h", min / 60)
    } else {
        format!("{min}min")
    }
}

// ---------------- candidatos ----------------

#[derive(Debug)]
pub struct Candidate {
    pub kind: &'static str,
    pub task_id: i64,
    pub score: i64,
    pub title: String,
    pub body: String,
    pub task_title: String,
}

const KINDS: [&str; 6] = ["deadline", "missed", "conflict", "free_time", "important", "reschedule"];

/// Tareas activas (pendiente o en curso) que se cruzan con [now-1d, now+36h].
fn active_tasks(db: &Db, now: i64) -> Vec<TaskRow> {
    db.list_range(now - 86_400_000, now + 36 * 3_600_000)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.status == "pendiente" || t.status == "en-curso")
        .collect()
}

/// Dedup anti-spam: descartado alguna vez, o disparado dentro de la cadencia.
/// "atrasada" es de una sola vez (cadencia infinita).
fn blocked(db: &Db, kind: &str, task_id: i64, p: &NotifPrefs, now: i64) -> bool {
    if db.notif_dismissed(kind, task_id).unwrap_or(false) {
        return true;
    }
    let since = if kind == "missed" { 0 } else { now - p.cooldown_ms };
    db.notif_fired_recently(kind, task_id, since).unwrap_or(false)
}

fn push(db: &Db, v: &mut Vec<Candidate>, p: &NotifPrefs, now: i64, c: Candidate) {
    if KINDS.contains(&c.kind) && !blocked(db, c.kind, c.task_id, p, now) {
        v.push(c);
    }
}

/// Genera todos los candidatos contextuales del momento.
pub fn collect(db: &Db, now: i64, p: &NotifPrefs) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let tasks = active_tasks(db, now);
    if tasks.is_empty() {
        return out;
    }

    let horizon = now + 24 * 3_600_000;
    let day = day_start(now);

    // deadline: pendiente con fin en < 24h
    for t in tasks.iter().filter(|t| t.status == "pendiente" && !t.all_day && t.end_at > now && t.end_at <= horizon)
    {
        let remaining = minutes_left(t.end_at, now, t.start_at);
        // si ya empezó es la más urgente; si aún no, ceden ante free_time/reschedule
        let score = if t.start_at < now { 100 } else { 60 };
        let mut body = format!(
            "«{}» termina {} a las {}.",
            t.title,
            day_label(t.end_at, now),
            fmt_hhmm(t.end_at)
        );
        if remaining > 0 {
            body.push_str(&format!(" Quedan {} de preparación.", fmt_duration(remaining)));
        }
        push(
            db,
            &mut out,
            p,
            now,
            Candidate {
                kind: "deadline",
                task_id: t.id,
                score,
                title: "Vence pronto".into(),
                body,
                task_title: t.title.clone(),
            },
        );
    }

    // missed: pendiente que ya terminó (reciente, una sola vez)
    for t in tasks
        .iter()
        .filter(|t| t.status == "pendiente" && !t.all_day && t.end_at < now && t.end_at >= now - 24 * 3_600_000)
    {
        push(
            db,
            &mut out,
            p,
            now,
            Candidate {
                kind: "missed",
                task_id: t.id,
                score: 60,
                title: "Tarea atrasada".into(),
                body: format!(
                    "«{}» terminó hace {} y sigue pendiente.",
                    t.title,
                    ago(t.end_at, now)
                ),
                task_title: t.title.clone(),
            },
        );
    }

    // conflict: pares de tareas con solapamiento real (>= 15 min)
    let timed: Vec<&TaskRow> = tasks.iter().filter(|t| !t.all_day).collect();
    for (i, a) in timed.iter().enumerate() {
        for b in timed.iter().skip(i + 1) {
            let (x, y) = if a.start_at <= b.start_at { (a, b) } else { (b, a) };
            let overlap = (x.end_at.min(y.end_at) - y.start_at) / 60_000;
            if overlap < 15 {
                continue;
            }
            let in_curso = x.status == "en-curso" || y.status == "en-curso";
            push(
                db,
                &mut out,
                p,
                now,
                Candidate {
                    kind: "conflict",
                    task_id: y.id,
                    score: if in_curso { 70 } else { 40 },
                    title: "Conflicto de horario".into(),
                    body: format!(
                        "«{}» se solapa con «{}» ({}–{}).",
                        x.title,
                        y.title,
                        fmt_hhmm(y.start_at),
                        fmt_hhmm(y.end_at)
                    ),
                    task_title: y.title.clone(),
                },
            );
        }
    }

    // free_time: primer hueco libre >= umbral hoy, con tarea candidata
    if let Some((free_start, free_end)) = first_free_block(&timed, now, day, p.free_minutes) {
        if let Some(t) = best_free_candidate(&tasks, free_start) {
            let label = if local(free_start).hour() >= 12 { "esta tarde" } else { "hoy" };
            push(
                db,
                &mut out,
                p,
                now,
                Candidate {
                    kind: "free_time",
                    task_id: t.id,
                    score: 75,
                    title: "Tiempo disponible".into(),
                    body: format!(
                        "Tienes {} libres {} ({}–{}). ¿Usarlas para «{}»?",
                        fmt_duration((free_end - free_start) / 60_000),
                        label,
                        fmt_hhmm(free_start),
                        fmt_hhmm(free_end),
                        t.title
                    ),
                    task_title: t.title.clone(),
                },
            );
        }
    }

    // important: compromiso de hoy (alta prioridad o todo el día)
    for t in tasks
        .iter()
        .filter(|t| t.status == "pendiente" && (t.priority == "alta" || t.all_day) && t.start_at >= day && t.start_at < day + 86_400_000)
    {
        let soon = !t.all_day && t.start_at - now <= 3 * 3_600_000 && t.start_at > now;
        let mut body = format!("Hoy: «{}».", t.title);
        if !t.all_day {
            body.push_str(&format!(" a las {}.", fmt_hhmm(t.start_at)));
        }
        push(
            db,
            &mut out,
            p,
            now,
            Candidate {
                kind: "important",
                task_id: t.id,
                score: if soon { 90 } else { 40 },
                title: "Compromiso importante".into(),
                body,
                task_title: t.title.clone(),
            },
        );
    }

    // reschedule: en curso + otra pendiente que arranca en < 1h
    for a in tasks.iter().filter(|t| t.status == "en-curso" && !t.all_day) {
        for b in tasks
            .iter()
            .filter(|t| t.status == "pendiente" && !t.all_day && t.id != a.id && t.start_at >= now && t.start_at <= now + 3_600_000)
        {
            push(
                db,
                &mut out,
                p,
                now,
                Candidate {
                    kind: "reschedule",
                    task_id: b.id,
                    score: 95,
                    title: "Sugerencia de reprogramación".into(),
                    body: format!(
                        "«{}» empieza a las {} mientras estás en «{}». ¿Moverlo a más tarde?",
                        b.title,
                        fmt_hhmm(b.start_at),
                        a.title
                    ),
                    task_title: b.title.clone(),
                },
            );
        }
    }

    out.sort_by(|a, b| b.score.cmp(&a.score));
    // una sola notificación por tarea por tick: gana la de mayor urgencia
    let mut seen = std::collections::HashSet::new();
    out.into_iter().filter(|c| seen.insert(c.task_id)).collect()
}

/// Primer hueco libre de >= `min` minutos entre [now, fin de día].
fn first_free_block(
    tasks: &[&TaskRow],
    now: i64,
    day: i64,
    min: i64,
) -> Option<(i64, i64)> {
    let mut busy: Vec<(i64, i64)> = tasks
        .iter()
        .filter(|t| t.end_at > now && t.start_at < day + 86_400_000)
        .map(|t| (t.start_at.max(now), t.end_at.min(day + 86_400_000)))
        .filter(|(s, e)| e > s)
        .collect();
    busy.sort_by_key(|b| b.0);
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for b in busy {
        if let Some(last) = merged.last_mut() {
            if b.0 <= last.1 {
                last.1 = last.1.max(b.1);
                continue;
            }
        }
        merged.push(b);
    }
    let mut cursor = now;
    for (s, e) in merged {
        if s - cursor >= min * 60_000 {
            return Some((cursor, s));
        }
        cursor = cursor.max(e);
    }
    let day_end = day + 86_400_000;
    if day_end - cursor >= min * 60_000 {
        Some((cursor, day_end))
    } else {
        None
    }
}

/// Mejor tarea para ofrecer en un hueco libre: prioridad alta primero, luego
/// la que termina antes. Debe empezar después de `free_start` (aún no hecha).
fn best_free_candidate(tasks: &[TaskRow], free_start: i64) -> Option<&TaskRow> {
    let mut cand: Vec<&TaskRow> = tasks
        .iter()
        .filter(|t| t.status == "pendiente" && !t.all_day && t.start_at >= free_start)
        .filter(|t| (t.end_at - t.start_at) / 60_000 >= 30)
        .collect();
    cand.sort_by(|a, b| {
        let pa = if a.priority == "alta" { 0 } else { 1 };
        let pb = if b.priority == "alta" { 0 } else { 1 };
        pa.cmp(&pb).then(a.end_at.cmp(&b.end_at))
    });
    cand.into_iter().next()
}

// ---------------- disparo ----------------

#[derive(Serialize, Clone)]
pub struct ContextualNotif {
    pub log_id: i64,
    pub kind: String,
    pub task_id: i64,
    pub title: String,
    pub body: String,
    pub task_title: String,
}

pub fn tick(app: &AppHandle) {
    let (cands, fired_today, p) = crate::sync::with_db(app, |db| {
        let p = prefs(db);
        let now = now_ms();
        let fired = db.notif_fired_today(day_start(now)).unwrap_or(0);
        (collect(db, now, &p), fired, p)
    });
    if !p.enabled {
        return;
    }
    let now = now_ms();
    let min = local(now).hour() * 60 + local(now).minute();
    if in_quiet_hours(min, &p) {
        return;
    }
    let budget = (p.daily_cap - fired_today).max(0);
    if budget == 0 {
        return;
    }
    for c in cands.into_iter().take(budget as usize) {
        fire(app, &c);
    }
}

fn fire(app: &AppHandle, c: &Candidate) {
    let log_id = crate::sync::with_db(app, |db| db.log_notification(c.kind, c.task_id, &c.task_title)).unwrap_or(0);
    match app
        .notification()
        .builder()
        .title(&c.title)
        .body(&c.body)
        .show()
    {
        Ok(_) => {
            crate::append_log(app, &format!("notif_fired kind={} task_id={} log_id={log_id}", c.kind, c.task_id));
            let _ = app.emit(
                "notif:contextual",
                ContextualNotif {
                    log_id,
                    kind: c.kind.to_string(),
                    task_id: c.task_id,
                    title: c.title.clone(),
                    body: c.body.clone(),
                    task_title: c.task_title.clone(),
                },
            );
        }
        Err(e) => crate::append_log(app, &format!("notif_show_error kind={} {e}", c.kind)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Db;

    fn db() -> Db {
        Db::open_memory_clean_pub().unwrap()
    }

    fn prefs_off_quiet() -> NotifPrefs {
        NotifPrefs {
            enabled: true,
            quiet_start_min: 0,
            quiet_end_min: 0,
            daily_cap: 5,
            free_minutes: 120,
            cooldown_ms: 24 * 3_600_000,
        }
    }

    fn at(day: i64, h: u32, m: u32) -> i64 {
        let base = day_start(now_ms());
        base + day * 86_400_000 + i64::from(h * 60 + m) * 60_000
    }

    #[test]
    fn quiet_hours_window() {
        let p = NotifPrefs {
            quiet_start_min: 22 * 60,
            quiet_end_min: 8 * 60,
            ..prefs_off_quiet()
        };
        assert!(in_quiet_hours(23 * 60, &p));
        assert!(in_quiet_hours(0, &p));
        assert!(in_quiet_hours(7 * 60, &p));
        assert!(!in_quiet_hours(12 * 60, &p));
        let p2 = NotifPrefs { quiet_start_min: 13 * 60, quiet_end_min: 14 * 60, ..p };
        assert!(in_quiet_hours(13 * 60 + 30, &p2));
        assert!(!in_quiet_hours(15 * 60, &p2));
        let p3 = NotifPrefs { quiet_start_min: 0, quiet_end_min: 0, ..p };
        assert!(!in_quiet_hours(23 * 60, &p3));
    }

    #[test]
    fn no_spam_dedup_and_cap() {
        let db = db();
        let now = now_ms();
        let t = db.create("Examen", "uni", "baja", now, now + 3600_000, false).unwrap();
        let p = prefs_off_quiet();
        assert_eq!(collect(&db, now, &p).iter().filter(|c| c.kind == "deadline").count(), 1);
        db.log_notification("deadline", t.id, "").unwrap();
        assert_eq!(collect(&db, now, &p).iter().filter(|c| c.kind == "deadline").count(), 0);
        db.set_notif_status(1, "dismissed").unwrap();
        assert!(collect(&db, now, &p).is_empty());
    }

    #[test]
    fn deadline_candidate_with_remaining() {
        let db = db();
        let now = now_ms();
        db.create("Estudiar cálculo", "uni", "media", now + 3_600_000, now + 5 * 3_600_000, false).unwrap();
        let c = collect(&db, now, &prefs_off_quiet());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, "deadline");
        assert!(c[0].body.contains("Quedan 4h"));
        assert!(c[0].body.contains("mañana") || c[0].body.contains("hoy"));
    }

    #[test]
    fn missed_is_one_shot_forever() {
        let db = db();
        let now = now_ms();
        db.create("Pasada", "uni", "baja", now - 5 * 3_600_000, now - 2 * 3_600_000, false).unwrap();
        let p = prefs_off_quiet();
        assert_eq!(collect(&db, now, &p).iter().filter(|c| c.kind == "missed").count(), 1);
        db.log_notification("missed", 1, "").unwrap();
        db.set_notif_status(1, "shown").unwrap();
        assert_eq!(collect(&db, now, &p).iter().filter(|c| c.kind == "missed").count(), 0);
    }

    #[test]
    fn conflict_pair_detected() {
        let db = db();
        let now = now_ms();
        db.create("A", "uni", "baja", now, now + 2 * 3_600_000, false).unwrap();
        db.create("B", "uni", "baja", now + 3_600_000, now + 26 * 3_600_000, false).unwrap();
        let c = collect(&db, now, &prefs_off_quiet());
        let conflict = c.iter().find(|x| x.kind == "conflict");
        assert!(conflict.is_some(), "candidates: {c:?}");
        assert!(conflict.unwrap().body.contains("se solapa"));
    }

    #[test]
    fn free_time_offer_needs_candidate() {
        let db = db();
        let now = now_ms();
        db.create("Cosas libres", "per", "baja", now, now + 3_600_000, false).unwrap();
        db.create("Preparar entrevista", "trab", "alta", at(1, 9, 0), at(1, 11, 0), false).unwrap();
        let c = collect(&db, now, &prefs_off_quiet());
        let free = c.iter().find(|x| x.kind == "free_time");
        assert!(free.is_some(), "candidates: {c:?}");
        assert!(free.unwrap().body.contains("¿Usarlas para «Preparar entrevista»?"));
    }

    #[test]
    fn important_commitment_today() {
        let db = db();
        let now = now_ms();
        db.create("Defensa", "uni", "alta", at(0, 0, 0), at(0, 0, 0) + 86_400_000, true).unwrap();
        db.create("Mañana", "uni", "alta", at(1, 9, 0), at(1, 10, 0), false).unwrap();
        let c = collect(&db, now, &prefs_off_quiet());
        let imp: Vec<_> = c.iter().filter(|x| x.kind == "important").collect();
        assert_eq!(imp.len(), 1, "candidates: {c:?}");
        assert!(imp[0].body.contains("Defensa"));
    }

    #[test]
    fn reschedule_suggestion_while_en_curso() {
        let db = db();
        let now = now_ms();
        db.create("Trabajando", "trab", "alta", now - 3_600_000, now + 3_600_000, false).unwrap();
        db.set_task_status(1, "en-curso").unwrap();
        db.create("Reunión", "trab", "media", now + 30 * 60_000, now + 90 * 60_000, false).unwrap();
        let c = collect(&db, now, &prefs_off_quiet());
        let r = c.iter().find(|x| x.kind == "reschedule");
        assert!(r.is_some(), "candidates: {c:?}");
        assert!(r.unwrap().body.contains("¿Moverlo"));
    }

    #[test]
    fn urgency_orders_candidates() {
        let db = db();
        let now = now_ms();
        db.create("Urgente", "uni", "alta", now, now + 60 * 60_000, false).unwrap();
        db.create("Tranquila", "uni", "baja", now + 10 * 3_600_000, now + 12 * 3_600_000, false).unwrap();
        let c = collect(&db, now, &prefs_off_quiet());
        assert!(c.len() >= 2);
        assert!(c[0].score >= c[1].score);
    }

    #[test]
    fn first_free_block_merges_busy() {
        let now = now_ms();
        let day = day_start(now);
        let mk = |s: i64, e: i64| TaskRow {
            id: 0,
            title: String::new(),
            category_id: String::new(),
            priority: String::new(),
            status: String::new(),
            start_at: s,
            end_at: e,
            all_day: false,
            progress: 0,
            completed_at: None,
            created_at: 0,
            description: String::new(),
            tags: String::new(),
            notes: String::new(),
            links: String::new(),
            reminder_minutes: None,
            reminder_fired_at: None,
            metadata: String::new(),
        };
        let t1 = mk(now + 60 * 60_000, now + 90 * 60_000);
        let t2 = mk(now + 80 * 60_000, now + 95 * 60_000);
        let busy: Vec<&TaskRow> = vec![&t1, &t2];
        // primer hueco [now, now+60m] = 60 min < 120 → se salta
        let (fs, fe) = first_free_block(&busy, now, day, 120).unwrap();
        assert_eq!(fs, now + 95 * 60_000);
        assert_eq!(fe, day + 86_400_000);
    }
}
