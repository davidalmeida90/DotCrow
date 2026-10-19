title: Adaptador Ollama (modelos locais)
labels: provedor,boa-primeira-issue
milestone: Fase 1 · Ninho
---
## Contexto
O Ollama roda LLMs no próprio computador. É o provedor mais simples de implementar (não precisa de chave de API) e o mais importante para privacidade: com ele, nada sai da máquina.

## O que fazer
- Criar `crates/dotcrow-providers/src/ollama.rs` implementando `LlmProvider` com id `ollama`.
- Usar o endpoint de chat do Ollama (padrão `http://localhost:11434`), com URL base e modelo configuráveis.
- Converter `CompletionRequest` para o formato do Ollama e a resposta para `CompletionResponse`.
- Erro de conexão (Ollama desligado) vira `ProviderError::Unavailable`.

## Critérios de aceite
- [ ] Testes com respostas gravadas em `tests/fixtures/ollama/`, sem precisar do Ollama no CI.
- [ ] Um teste `#[ignore]` que fala com um Ollama local, com instruções de como rodar.
- [ ] Linha atualizada na tabela de `docs/provedores.md` e no CHANGELOG.

## Dicas
Siga o passo a passo em [docs/provedores.md](https://github.com/napoleaomp4/dotcrow/blob/main/docs/provedores.md#adicionando-um-provedor). Para HTTP, sugerimos `reqwest` com `rustls`.
