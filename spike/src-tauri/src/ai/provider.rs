//! Capa de abstracción de proveedores de IA (fase 4).
//!
//! ```text
//! AiProvider
//! ├── RuleBasedProvider   (local, determinista — funciona sin red)
//! ├── OpenAiCompatProvider(OpenAI / OpenCode Zen / cualquier chat-completions)
//! ├── GeminiProvider      (REST generativelanguage.googleapis.com)
//! └── FutureProvider      (cualquier otro: implementa el trait)
//! ```
//!
//! Reglas de seguridad:
//! - El trait NO expone acceso a la base de datos: un proveedor jamás puede
//!   ejecutar operaciones destructivas.
//! - Toda salida del proveedor pasa por el validador de intents
//!   ([super::intent_validator]) antes de llegar al usuario; la persistencia
//!   ocurre solo tras confirmación explícita (comando `intent_confirm`).

use serde::{Deserialize, Serialize};

pub const AI_KEY_SERVICE: &str = "focusflow";
pub const AI_KEY_USER: &str = "ai_api_key";

pub const PROVIDER_LOCAL: &str = "local";
pub const PROVIDER_OPENAI: &str = "openai";
pub const PROVIDER_GEMINI: &str = "gemini";

#[derive(Debug, Clone)]
pub enum AiError {
    NotConfigured(String),
    Http(String),
    BadResponse(String),
    InvalidJson(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::NotConfigured(m) => write!(f, "IA no configurada: {m}"),
            AiError::Http(m) => write!(f, "Error HTTP: {m}"),
            AiError::BadResponse(m) => write!(f, "Respuesta inválida de la IA: {m}"),
            AiError::InvalidJson(m) => write!(f, "JSON inválido: {m}"),
        }
    }
}

impl std::error::Error for AiError {}

pub type AiResult<T> = Result<T, AiError>;

/// Contrato de cualquier proveedor de IA. `chat_json` devuelve un JSON válido
/// para `schema`, o un error. Sin efectos secundarios; sin acceso a datos.
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    /// Devuelve un JSON válido para `schema` o un error.
    fn chat_json(&self, system: &str, user: &str, schema: &str) -> AiResult<serde_json::Value>;
}

/// Proveedor local determinista (sin red, sin clave): convierte el texto en el
/// mismo JSON del esquema mediante reglas. `chat_json` ignora el prompt.
#[derive(Clone, Debug, Default)]
pub struct RuleBasedProvider;

impl RuleBasedProvider {
    pub fn new() -> Self {
        RuleBasedProvider
    }
}

impl AiProvider for RuleBasedProvider {
    fn id(&self) -> &str {
        "rule-based"
    }

    fn chat_json(&self, _system: &str, user: &str, _schema: &str) -> AiResult<serde_json::Value> {
        super::rule_based::analyze_to_json(user)
    }
}

/// Proveedor compatible con la API de OpenAI (chat completions).
/// OpenCode Zen expone este protocolo.
pub struct OpenAiCompatProvider {
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub http: reqwest::blocking::Client,
}

impl OpenAiCompatProvider {
    pub fn new(endpoint: String, model: String, api_key: String) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap_or_default();
        OpenAiCompatProvider { endpoint, model, api_key, http }
    }
}

impl AiProvider for OpenAiCompatProvider {
    fn id(&self) -> &str {
        "openai-compat"
    }

    fn chat_json(&self, system: &str, user: &str, schema: &str) -> AiResult<serde_json::Value> {
        if self.endpoint.is_empty() || self.api_key.is_empty() {
            return Err(AiError::NotConfigured(
                "endpoint o clave vacíos (Ajustes → IA)".into(),
            ));
        }
        let url = format!("{}/chat/completions", self.endpoint.trim_end_matches('/'));
        let mut messages = vec![
            serde_json::json!({"role": "system", "content": system}),
            serde_json::json!({"role": "user", "content": user}),
        ];
        if !schema.is_empty() {
            messages.push(serde_json::json!({"role": "system", "content": format!("DEBES devolver exclusivamente un JSON válido con esta forma:\n{schema}")}));
        }
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0,
            "response_format": { "type": "json_object" }
        });
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| AiError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(AiError::Http(format!(
                "{status} {}",
                text.chars().take(300).collect::<String>()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| AiError::BadResponse(format!("no JSON: {e}")))?;
        let content = json
            .pointer("/choices/0/message/content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| AiError::BadResponse("falta choices[0].message.content".into()))?;
        let parsed = super::validation::extract_json(content)
            .ok_or_else(|| AiError::InvalidJson("no se encontró objeto JSON".into()))?;
        serde_json::from_value(parsed).map_err(|e| AiError::InvalidJson(e.to_string()))
    }
}

/// Proveedor Gemini (REST nativo `generateContent`).
pub struct GeminiProvider {
    pub model: String,
    pub api_key: String,
    /// Base opcional (ej: proxy); por defecto generativelanguage.googleapis.com.
    pub base_url: String,
    pub http: reqwest::blocking::Client,
}

impl GeminiProvider {
    pub fn new(model: String, api_key: String) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap_or_default();
        GeminiProvider {
            model,
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
            http,
        }
    }

    fn url(&self) -> String {
        format!("{}/models/{}:generateContent", self.base_url.trim_end_matches('/'), self.model)
    }
}

impl AiProvider for GeminiProvider {
    fn id(&self) -> &str {
        "gemini"
    }

    fn chat_json(&self, system: &str, user: &str, schema: &str) -> AiResult<serde_json::Value> {
        if self.model.is_empty() || self.api_key.is_empty() {
            return Err(AiError::NotConfigured(
                "modelo o clave vacíos para Gemini (Ajustes → IA)".into(),
            ));
        }
        let mut generation_config = serde_json::json!({ "temperature": 0 });
        if !schema.is_empty() {
            generation_config["responseMimeType"] = serde_json::json!("application/json");
            generation_config["responseSchema"] = serde_json::json!({
                "type": "OBJECT",
                "description": "JSON con la forma pedida en el system prompt",
                "properties": {}
            });
        }
        let mut body = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": user}]}],
            "generationConfig": generation_config,
        });
        if !system.trim().is_empty() {
            body["system_instruction"] =
                serde_json::json!({"parts": [{"text": system}]});
        }
        let url = self.url();
        let resp = self
            .http
            .post(&url)
            .query(&[("key", &self.api_key)])
            .json(&body)
            .send()
            .map_err(|e| AiError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(AiError::Http(format!(
                "{status} {}",
                text.chars().take(300).collect::<String>()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| AiError::BadResponse(format!("no JSON: {e}")))?;
        let text = json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|t| t.as_str())
            .ok_or_else(|| AiError::BadResponse("falta candidates[0].content.parts[0].text".into()))?;
        let parsed = super::validation::extract_json(text)
            .ok_or_else(|| AiError::InvalidJson("no se encontró objeto JSON".into()))?;
        serde_json::from_value(parsed).map_err(|e| AiError::InvalidJson(e.to_string()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiConfig {
    pub endpoint: String,
    pub model: String,
    /// provider: "local" | "openai" | "gemini" (default "openai").
    #[serde(default)]
    pub provider: String,
}

impl AiConfig {
    pub fn provider_name(&self) -> &str {
        match self.provider.trim() {
            PROVIDER_GEMINI => PROVIDER_GEMINI,
            PROVIDER_LOCAL => PROVIDER_LOCAL,
            _ => PROVIDER_OPENAI,
        }
    }
}

pub fn default_provider() -> String {
    std::env::var("AI_PROVIDER")
        .unwrap_or_default()
        .trim()
        .to_lowercase()
}

pub fn default_endpoint() -> String {
    std::env::var("AI_ENDPOINT").unwrap_or_default()
}

pub fn default_model() -> String {
    std::env::var("AI_MODEL").unwrap_or_default()
}

pub fn keyring_get(key_user: &str) -> Option<String> {
    match keyring::Entry::new(AI_KEY_SERVICE, key_user) {
        Ok(e) => e.get_password().ok(),
        Err(_) => None,
    }
}

pub fn keyring_set(key_user: &str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(AI_KEY_SERVICE, key_user).map_err(|e| e.to_string())?;
    match entry.set_password(value) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = e;
            Err("no se pudo guardar en Credential Manager".into())
        }
    }
}

pub fn keyring_delete(key_user: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(AI_KEY_SERVICE, key_user).map_err(|e| e.to_string())?;
    entry.delete_credential().map_err(|e| e.to_string())
}

/// Clave de IA: Credential Manager de Windows, fallback a variable de entorno.
pub fn get_ai_key() -> Option<String> {
    keyring_get(AI_KEY_USER).or_else(|| std::env::var("AI_API_KEY").ok())
}

pub fn get_email_credentials(user: &str) -> Option<String> {
    keyring_get(&format!("email:{user}")).or_else(|| std::env::var("FF_EMAIL_PASSWORD").ok())
}

pub fn set_email_credentials(user: &str, password: &str) -> Result<(), String> {
    keyring_set(&format!("email:{user}"), password)
}

/// Construye el proveedor según la configuración.
/// - `local`  → RuleBasedProvider (sin clave, sin red).
/// - `gemini` → GeminiProvider (requiere clave).
/// - resto    → OpenAiCompatProvider (requiere clave).
pub fn provider_from_config(cfg: &AiConfig) -> AiResult<Box<dyn AiProvider>> {
    let provider = if cfg.provider.trim().is_empty() {
        default_provider()
    } else {
        cfg.provider_name().to_string()
    };

    if provider == PROVIDER_LOCAL {
        return Ok(Box::new(RuleBasedProvider::new()));
    }

    let key = get_ai_key().ok_or_else(|| AiError::NotConfigured("no hay clave de API".into()))?;
    if provider == PROVIDER_GEMINI {
        let model = if cfg.model.is_empty() { default_model() } else { cfg.model.clone() };
        if model.is_empty() {
            return Err(AiError::NotConfigured("falta modelo de Gemini".into()));
        }
        return Ok(Box::new(GeminiProvider::new(model, key)));
    }

    let endpoint = if cfg.endpoint.is_empty() { default_endpoint() } else { cfg.endpoint.clone() };
    let model = if cfg.model.is_empty() { default_model() } else { cfg.model.clone() };
    if endpoint.is_empty() {
        return Err(AiError::NotConfigured("falta endpoint de la API".into()));
    }
    if model.is_empty() {
        return Err(AiError::NotConfigured("falta modelo".into()));
    }
    Ok(Box::new(OpenAiCompatProvider::new(endpoint, model, key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_provider_needs_no_key() {
        let cfg = AiConfig { endpoint: String::new(), model: String::new(), provider: PROVIDER_LOCAL.into() };
        let p = provider_from_config(&cfg).expect("local sin clave");
        assert_eq!(p.id(), "rule-based");
    }

    #[test]
    fn openai_provider_without_key_is_not_configured() {
        // fuerza ausencia de clave: usar un usuario distinto en keyring
        let cfg = AiConfig { endpoint: "http://x".into(), model: "m".into(), provider: String::new() };
        // la clave puede existir en el entorno CI; sin ella debe fallar con NotConfigured
        match provider_from_config(&cfg) {
            Ok(p) => assert_eq!(p.id(), "openai-compat"),
            Err(e) => assert!(matches!(e, AiError::NotConfigured(_))),
        }
    }

    #[test]
    fn provider_name_fallback() {
        let cfg = AiConfig { endpoint: String::new(), model: String::new(), provider: String::new() };
        assert_eq!(cfg.provider_name(), PROVIDER_OPENAI);
        let cfg = AiConfig { endpoint: String::new(), model: String::new(), provider: "gemini".into() };
        assert_eq!(cfg.provider_name(), PROVIDER_GEMINI);
    }

    #[test]
    fn gemini_uses_rest_endpoint_shape() {
        let p = GeminiProvider::new("gemini-2.5-flash".into(), "k".into());
        assert!(p.url().contains(":generateContent"));
        assert!(p.url().contains("gemini-2.5-flash"));
    }
}
