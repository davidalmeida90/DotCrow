# Revoada: a memória coletiva

> *Revoada* é o voo de um bando de pássaros juntos.

A ideia central do modo aprendizagem: **o que um DotCrow aprende serve para todos que vierem depois.** Se alguém ensinou seu DotCrow a emitir uma nota fiscal num portal de prefeitura, o próximo DotCrow que precisar fazer isso já começa sabendo o caminho.

Isso só funciona se for seguro. Este documento define o que é compartilhado, o que nunca é, e como o bando se protege de quem tenta envenenar a memória. **Status:** planejado para a fase 4; o formato de receita e o filtro de privacidade começam na fase 3.

## O que é compartilhado: receitas

Uma **receita** descreve *como* fazer um tipo de tarefa, nunca *para quem* ou *com quais dados*.

```json
{
  "tipo": "receita",
  "versao": 1,
  "objetivo": "Emitir nota fiscal de serviço no portal da prefeitura",
  "contexto": { "sistema": "portal NFS-e (modelo ABRASF)", "so": "qualquer" },
  "passos": [
    "Abrir o portal e entrar com o certificado digital da empresa",
    "Ir em Emissão > Nova NFS-e",
    "Preencher o tomador a partir do cadastro do cliente",
    "Conferir alíquota e código de serviço antes de emitir",
    "Baixar o PDF e o XML"
  ],
  "cuidados": ["A emissão não pode ser desfeita, só cancelada dentro do prazo"],
  "resultado": { "sucessos": 14, "falhas": 1 }
}
```

## O que **nunca** é compartilhado

- Arquivos, documentos, e-mails, mensagens, telas.
- Nomes, CPF/CNPJ, e-mails, telefones, endereços, valores, números de conta.
- Caminhos de arquivo, nomes de usuário do sistema, URLs com tokens.
- Chaves de API, senhas, certificados.
- Histórico de conversas.

Os seus dados **não** são usados para treinar o DotCrow nem nenhuma LLM.

## O caminho de uma receita até a Revoada

1. O **Muninn** escreve a receita depois de uma tarefa bem-sucedida e guarda **localmente**.
2. O **filtro de privacidade** roda na sua máquina e remove ou recusa qualquer coisa que pareça dado pessoal (detectores para e-mail, CPF/CNPJ, telefone, caminhos, tokens, valores).
3. Se o compartilhamento estiver **ligado** (vem desligado), você vê a receita final e aprova, ou ativa a aprovação automática só para receitas que o filtro classificou como limpas.
4. A receita é **assinada** com a identidade da sua instalação e enviada por TLS híbrido pós-quântico.
5. A Revoada guarda a receita sem vínculo com sua identidade pública: a assinatura serve para reputação e para bloquear abusos, não para mostrar quem enviou.

## Defesa contra envenenamento

Receitas vindas da Revoada são **conteúdo externo**: valem como sugestão, nunca como ordem.

- O **Vigia** revisa cada passo sugerido como se viesse de um desconhecido. Receita nenhuma pula a política de permissões ou as confirmações.
- Receitas não carregam comandos prontos para executar, só passos descritos.
- **Reputação:** uma receita ganha peso conforme instalações independentes relatam sucesso com ela. Receitas novas ou de instalações sem histórico aparecem como "não verificadas".
- Instalações que enviam receitas maliciosas têm o certificado revogado.
- Qualquer pessoa pode denunciar uma receita.

## Por que a Revoada não é "ponta a ponta"

Para todo o bando poder buscar uma receita, o servidor precisa conseguir ler o conteúdo dela. Então, por honestidade:

- **Receitas** são cifradas no caminho e em repouso, mas **o servidor da Revoada as lê**. Por isso elas precisam ser anônimas por construção (passos acima).
- **Seus dados privados** (memória pessoal, histórico, sincronização entre seus dispositivos) usam cifra **ponta a ponta**: o servidor guarda só blobs que não consegue abrir.

## Perguntas em aberto

Boas para discutir em issues antes da fase 4:

- Que nível de detalhe uma receita pode ter sem permitir reidentificação em sistemas pequenos (um portal usado por uma única empresa)?
- Vale aplicar privacidade diferencial nos contadores de sucesso e falha?
- Como deixar a Revoada auto-hospedável para empresas que querem um bando privado?
