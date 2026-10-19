title: Adaptador OpenAI e APIs compatíveis
labels: provedor,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## Contexto
Muitos provedores (Groq, DeepSeek, Together, Maritaca, servidores locais como LM Studio) aceitam o formato de chat da OpenAI. Um adaptador bem feito cobre todos eles mudando só a URL base.

## O que fazer
- `crates/dotcrow-providers/src/openai.rs` com id configurável (padrão `openai`) e URL base configurável.
- Chat Completions, autenticação `Bearer` via `Lease`.
- Classificar `insufficient_quota` como `QuotaExhausted` e ler `retry-after` em 429.

## Critérios de aceite
- [ ] Testes com respostas gravadas.
- [ ] Exemplo na documentação de como usar com outro provedor compatível.
