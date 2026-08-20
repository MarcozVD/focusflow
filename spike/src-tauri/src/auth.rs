//! Autenticación Google OAuth2 PKCE (CAMBIO 2).
//!
//! Flujo para aplicaciones de escritorio:
//! 1. Se genera `code_verifier`/`code_challenge` (PKCE S256) y un `state`.
//! 2. Se abre el navegador con la URL de autorización de Google.
//! 3. Un servidor HTTP efímero en `127.0.0.1:0` recibe el callback con `code`.
//! 4. Se intercambia `code` por `access_token` + `refresh_token` (+ `id_token`).
//! 5. Los tokens se guardan en la DB local (tabla `auth_sessions`), NO en
//!    Credential Manager. El usuario decide (prompt) qué cuenta usar.
//!
//! Scopes: identidad (openid email profile) + lectura de Gmail (IMAP XOAUTH2).
//! `access_type=offline` + `prompt=consent` garantizan `refresh_token`.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::store::{AuthSession, Db};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "openid email profile https://www.googleapis.com/auth/gmail.readonly";
const CALLBACK_TIMEOUT_SECS: u64 = 120;

/// ID/secret de cliente incrustados en build-time (build.rs lee `.env`).
/// Opcionales: sin ellos la app compila pero el login Google está deshabilitado.
pub fn google_client_id() -> Option<String> {
    let v = option_env!("GOOGLE_CLIENT_ID").unwrap_or("");
    if v.trim().is_empty() {
        None
    } else {
        Some(v.trim().to_string())
    }
}

pub fn google_client_secret() -> Option<String> {
    let v = option_env!("GOOGLE_CLIENT_SECRET").unwrap_or("");
    if v.trim().is_empty() {
        None
    } else {
        Some(v.trim().to_string())
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct AuthSessionView {
    pub email: String,
    pub name: String,
    pub user_id: String,
    pub gmail_connected: bool,
}

fn view(s: &AuthSession) -> AuthSessionView {
    AuthSessionView {
        email: s.email.clone(),
        name: s.name.clone(),
        user_id: s.user_id.clone(),
        gmail_connected: !s.access_token.is_empty(),
    }
}

pub fn to_view(s: &AuthSession) -> AuthSessionView {
    view(s)
}

/// Estado actual de la sesión, sin red.
pub fn status(db: &Db) -> Option<AuthSessionView> {
    db.auth_load().ok().flatten().map(|s| view(&s))
}

/// ¿Hay sesión activa con tokens (para saber si el email está disponible)?
pub fn has_session(db: &Db) -> bool {
    db.auth_load()
        .ok()
        .flatten()
        .map(|s| !s.refresh_token.is_empty() || !s.access_token.is_empty())
        .unwrap_or(false)
}

/// Access token válido (refresca automáticamente si expiró).
pub fn access_token(db: &Db) -> Result<String, String> {
    let mut s = db
        .auth_load()
        .map_err(|e| format!("auth_load: {e}"))?
        .ok_or_else(|| "no hay sesión de Google: inicia sesión primero".to_string())?;
    if !s.access_token.is_empty() && s.expires_at > crate::store::now_ms() {
        return Ok(s.access_token);
    }
    if s.refresh_token.is_empty() {
        return Err("la sesión no tiene refresh_token; cierra sesión y vuelve a entrar".into());
    }
    let (new_access, expires_in) = refresh(&s.refresh_token)?;
    s.access_token = new_access;
    s.expires_at = crate::store::now_ms() + (expires_in as i64) * 1000;
    db.auth_save(&s).map_err(|e| e.to_string())?;
    Ok(s.access_token)
}

/// Configuración IMAP de Gmail derivada de la sesión (host/puerto/TLS fijos).
/// El `user` se rellena con el email de la sesión por el caller.
pub fn gmail_email_config(session_email: &str) -> crate::email::EmailConfig {
    let mut cfg = crate::email::EmailConfig::default();
    cfg.host = "imap.gmail.com".into();
    cfg.port = 993;
    cfg.user = session_email.to_string();
    cfg.auth = "oauth2".into();
    cfg.ssl = true;
    cfg.mailboxes = vec!["INBOX".into()];
    cfg
}

pub fn sign_out(db: &Db) -> Result<(), String> {
    db.auth_clear().map_err(|e| e.to_string())
}

/// Flujo completo de inicio de sesión: navegador + callback + intercambio.
/// NO toca la DB (el caller guarda después con `db.auth_save`); así el lock
/// de la DB no se retiene durante los hasta 120 s del callback del navegador.
pub fn perform_login() -> Result<AuthSession, String> {
    let client_id = google_client_id()
        .ok_or_else(|| "Google OAuth no está configurado en este build. Añade GOOGLE_CLIENT_ID a spike/src-tauri/.env y recompila.".to_string())?;
    let client_secret = google_client_secret().unwrap_or_default();

    // PKCE S256
    let verifier = base64url(rand_bytes(32)); // 43 chars, sin padding
    let challenge = base64url(Sha256::digest(verifier.as_bytes()));
    let state = base64url(rand_bytes(16));

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("no se pudo abrir el puerto de callback: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let auth_url = format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        url_encode(&client_id),
        url_encode(&redirect_uri),
        url_encode(SCOPE),
        challenge,
        state,
    );

    open::that(&auth_url)
        .map_err(|e| format!("no se pudo abrir el navegador: {e} — abre manualmente: {auth_url}"))?;

    let code = recv_callback(listener, &state)?;

    let tokens = exchange_code(&client_id, &client_secret, &redirect_uri, &code, &verifier)?;

    let id_token = tokens
        .id_token
        .ok_or_else(|| "Google no devolvió id_token".to_string())?;
    let profile = parse_id_token(&id_token)?;
    let refresh_token = tokens
        .refresh_token
        .ok_or_else(|| "Google no concedió refresh_token (revisa access_type=offline y el consentimiento)".to_string())?;

    let session = AuthSession {
        user_id: profile.sub,
        email: profile.email,
        name: profile.name,
        access_token: tokens.access_token,
        refresh_token,
        expires_at: crate::store::now_ms() + (tokens.expires_in as i64) * 1000,
    };
    Ok(session)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
}

fn exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<TokenResponse, String> {
    post_token(&[
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
        ("code_verifier", verifier),
    ])
}

pub fn refresh(refresh_token: &str) -> Result<(String, u64), String> {
    let client_id = google_client_id().ok_or_else(|| "Google OAuth no configurado".to_string())?;
    let client_secret = google_client_secret().unwrap_or_default();
    let t = post_token(&[
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ])?;
    Ok((t.access_token, t.expires_in))
}

fn post_token(params: &[(&str, &str)]) -> Result<TokenResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .map_err(|e| format!("token request: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("token response: {e}"))?;
    if !status.is_success() {
        return Err(format!("Google devolvió {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("token parse: {e} ({text})"))
}

/// Recibe el callback HTTP en `listener` y extrae el `code`, validando `state`.
/// Devuelve la respuesta HTTP "puedes cerrar esta pestaña" al navegador.
fn recv_callback(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(CALLBACK_TIMEOUT_SECS);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return handle_connection(stream, expected_state),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() > deadline {
                    return Err(format!("la autorización tardó más de {CALLBACK_TIMEOUT_SECS} s. Inténtalo de nuevo."));
                }
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(format!("accept: {e}")),
        }
    }
}

fn handle_connection(mut stream: TcpStream, expected_state: &str) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 8192];
    let n = stream
        .read(&mut buf)
        .map_err(|e| format!("leer request: {e}"))?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();

    let first = req.lines().next().unwrap_or("");
    let path = first.split_whitespace().nth(1).unwrap_or("");
    let (code, state) = parse_callback_query(path);

    let response = match (code.clone(), state) {
        (Some(_), Some(state)) if state == expected_state => {
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 61\r\nConnection: close\r\n\r\nAutorizado. Ya puedes cerrar esta pestaña y volver a FocusFlow."
        }
        (_, Some(_)) => {
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nstate no coincide; cierra la pestaña e inténtalo de nuevo."
        }
        _ => "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nCallback sin code.",
    };
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    code.ok_or_else(|| "no se recibió el code de Google".to_string())
}

fn parse_callback_query(path: &str) -> (Option<String>, Option<String>) {
    let Some(q) = path.split_once('?') else {
        return (None, None);
    };
    let mut code = None;
    let mut state = None;
    for kv in q.1.split('&') {
        let Some((k, v)) = kv.split_once('=') else { continue };
        match k {
            "code" => code = Some(url_decode(v)),
            "state" => state = Some(url_decode(v)),
            _ => {}
        }
    }
    (code, state)
}

struct IdTokenProfile {
    sub: String,
    email: String,
    name: String,
}

fn parse_id_token(token: &str) -> Result<IdTokenProfile, String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| "id_token sin payload".to_string())?;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("id_token decode: {e}"))?;
    #[derive(Deserialize)]
    struct Claims {
        #[serde(default)]
        sub: String,
        #[serde(default)]
        email: String,
        #[serde(default)]
        name: String,
    }
    let c: Claims = serde_json::from_slice(&json).map_err(|e| format!("id_token parse: {e}"))?;
    Ok(IdTokenProfile {
        sub: c.sub,
        email: c.email,
        name: c.name,
    })
}

fn rand_bytes(n: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut v = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut v);
    v
}

fn base64url(b: impl AsRef<[u8]>) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_roundtrip() {
        let s = "a b+c/d?e=f&g=h%20i";
        let enc = url_encode(s);
        assert_eq!(url_decode(&enc), s);
    }

    #[test]
    fn url_encode_reserves_safe_chars() {
        assert_eq!(url_encode("abc-._~"), "abc-._~");
        assert_eq!(url_encode("a b"), "a%20b");
    }

    #[test]
    fn parse_callback_extracts_code_and_state() {
        let (code, state) = parse_callback_query("/callback?code=ABC123&state=xyz");
        assert_eq!(code.as_deref(), Some("ABC123"));
        assert_eq!(state.as_deref(), Some("xyz"));
    }

    #[test]
    fn parse_callback_handles_urlencoded_values() {
        let (code, state) = parse_callback_query("/callback?code=a%2Bb&state=x%20y");
        assert_eq!(code.as_deref(), Some("a+b"));
        assert_eq!(state.as_deref(), Some("x y"));
    }

    #[test]
    fn parse_callback_ignores_missing() {
        let (code, state) = parse_callback_query("/");
        assert!(code.is_none());
        assert!(state.is_none());
    }

    #[test]
    fn pkce_challenge_is_43_char_base64url() {
        let verifier = base64url(rand_bytes(32));
        assert_eq!(verifier.len(), 43);
        let challenge = base64url(Sha256::digest(verifier.as_bytes()));
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('+') && !challenge.contains('/') && !challenge.contains('='));
    }
}