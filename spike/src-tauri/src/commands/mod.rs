//! Capa de comandos Tauri, un módulo por dominio.
//!
//! Cada módulo agrupa los `#[tauri::command]` de UN dominio de la app:
//! cambiar un dominio no toca a los demás, y añadir un comando nuevo es
//! (1) escribirlo en su módulo y (2) añadirlo a la macro `for_each_command!`
//! en `lib.rs`. Los módulos son finos a propósito: delegan la lógica en las
//! capas de negocio (`store`, `sync`, `planning`, `assistant`, …).
//!
//! Convenciones compartidas (se aplican en todos los módulos):
//! - Lock de DB BREVE: leer/escribir y soltar. Nada de red dentro del lock
//!   (la red va a `spawn_blocking`, ver `task_from_text` como ejemplo).
//! - Eventos al frontend: `app.emit("tasks:changed", …)` tras cada mutación.
//! - Logging: `crate::append_log(&app, "…")` en cada operación notable.

pub mod assistant;
pub mod auth;
pub mod email;
pub mod plans;
pub mod suggestions;
pub mod tasks;
pub mod ui;
pub mod widget;

use crate::store::{lock_recover, Db};
use std::sync::Mutex;

/// Ejecuta `f` con la DB bajo lock breve (con recuperación de poison) y
/// devuelve su resultado. El guard se suelta al salir: nunca retener el
/// mutex durante red ni cálculos largos.
pub(crate) fn with_db<R>(state: &Mutex<Db>, f: impl FnOnce(&Db) -> R) -> R {
    let db = lock_recover(state);
    f(&db)
}
