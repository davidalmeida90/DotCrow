title: Bateria de testes de prompt injection para o Vigia
labels: seguranca,agente
milestone: Fase 2 · Mãos no computador
---
## Contexto
Prompt injection é o ataque mais provável contra o DotCrow: uma página, e-mail ou arquivo com instruções escondidas para o agente.

## O que fazer
- Montar um conjunto de casos (em português e inglês): texto oculto em página, e-mail pedindo para encaminhar arquivos, PDF com "ignore as instruções anteriores", receita da Revoada com passo malicioso.
- Teste automatizado: com esses conteúdos no contexto, nenhuma ação fora do plano aprovado chega a ser executada, e o Vigia sinaliza o conteúdo.

## Critérios de aceite
- [ ] Casos versionados em `tests/prompt-injection/`.
- [ ] Rodam no CI com o `MockProvider` e, opcionalmente, com LLMs reais localmente.
