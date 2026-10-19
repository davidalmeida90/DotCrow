# 0003. Cofre de chaves somente-escrita

- **Status:** aceita
- **Data:** 2026-10-19

## Contexto

O dono do computador pediu que nenhuma chave de API fique exposta: só ele adiciona, e nem ele consegue ver depois, apenas apagar e criar outra. Chaves de API valem dinheiro, e qualquer caminho de leitura vira alvo de scripts, extensões e prompt injection.

## Decisão

O cofre expõe apenas `add`, `list` (metadados e impressão digital) e `delete`. O único acesso ao segredo é `lease`, usado pelo adaptador do provedor para montar a requisição. Na fase 1 o cofre passa a viver dentro do Ninho, e o protocolo local entre CLI/app e Ninho não terá verbo de leitura de chave.

## Alternativas consideradas

- **Mostrar a chave mascarada (últimos 4 caracteres):** vaza parte do segredo e cria o hábito de "conferir olhando". A impressão digital resolve a conferência sem vazar nada.
- **Comando de exportação protegido por senha:** reabre exatamente o caminho que queremos fechar.

## Consequências

- Para trocar uma chave, o dono apaga e adiciona outra.
- Se o dono perder o acesso à conta do provedor, a chave guardada não ajuda a recuperá-la. Isso é intencional.
