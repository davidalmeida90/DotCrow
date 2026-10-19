title: Guardar a chave mestra do cofre no chaveiro do sistema
labels: seguranca,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## Contexto
Hoje a chave mestra do cofre fica em `master.key` (permissão 0600) ao lado do `vault.bin`. Quem copiar os dois arquivos abre o cofre. Veja `docs/cofre-de-chaves.md`.

## O que fazer
- Guardar a chave mestra no Keychain (macOS), Credential Manager (Windows) e Secret Service (Linux), por exemplo com o crate `keyring`.
- Migração automática: se existir `master.key`, mover para o chaveiro e apagar o arquivo com segurança.
- Plano B documentado para servidores Linux sem Secret Service.

## Critérios de aceite
- [ ] Funciona nos três sistemas (testes manuais descritos no PR).
- [ ] CI continua passando sem chaveiro disponível (backend de teste).
- [ ] `docs/cofre-de-chaves.md` atualizado.
- [ ] Duas aprovações (área sensível).
