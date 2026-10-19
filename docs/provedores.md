# Provedores de LLM e troca automática

O DotCrow funciona com qualquer LLM que tenha API. O dono adiciona as chaves dos provedores que quiser, e o **roteador** decide quem responde a cada pedido.

## Como a troca funciona

O roteador tenta os provedores na ordem de preferência. Quando um falha, ele classifica o erro:

| Erro | Exemplo | Troca de provedor? | Fica "de castigo" por |
|---|---|:-:|---|
| `QuotaExhausted` | créditos acabaram | ✅ | 1 hora |
| `RateLimited` | muitas requisições | ✅ | o tempo que o provedor pedir (ou 30 s) |
| `Unauthorized` | chave revogada ou errada | ✅ | 10 minutos |
| `NotConfigured` | sem chave no cofre | ✅ | 10 minutos |
| `Unavailable` | fora do ar, sobrecarregado, timeout | ✅ | 20 segundos |
| `BadRequest` | pedido inválido | ❌ (o próximo falharia igual) | — |

Provedores de castigo são pulados nos pedidos seguintes, sem gastar tempo. Se **todos** estiverem de castigo, o roteador tenta o que volta mais cedo, para não deixar o dono sem resposta.

Veja funcionando, com provedores simulados:

```bash
dotcrow demo troca
```

```
Pedido 1:
  ✗ anthropic  cota ou créditos esgotados → trocando
  ✗ openai     limite de requisições atingido → trocando
  ✓ gemini     [gemini] recebido: organize minha pasta Downloads

Pedido 2:
  ↷ anthropic  de castigo, pulado
  ↷ openai     de castigo, pulado
  ✓ gemini     [gemini] recebido: organize minha pasta Downloads
```

O código está em [`crates/dotcrow-providers/src/router.rs`](../crates/dotcrow-providers/src/router.rs).

## Adicionando um provedor

Esta é uma das melhores formas de começar a contribuir. Há uma issue para cada provedor que queremos, com o rótulo `provedor`.

### 1. Implemente `LlmProvider`

Crie `crates/dotcrow-providers/src/<nome>.rs`:

```rust
pub struct Ollama { base_url: String, model: String, client: reqwest::Client }

#[async_trait::async_trait]
impl LlmProvider for Ollama {
    fn id(&self) -> &str { "ollama" }

    async fn complete(&self, req: &CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        // 1. converta `req.messages` para o formato da API
        // 2. faça a chamada HTTP
        // 3. converta erros com ProviderError::from_http_status(...)
        // 4. devolva CompletionResponse { text, model }
        todo!()
    }
}
```

O `id` precisa ser igual ao nome usado no cofre (`dotcrow chaves adicionar <id>`).

### 2. Use a chave só na hora

O adaptador recebe a chave pelo `Lease` do cofre, no momento de montar o cabeçalho:

```rust
let header = lease.with_secret(|key| format!("Bearer {key}"));
```

Não guarde a chave em campos da struct, não coloque em mensagens de erro e não registre em log.

### 3. Classifique bem os erros

`ProviderError::from_http_status` cobre o comum (401/403, 402, 429, 5xx). Refine o que for específico do provedor. Exemplos que fazem diferença:

- alguns provedores avisam "sem créditos" com status 400 e uma mensagem no corpo: isso é `QuotaExhausted`, não `BadRequest`;
- leia o cabeçalho `retry-after` quando existir e passe em `RateLimited { retry_after }`;
- erro de conexão e timeout são `Unavailable`.

Uma classificação errada faz o roteador trocar quando não devia, ou insistir quando devia trocar.

### 4. Teste sem chamar a API

No CI não há chaves. Teste a conversão de mensagens e a classificação de erros com respostas gravadas (JSON de exemplo em `tests/fixtures/`). Se quiser um teste contra a API real, marque com `#[ignore]` e documente como rodar localmente.

### 5. Documente

Acrescente o provedor na tabela abaixo e no CHANGELOG.

## Provedores

| Provedor | id | Estado |
|---|---|---|
| Anthropic (Claude) | `anthropic` | procurando quem faça |
| OpenAI (GPT) | `openai` | procurando quem faça |
| Google (Gemini) | `gemini` | procurando quem faça |
| OpenRouter | `openrouter` | procurando quem faça |
| Ollama (modelos locais) | `ollama` | procurando quem faça · boa primeira issue |
| Simulado (testes) | qualquer | ✅ `mock.rs` |
