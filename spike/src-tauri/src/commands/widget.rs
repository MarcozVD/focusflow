//! Dominio WIDGET: la ventanita de escritorio, sus acciones rápidas sobre
//! tareas y el comportamiento de arranque minimizado. También el ciclo de
//! vida de la ventana principal (mostrar / minimizar a bandeja).

use tauri::AppHandle;

use tauri::Manager;

use crate::append_log;
use crate::store::Db;

/// Crea (o re-crea) la ventana del widget en la esquina inferior derecha.
pub fn create_widget(app: &AppHandle) -> Result<(), String> {
    let w = tauri::WebviewWindowBuilder::new(
        app,
        "widget",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("FocusFlow Widget")
    .inner_size(340.0, 500.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    // Fondo del webview explícitamente transparente: sin esto, en algunas
    // configuraciones de Windows (efectos visuales reducidos) la superficie
    // del webview pinta un recuadro sólido alrededor del widget.
    .background_color(tauri::utils::config::Color::from((0, 0, 0, 0)))
    // Se queda detrás de las ventanas normales (solo escritorio),
    // no encima de las apps como el always_on_top.
    .always_on_bottom(true)
    .skip_taskbar(true)
    .build()
    .map_err(|e| e.to_string())?;
    // esquina inferior derecha del área de trabajo (queda en el escritorio, no encima de apps)
    if let Ok(Some(mon)) = app.primary_monitor() {
        let wa = mon.work_area();
        let size = w.outer_size().unwrap_or(tauri::PhysicalSize::new(320, 260));
        let x = wa.position.x + wa.size.width as i32 - size.width as i32 - 16;
        let y = wa.position.y + wa.size.height as i32 - size.height as i32 - 16;
        let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x, y,
        )));
    }
    append_log(app, "widget_created");
    Ok(())
}

#[tauri::command]
pub fn toggle_widget(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("widget") {
        if w.is_visible().unwrap_or(false) {
            w.hide().map_err(|e| e.to_string())?;
            append_log(&app, "widget_hidden");
        } else {
            w.show().map_err(|e| e.to_string())?;
            append_log(&app, "widget_shown");
        }
    } else {
        create_widget(&app)?;
    }
    Ok(())
}

#[tauri::command]
pub fn widget_info(app: AppHandle) -> String {
    match app.get_webview_window("widget") {
        Some(w) => {
            let vis = w.is_visible().unwrap_or(false);
            append_log(&app, &format!("widget_info visible={vis}"));
            format!("visible={vis}")
        }
        None => {
            append_log(&app, "widget_info none");
            "widget no creada".to_string()
        }
    }
}

#[tauri::command]
pub fn open_app(app: AppHandle) -> Result<(), String> {
    crate::show_main(&app);
    if let Some(w) = app.get_webview_window("widget") {
        let _ = w.hide();
    }
    append_log(&app, "open_app_from_widget");
    Ok(())
}

/// Nuevo span de "posponer" del widget: con hora → +1 h; all-day → al día
/// siguiente (medianoche local, span >= 24 h). Sumar 1 h a un marcador
/// all-day lo sacaba de la medianoche y lo hacía abarcar dos días.
fn postpone_span(start: i64, end: i64, all_day: bool) -> (i64, i64) {
    const HOUR: i64 = 3_600_000;
    if all_day {
        // +36 h y floor a medianoche: robusto a días de 23/25 h (DST)
        let s = crate::engine::local_midnight(start + crate::engine::DAY_MS + 12 * HOUR);
        (s, s + (end - start).max(crate::engine::DAY_MS))
    } else {
        (start + HOUR, end + HOUR)
    }
}

/// Acción rápida del widget, aplicada vía los servicios existentes del store:
/// - complete  → set_completed
/// - postpone  → move_to (+1 h)
/// - start     → set_task_status('en-curso')
#[tauri::command]
pub fn widget_action(
    app: AppHandle,
    state: tauri::State<'_, std::sync::Mutex<Db>>,
    id: i64,
    action: String,
) -> Result<String, String> {
    use tauri::Emitter;
    crate::commands::with_db(&state, |db| {
        let t = db
            .get_task(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "tarea no encontrada".to_string())?;
        match action.as_str() {
            "complete" => {
                db.set_completed(id, true).map_err(|e| e.to_string())?;
            }
            "postpone" => {
                // all-day → al día siguiente (sumar 1 h desalineaba el
                // marcador de la medianoche); con hora → +1 h con la misma
                // política de conflictos que task_move (estricto bloquea)
                let (s, e) = postpone_span(t.start_at, t.end_at, t.all_day);
                if let Some(other) =
                    crate::commands::tasks::check_conflict(db, id, s, e, t.all_day)?
                {
                    append_log(
                        &app,
                        &format!("widget_postpone_overlap id={id} con={other}"),
                    );
                }
                db.move_to(id, s, e, Some(t.all_day))
                    .map_err(|e| e.to_string())?;
            }
            "start" => {
                db.set_task_status(id, "en-curso")
                    .map_err(|e| e.to_string())?;
            }
            other => return Err(format!("acción desconocida: {other}")),
        }
        Ok(())
    })?;
    append_log(&app, &format!("widget_action id={id} action={action}"));
    let _ = app.emit("tasks:changed", ());
    Ok(action)
}

/// Comportamiento de arranque minimizado: oculta la ventana principal y
/// muestra el widget (lo crea si no existía).
pub fn auto_start_behavior(app: &AppHandle, db: &Db) {
    if crate::setting_bool(db, "general.start_minimized", false) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
        if app.get_webview_window("widget").is_none() {
            let _ = create_widget(app);
        }
        if let Some(w) = app.get_webview_window("widget") {
            let _ = w.show();
        }
        append_log(app, "start_minimized_widget_shown");
    }
}

#[cfg(test)]
mod tests {
    use super::postpone_span;
    use crate::engine::{local_midnight, DAY_MS};

    #[test]
    fn postpone_all_day_moves_to_next_day_and_timed_one_hour() {
        let day = local_midnight(crate::store::now_ms());
        // all-day: al día siguiente completo, no 01:00 → 01:00 del otro día
        let (s, e) = postpone_span(day, day + DAY_MS, true);
        assert_eq!(s, local_midnight(day + DAY_MS + 3_600_000));
        assert_eq!(s, local_midnight(s));
        assert!(e - s >= DAY_MS - 3_600_000);
        // con hora: +1 h
        let (s, e) = postpone_span(day + 9 * 3_600_000, day + 10 * 3_600_000, false);
        assert_eq!((s, e), (day + 10 * 3_600_000, day + 11 * 3_600_000));
    }
}
