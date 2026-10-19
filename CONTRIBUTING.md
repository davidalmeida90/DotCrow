# Como contribuir com o DotCrow

Que bom que você chegou. O DotCrow é construído em público e toda ajuda conta: código, testes, documentação, tradução, design, revisão de segurança, ou simplesmente usar e contar o que deu errado.

Antes de tudo, leia o [Código de Conduta](CODE_OF_CONDUCT.md). Ele vale para issues, pull requests, discussões e qualquer espaço do projeto.

## Por onde começar

As issues usam estes rótulos:

| Rótulo | O que significa |
|---|---|
| `boa-primeira-issue` | Escopo pequeno e bem explicado. Ótima para quem nunca contribuiu. |
| `ajuda-bem-vinda` | Os mantenedores querem ajuda aqui. |
| `provedor` | Adaptadores de LLM (Claude, GPT, Gemini, Ollama...). |
| `agente` | Comportamento dos corvos e do orquestrador. |
| `seguranca` | Criptografia, cofre, permissões, prompt injection. Revisão redobrada. |
| `revoada` | Memória coletiva e privacidade das receitas. |
| `docs` | Documentação e tradução. |

**Comente na issue antes de começar**, dizendo que vai pegar. Assim ninguém faz o mesmo trabalho em dobro. Se em 14 dias não houver novidade, a issue volta a ficar livre.

Quer propor algo novo e grande (um agente novo, uma mudança na criptografia, um formato de dados)? Abra primeiro uma issue do tipo "Ideia ou recurso" para conversar. Decisões de arquitetura viram um registro em [`docs/adr/`](docs/adr/).

## Preparando o ambiente

1. Instale o [Rust](https://rustup.rs) 1.88 ou mais novo.
2. Faça um fork do repositório e clone o seu fork.
3. Rode:

```bash
cargo build
cargo test
```

Antes de abrir o pull request, rode as mesmas verificações do CI:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

## Fluxo de trabalho

1. Crie um branch a partir de `main`, com nome curto: `feat/provedor-ollama`, `fix/cofre-duplicado`, `docs/agentes-ingles`.
2. Faça commits pequenos, com mensagens no padrão [Conventional Commits](https://www.conventionalcommits.org/pt-br/v1.0.0/):
   - `feat(providers): adiciona adaptador Ollama`
   - `fix(vault): recusa chave com espaços no meio`
   - `docs: explica a troca automática de provedor`
3. Assine cada commit com `git commit -s`. Isso adiciona a linha `Signed-off-by` e declara que você tem o direito de enviar esse código sob a licença do projeto, conforme o [Developer Certificate of Origin](https://developercertificate.org/).
4. Abra o pull request preenchendo o modelo. Ligue a issue com `Closes #123`.
5. O CI precisa passar. Pelo menos um mantenedor revisa. Mudanças em `dotcrow-crypto`, `dotcrow-vault`, na política de permissões ou no Vigia precisam de **duas** aprovações assim que o projeto tiver dois mantenedores ou mais.

## Padrões de código

- **Identificadores em inglês** (`Vault`, `add`, `ProviderError`). Documentação, mensagens para o usuário e comentários podem ser em português; inglês também é bem-vindo.
- Sem `unsafe`. O workspace proíbe.
- Sem `unwrap()` e `expect()` fora de testes: trate o erro ou devolva `Result`.
- Todo item público precisa de comentário de documentação (`///`).
- Teste o comportamento novo. Para correção de bug, escreva primeiro o teste que falha.
- Mensagens de erro para o usuário em português claro, dizendo o que fazer a seguir.

### Regras para código sensível

Estas regras protegem as pessoas que vão confiar no DotCrow:

- **Não invente criptografia.** Use os crates do [RustCrypto](https://github.com/RustCrypto) e da família `dalek`. Mudanças de esquema passam por um ADR.
- **Segredos nunca aparecem em logs, erros, `Debug` ou `Display`.** Use `zeroize::Zeroizing` para guardá-los e teste que não vazam.
- **O cofre não ganha operação de leitura de chave para a interface.** Nem "só para depurar".
- **Conteúdo externo é dado, não ordem.** Páginas, e-mails, arquivos e receitas da Revoada nunca podem disparar ações sem passar pelo Vigia e pela política.
- **Nada irreversível sem confirmação** do dono do computador.

## Adicionando um provedor de LLM

É a forma mais rápida de causar impacto. Veja o passo a passo em [docs/provedores.md](docs/provedores.md#adicionando-um-provedor). Em resumo:

1. Crie um módulo em `crates/dotcrow-providers/src/` implementando `LlmProvider`.
2. Converta erros HTTP com `ProviderError::from_http_status` e refine o que for específico daquele provedor (por exemplo, "créditos esgotados" vindo com status 400).
3. Pegue a chave pelo `Lease` do cofre, só na hora de montar a requisição. Não guarde cópia.
4. Escreva testes com respostas gravadas (sem chamar a API de verdade no CI).

## Mudando ou criando um agente

Os perfis ficam em `crates/dotcrow-agents/src/lib.rs`. Cada agente tem um jeito de pensar e de organizar que precisa continuar **diferente** dos outros; esse contraste é o que faz o bando funcionar. Explique no pull request que tipo de tarefa melhora com a mudança e mostre um antes e depois.

## Contribuições feitas com ajuda de IA

São bem-vindas. Você continua responsável por entender, testar e defender cada linha. Diga no pull request quais partes tiveram ajuda de IA.

## Dúvidas

Abra uma issue com a sua pergunta. Pergunta boba não existe; se você ficou com dúvida, a documentação precisa melhorar.
