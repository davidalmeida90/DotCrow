title: Adaptador Anthropic (Claude)
labels: provedor,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## Contexto
O Claude é um dos provedores principais do DotCrow. O adaptador precisa classificar bem os erros para a troca automática funcionar: créditos esgotados, limite de uso e indisponibilidade têm tratamentos diferentes.

## O que fazer
- `crates/dotcrow-providers/src/anthropic.rs` implementando `LlmProvider` com id `anthropic`, usando a Messages API.
- Mensagens de papel `System` vão no campo de sistema da requisição, não na lista de mensagens.
- A chave vem do `Lease` do cofre, só na hora de montar o cabeçalho.
- Classificação de erros: 401/403 → `Unauthorized`; 429 → `RateLimited` lendo `retry-after`; 529 e 5xx → `Unavailable`; aviso de saldo insuficiente → `QuotaExhausted`, mesmo que venha com status 400.

## Critérios de aceite
- [ ] Testes com respostas gravadas cobrindo sucesso e cada tipo de erro.
- [ ] A chave não aparece em `Debug`, erros ou logs (com teste).
- [ ] Documentação e CHANGELOG atualizados.
