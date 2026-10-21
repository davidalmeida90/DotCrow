//! Testes dos adaptadores contra um servidor HTTP falso, sem chamar nenhuma API real.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};

use dotcrow_providers::{
    CompletionRequest, KeySource, Message, ProviderError, Role, Router,
    catalog::{self, ProviderSettings},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Chaves de teste, uma por provedor.
struct TestKeys;

impl KeySource for TestKeys {
    fn with_key(&self, provider: &str, f: &mut dyn FnMut(&str)) -> bool {
        f(&format!("chave-secreta-de-{provider}-1234567890"));
        true
    }
}

/// Servidor que responde sempre `status` com `body` e guarda as requisições recebidas.
async fn fake(status: u16, body: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            // Lê cabeçalhos e corpo (Content-Length).
            loop {
                let n = sock.read(&mut chunk).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_string()))
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(0);
                    if buf.len() >= end + 4 + len {
                        break;
                    }
                }
            }
            seen2.lock().unwrap().push(String::from_utf8_lossy(&buf).to_string());
            let reply = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(reply.as_bytes()).await;
        }
    });
    (url, seen)
}

fn req() -> CompletionRequest {
    CompletionRequest {
        messages: vec![
            Message { role: Role::System, content: "Você é Huginn.".into() },
            Message { role: Role::User, content: "planeje".into() },
        ],
        max_tokens: 64,
    }
}

fn settings(url: &str) -> ProviderSettings {
    ProviderSettings { model: None, base_url: Some(url.to_string()) }
}

#[tokio::test]
async fn anthropic_sends_key_header_and_reads_text() {
    let (url, seen) = fake(200, r#"{"model":"claude-teste","content":[{"type":"text","text":"Plano pronto"}]}"#).await;
    let p = catalog::build("anthropic", Arc::new(TestKeys), &settings(&url)).unwrap();
    let out = p.complete(&req()).await.unwrap();
    assert_eq!(out.text, "Plano pronto");
    let raw = seen.lock().unwrap()[0].to_lowercase();
    assert!(raw.starts_with("post /v1/messages"));
    assert!(raw.contains("x-api-key: chave-secreta-de-anthropic"));
    assert!(raw.contains("anthropic-version: 2023-06-01"));
    assert!(raw.contains(r#""system":"você é huginn.""#));
}

#[tokio::test]
async fn openai_compatible_uses_bearer() {
    let (url, seen) = fake(200, r#"{"model":"gpt-teste","choices":[{"message":{"content":"Oi"}}]}"#).await;
    let p = catalog::build("groq", Arc::new(TestKeys), &settings(&url)).unwrap();
    assert_eq!(p.complete(&req()).await.unwrap().text, "Oi");
    let raw = seen.lock().unwrap()[0].to_lowercase();
    assert!(raw.starts_with("post /chat/completions"));
    assert!(raw.contains("authorization: bearer chave-secreta-de-groq"));
}

#[tokio::test]
async fn gemini_keeps_key_out_of_url() {
    let (url, seen) = fake(200, r#"{"candidates":[{"content":{"parts":[{"text":"Olá"}]}}]}"#).await;
    let p = catalog::build("gemini", Arc::new(TestKeys), &settings(&url)).unwrap();
    assert_eq!(p.complete(&req()).await.unwrap().text, "Olá");
    let raw = seen.lock().unwrap()[0].clone();
    let first_line = raw.lines().next().unwrap();
    assert!(first_line.contains(":generateContent"));
    assert!(!first_line.contains("chave-secreta"), "a chave não pode ir na URL");
    assert!(raw.to_lowercase().contains("x-goog-api-key: chave-secreta-de-gemini"));
}

#[tokio::test]
async fn errors_never_carry_the_key() {
    let (url, _) = fake(401, r#"{"error":{"message":"Incorrect API key provided: chave-secreta-de-openai-1234567890","code":"invalid_api_key"}}"#).await;
    let p = catalog::build("openai", Arc::new(TestKeys), &settings(&url)).unwrap();
    let err = p.complete(&req()).await.unwrap_err();
    assert_eq!(err, ProviderError::Unauthorized);
    assert!(!err.to_string().contains("chave-secreta"));
}

#[tokio::test]
async fn router_fails_over_over_real_http() {
    let (dead, _) =
        fake(429, r#"{"error":{"message":"You exceeded your current quota","code":"insufficient_quota"}}"#).await;
    let (alive, _) = fake(200, r#"{"candidates":[{"content":{"parts":[{"text":"Respondi eu"}]}}]}"#).await;
    let keys: Arc<dyn KeySource> = Arc::new(TestKeys);
    let router = Router::new(vec![
        catalog::build("openai", keys.clone(), &settings(&dead)).unwrap(),
        catalog::build("gemini", keys, &settings(&alive)).unwrap(),
    ]);
    let out = router.complete(&req()).await.unwrap();
    assert_eq!(out.provider, "gemini");
    assert_eq!(out.response.text, "Respondi eu");
    assert_eq!(out.attempts[0].error, Some(ProviderError::QuotaExhausted));
}

#[tokio::test]
async fn ollama_works_without_key() {
    let (url, seen) = fake(200, r#"{"choices":[{"message":{"content":"local"}}]}"#).await;
    let p = catalog::build("ollama", Arc::new(dotcrow_providers::NoKeys), &settings(&url)).unwrap();
    assert_eq!(p.complete(&req()).await.unwrap().text, "local");
    assert!(!seen.lock().unwrap()[0].to_lowercase().contains("authorization:"));
}

#[tokio::test]
async fn unreachable_server_is_unavailable() {
    // Porta fechada: ninguém escuta aqui.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let p = catalog::build("ollama", Arc::new(dotcrow_providers::NoKeys), &settings(&url)).unwrap();
    assert!(matches!(p.complete(&req()).await.unwrap_err(), ProviderError::Unavailable(_)));
}
