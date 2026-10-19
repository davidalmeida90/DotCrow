//! Provedor de mentira, para testes e para a demonstração `dotcrow demo troca`.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::{CompletionRequest, CompletionResponse, LlmProvider, ProviderError, Role};

/// Provedor simulado que responde sempre igual ou falha sempre com o mesmo erro.
pub struct MockProvider {
    id: String,
    fail_with: Option<ProviderError>,
    calls: AtomicUsize,
}

impl MockProvider {
    /// Provedor que sempre responde ecoando a última mensagem do usuário.
    pub fn ok(id: &str) -> Arc<Self> {
        Arc::new(Self { id: id.into(), fail_with: None, calls: AtomicUsize::new(0) })
    }

    /// Provedor que sempre falha com `error`.
    pub fn failing(id: &str, error: ProviderError) -> Arc<Self> {
        Arc::new(Self { id: id.into(), fail_with: Some(error), calls: AtomicUsize::new(0) })
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
        if let Some(e) = &self.fail_with {
            return Err(e.clone());
        }
        let last =
            request.messages.iter().rev().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap_or("");
        Ok(CompletionResponse { text: format!("[{}] recebido: {last}", self.id), model: format!("{}-mock", self.id) })
    }
}
