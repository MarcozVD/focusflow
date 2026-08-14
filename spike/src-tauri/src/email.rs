use std::io::{Read, Write};
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
            port: 993,
            user: String::new(),
            auth: "password".into(),
            mailboxes: vec!["INBOX".into()],
            filters: EmailFilters::default(),
            ssl: true,
        }
    }
}

fn default_port() -> u16 {
    993
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
}

impl SyncCheckpoint {
    pub fn empty() -> Self {
        SyncCheckpoint {
            uid: 0,
            uidvalidity: 0,
            last_reviewed_date: now_ms(),
        }
    }
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
        if f.keywords.iter().any(|k| body_lower.contains(&k.to_lowercase())) {
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

pub trait IoStream: Read + Write {}
impl<T: Read + Write> IoStream for T {}

pub type ImapSession = imap::Session<Box<dyn IoStream>>;

pub fn connect(config: &EmailConfig) -> Result<ImapSession, String> {
    // TLS implícito es obligatorio salvo servidor local (pruebas). Sin
    // cifrado, la contraseña y el correo viajan en claro (riesgo MITM).
    let is_local = config.host == "localhost" || config.host == "127.0.0.1" || config.host == "::1";
    if !config.ssl && !is_local {
        return Err("se requiere TLS (activar 'Usar conexión segura')".into());
    }
    let tcp = std::net::TcpStream::connect((config.host.as_str(), config.port))
        .map_err(|e| format!("tcp connect {}:{}: {e}", config.host, config.port))?;
    let stream: Box<dyn IoStream> = if config.ssl || config.port == 993 {
        let connector = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
        let tls = connector
            .connect(&config.host, tcp)
            .map_err(|e| format!("tls connect: {e}"))?;
        Box::new(tls)
    } else {
        Box::new(tcp)
    };

    let client = imap::Client::new(stream);
    let password = crate::ai::get_email_credentials(&config.user).unwrap_or_default();
    let session = client
        .login(&config.user, &password)
        .map_err(|(e, _)| format!("login: {e}"))?;
    Ok(session)
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

/// Prueba de conexión: login + SELECT de la primera bandeja + logout.
/// Devuelve (bandeja, nº de correos) si todo va bien.
pub fn test_connection(config: &EmailConfig) -> Result<(String, u32), String> {
    if config.host.is_empty() || config.user.is_empty() {
        return Err("host y usuario requeridos".into());
    }
    if crate::ai::get_email_credentials(&config.user).is_none() {
        return Err("falta la contraseña de aplicación".into());
    }
    let mailbox = config
        .mailboxes
        .first()
        .cloned()
        .ok_or_else(|| "no hay bandejas configuradas".to_string())?;
    let mut session = connect(config)?;
    let mb = session
        .select(&mailbox)
        .map_err(|e| format!("select {mailbox}: {e}"))?;
    let exists = mb.exists;
    let _ = session.logout();
    Ok((mailbox, exists))
}

pub fn fetch_mailbox(
    session: &mut ImapSession,
    mailbox: &str,
    checkpoint: &SyncCheckpoint,
    since_days: u32,
) -> Result<(Vec<RawEmail>, SyncCheckpoint), String> {
    session
        .select(mailbox)
        .map_err(|e| format!("select {mailbox}: {e}"))?;

    let mut new_checkpoint = checkpoint.clone();
    new_checkpoint.uidvalidity = 0; // imap 2.x no expone UIDVALIDITY; reseteo heurístico abajo

    let start_uid = checkpoint.uid + 1;
    let search_expr = if start_uid > 1 {
        format!("UID {start_uid}:*")
    } else if since_days > 0 {
        // primera pasada (sin checkpoint): solo la ventana reciente, no el buzón histórico
        let since = chrono::Local::now().date_naive() - chrono::Duration::days(since_days as i64);
        format!("SINCE {}", since.format("%d-%b-%Y"))
    } else {
        "1:*".into()
    };
    let searched = session
        .uid_search(search_expr.as_str())
        .map_err(|e| format!("uid_search({search_expr}): {e}"))?;
    let ids: Vec<u32> = searched.iter().copied().collect();

    let take: Vec<u32> = ids.into_iter().take(MAX_FETCH_PER_SYNC).collect();
    if take.is_empty() {
        return Ok((Vec::new(), new_checkpoint));
    }

    let uids_str = take
        .iter()
        .map(|u| u.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let fetched = session
        .uid_fetch(uids_str.as_str(), "(BODY.PEEK[])")
        .map_err(|e| format!("uid_fetch: {e}"))?;

    let cutoff = chrono::Utc::now() - chrono::Duration::days(since_days as i64);
    let mut emails = Vec::new();
    let mut max_uid: u32 = 0;
    for f in fetched.iter() {
        let Some(body) = f.body() else { continue };
        let Ok(pm) = mailparse::parse_mail(body) else { continue };
        let uid = f.uid.unwrap_or(0);
        if uid > max_uid {
            max_uid = uid;
        }

        let header = |key: &str| -> String {
            pm.headers
                .iter()
                .find(|h| h.get_key().eq_ignore_ascii_case(key))
                .map(|h| h.get_value())
                .unwrap_or_default()
        };

        // fuera de la ventana temporal → se marca revisado pero no se procesa
        let date_raw = header("Date");
        if since_days > 0 {
            if let Ok(t) = mailparse::dateparse(&date_raw) {
                if (t as i64) < cutoff.timestamp() {
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
            message_id: header("Message-ID"),
            thread,
            subject: header("Subject"),
            sender: header("From"),
            date: date_raw,
            body: body_text,
        });
    }

    // reseteo heurístico: el servidor volvió a UIDs más bajos → mailbox reiniciado
    if checkpoint.uid > 0 && max_uid < checkpoint.uid {
        new_checkpoint.uid = 0;
        new_checkpoint.uidvalidity = max_uid;
    } else {
        new_checkpoint.uid = max_uid;
    }
    new_checkpoint.last_reviewed_date = now_ms();
    Ok((emails, new_checkpoint))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            senders: vec!["notifications@instructure.com".into(), "gosma@unab.edu.co".into()],
            domains: vec!["unab.edu.co".into()],
            keywords: vec!["examen".into()],
        };
        // remitente en la lista, dominio fuera → pasa (antes: AND lo rechazaba)
        let canvas = raw("UNAB Canvas <notifications@instructure.com>", "Tarea calificada", "hola");
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
        let f = EmailFilters { senders: vec!["jefe@corp.com".into()], ..EmailFilters::default() };
        assert!(matches_filters(&raw("Jefe <jefe@corp.com>", "reunión", ""), &f));
        assert!(!matches_filters(&raw("otro@corp.com", "reunión", ""), &f));
    }

    #[test]
    fn plaintext_imap_rejected_outside_localhost() {
        let mut cfg = EmailConfig {
            host: "imap.proveedor.com".into(),
            port: 143,
            ssl: false,
            ..EmailConfig::default()
        };
        let err = match connect(&cfg) {
            Ok(_) => panic!("sin TLS en remoto no debe conectar"),
            Err(e) => e,
        };
        assert!(err.contains("TLS"), "{err}");
        // localhost no recibe el guard: falla por red, no por el guard
        cfg.host = "localhost".into();
        match connect(&cfg) {
            Err(e) => assert!(!e.contains("se requiere TLS"), "el guard no debe aplicar a localhost"),
            Ok(_) => {}
        }
    }
}
