//! Provedor de mentira, para testes e para a demonstração `dotcrow demo troca`.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use crate::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError, Role};

/// Provedor simulado que ecoa o pedido, segue um roteiro de respostas ou falha
/// sempre com o mesmo erro.
pub struct MockProvider {
    id: String,
    fail_with: Option<ProviderError>,
    script: Mutex<Vec<String>>,
    calls: AtomicUsize,
    requests: Mutex<Vec<CompletionRequest>>,
}

impl MockProvider {
    /// Provedor que sempre responde ecoando a última mensagem do usuário.
    pub fn ok(id: &str) -> Arc<Self> {
        Self::build(id, None, Vec::new())
    }

    /// Provedor que sempre falha com `error`.
    pub fn failing(id: &str, error: ProviderError) -> Arc<Self> {
        Self::build(id, Some(error), Vec::new())
    }

    /// Provedor que devolve as respostas de `script`, uma por chamada. Depois que o
    /// roteiro acaba, volta a ecoar o pedido.
    pub fn scripted(id: &str, script: &[&str]) -> Arc<Self> {
        Self::build(id, None, script.iter().rev().map(|s| s.to_string()).collect())
    }

    fn build(id: &str, fail_with: Option<ProviderError>, script: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            id: id.into(),
            fail_with,
            script: Mutex::new(script),
            calls: AtomicUsize::new(0),
            requests: Mutex::new(Vec::new()),
        })
    }

    /// Todos os pedidos recebidos, em ordem.
    pub fn requests(&self) -> Vec<CompletionRequest> {
        self.requests.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Quantas vezes foi chamado.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap_or_else(|e| e.into_inner()).push(request.clone());
        if let Some(e) = &self.fail_with {
            return Err(e.clone());
        }
        if let Some(text) = self.script.lock().unwrap_or_else(|e| e.into_inner()).pop() {
            return Ok(CompletionResponse { text, model: format!("{}-mock", self.id) });
        }
        let last =
            request.messages.iter().rev().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap_or("");
        Ok(CompletionResponse { text: format!("[{}] recebido: {last}", self.id), model: format!("{}-mock", self.id) })
    }
}
