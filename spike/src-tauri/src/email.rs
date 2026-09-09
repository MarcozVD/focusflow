use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::ai::email_parser::html_to_text;

const MAX_BODY_CHARS: usize = 8000;
const MAX_FETCH_PER_SYNC: usize = 50;

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailFilters {
    #[serde(default)]
    pub senders: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl Default for EmailFilters {
    fn default() -> Self {
        EmailFilters {
            senders: Vec::new(),
            domains: Vec::new(),
            keywords: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailConfig {
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default = "default_auth")]
    pub auth: String,
    #[serde(default = "default_mailboxes")]
    pub mailboxes: Vec<String>,
    #[serde(default)]
    pub filters: EmailFilters,
    #[serde(default = "default_ssl")]
    pub ssl: bool,
}

impl Default for EmailConfig {
    fn default() -> Self {
        EmailConfig {
            host: String::new(),
            port: 443,
            user: String::new(),
            auth: "password".into(),
            mailboxes: vec!["INBOX".into()],
            filters: EmailFilters::default(),
            ssl: true,
        }
    }
}

fn default_port() -> u16 {
    443
}
fn default_auth() -> String {
    "password".into()
}
fn default_mailboxes() -> Vec<String> {
    vec!["INBOX".into()]
}
fn default_ssl() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncCheckpoint {
    #[serde(default)]
    pub uid: u32,
    #[serde(default)]
    pub uidvalidity: u32,
    #[serde(default)]
    pub last_reviewed_date: i64,
    /// Correo que falló por red/IA en el último intento y cuántas veces
    /// seguidas lleva fallando. Si el mismo correo falla MAX_SYNC_RETRIES
    /// veces seguidas, se salta para no congelar el checkpoint para siempre
    /// (un rescan manual lo recupera).
    #[serde(default)]
    pub fail_uid: u32,
    #[serde(default)]
    pub fail_count: u32,
}

impl SyncCheckpoint {
    pub fn empty() -> Self {
        SyncCheckpoint {
            uid: 0,
            uidvalidity: 0,
            last_reviewed_date: now_ms(),
            fail_uid: 0,
            fail_count: 0,
        }
    }
}

/// Fallos seguidos de red/IA sobre el MISMO correo antes de saltarlo.
pub const MAX_SYNC_RETRIES: u32 = 3;

/// Registra un fallo transitorio (red/IA) sobre `uid`. Devuelve el checkpoint
/// actualizado y `true` si el correo lleva MAX_SYNC_RETRIES fallos seguidos y
/// debe saltarse para no congelar el sync.
pub fn register_fail(cp: &SyncCheckpoint, uid: u32) -> (SyncCheckpoint, bool) {
    let mut next = cp.clone();
    if next.fail_uid == uid {
        next.fail_count += 1;
    } else {
        next.fail_uid = uid;
        next.fail_count = 1;
    }
    let skip = next.fail_count >= MAX_SYNC_RETRIES;
    (next, skip)
}

#[derive(Debug, Clone)]
pub struct RawEmail {
    pub mailbox: String,
    pub uid: u32,
    pub message_id: String,
    /// Identificadores de la conversación (In-Reply-To + References), para
    /// detectar correcciones dentro de un hilo sin duplicar compromisos.
    pub thread: Vec<String>,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub body: String,
}

/// Filtros: si la lista está vacía → pasa todo. Si no, debe cumplir algún criterio.
/// ¿Hay algún filtro configurado? (false → todo pasa)
pub fn has_filters(f: &EmailFilters) -> bool {
    !f.senders.is_empty() || !f.domains.is_empty() || !f.keywords.is_empty()
}

/// Unión de filtros: el correo pasa si coincide con CUALQUIER grupo
/// configurado (remitente O dominio O palabra clave). Con ningún filtro
/// configurado, todo pasa. Si se configuraron grupos y ninguno coincide,
/// el correo se descarta (y se registra en el log + rollback de checkpoint
/// para poder recuperarlo al ajustar los filtros).
pub fn matches_filters(e: &RawEmail, f: &EmailFilters) -> bool {
    let sender_lower = e.sender.to_lowercase();
    let domain = sender_lower
        .split('@')
        .nth(1)
        .unwrap_or("")
        .trim_end_matches('>')
        .to_string();
    let body_lower = format!("{} {}", e.subject.to_lowercase(), e.body.to_lowercase());

    if !f.senders.is_empty() || !f.domains.is_empty() || !f.keywords.is_empty() {
        if f.senders
            .iter()
            .any(|s| sender_lower.contains(&s.to_lowercase()))
        {
            return true;
        }
        if f.domains
            .iter()
            .any(|d| domain.contains(&d.to_lowercase()) || sender_lower.contains(&d.to_lowercase()))
        {
            return true;
        }
        if f.keywords
            .iter()
            .any(|k| body_lower.contains(&k.to_lowercase()))
        {
            return true;
        }
        return false;
    }
    true
}

fn parse_body(pm: &mailparse::ParsedMail) -> String {
    let try_sub = |sub: &mailparse::ParsedMail| -> Option<String> {
        let ct = sub.ctype.mimetype.as_str();
        if ct == "text/plain" {
            let raw = sub.get_body_raw().unwrap_or_default();
            let txt = String::from_utf8_lossy(&raw).to_string();
            if !txt.trim().is_empty() {
                return Some(txt);
            }
        }
        None
    };
    for sub in &pm.subparts {
        if let Some(t) = try_sub(sub) {
            return t;
        }
    }
    for sub in &pm.subparts {
        let ct = sub.ctype.mimetype.as_str();
        if ct == "text/html" {
            let raw = sub.get_body_raw().unwrap_or_default();
            let txt = String::from_utf8_lossy(&raw).to_string();
            if !txt.trim().is_empty() {
                return html_to_text(&txt);
            }
        }
    }
    let ct = pm.ctype.mimetype.as_str();
    if ct == "text/plain" {
        let raw = pm.get_body_raw().unwrap_or_default();
        return String::from_utf8_lossy(&raw).to_string();
    }
    if ct == "text/html" {
        let raw = pm.get_body_raw().unwrap_or_default();
        return html_to_text(&String::from_utf8_lossy(&raw));
    }
    String::new()
}

pub type ImapSession = gmail::GmailClient;

/// Cliente REST de Gmail API (scope `gmail.readonly`, SENSIBLE — no
/// restringido). Sustituye al IMAP XOAUTH2 (scope `mail.google.com`,
/// RESTRINGIDO → verificación + CASA anual de pago). Mantiene el contrato
/// que usa sync.rs: `connect` → `fetch_mailbox` → `logout`.
pub mod gmail {
    use super::{now_ms, parse_body, RawEmail, SyncCheckpoint, MAX_BODY_CHARS, MAX_FETCH_PER_SYNC};

    const GMAIL_API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

    pub struct GmailClient {
        http: reqwest::blocking::Client,
        token: String,
    }

    #[derive(serde::Deserialize)]
    struct MsgRef {
        #[serde(default)]
        id: String,
    }

    #[derive(serde::Deserialize)]
    struct ListResponse {
        #[serde(default)]
        messages: Vec<MsgRef>,
    }

    #[derive(serde::Deserialize)]
    struct GetResponse {
        /// Mensaje completo MIME en base64url (format=raw).
        #[serde(default)]
        raw: String,
        /// Milisegundos desde época (string) de la fecha interna del mensaje.
        #[serde(default)]
        internal_date: String,
    }

    #[derive(serde::Deserialize)]
    struct Profile {
        #[serde(default)]
        messages_total: u32,
        #[serde(default)]
        email_address: String,
    }

    impl GmailClient {
        pub fn connect(_config: &super::EmailConfig, access_token: &str) -> Result<Self, String> {
            if access_token.is_empty() {
                return Err("no hay sesión de Google: inicia sesión para conectar Gmail".into());
            }
            Ok(GmailClient {
                http: reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .map_err(|e| e.to_string())?,
                token: access_token.to_string(),
            })
        }

        fn get(&self, path: &str) -> Result<String, String> {
            let resp = self
                .http
                .get(format!("{GMAIL_API}{path}"))
                .bearer_auth(&self.token)
                .send()
                .map_err(|e| format!("gmail api: {e}"))?;
            let status = resp.status();
            if !status.is_success() {
                // Retry-After si Gmail lo envía (429); el cuerpo JSON crudo
                // queda para el log a través del mensaje completo, pero el
                // error que sube a la UI es corto y legible.
                let retry_after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok());
                let text = resp.text().unwrap_or_default();
                return Err(Self::short_gmail_error(status, &text, retry_after));
            }
            resp.text().map_err(|e| format!("gmail api body: {e}"))
        }

        /// Traduce errores HTTP de Gmail API a mensajes cortos y accionables
        /// para la UI (el cuerpo JSON crudo es técnico: solo log).
        #[cfg(test)]
        pub(crate) fn short_gmail_error_for_test(
            status: reqwest::StatusCode,
            body: &str,
            retry_after: Option<u64>,
        ) -> String {
            Self::short_gmail_error(status, body, retry_after)
        }

        fn short_gmail_error(status: reqwest::StatusCode, body: &str, retry_after: Option<u64>) -> String {
            let code = status.as_u16();
            match code {
                401 => "La sesión de Google caducó: cierra sesión y vuelve a entrar en Ajustes."
                    .into(),
                403 if body.contains("SERVICE_DISABLED") || body.contains("accessNotConfigured") => {
                    "Gmail API deshabilitada en el proyecto de Google (error de configuración del servicio; no es culpa tuya). Se reintentará más tarde.".into()
                }
                403 => "Sin permiso para leer Gmail: revisa los permisos de la cuenta en Ajustes.".into(),
                429 => {
                    let w = retry_after
                        .map(|s| format!(" Reintento automático en ~{s} s."))
                        .unwrap_or_else(|| " Reintento automático en un rato.".into());
                    format!("Límite de consultas a Gmail alcanzado.{w}")
                }
                500..=599 => "Gmail está temporalmente caído; se reintentará automáticamente.".into(),
                _ => format!("Error de Gmail ({code}); se reintentará automáticamente."),
            }
        }

        /// Prueba de conexión: perfil del buzón (dirección + nº de correos).
        pub fn test_connection(&self) -> Result<(String, u32), String> {
            let text = self.get("/profile")?;
            let p: Profile =
                serde_json::from_str(&text).map_err(|e| format!("perfil gmail: {e} ({text})"))?;
            let label = if p.email_address.is_empty() {
                "Gmail".to_string()
            } else {
                p.email_address
            };
            Ok((label, p.messages_total))
        }

        /// Mensajes nuevos de una bandeja desde el checkpoint. Equivalente
        /// REST del antiguo `uid_search` + `uid_fetch` de IMAP:
        /// - cursor: segundos de `internalDate` (monótono, cabe en u32 hasta 2106)
        /// - ventana: `after:` con fecha del checkpoint (con 1 día de margen)
        ///   o, en primera pasada, `today - since_days`.
        pub fn fetch_mailbox(
            &self,
            mailbox: &str,
            checkpoint: &SyncCheckpoint,
            since_days: u32,
        ) -> Result<(Vec<RawEmail>, SyncCheckpoint), String> {
            let mut new_checkpoint = checkpoint.clone();

            // consulta Gmail: bandeja + ventana temporal
            let mut query = if mailbox == "INBOX" {
                "in:inbox".to_string()
            } else {
                format!("label:{mailbox}")
            };
            if checkpoint.uid > 0 {
                // margen de 1 día: emails cuyo Date difiere de internalDate
                let cp_date = chrono::DateTime::from_timestamp(checkpoint.uid as i64, 0)
                    .map(|d| d.with_timezone(&chrono::Local).date_naive())
                    .unwrap_or_else(|| chrono::Local::now().date_naive())
                    - chrono::Duration::days(1);
                query.push_str(&format!(" after:{}", cp_date.format("%Y/%m/%d")));
            } else if since_days > 0 {
                let since =
                    chrono::Local::now().date_naive() - chrono::Duration::days(since_days as i64);
                query.push_str(&format!(" after:{}", since.format("%Y/%m/%d")));
            }

            let path = format!(
                "/messages?maxResults={MAX_FETCH_PER_SYNC}&q={}",
                urlencode(&query)
            );
            let text = self.get(&path)?;
            let list: ListResponse =
                serde_json::from_str(&text).map_err(|e| format!("list gmail: {e} ({text})"))?;

            let cutoff = chrono::Utc::now() - chrono::Duration::days(since_days as i64);
            let mut emails = Vec::new();
            let mut max_uid: u32 = checkpoint.uid;

            for m in &list.messages {
                if m.id.is_empty() {
                    continue;
                }
                let get_path = format!("/messages/{}?format=raw", m.id);
                let gtext = self.get(&get_path)?;
                let g: GetResponse =
                    serde_json::from_str(&gtext).map_err(|e| format!("get {}: {e}", m.id))?;
                let uid = msg_uid(&g.internal_date, &m.id);
                if uid > max_uid {
                    max_uid = uid;
                }
                if uid <= checkpoint.uid {
                    continue; // ya cubierto por el checkpoint (o anterior a él)
                }

                let mime_bytes = match b64url_decode(&g.raw) {
                    Some(b) => b,
                    None => continue,
                };
                let Ok(pm) = mailparse::parse_mail(&mime_bytes) else {
                    continue;
                };

                let header = |key: &str| -> String {
                    pm.headers
                        .iter()
                        .find(|h| h.get_key().eq_ignore_ascii_case(key))
                        .map(|h| h.get_value())
                        .unwrap_or_default()
                };

                // fuera de la ventana temporal → no se procesa (el cursor ya avanzó)
                let date_raw = header("Date");
                if since_days > 0 {
                    if let Ok(t) = mailparse::dateparse(&date_raw) {
                        if t < cutoff.timestamp() {
                            continue;
                        }
                    }
                }

                let mut body_text = parse_body(&pm);
                body_text.truncate(MAX_BODY_CHARS);

                // hilo: In-Reply-To (padre inmediato) + References (toda la cadena)
                let thread: Vec<String> = [header("In-Reply-To"), header("References")]
                    .join(" ")
                    .split_whitespace()
                    .map(|s| s.trim_matches(|c| c == '<' || c == '>').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                emails.push(RawEmail {
                    mailbox: mailbox.to_string(),
                    uid,
                    message_id: {
                        let mid = header("Message-ID");
                        if mid.is_empty() {
                            format!("gmail-{}", m.id)
                        } else {
                            mid
                        }
                    },
                    thread,
                    subject: header("Subject"),
                    sender: header("From"),
                    date: date_raw,
                    body: body_text,
                });
            }

            new_checkpoint.uid = max_uid;
            new_checkpoint.last_reviewed_date = now_ms();
            Ok((emails, new_checkpoint))
        }

        pub fn logout(&self) {}
    }

    /// Cursor a partir de `internalDate` (ms desde época, string JSON).
    /// Segundos desde época en u32 (válido hasta 2106). Si falta el campo,
    /// fallback: hash FNV-1a del id (estable, sin orden temporal).
    fn msg_uid(internal_date: &str, id: &str) -> u32 {
        if let Ok(ms) = internal_date.parse::<i64>() {
            if ms > 0 {
                return (ms / 1000).min(u32::MAX as i64) as u32;
            }
        }
        let mut h: u32 = 2166136261;
        for b in id.as_bytes() {
            h = h.wrapping_mul(16777619).wrapping_add(*b as u32);
        }
        h
    }

    #[cfg(test)]
    pub fn msg_uid_for_test(internal_date: &str, id: &str) -> u32 {
        msg_uid(internal_date, id)
    }

    fn urlencode(s: &str) -> String {
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

    fn b64url_decode(s: &str) -> Option<Vec<u8>> {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(s))
            .ok()
    }
}

pub fn connect(config: &EmailConfig, access_token: &str) -> Result<ImapSession, String> {
    gmail::GmailClient::connect(config, access_token)
}

/// Extrae la dirección de email pura de un encabezado From
/// ("Jefa <jefa@x.com>" -> "jefa@x.com"). Normaliza a minúsculas.
pub fn sender_email(raw: &str) -> String {
    if let Some(open) = raw.rfind('<') {
        if let Some(close) = raw.rfind('>') {
            if close > open {
                return raw[open + 1..close].trim().to_lowercase();
            }
        }
    }
    raw.trim().to_lowercase()
}

/// Prueba de conexión: perfil del buzón de Gmail (dirección + nº de correos).
pub fn test_connection(config: &EmailConfig, access_token: &str) -> Result<(String, u32), String> {
    if config.host.is_empty() || config.user.is_empty() {
        return Err("host y usuario requeridos".into());
    }
    if access_token.is_empty() {
        return Err("no hay sesión de Google: inicia sesión para conectar Gmail".into());
    }
    let client = gmail::GmailClient::connect(config, access_token)?;
    client.test_connection()
}

pub fn fetch_mailbox(
    session: &mut ImapSession,
    mailbox: &str,
    checkpoint: &SyncCheckpoint,
    since_days: u32,
) -> Result<(Vec<RawEmail>, SyncCheckpoint), String> {
    session.fetch_mailbox(mailbox, checkpoint, since_days)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_fail_skips_after_max_retries_on_same_uid() {
        let cp = SyncCheckpoint::empty();
        let (cp, skip) = register_fail(&cp, 100);
        assert!(!skip && cp.fail_count == 1);
        let (cp, skip) = register_fail(&cp, 100);
        assert!(!skip && cp.fail_count == 2);
        let (cp, skip) = register_fail(&cp, 100);
        assert!(skip && cp.fail_count == 3, "3 fallos seguidos → saltar");
        // otro uid reinicia el contador
        let (cp, skip) = register_fail(&cp, 200);
        assert!(!skip && cp.fail_count == 1 && cp.fail_uid == 200);
    }

    #[test]
    fn checkpoint_fail_fields_default_to_zero_in_old_json() {
        // checkpoints guardados antes de este campo deben seguir parseando
        let cp: SyncCheckpoint =
            serde_json::from_str(r#"{"uid":10,"uidvalidity":3,"last_reviewed_date":1}"#).unwrap();
        assert_eq!(cp.uid, 10);
        assert_eq!(cp.fail_count, 0);
    }

    fn raw(sender: &str, subject: &str, body: &str) -> RawEmail {
        RawEmail {
            mailbox: "INBOX".into(),
            uid: 1,
            message_id: "m1".into(),
            thread: Vec::new(),
            subject: subject.into(),
            sender: sender.into(),
            date: "2026-08-08".into(),
            body: body.into(),
        }
    }

    #[test]
    fn union_semantics_sender_or_domain_or_keyword() {
        let f = EmailFilters {
            senders: vec![
                "notifications@instructure.com".into(),
                "gosma@unab.edu.co".into(),
            ],
            domains: vec!["unab.edu.co".into()],
            keywords: vec!["examen".into()],
        };
        // remitente en la lista, dominio fuera → pasa (antes: AND lo rechazaba)
        let canvas = raw(
            "UNAB Canvas <notifications@instructure.com>",
            "Tarea calificada",
            "hola",
        );
        assert!(matches_filters(&canvas, &f));
        // dominio universitario, remitente fuera de la lista → pasa
        let uni = raw("jpinzon408@unab.edu.co", "Clase de IoT", "hola");
        assert!(matches_filters(&uni, &f));
        // palabra clave en el asunto, sin remitente/dominio → pasa
        let kw = raw("alguien@outlook.com", "Examen de cálculo", "hola");
        assert!(matches_filters(&kw, &f));
        // sin coincidencia en ningún grupo → descartado
        let spam = raw("publicidad@outlook.com", "Oferta", "compra");
        assert!(!matches_filters(&spam, &f));
    }

    #[test]
    fn no_filters_means_everything_passes() {
        let f = EmailFilters::default();
        let e = raw("cualquiera@x.com", "asunto", "cuerpo");
        assert!(matches_filters(&e, &f));
        assert!(!has_filters(&f));
    }

    #[test]
    fn single_group_still_filters() {
        let f = EmailFilters {
            senders: vec!["jefe@corp.com".into()],
            ..EmailFilters::default()
        };
        assert!(matches_filters(
            &raw("Jefe <jefe@corp.com>", "reunión", ""),
            &f
        ));
        assert!(!matches_filters(&raw("otro@corp.com", "reunión", ""), &f));
    }

    #[test]
    fn connect_requires_token() {
        let cfg = EmailConfig::default();
        let err = match connect(&cfg, "") {
            Ok(_) => panic!("sin token no debe conectar"),
            Err(e) => e,
        };
        assert!(err.contains("inicia sesión"), "{err}");
    }

    #[test]
    fn internal_date_maps_to_epoch_seconds_cursor() {
        // 2026-09-09T00:00:00Z = 1_788_912_000 s (cabe en u32)
        assert_eq!(
            gmail::msg_uid_for_test("1788912000000", "abc"),
            1_788_912_000
        );
        // sin internalDate → hash estable del id
        let h1 = gmail::msg_uid_for_test("", "abc");
        let h2 = gmail::msg_uid_for_test("", "abc");
        assert_eq!(h1, h2);
        assert_ne!(h1, gmail::msg_uid_for_test("", "xyz"));
    }

    #[test]
    fn gmail_errors_are_short_and_actionable() {
        use reqwest::StatusCode;
        // 403 SERVICE_DISABLED (caso real del usuario)
        let e = gmail::GmailClient::short_gmail_error_for_test(
            StatusCode::FORBIDDEN,
            r#"{"error":{"message":"Gmail API has not been used in project…","status":"PERMISSION_DENIED"},"details":[{"reason":"SERVICE_DISABLED"}]}"#,
            None,
        );
        assert!(e.contains("Gmail API deshabilitada"), "{e}");
        assert!(!e.contains('{'), "sin JSON crudo: {e}");
        // 429 con Retry-After
        let e = gmail::GmailClient::short_gmail_error_for_test(
            StatusCode::TOO_MANY_REQUESTS,
            "{\"error\":\"quota\"}",
            Some(90),
        );
        assert!(e.contains("~90 s"), "{e}");
        // 401 → sesión caducada
        let e = gmail::GmailClient::short_gmail_error_for_test(StatusCode::UNAUTHORIZED, "", None);
        assert!(e.contains("caducó"), "{e}");
        // 403 permisos normales
        let e = gmail::GmailClient::short_gmail_error_for_test(StatusCode::FORBIDDEN, "forbidden", None);
        assert!(e.contains("permiso"), "{e}");
        // 5xx → temporal
        let e = gmail::GmailClient::short_gmail_error_for_test(
            StatusCode::BAD_GATEWAY,
            "upstream",
            None,
        );
        assert!(e.contains("temporalmente"), "{e}");
    }
}
