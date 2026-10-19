# 0004. Aprendizado coletivo opcional e sem dados pessoais

- **Status:** aceita
- **Data:** 2026-10-19

## Contexto

O modo aprendizagem do DotCrow faz o conhecimento de uma instalação servir para todas (a Revoada). Ao mesmo tempo, os dados sensíveis de cada pessoa não podem ser coletados nem usados para treinar nada.

## Decisão

- O compartilhamento vem **desligado** e só é ligado pelo dono.
- Só são compartilhadas **receitas** (como fazer um tipo de tarefa), depois de um filtro de privacidade local e da aprovação do dono.
- Receitas recebidas são tratadas como conteúdo externo não confiável: passam pelo Vigia e pela política, e têm reputação.
- Dados privados do dono usam cifra ponta a ponta; receitas são legíveis pelo servidor e por isso precisam ser anônimas por construção.

## Alternativas consideradas

- **Compartilhar conversas completas:** vaza dados pessoais inevitavelmente.
- **Ponta a ponta também nas receitas:** impediria a busca coletiva, que é o objetivo da Revoada.
- **Ligado por padrão:** contraria a promessa de não coletar dados sem consentimento.

## Consequências

- O valor da Revoada cresce mais devagar, porque depende de quem opta por compartilhar.
- Precisamos investir em bons detectores de dados pessoais em português (CPF, CNPJ, telefones brasileiros).
