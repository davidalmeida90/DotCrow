title: Filtro de privacidade para receitas
labels: revoada,boa-primeira-issue
milestone: Fase 3 · Memória
---
## Contexto
Nenhuma receita pode levar dado pessoal para a Revoada. O filtro roda na máquina do dono antes de qualquer envio.

## O que fazer
Começar com detectores simples e bem testados:
- e-mail, telefone brasileiro (com e sem DDD), CPF e CNPJ (com validação dos dígitos), CEP;
- caminhos de arquivo e nomes de usuário do sistema;
- URLs com parâmetros de token, chaves de API comuns (`sk-...`, `AIza...`);
- valores em reais.

O filtro devolve a receita limpa **e** a lista do que encontrou, para o dono revisar.

## Critérios de aceite
- [ ] Testes com exemplos positivos e negativos para cada detector.
- [ ] Nenhum falso negativo nos exemplos de CPF, CNPJ e e-mail.
