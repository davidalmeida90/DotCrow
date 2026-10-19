title: Fuzzing e vetores de teste do envelope híbrido
labels: seguranca,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## O que fazer
- Alvos `cargo-fuzz` para `envelope::open`, `sym::open` e `PublicIdentity::from_bytes`: nenhuma entrada pode causar pânico.
- Vetores de teste fixos (identidade, aleatoriedade e envelope esperado) para detectar mudanças acidentais no formato.
- Teste de propriedade: abrir(selar(x)) == x para mensagens aleatórias.

## Critérios de aceite
- [ ] Fuzzing roda por alguns minutos sem achados (documente como rodar).
- [ ] Vetores em `crates/dotcrow-crypto/tests/vectors/`.
