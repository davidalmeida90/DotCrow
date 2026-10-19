title: Ordem de preferência dos provedores configurável
labels: cli,boa-primeira-issue
milestone: Fase 1 · Ninho
---
## Contexto
O roteador tenta os provedores numa ordem. Hoje ela não é configurável.

## O que fazer
- Arquivo `config.toml` na pasta de dados do DotCrow com `provedores.ordem = ["anthropic", "openai", "ollama"]`.
- Comandos: `dotcrow provedores ordem anthropic openai ollama` e `dotcrow provedores` (mostra a ordem e quais têm chave no cofre, sem mostrar chaves).
- Provedores sem chave no cofre (exceto `ollama`) aparecem como "sem chave".

## Critérios de aceite
- [ ] Testes para leitura e gravação da configuração.
- [ ] Mensagens em português claro.
