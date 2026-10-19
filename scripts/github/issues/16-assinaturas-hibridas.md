title: Assinaturas híbridas Ed25519 + ML-DSA-65
labels: seguranca
milestone: Fase 4 · Revoada
---
## Contexto
Receitas e certificados da Revoada precisam de assinaturas resistentes a computadores quânticos. Seguindo o ADR 0002, a assinatura é híbrida: as duas precisam ser válidas.

## O que fazer
- `dotcrow-crypto::sign` com geração de chaves, assinatura e verificação Ed25519 + ML-DSA-65 (FIPS 204), usando crates do RustCrypto e dalek.
- Formato versionado; separação de domínio por contexto (`receita`, `certificado`...).
- A identidade da instalação passa a ter as chaves de assinatura.

## Critérios de aceite
- [ ] Assinatura com só uma das partes válida é rejeitada (teste).
- [ ] ADR com o formato.
- [ ] Duas aprovações.
