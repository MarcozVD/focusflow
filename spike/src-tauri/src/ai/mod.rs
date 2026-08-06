use serde::{Deserialize, Serialize};

pub mod email_parser;
pub mod nl;
pub mod task_parser;
pub mod validation;

pub const AI_KEY_SERVICE: &str = "focusflow";
pub const AI_KEY_USER: &str = "ai_api_key";

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

/// Capa de abstracción de proveedores de IA. Cualquier proveedor
/// (OpenCode Zen, OpenAI, Anthropic, Gemini…) implementa este trait.
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    /// Devuelve un JSON válido para `schema` o un error.
    fn chat_json(&self, system: &str, user: &str, schema: &str) -> AiResult<serde_json::Value>;
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
        ];        if !schema.is_empty() {
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
        let parsed = validation::extract_json(content)
            .ok_or_else(|| AiError::InvalidJson("no se encontró objeto JSON".into()))?;
        serde_json::from_value(parsed).map_err(|e| AiError::InvalidJson(e.to_string()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiConfig {
    pub endpoint: String,
    pub model: String,
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

/// Construye el proveedor según la configuración. Devuelve Err si no está listo.
pub fn provider_from_config(cfg: &AiConfig) -> AiResult<Box<dyn AiProvider>> {
    let key = get_ai_key().ok_or_else(|| AiError::NotConfigured("no hay clave de API".into()))?;
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
