//! ════════════════════════════════════════════════════════════════════════
//! MÓDULO OPCIONAL: REPORTE DE ERRORES POR CORREO
//! ════════════════════════════════════════════════════════════════════════
//! Envía un correo a REPORT_DEST con la descripción del usuario y las
//! últimas líneas de error del log local, usando la cuenta de correo que la
//! persona ya configuró en la app (SMTP sobre TLS, puerto 465).
//!
//! Es auto-contenido a propósito, para que sea fácil de retirar:
//!   1. Borrar este archivo (`report.rs`).
//!   2. Quitar `mod report;` en `lib.rs`.
//!   3. Quitar el comando `report_send` y su registro en `invoke_handler`.
//!   4. Quitar el <section> "Reportar un error" en `Settings.svelte`.
//! ════════════════════════════════════════════════════════════════════════

use std::io::{Read, Write};
use std::net::TcpStream;

use crate::ai::provider::get_email_credentials;
use crate::email::EmailConfig;

/// Destinatario de los reportes.
pub const REPORT_DEST: &str = "mmvaleradaza@gmail.com";

const MAX_DESCRIPTION_CHARS: usize = 2000;
const MAX_ERROR_LINES: usize = 30;

/// Deriva el host SMTP a partir del host IMAP configurado
/// (`imap.gmail.com` → `smtp.gmail.com`; si no hay prefijo `imap.`, se antepone `smtp.`).
pub fn smtp_host_for(imap_host: &str) -> String {
    let h = imap_host.trim();
    if let Some(rest) = h.strip_prefix("imap.") {
        format!("smtp.{rest}")
    } else if h.starts_with("smtp.") {
        h.to_string()
    } else {
        format!("smtp.{h}")
    }
}

/// Últimas líneas del log local que parecen errores (sin datos sensibles:
/// `append_log` ya sanea saltos de línea; aquí además se recorta el largo).
pub fn recent_error_lines(max: usize) -> Vec<String> {
    let Some(dir) = crate::log_dir() else { return Vec::new() };
    let Ok(content) = std::fs::read_to_string(dir.join("spike.log")) else {
        return Vec::new();
    };
    let mut lines: Vec<String> = content
        .lines()
        .filter(|l| {
            let l = l.to_lowercase();
            l.contains("error") || l.contains("fail") || l.contains("panic")
        })
        .map(|l| l.chars().take(300).collect())
        .collect();
    if lines.len() > max {
        lines = lines.split_off(lines.len() - max);
    }
    lines
}

/// base64 mínimo (AUTH LOGIN lo requiere); sin dependencias extra.
fn b64(input: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

struct Smtp {
    stream: native_tls::TlsStream<TcpStream>,
    buf: Vec<u8>,
}

impl Smtp {
    fn connect(host: &str, port: u16) -> Result<Self, String> {
        let tcp = TcpStream::connect((host, port)).map_err(|e| format!("no se pudo conectar a {host}:{port}: {e}"))?;
        tcp.set_read_timeout(Some(std::time::Duration::from_secs(15))).ok();
        tcp.set_write_timeout(Some(std::time::Duration::from_secs(15))).ok();
        let cx = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
        let tls = cx.connect(host, tcp).map_err(|e| format!("TLS con {host}: {e}"))?;
        let mut s = Smtp { stream: tls, buf: Vec::new() };
        s.expect("220")?;
        Ok(s)
    }

    fn read_line(&mut self) -> Result<String, String> {
        loop {
            if let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=pos).collect();
                return String::from_utf8(line).map_err(|e| e.to_string());
            }
            let mut chunk = [0u8; 1024];
            let n = self.stream.read(&mut chunk).map_err(|e| format!("SMTP lectura: {e}"))?;
            if n == 0 {
                return Err("SMTP: conexión cerrada".into());
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    fn expect(&mut self, code: &str) -> Result<String, String> {
        let mut last;
        loop {
            let line = self.read_line()?;
            last = line.trim().to_string();
            // respuestas multilínea: "250-..." hasta "250 ..."
            if line.len() < 4 || line.as_bytes()[3] == b' ' {
                break;
            }
        }
        if last.starts_with(code) {
            Ok(last)
        } else {
            Err(format!("SMTP: esperaba {code}, llegó «{last}»"))
        }
    }

    fn cmd(&mut self, c: &str, expect: &str) -> Result<String, String> {
        self.stream.write_all(c.as_bytes()).and_then(|_| self.stream.write_all(b"\r\n")).map_err(|e| e.to_string())?;
        self.stream.flush().map_err(|e| e.to_string())?;
        self.expect(expect)
    }
}

/// Envía el reporte. Devuelve un mensaje de confirmación legible.
pub fn send_report(cfg: &EmailConfig, description: &str) -> Result<String, String> {
    if cfg.host.trim().is_empty() || cfg.user.trim().is_empty() {
        return Err("Configura primero tu correo en Ajustes → Correo electrónico.".into());
    }
    let pass = get_email_credentials(&cfg.user)
        .ok_or_else(|| "No hay contraseña guardada para tu correo. Guárdala en Ajustes → Correo electrónico.".to_string())?;

    let desc: String = description.trim().chars().take(MAX_DESCRIPTION_CHARS).collect();
    let errors = recent_error_lines(MAX_ERROR_LINES);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();

    let body = format!(
        "Reporte de error — FocusFlow\r\n\
         Fecha: {now}\r\n\
         Cuenta: {user}\r\n\
         Versión: {ver}\r\n\
         \r\n\
         Descripción del usuario:\r\n{desc}\r\n\
         \r\n\
         Últimos errores del log:\r\n{errs}\r\n",
        user = cfg.user,
        ver = env!("CARGO_PKG_VERSION"),
        desc = if desc.is_empty() { "(sin descripción)" } else { &desc },
        errs = if errors.is_empty() { "(sin errores recientes en el log)".to_string() } else { errors.join("\r\n") },
    );

    let host = smtp_host_for(&cfg.host);
    let mut smtp = Smtp::connect(&host, 465)?;
    smtp.cmd("EHLO focusflow.local", "250")?;
    smtp.cmd("AUTH LOGIN", "334")?;
    smtp.cmd(&b64(&cfg.user), "334")?;
    smtp.cmd(&b64(&pass), "235")?;
    smtp.cmd(&format!("MAIL FROM:<{}>", cfg.user), "250")?;
    smtp.cmd(&format!("RCPT TO:<{REPORT_DEST}>"), "250")?;
    smtp.cmd("DATA", "354")?;
    // Cabeceras mínimas; el cuerpo va en texto plano ASCII/UTF-8.
    let msg = format!(
        "From: {}\r\nTo: {}\r\nSubject: [FocusFlow] Reporte de error — {}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{}\r\n.\r\n",
        cfg.user, REPORT_DEST, now, body
    );
    smtp.stream.write_all(msg.as_bytes()).map_err(|e| e.to_string())?;
    smtp.stream.flush().map_err(|e| e.to_string())?;
    smtp.expect("250")?;
    let _ = smtp.cmd("QUIT", "221");
    Ok(format!("Reporte enviado desde {} a {}", cfg.user, REPORT_DEST))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smtp_host_derivation() {
        assert_eq!(smtp_host_for("imap.gmail.com"), "smtp.gmail.com");
        assert_eq!(smtp_host_for("smtp.office365.com"), "smtp.office365.com");
        assert_eq!(smtp_host_for("mail.midominio.com"), "smtp.mail.midominio.com");
    }

    #[test]
    fn b64_known_values() {
        assert_eq!(b64(""), "");
        assert_eq!(b64("a"), "YQ==");
        assert_eq!(b64("ab"), "YWI=");
        assert_eq!(b64("abc"), "YWJj");
        assert_eq!(b64("hola@correo.com"), "aG9sYUBjb3JyZW8uY29t");
    }
}
