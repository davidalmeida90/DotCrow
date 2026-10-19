title: Adaptador OpenRouter
labels: provedor,boa-primeira-issue,bloqueada
milestone: Fase 1 · Ninho
---
## Contexto
O OpenRouter dá acesso a dezenas de modelos com uma chave só. A API é compatível com a da OpenAI.

## O que fazer
Depois do adaptador OpenAI (#3), registrar o `openrouter` reaproveitando aquele código, com a URL base do OpenRouter e os cabeçalhos opcionais de identificação do app (`DotCrow`, `https://dotcrow.si`).

## Critérios de aceite
- [ ] Teste com resposta gravada.
- [ ] Linha na tabela de `docs/provedores.md`.
