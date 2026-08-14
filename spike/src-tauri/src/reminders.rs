//! Motor de recordatorios: dispara un toast nativo de Windows cuando una tarea
//! entra en la ventana "start_at - reminder_minutes".
//!
//! Límite conocido: con la app completamente cerrada no hay proceso que dispare.
//! La bandeja + autostart mantienen el proceso vivo y cubren el caso real
//! ("app en bandeja"). Al abrir la app se disparan de inmediato las ventanas
//! vencidas ("mientras no estabas" básico).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Timelike;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::store::DueReminder;

pub fn reminder_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tick(&app);
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn tick(app: &AppHandle) {
    // respeta preferencias de notificación (kill-switch y horario de silencio)
    let quiet = crate::sync::with_db(app, |db| {
        let p = crate::notify::prefs(db);
        if !p.enabled {
            return true;
        }
        let now = chrono::Local::now();
        crate::notify::in_quiet_hours(now.hour() * 60 + now.minute(), &p)
    });
    if quiet {
        return;
    }
    crate::notify::tick(app);
    let due = crate::sync::with_db(app, |db| db.due_reminders(now_ms()));
    let due = match due {
        Ok(v) => v,
        Err(e) => {
            crate::append_log(app, &format!("reminders_query_error: {e}"));
            return;
        }
    };
    if due.is_empty() {
        return;
    }
    let mut fired = 0usize;
    for r in due {
        match app
            .notification()
            .builder()
            .title(&r.title)
            .body(reminder_body(&r))
            .show()
        {
            Ok(_) => {
                let _ = crate::sync::with_db(app, |db| db.mark_reminder_fired(r.task_id));
                fired += 1;
                crate::append_log(app, &format!("reminder_fired id={} title={}", r.task_id, r.title));
            }
            Err(e) => crate::append_log(app, &format!("reminder_show_error id={} {e}", r.task_id)),
        }
    }
    if fired > 0 {
        crate::append_log(app, &format!("reminders_fired count={fired}"));
    }
    let _ = app.emit("reminders:fired", fired);
}

fn reminder_body(r: &DueReminder) -> String {
    if r.all_day {
        return "Todo el día".into();
    }
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(r.start_at).single() {
        Some(dt) if dt.date_naive() == chrono::Local::now().date_naive() => {
            format!("Hoy a las {}", dt.format("%H:%M"))
        }
        Some(dt) => format!("{} a las {}", dt.format("%d/%m"), dt.format("%H:%M")),
        None => String::new(),
    }
}

/// Convierte "1d"/"3h"/"30m"/"1w" (o "1 día", "2 horas", minutos planos) a minutos.
pub fn parse_reminder_minutes(s: &str) -> Option<i64> {
    let s = s.trim().to_lowercase();
    let (num, mult) = if let Some(n) = s.strip_suffix("días") {
        (n, 1440)
    } else if let Some(n) = s.strip_suffix("dias") {
        (n, 1440)
    } else if let Some(n) = s.strip_suffix("día") {
        (n, 1440)
    } else if let Some(n) = s.strip_suffix("dia") {
        (n, 1440)
    } else if let Some(n) = s.strip_suffix("d") {
        (n, 1440)
    } else if let Some(n) = s.strip_suffix("horas") {
        (n, 60)
    } else if let Some(n) = s.strip_suffix("hora") {
        (n, 60)
    } else if let Some(n) = s.strip_suffix("h") {
        (n, 60)
    } else if let Some(n) = s.strip_suffix("minutos") {
        (n, 1)
    } else if let Some(n) = s.strip_suffix("minuto") {
        (n, 1)
    } else if let Some(n) = s.strip_suffix("min") {
        (n, 1)
    } else if let Some(n) = s.strip_suffix("m") {
        (n, 1)
    } else if let Some(n) = s.strip_suffix("w") {
        (n, 10_080)
    } else {
        (s.as_str(), 1)
    };
    let n: i64 = num.trim().parse().ok()?;
    if n <= 0 {
        return None;
    }
    Some(n.saturating_mul(mult))
}

#[cfg(test)]
mod tests {
    use super::parse_reminder_minutes;
    use super::reminder_body;
    use crate::store::DueReminder;

    fn due(start_at: i64, all_day: bool) -> DueReminder {
        DueReminder {
            task_id: 1,
            title: "Tarea".into(),
            start_at,
            end_at: start_at + 3_600_000,
            all_day,
            reminder_minutes: 60,
        }
    }

    #[test]
    fn parses_short_forms() {
        assert_eq!(parse_reminder_minutes("1d"), Some(1440));
        assert_eq!(parse_reminder_minutes("3h"), Some(180));
        assert_eq!(parse_reminder_minutes("30m"), Some(30));
        assert_eq!(parse_reminder_minutes("1w"), Some(10_080));
        assert_eq!(parse_reminder_minutes("90"), Some(90));
    }

    #[test]
    fn parses_long_forms() {
        assert_eq!(parse_reminder_minutes("1 día"), Some(1440));
        assert_eq!(parse_reminder_minutes("2 horas"), Some(120));
        assert_eq!(parse_reminder_minutes("15 minutos"), Some(15));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_reminder_minutes("xyz"), None);
        assert_eq!(parse_reminder_minutes(""), None);
        assert_eq!(parse_reminder_minutes("-5"), None);
        assert_eq!(parse_reminder_minutes("0d"), None);
    }

    #[test]
    fn reminder_body_all_day_and_timed() {
        let now = chrono::Local::now();
        assert_eq!(reminder_body(&due(now.timestamp_millis(), true)), "Todo el día");
        assert_eq!(
            reminder_body(&due(now.timestamp_millis(), false)),
            format!("Hoy a las {}", now.format("%H:%M"))
        );
        let mañana = now + chrono::Duration::days(1);
        let b = reminder_body(&due(mañana.timestamp_millis(), false));
        assert_eq!(b, format!("{} a las {}", mañana.format("%d/%m"), mañana.format("%H:%M")));
    }
}
