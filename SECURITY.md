# Política de segurança

O DotCrow vai ter acesso ao computador das pessoas e às chaves de API delas. Levamos falhas de segurança a sério e agradecemos muito a quem as encontra e avisa de forma responsável.

## Como reportar uma falha

**Não abra issue pública, não comente em pull request e não poste em redes sociais.**

Use um destes canais privados:

1. **GitHub:** aba *Security* do repositório → *Report a vulnerability* (relato privado de vulnerabilidade).
2. **E-mail:** contato.napoleao2023@gmail.com, com o assunto `[SEGURANÇA] DotCrow`.

Inclua, se puder:

- o que a falha permite fazer e qual o impacto;
- passos para reproduzir ou uma prova de conceito;
- versão ou commit afetado e sistema operacional;
- como você gostaria de ser creditado (ou se prefere ficar anônimo).

## O que acontece depois

| Etapa | Prazo |
|---|---|
| Confirmamos que recebemos | até 3 dias |
| Avaliação inicial e gravidade | até 7 dias |
| Correção de falhas críticas | o mais rápido possível, com atualização a cada 7 dias |
| Divulgação pública | depois da correção, combinada com você, em até 90 dias |

Você será creditado no aviso de segurança e no CHANGELOG, se quiser.

## Escopo

Nos interessam principalmente:

- qualquer forma de **ler uma chave de API** guardada no cofre por um caminho que não seja o runtime do provedor (CLI, interface, API local, logs, arquivos temporários, mensagens de erro);
- falhas na criptografia (`dotcrow-crypto`): quebra do envelope híbrido, reutilização de nonce, derivação de chave fraca, vazamento por tempo de execução;
- **burlar a política de permissões**: um agente executar algo irreversível ou proibido sem a confirmação do dono;
- **prompt injection**: conteúdo de uma página, e-mail, arquivo ou receita da Revoada que faça o DotCrow agir como se fosse uma ordem do dono;
- vazamento de dados pessoais para a Revoada (memória coletiva);
- falsificação de identidade ou certificado de uma instalação;
- dependências com vulnerabilidades conhecidas que afetem o DotCrow.

Fora do escopo:

- ataques que exigem acesso de administrador ou malware já instalado na máquina da vítima (a partir daí, nenhum programa local consegue se proteger totalmente; veja o modelo de ameaças em [docs/seguranca.md](docs/seguranca.md));
- engenharia social contra mantenedores;
- negação de serviço por volume contra serviços de terceiros.

## Versões com suporte

O projeto está em **pré-alfa**. Só o branch `main` recebe correções. Esta tabela será atualizada a partir da primeira versão estável.

| Versão | Suporte |
|---|---|
| `main` | ✅ |
| outras | ❌ |

## Porto seguro

Não vamos tomar medidas legais contra quem pesquisar e reportar de boa-fé, seguindo esta política: sem acessar dados de outras pessoas, sem degradar serviços e dando tempo razoável para a correção antes de divulgar.
