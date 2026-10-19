# Governança

## Papéis

**Fundador e mantenedor principal:** Napoleão Junior ([@napoleaomp4](https://github.com/napoleaomp4)). Define a direção do projeto, aprova a entrada de novos mantenedores e dá a palavra final quando não houver consenso.

**Mantenedores:** revisam e aceitam pull requests, organizam issues e cuidam de uma ou mais áreas do projeto. A lista fica em [`.github/CODEOWNERS`](.github/CODEOWNERS).

**Contribuidores:** qualquer pessoa que envia código, documentação, testes, traduções, design ou relatos de bug.

## Como alguém vira mantenedor

Depois de contribuições consistentes e de boa qualidade numa área (em geral, alguns pull requests aceitos e revisões úteis em PRs de outras pessoas), um mantenedor pode indicar a pessoa. O fundador decide, ouvindo os demais mantenedores.

Mantenedores inativos por mais de seis meses passam a "emérito", com agradecimento, e podem voltar quando quiserem.

## Como as decisões são tomadas

- **Mudanças do dia a dia** (correções, provedores novos, documentação): decididas no próprio pull request, com a aprovação de um mantenedor.
- **Áreas sensíveis** (`dotcrow-crypto`, `dotcrow-vault`, política de permissões, Vigia, Revoada): duas aprovações de mantenedores, assim que houver dois ou mais.
- **Decisões de arquitetura** (formato de dados, esquema criptográfico, um agente novo, licença): abertas como issue de proposta, discutidas por pelo menos 7 dias e registradas como ADR em [`docs/adr/`](docs/adr/).

Buscamos consenso. Quando não houver, o fundador decide e registra o motivo no ADR.

## Marca

O código é livre sob a Apache 2.0. O nome **DotCrow** e o logo identificam a distribuição oficial. Forks são bem-vindos, mas devem usar outro nome e outro logo para não confundir as pessoas.
