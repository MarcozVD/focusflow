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

/// Corta `s` a `max` CARACTERES (nunca en mitad de un carácter UTF-8).
pub(crate) fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Cuerpo legible del correo. Recorre el árbol MIME completo (p. ej.
/// multipart/mixed → multipart/alternative → text/plain): primero el primer
/// text/plain no vacío; si no hay, el primer text/html convertido a texto.
/// `get_body()` decodifica transfer-encoding Y charset (latin-1, etc.); el
/// antiguo `get_body_raw` + `from_utf8_lossy` solo miraba el primer nivel y
/// rompía acentos de correos no-UTF-8.
fn parse_body(pm: &mailparse::ParsedMail) -> String {
    fn first_of(pm: &mailparse::ParsedMail, mime: &str) -> Option<String> {
        if pm.subparts.is_empty() {
            if pm.ctype.mimetype.eq_ignore_ascii_case(mime) {
                let txt = pm.get_body().unwrap_or_else(|_| {
                    String::from_utf8_lossy(&pm.get_body_raw().unwrap_or_default()).to_string()
                });
                if !txt.trim().is_empty() {
                    return Some(txt);
                }
            }
            return None;
        }
        pm.subparts.iter().find_map(|sub| first_of(sub, mime))
    }
    if let Some(t) = first_of(pm, "text/plain") {
        return t;
    }
    if let Some(h) = first_of(pm, "text/html") {
        return html_to_text(&h);
    }
    String::new()
}

pub type ImapSession = gmail::GmailClient;

/// Cliente REST de Gmail API (scope `gmail.readonly`). OJO: Google clasifica
/// TODOS los scopes de Gmail (incluido gmail.readonly) como RESTRINGIDOS →
/// verificación de OAuth + CASA anual de pago. No existe scope de Gmail
/// no-restringido. Este cliente no usa IMAP/SMTP (mail.google.com, también
/// restricted). Mantiene el contrato
/// que usa sync.rs: `connect` → `fetch_mailbox` → `logout`.
pub mod gmail {
    use super::{
        now_ms, parse_body, truncate_chars, RawEmail, SyncCheckpoint, MAX_BODY_CHARS,
        MAX_FETCH_PER_SYNC,
    };

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
    #[serde(rename_all = "camelCase")]
    struct ListResponse {
        #[serde(default)]
        messages: Vec<MsgRef>,
        #[serde(default)]
        next_page_token: Option<String>,
    }

    /// Ids por página de messages.list (máx. de la API: 500).
    const LIST_PAGE_SIZE: usize = 500;
    /// Tope de páginas por sync (5000 ids): suficiente para la ventana
    /// since_days/checkpoint; evita bucles infinitos si la API se porta mal.
    const MAX_LIST_PAGES: usize = 10;
    /// Margen (s) de la ventana `after:` respecto al checkpoint.
    const CHECKPOINT_MARGIN_SECS: u32 = 3600;


    #[derive(serde::Deserialize)]
    // Gmail API responde en camelCase (internalDate, messagesTotal,
    // emailAddress). Sin el rename, serde busca snake_case, el campo queda
    // vacío por `default` y `msg_uid` cae AL FALLBACK DE HASH: un uid sin
    // orden temporal (p. ej. 4202791636 ≈ año 2103) que envenena el
    // checkpoint y deja el sync mudo para siempre. Regresión real.
    #[serde(rename_all = "camelCase")]
    pub(crate) struct GetResponse {
        /// Mensaje completo MIME en base64url (format=raw).
        #[serde(default)]
        pub(crate) raw: String,
        /// Milisegundos desde época (string) de la fecha interna del mensaje.
        #[serde(default)]
        pub(crate) internal_date: String,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct Profile {
        #[serde(default)]
        pub(crate) messages_total: u32,
        #[serde(default)]
        pub(crate) email_address: String,
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

        fn short_gmail_error(
            status: reqwest::StatusCode,
            body: &str,
            retry_after: Option<u64>,
        ) -> String {
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
        ) -> Result<(Vec<RawEmail>, SyncCheckpoint, Vec<String>), String> {
            let mut new_checkpoint = checkpoint.clone();

            // consulta Gmail: bandeja + ventana temporal
            let mut query = if mailbox == "INBOX" {
                "in:inbox".to_string()
            } else {
                format!("label:{mailbox}")
            };
            // Autorreparo: un checkpoint cuyo uid cae en el futuro (p. ej. el
            // hash de un id interpretado como segundos → año 2103) envenena la
            // ventana `after:` y el sync devuelve 0 correos para siempre. Se
            // reinicia a la ventana `since_days` (el dedupe por message_id y
            // email_seen evita re-analizar lo ya procesado).
            let checkpoint_uid = guard_future_uid(checkpoint.uid);
            let now_epoch = chrono::Local::now().timestamp().max(0) as u32;

            if checkpoint_uid > 0 {
                // `after:` en segundos epoch (Gmail lo acepta) con margen de
                // 1 h. Antes era la FECHA del checkpoint −1 día: con la
                // paginación completa eso listaba ~2 días de correo y cada
                // sync descargaba en crudo cientos de mensajes ya revisados
                // solo para descartarlos por `uid <= checkpoint`.
                let after = checkpoint_uid.saturating_sub(CHECKPOINT_MARGIN_SECS);
                query.push_str(&format!(" after:{after}"));
            } else if since_days > 0 {
                let since =
                    chrono::Local::now().date_naive() - chrono::Duration::days(since_days as i64);
                query.push_str(&format!(" after:{}", since.format("%Y/%m/%d")));
            }

            // Paginación completa (bug: solo se leía la 1.ª página de 50 y el
            // checkpoint saltaba al más nuevo → correos viejos perdidos).
            let ids = list_all_ids(|p| self.get(p), &query)?;

            let cutoff = chrono::Utc::now() - chrono::Duration::days(since_days as i64);
            let mut emails = Vec::new();
            let mut parse_failures: Vec<String> = Vec::new();
            let mut max_uid: u32 = checkpoint_uid;
            // Gmail lista de más nuevo a más viejo: se procesa de VIEJO a
            // NUEVO para que, si el tope MAX_FETCH_PER_SYNC corta el lote,
            // el checkpoint quede en el último procesado y el siguiente sync
            // continúe desde ahí (en vez de saltar por encima de los viejos).
            let mut fetched_new = 0usize;
            let mut capped = false;
            for id in ids.iter().rev() {
                if fetched_new >= MAX_FETCH_PER_SYNC {
                    capped = true; // el resto lo recoge el siguiente sync
                    break;
                }
                let get_path = format!("/messages/{id}?format=raw");
                let gtext = self.get(&get_path)?;
                let g: GetResponse =
                    serde_json::from_str(&gtext).map_err(|e| format!("get {id}: {e}"))?;
                // un uid en el futuro (hash de id por fallback, reloj
                // desalineado) no es un cursor temporal válido: se ancla a
                // `now_epoch` para no envenenar el checkpoint (próximos syncs
                // ciegos) ni el rollback; el dedupe real es por message_id.
                let uid = msg_uid(&g.internal_date, id);
                let uid = uid.min(now_epoch);
                if uid <= checkpoint_uid {
                    continue; // ya cubierto por el checkpoint (o anterior a él)
                }
                fetched_new += 1;
                if uid > max_uid {
                    max_uid = uid;
                }

                // MIME roto: se cuenta como procesado (no se puede reintentar
                // con éxito) pero queda registrado, sin contenido, para el log.
                let Some(mime_bytes) = b64url_decode(&g.raw) else {
                    parse_failures.push(id.clone());
                    continue;
                };
                let Ok(pm) = mailparse::parse_mail(&mime_bytes) else {
                    parse_failures.push(id.clone());
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

                // tope por CARACTERES: `String::truncate` corta por bytes y
                // hace panic en mitad de un carácter multibyte (á, emoji) →
                // con panic=abort la app se cerraba en bucle al arrancar.
                let body_text = truncate_chars(&parse_body(&pm), MAX_BODY_CHARS);

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
                            format!("gmail-{id}")
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

            // cortado por tope: el cursor (segundos) queda 1 s antes del
            // último procesado para no saltar correos del MISMO segundo que
            // quedaron fuera; el re-fetch de ese segundo lo absorbe el dedupe
            // por message_id (email_seen)
            new_checkpoint.uid = if capped {
                let back = max_uid.saturating_sub(1);
                // si retroceder 1 s deja el cursor donde estaba (≥ tope de
                // correos en el mismo segundo) el sync reprocesaría el mismo
                // lote para siempre: se avanza al segundo completo
                if back <= checkpoint_uid {
                    max_uid
                } else {
                    back
                }
            } else {
                max_uid
            };
            new_checkpoint.last_reviewed_date = now_ms();
            Ok((emails, new_checkpoint, parse_failures))
        }

        pub fn logout(&self) {}
    }

    /// Un uid de checkpoint que cae en el futuro (> ahora + 1 día) proviene de
    /// el fallback hash-del-id de `msg_uid` (un hash no es una fecha: puede
    /// apuntar a 2078 o 2103). Persistido, envenena la ventana `after:` y el
    /// filtro `uid <= checkpoint.uid` y el sync queda mudo para siempre.
    /// Función pura para poder testearla.
    pub(crate) fn guard_future_uid(uid: u32) -> u32 {
        let now_epoch = chrono::Local::now().timestamp().max(0) as u32;
        if uid > now_epoch + 86_400 {
            0
        } else {
            uid
        }
    }

    #[cfg(test)]
    pub fn guard_future_uid_for_test(uid: u32) -> u32 {
        guard_future_uid(uid)
    }

    /// Lista TODOS los ids de la consulta siguiendo `nextPageToken` (tope
    /// MAX_LIST_PAGES páginas por seguridad). `get` = GET relativo a la API;
    /// inyectable para testear sin red.
    pub(crate) fn list_all_ids(
        mut get: impl FnMut(&str) -> Result<String, String>,
        query: &str,
    ) -> Result<Vec<String>, String> {
        let mut ids: Vec<String> = Vec::new();
        let mut page_token: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let mut path = format!(
                "/messages?maxResults={LIST_PAGE_SIZE}&q={}",
                urlencode(query)
            );
            if let Some(t) = &page_token {
                path.push_str(&format!("&pageToken={}", urlencode(t)));
            }
            let text = get(&path)?;
            // sin el cuerpo crudo en el error (puede ser largo/ruidoso)
            let list: ListResponse =
                serde_json::from_str(&text).map_err(|e| format!("list gmail: {e}"))?;
            ids.extend(list.messages.into_iter().map(|m| m.id).filter(|id| !id.is_empty()));
            match list.next_page_token.filter(|t| !t.is_empty()) {
                Some(t) => page_token = Some(t),
                None => return Ok(ids),
            }
        }
        Ok(ids)
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
) -> Result<(Vec<RawEmail>, SyncCheckpoint, Vec<String>), String> {
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
    fn gmail_json_camel_case_fields_populate_structs() {
        // regresión real: Gmail responde camelCase; si serde no lo renombra,
        // internalDate cae a "" y msg_uid usa el hash (checkpoint envenenado)
        let g: gmail::GetResponse =
            serde_json::from_str(r#"{"id":"abc","raw":"aGk","internalDate":"1788912000000"}"#)
                .unwrap();
        assert_eq!(g.internal_date, "1788912000000");
        let p: gmail::Profile =
            serde_json::from_str(r#"{"emailAddress":"me@x.com","messagesTotal":4242}"#).unwrap();
        assert_eq!(p.messages_total, 4242);
        assert_eq!(p.email_address, "me@x.com");
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
    fn future_checkpoint_uid_is_self_healed() {
        // 2026-09-09 ≈ 1_789_000_000 s. Un uid de hash (2103 → 4_202_791_636,
        // el bug real que dejó el sync mudo) debe tratarse como "sin checkpoint"
        // para que la ventana since_days vuelva a cubrir los correos.
        let now = chrono::Local::now().timestamp() as u32;
        assert_eq!(
            gmail::guard_future_uid_for_test(now - 1_000_000),
            now - 1_000_000
        );
        assert_eq!(gmail::guard_future_uid_for_test(4_202_791_636), 0);
        assert_eq!(gmail::guard_future_uid_for_test(3_427_686_704), 0);
        // un desfase pequeño de reloj (≤ 1 día) NO se cura: sigue siendo cursor
        assert_eq!(gmail::guard_future_uid_for_test(now + 3_600), now + 3_600);
        // más de un día en el futuro → reset a la ventana since_days
        assert_eq!(gmail::guard_future_uid_for_test(now + 2 * 86_400), 0);
    }

    #[test]
    fn truncate_chars_never_splits_multibyte() {
        // regresión #1: `String::truncate(8000)` sobre 'á' (2 bytes) caía en
        // mitad de un carácter → panic (abort en release)
        let body = "á".repeat(MAX_BODY_CHARS + 10);
        let t = truncate_chars(&body, MAX_BODY_CHARS);
        assert_eq!(t.chars().count(), MAX_BODY_CHARS);
        let emoji = "📅".repeat(5);
        assert_eq!(truncate_chars(&emoji, 3), "📅📅📅");
        assert_eq!(truncate_chars("corto", 100), "corto");
    }

    #[test]
    fn parse_body_walks_nested_mime_and_decodes_charset() {
        // multipart/mixed → multipart/alternative → text/plain (latin-1 QP)
        let raw = concat!(
            "From: a@x.com\r\n",
            "Subject: t\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=\"OUT\"\r\n\r\n",
            "--OUT\r\n",
            "Content-Type: multipart/alternative; boundary=\"IN\"\r\n\r\n",
            "--IN\r\n",
            "Content-Type: text/plain; charset=iso-8859-1\r\n",
            "Content-Transfer-Encoding: quoted-printable\r\n\r\n",
            "Entrega el mi=E9rcoles a las 10\r\n",
            "--IN\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>html</p>\r\n",
            "--IN--\r\n",
            "--OUT\r\n",
            "Content-Type: application/pdf\r\n\r\n",
            "xx\r\n",
            "--OUT--\r\n"
        );
        let pm = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let body = parse_body(&pm);
        assert!(body.contains("miércoles"), "charset latin-1 decodificado: {body}");
        assert!(!body.contains("<p>"), "prefiere text/plain");

        // solo html anidado → texto
        let raw = concat!(
            "Content-Type: multipart/mixed; boundary=\"B\"\r\n\r\n",
            "--B\r\n",
            "Content-Type: multipart/alternative; boundary=\"C\"\r\n\r\n",
            "--C\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>Examen <b>viernes</b></p>\r\n",
            "--C--\r\n",
            "--B--\r\n"
        );
        let pm = mailparse::parse_mail(raw.as_bytes()).unwrap();
        let body = parse_body(&pm);
        assert!(body.contains("Examen") && body.contains("viernes"), "{body}");
        assert!(!body.contains("<b>"), "{body}");
    }

    #[test]
    fn list_all_ids_follows_next_page_token() {
        // regresión #2: solo se leía la primera página de messages.list
        let mut calls: Vec<String> = Vec::new();
        let ids = gmail::list_all_ids(
            |p| {
                calls.push(p.to_string());
                Ok(if p.contains("pageToken=T2") {
                    r#"{"messages":[{"id":"c"}]}"#.to_string()
                } else {
                    r#"{"messages":[{"id":"a"},{"id":"b"}],"nextPageToken":"T2"}"#.to_string()
                })
            },
            "in:inbox",
        )
        .unwrap();
        assert_eq!(ids, vec!["a", "b", "c"]);
        assert_eq!(calls.len(), 2);
        // sin mensajes → lista vacía, sin error
        let ids = gmail::list_all_ids(|_| Ok("{}".to_string()), "q").unwrap();
        assert!(ids.is_empty());
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
        let e = gmail::GmailClient::short_gmail_error_for_test(
            StatusCode::FORBIDDEN,
            "forbidden",
            None,
        );
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
