title: Binários prontos para Windows, macOS e Linux nas releases
labels: ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## Contexto
Para testar o DotCrow sem instalar Rust, as pessoas precisam baixar o binário pronto.

## O que fazer
- Workflow de release disparado por tag `v*`, gerando `dotcrow` para Windows (x86_64), macOS (Apple Silicon e Intel) e Linux (x86_64 e arm64). Ferramentas como `cargo-dist` ajudam.
- Publicar somas SHA-256 junto dos binários.
- Instruções de instalação no README.

## Critérios de aceite
- [ ] Uma release de teste gerada num fork.
