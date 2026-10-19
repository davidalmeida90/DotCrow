# Identidade visual do DotCrow

O corvo do DotCrow é desenhado em linhas geométricas de traço único. O olho é um ponto sólido, o "dot" do nome, e é o único elemento preenchido do desenho.

## Arquivos

| Arquivo | Uso |
|---|---|
| `dotcrow-logo-light.svg` | Logo com nome, para fundos claros |
| `dotcrow-logo-dark.svg` | Logo com nome, para fundos escuros |
| `dotcrow-mark-light.svg` | Só o corvo, para fundos claros |
| `dotcrow-mark-dark.svg` | Só o corvo, para fundos escuros |
| `dotcrow-favicon.svg` | Favicon que troca de cor sozinho no modo escuro do navegador |
| `dotcrow-app-icon.svg` | Ícone de app (fundo escuro arredondado) |
| `png/` | Versões em PNG, incluindo a capa do GitHub (`dotcrow-social-preview.png`, 1280×640) |

O nome "DotCrow" nos SVGs já está convertido em contornos, então o logo aparece igual em qualquer máquina, sem depender de fonte instalada.

## Cores

| Nome | Hex | Uso |
|---|---|---|
| Tinta | `#0D1117` | Traço sobre fundo claro, fundo escuro |
| Papel | `#F0F6FC` | Traço sobre fundo escuro |
| Cinza | `#8B949E` | Textos de apoio |

Fonte do nome: Inter SemiBold (licença SIL Open Font).

## No README do repositório

Este bloco mostra a versão certa conforme o tema claro ou escuro do GitHub:

```html
<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/dotcrow-logo-dark.svg">
    <img alt="DotCrow" src="assets/brand/dotcrow-logo-light.svg" width="360">
  </picture>
</p>
```

## Regras de uso

- Não distorça, não gire e não mude a espessura do traço.
- Não adicione sombra, degradê ou contorno extra.
- Mantenha um respiro em volta do logo de pelo menos a altura do olho multiplicada por quatro.
- Tamanho mínimo do corvo sozinho: 16 px.

O código do DotCrow segue a licença do repositório. O nome e o logo identificam o projeto: podem ser usados para falar dele ou apontar para ele, mas não para sugerir que um fork ou produto de terceiros é o DotCrow oficial.
