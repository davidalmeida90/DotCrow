title: Adaptador Google Gemini
labels: provedor,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## O que fazer
- `crates/dotcrow-providers/src/gemini.rs` com id `gemini`.
- Converter o histórico para o formato de conteúdos do Gemini (papéis `user` e `model`, instruções de sistema separadas).
- Erros de cota (`RESOURCE_EXHAUSTED`) viram `QuotaExhausted` ou `RateLimited` conforme o caso.

## Critérios de aceite
- [ ] Testes com respostas gravadas cobrindo sucesso e erros.
- [ ] Documentação e CHANGELOG atualizados.
