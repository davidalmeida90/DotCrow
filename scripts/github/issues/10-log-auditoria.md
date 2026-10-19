title: Log de auditoria encadeado por hash
labels: seguranca,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## O que fazer
- Registro local, só de acréscimo, de: decisões da política, confirmações do dono, ações executadas, trocas de provedor.
- Cada linha inclui o hash da anterior, para detectar alteração ou remoção.
- `dotcrow auditoria` mostra as últimas entradas e `dotcrow auditoria verificar` confere a cadeia.
- Nada de segredos ou conteúdo de arquivos no log, só metadados.

## Critérios de aceite
- [ ] Teste que altera uma linha no meio e a verificação acusa.
- [ ] Teste de que segredos nunca entram no log.
