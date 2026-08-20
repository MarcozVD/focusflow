use std::path::Path;

/// Carga secretos desde `spike/src-tauri/.env` (gitignored) y los incrusta
/// en el binario en tiempo de compilación via `cargo:rustc-env`.
///
/// - `AI_API_KEY` es OBLIGATORIA: sin ella el build falla con mensaje claro.
/// - `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` son opcionales: sin ellas la
///   app compila pero la autenticación Google queda deshabilitada.
fn main() {
    println!("cargo:rerun-if-env-changed=AI_API_KEY");
    println!("cargo:rerun-if-env-changed=GOOGLE_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=GOOGLE_CLIENT_SECRET");
    println!("cargo:rerun-if-changed=.env");

    let mut vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let env_path = Path::new(".env");
    if env_path.exists() {
        if let Ok(content) = std::fs::read_to_string(env_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    vars.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }

    let api_key = match vars
        .get("AI_API_KEY")
        .map(|s| s.trim().to_string())
        .or_else(|| std::env::var("AI_API_KEY").ok())
    {
        Some(k) if !k.is_empty() => k,
        _ => {
            panic!(
                "AI_API_KEY no está definida. Crea spike/src-tauri/.env con \
                 AI_API_KEY=tu-clave (el archivo .env está en .gitignore, nunca lo commitees)."
            );
        }
    };
    println!("cargo:rustc-env=AI_API_KEY={api_key}");

    if let Some(v) = vars.get("GOOGLE_CLIENT_ID") {
        println!("cargo:rustc-env=GOOGLE_CLIENT_ID={v}");
    }
    if let Some(v) = vars.get("GOOGLE_CLIENT_SECRET") {
        println!("cargo:rustc-env=GOOGLE_CLIENT_SECRET={v}");
    }

    tauri_build::build()
}