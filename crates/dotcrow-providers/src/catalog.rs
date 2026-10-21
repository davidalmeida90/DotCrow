//! Catálogo dos provedores que o DotCrow sabe usar e a fábrica que monta cada um.

use std::sync::Arc;

use serde::Serialize;

use crate::{
    KeySource, LlmProvider, ProviderError, anthropic,
    gemini::{self, Gemini},
    openai::{self, Flavor, OpenAiCompatible},
};

/// Informações públicas de um provedor, para a CLI e o painel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderInfo {
    /// Id estável, igual ao do cofre.
    pub id: &'static str,
    /// Nome para mostrar.
    pub name: &'static str,
    /// Modelo padrão.
    pub default_model: &'static str,
    /// Endereço padrão da API.
    pub default_base_url: &'static str,
    /// Se precisa de chave de API.
    pub needs_key: bool,
    /// Onde o dono consegue a chave (ou instala o programa, no caso do Ollama).
    pub key_url: &'static str,
}

/// Provedores conhecidos, na ordem padrão de preferência.
pub const PROVIDERS: [ProviderInfo; 7] = [
    ProviderInfo {
        id: anthropic::ID,
        name: "Anthropic (Claude)",
        default_model: anthropic::DEFAULT_MODEL,
        default_base_url: anthropic::DEFAULT_BASE_URL,
        needs_key: true,
        key_url: "https://console.anthropic.com/settings/keys",
    },
    from_flavor(openai::OPENAI, "OpenAI (GPT)", "https://platform.openai.com/api-keys"),
    ProviderInfo {
        id: gemini::ID,
        name: "Google (Gemini)",
        default_model: gemini::DEFAULT_MODEL,
        default_base_url: gemini::DEFAULT_BASE_URL,
        needs_key: true,
        key_url: "https://aistudio.google.com/apikey",
    },
    from_flavor(openai::OPENROUTER, "OpenRouter", "https://openrouter.ai/keys"),
    from_flavor(openai::DEEPSEEK, "DeepSeek", "https://platform.deepseek.com/api_keys"),
    from_flavor(openai::GROQ, "Groq", "https://console.groq.com/keys"),
    from_flavor(openai::OLLAMA, "Ollama (no seu computador)", "https://ollama.com/download"),
];

const fn from_flavor(f: Flavor, name: &'static str, key_url: &'static str) -> ProviderInfo {
    ProviderInfo {
        id: f.id,
        name,
        default_model: f.default_model,
        default_base_url: f.base_url,
        needs_key: f.needs_key,
        key_url,
    }
}

/// Informações de um provedor pelo id.
pub fn info(id: &str) -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// Ajustes do dono para um provedor. Vazio usa os padrões.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderSettings {
    /// Modelo a usar.
    pub model: Option<String>,
    /// Endereço da API (útil para Ollama em outra máquina).
    pub base_url: Option<String>,
}

/// Monta o provedor `id` com as chaves de `keys`.
pub fn build(
    id: &str,
    keys: Arc<dyn KeySource>,
    settings: &ProviderSettings,
) -> Result<Arc<dyn LlmProvider>, ProviderError> {
    let model = settings.model.clone();
    let url = settings.base_url.clone();
    let flavor = |f: Flavor| -> Result<Arc<dyn LlmProvider>, ProviderError> {
        Ok(Arc::new(OpenAiCompatible::new(f, keys.clone(), model.clone(), url.clone())?))
    };
    match id {
        anthropic::ID => Ok(Arc::new(anthropic::Anthropic::new(keys.clone(), model.clone(), url.clone())?)),
        gemini::ID => Ok(Arc::new(Gemini::new(keys.clone(), model.clone(), url.clone())?)),
        "openai" => flavor(openai::OPENAI),
        "openrouter" => flavor(openai::OPENROUTER),
        "deepseek" => flavor(openai::DEEPSEEK),
        "groq" => flavor(openai::GROQ),
        "ollama" => flavor(openai::OLLAMA),
        other => Err(ProviderError::Misconfigured(format!("provedor desconhecido: {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoKeys;

    #[test]
    fn every_listed_provider_can_be_built() {
        for p in &PROVIDERS {
            let built = build(p.id, Arc::new(NoKeys), &ProviderSettings::default()).unwrap();
            assert_eq!(built.id(), p.id);
            assert!(p.key_url.starts_with("https://"));
        }
        assert!(build("nao-existe", Arc::new(NoKeys), &ProviderSettings::default()).is_err());
    }

    #[test]
    fn ids_are_unique_and_only_ollama_runs_without_key() {
        let mut ids: Vec<_> = PROVIDERS.iter().map(|p| p.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), PROVIDERS.len());
        assert!(PROVIDERS.iter().all(|p| p.needs_key == (p.id != "ollama")));
    }

    #[tokio::test]
    async fn missing_key_means_not_configured_without_network() {
        let p = build("anthropic", Arc::new(NoKeys), &ProviderSettings::default()).unwrap();
        let req = crate::CompletionRequest { messages: vec![], max_tokens: 1 };
        assert_eq!(p.complete(&req).await.unwrap_err(), ProviderError::NotConfigured);
    }
}
