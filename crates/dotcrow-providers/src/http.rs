//! Peças comuns aos adaptadores HTTP: cliente, cabeçalho com segredo, envio e
//! limpeza das mensagens de erro.

use std::time::Duration;

use reqwest::header::{HeaderName, HeaderValue};
use serde_json::Value;
use zeroize::Zeroizing;

use crate::{KeySource, ProviderError};

/// Tempo máximo para abrir a conexão.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Tempo máximo de uma resposta inteira. Modelos que "pensam" podem demorar.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);
/// Tamanho máximo do detalhe de erro mostrado ao dono.
const MAX_DETAIL: usize = 300;

/// Cria o cliente HTTP com TLS do `rustls` (sem OpenSSL) e os certificados do sistema
/// (no Android, os da Mozilla embutidos; veja [`crate::tls`]).
pub(crate) fn client() -> Result<reqwest::Client, ProviderError> {
    // Instala o provedor de criptografia `ring` uma vez por processo. Se já houver
    // um instalado, a chamada só devolve erro e seguimos com ele.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let cliente = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("dotcrow/", env!("CARGO_PKG_VERSION")));
    crate::tls::preparar(cliente)
        .build()
        .map_err(|e| ProviderError::Misconfigured(format!("não consegui preparar o cliente HTTP: {e}")))
}

/// Monta um cabeçalho com a chave do `provider`, pedida ao cofre só agora.
///
/// O valor é marcado como sensível, para não aparecer em `Debug`.
pub(crate) fn secret_header(
    keys: &dyn KeySource,
    provider: &str,
    make: impl Fn(&str) -> String,
) -> Result<HeaderValue, ProviderError> {
    let mut out = None;
    let found = keys.with_key(provider, &mut |key| {
        let text = Zeroizing::new(make(key));
        out = Some(HeaderValue::from_str(&text).map(|mut v| {
            v.set_sensitive(true);
            v
        }));
    });
    match out {
        Some(Ok(v)) if found => Ok(v),
        // Chave com caractere que não cabe num cabeçalho: está errada.
        Some(Err(_)) => Err(ProviderError::Unauthorized),
        _ => Err(ProviderError::NotConfigured),
    }
}

/// Falha HTTP ainda não classificada; cada adaptador decide o que ela significa.
#[derive(Debug)]
pub(crate) enum HttpFailure {
    /// A requisição nem chegou a ter resposta.
    Network(ProviderError),
    /// Resposta com status de erro.
    Status {
        /// Código HTTP.
        status: u16,
        /// Cabeçalho `retry-after`, quando veio em segundos.
        retry_after: Option<Duration>,
        /// Corpo da resposta (texto).
        body: String,
    },
}

/// Envia `body` como JSON e devolve o JSON da resposta.
pub(crate) async fn post_json(
    client: &reqwest::Client,
    url: &str,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: &Value,
) -> Result<Value, HttpFailure> {
    let mut req = client.post(url).json(body);
    for (name, value) in headers {
        req = req.header(name, value);
    }
    let resp = req.send().await.map_err(|e| HttpFailure::Network(network_error(&e)))?;
    let status = resp.status().as_u16();
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs);
    let text = resp.text().await.map_err(|e| HttpFailure::Network(network_error(&e)))?;
    if !(200..300).contains(&status) {
        return Err(HttpFailure::Status { status, retry_after, body: text });
    }
    serde_json::from_str(&text)
        .map_err(|_| HttpFailure::Network(ProviderError::Unavailable("resposta que não é JSON".into())))
}

/// Envia `body` e lê a resposta como *Server-Sent Events*, chamando `on_event` com o
/// conteúdo de cada linha `data:` assim que ela chega. Serve para o streaming ao vivo.
pub(crate) async fn post_sse(
    client: &reqwest::Client,
    url: &str,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: &Value,
    on_event: &mut (dyn FnMut(&str) + Send),
) -> Result<(), HttpFailure> {
    let mut req = client.post(url).json(body);
    for (name, value) in headers {
        req = req.header(name, value);
    }
    let mut resp = req.send().await.map_err(|e| HttpFailure::Network(network_error(&e)))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let text = resp.text().await.unwrap_or_default();
        return Err(HttpFailure::Status { status, retry_after, body: text });
    }
    let mut buf = String::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| HttpFailure::Network(network_error(&e)))? {
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(nl) = buf.find('\n') {
            let linha: String = buf.drain(..=nl).collect();
            let linha = linha.trim_end();
            if let Some(dados) = linha.strip_prefix("data:") {
                on_event(dados.trim());
            }
        }
    }
    Ok(())
}

fn network_error(e: &reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Unavailable("tempo de resposta esgotado".into())
    } else if e.is_connect() {
        ProviderError::Unavailable("sem conexão com o provedor".into())
    } else {
        ProviderError::Unavailable("falha de rede".into())
    }
}

/// Mensagem de erro de um corpo JSON no formato `{"error": {"message": ...}}` ou
/// `{"error": "..."}`, ou o próprio texto. Curta e sem nada que pareça uma chave.
pub(crate) fn error_detail(body: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let msg = parsed
        .as_ref()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("error").filter(|e| e.is_string()))
                .or_else(|| v.get("message"))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string());
    let clean = redact_tokens(msg.trim());
    if clean.chars().count() > MAX_DETAIL {
        let cut: String = clean.chars().take(MAX_DETAIL).collect();
        format!("{cut}…")
    } else {
        clean
    }
}

/// Mascara sequências longas que parecem chaves (24+ letras, dígitos, `-` ou `_`).
pub(crate) fn redact_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.chars().count() >= 24 {
            out.push_str("***");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Junta as mensagens de sistema em um texto só (Anthropic e Gemini recebem à parte).
pub(crate) fn system_text(messages: &[crate::Message]) -> Option<String> {
    let parts: Vec<&str> =
        messages.iter().filter(|m| m.role == crate::Role::System).map(|m| m.content.as_str()).collect();
    if parts.is_empty() { None } else { Some(parts.join("\n\n")) }
}

/// Recusa bases de URL que não sejam http(s), para um erro de digitação não virar
/// requisição para lugar estranho.
pub(crate) fn check_base_url(url: &str) -> Result<String, ProviderError> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Ok(trimmed.to_string())
    } else {
        Err(ProviderError::Misconfigured(format!("endereço inválido: {trimmed} (use http:// ou https://)")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OneKey;
    impl KeySource for OneKey {
        fn with_key(&self, provider: &str, f: &mut dyn FnMut(&str)) -> bool {
            if provider == "x" {
                f("sk-test-1234567890");
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn secret_header_is_sensitive_and_missing_key_is_not_configured() {
        let h = secret_header(&OneKey, "x", |k| format!("Bearer {k}")).unwrap();
        assert!(h.is_sensitive());
        assert!(!format!("{h:?}").contains("sk-test"));
        assert_eq!(secret_header(&OneKey, "y", |k| k.to_string()).unwrap_err(), ProviderError::NotConfigured);
    }

    #[test]
    fn details_are_short_and_redacted() {
        let body = r#"{"error":{"message":"Incorrect API key provided: sk-proj-abcdefghijklmnopqrstuvwxyz123"}}"#;
        let d = error_detail(body);
        assert_eq!(d, "Incorrect API key provided: ***");
        assert!(error_detail(&"a ".repeat(400)).ends_with('…'));
        assert_eq!(error_detail("texto puro"), "texto puro");
    }

    #[test]
    fn base_url_must_be_http() {
        assert_eq!(check_base_url("http://localhost:11434/v1/").unwrap(), "http://localhost:11434/v1");
        assert!(check_base_url("localhost:11434").is_err());
    }
}
