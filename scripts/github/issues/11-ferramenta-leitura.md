title: Ferramenta de leitura de arquivos com escopo de pastas
labels: ninho,seguranca
milestone: Fase 1 · Ninho
---
## Contexto
Primeira "ferramenta" dos agentes: listar e ler arquivos, só dentro das pastas que o dono liberou.

## O que fazer
- Configuração das pastas liberadas.
- Normalizar caminhos e **bloquear escapes**: `..`, links simbólicos e junções apontando para fora, caminhos UNC no Windows.
- Limites de tamanho e de quantidade de arquivos por chamada.
- Toda leitura passa pela `Policy` como `Permission::Read`.

## Critérios de aceite
- [ ] Testes de escape com `..` e com symlink para fora da pasta.
- [ ] Testes em Windows no CI.
