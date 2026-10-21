//! Adaptador da API da Anthropic (Claude), endpoint `POST /v1/messages`.

use std::sync::Arc;

use reqwest::header::{HeaderName, HeaderValue};
use serde_json::{Value, json};

use crate::http::{self, HttpFailure};
use crate::{CompletionRequest, CompletionResponse, KeySource, LlmProvider, ProviderError, Role};

/// Id usado no cofre.
pub const ID: &str = "anthropic";
/// Modelo usado quando o dono não escolhe outro.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
/// Endereço padrão da API.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// Provedor Claude.
pub struct Anthropic {
    base_url: String,
    model: String,
    keys: Arc<dyn KeySource>,
    client: reqwest::Client,
}

impl Anthropic {
    /// Cria o provedor. `model` e `base_url` vazios usam os padrões.
    pub fn new(
        keys: Arc<dyn KeySource>,
        model: Option<String>,
        base_url: Option<String>,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            base_url: http::check_base_url(base_url.as_deref().unwrap_or(DEFAULT_BASE_URL))?,
            model: model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| DEFAULT_MODEL.into()),
            keys,
            client: http::client()?,
        })
    }
}

/// Corpo da requisição no formato da API de mensagens.
fn body(model: &str, req: &CompletionRequest) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != Role::System)
        .map(|m| {
            let role = if m.role == Role::Assistant { "assistant" } else { "user" };
            json!({ "role": role, "content": m.content })
        })
        .collect();
    let mut b = json!({ "model": model, "max_tokens": req.max_tokens, "messages": messages });
    if let Some(system) = http::system_text(&req.messages) {
        b["system"] = Value::String(system);
    }
    b
}

fn parse(v: &Value, fallback_model: &str) -> Result<CompletionResponse, ProviderError> {
    let text: String = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();
    if text.trim().is_empty() {
        let reason = v.get("stop_reason").and_then(Value::as_str).unwrap_or("desconhecido");
        if reason == "refusal" {
            return Err(ProviderError::BadRequest("o modelo recusou este pedido".into()));
        }
        return Err(ProviderError::Unavailable(format!("resposta sem texto (stop_reason: {reason})")));
    }
    let model = v.get("model").and_then(Value::as_str).unwrap_or(fallback_model).to_string();
    Ok(CompletionResponse { text, model })
}

/// Classifica erros da Anthropic. O tipo vem em `error.type`.
fn classify(status: u16, retry_after: Option<std::time::Duration>, body: &str, model: &str) -> ProviderError {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let kind = parsed.as_ref().and_then(|v| v.pointer("/error/type")).and_then(Value::as_str).unwrap_or("");
    let detail = http::error_detail(body);
    let lower = detail.to_lowercase();
    match kind {
        "authentication_error" | "permission_error" => ProviderError::Unauthorized,
        "rate_limit_error" => ProviderError::RateLimited { retry_after },
        "overloaded_error" | "api_error" | "timeout_error" => ProviderError::Unavailable(detail),
        "billing_error" => ProviderError::QuotaExhausted,
        // A Anthropic avisa "credit balance is too low" com status 400.
        _ if lower.contains("credit balance") || lower.contains("billing") => ProviderError::QuotaExhausted,
        "not_found_error" => ProviderError::Misconfigured(format!("modelo \"{model}\" não encontrado: {detail}")),
        _ => ProviderError::from_http_status(status, retry_after, &detail),
    }
}

#[async_trait::async_trait]
impl LlmProvider for Anthropic {
    fn id(&self) -> &str {
        ID
    }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        let key = http::secret_header(self.keys.as_ref(), ID, |k| k.to_string())?;
        let headers = vec![
            (HeaderName::from_static("x-api-key"), key),
            (HeaderName::from_static("anthropic-version"), HeaderValue::from_static(API_VERSION)),
        ];
        let url = format!("{}/v1/messages", self.base_url);
        match http::post_json(&self.client, &url, headers, &body(&self.model, req)).await {
            Ok(v) => parse(&v, &self.model),
            Err(HttpFailure::Network(e)) => Err(e),
            Err(HttpFailure::Status { status, retry_after, body }) => {
                Err(classify(status, retry_after, &body, &self.model))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    fn req() -> CompletionRequest {
        CompletionRequest {
            messages: vec![
                Message { role: Role::System, content: "Você é Huginn.".into() },
                Message { role: Role::User, content: "organize".into() },
            ],
            max_tokens: 100,
        }
    }

    #[test]
    fn system_goes_to_its_own_field() {
        let b = body("claude-x", &req());
        assert_eq!(b["system"], "Você é Huginn.");
        assert_eq!(b["messages"].as_array().unwrap().len(), 1);
        assert_eq!(b["messages"][0]["role"], "user");
        assert_eq!(b["max_tokens"], 100);
    }

    #[test]
    fn parses_text_blocks() {
        let v =
            json!({"model":"claude-x-1","content":[{"type":"thinking","thinking":"..."},{"type":"text","text":"Olá"}]});
        let r = parse(&v, "m").unwrap();
        assert_eq!(r.text, "Olá");
        assert_eq!(r.model, "claude-x-1");
        assert!(parse(&json!({"content":[]}), "m").is_err());
    }

    #[test]
    fn classifies_errors() {
        let e = |t: &str, m: &str| format!(r#"{{"type":"error","error":{{"type":"{t}","message":"{m}"}}}}"#);
        assert_eq!(
            classify(401, None, &e("authentication_error", "invalid x-api-key"), "m"),
            ProviderError::Unauthorized
        );
        assert_eq!(
            classify(400, None, &e("invalid_request_error", "Your credit balance is too low"), "m"),
            ProviderError::QuotaExhausted
        );
        assert!(matches!(classify(429, None, &e("rate_limit_error", "x"), "m"), ProviderError::RateLimited { .. }));
        assert!(matches!(
            classify(529, None, &e("overloaded_error", "Overloaded"), "m"),
            ProviderError::Unavailable(_)
        ));
        assert!(matches!(classify(404, None, &e("not_found_error", "model: x"), "m"), ProviderError::Misconfigured(_)));
        assert!(matches!(
            classify(400, None, &e("invalid_request_error", "prompt is too long"), "m"),
            ProviderError::BadRequest(_)
        ));
    }
}
