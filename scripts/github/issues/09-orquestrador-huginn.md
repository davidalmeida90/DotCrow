title: Orquestrador: Huginn gera um plano estruturado com uma LLM real
labels: agente,ajuda-bem-vinda
milestone: Fase 1 · Ninho
---
## Contexto
O primeiro passo de toda tarefa é o Huginn transformar o pedido em plano. É só leitura e planejamento, então dá para fazer com segurança antes de o DotCrow ter "mãos".

## O que fazer
- Definir o formato do plano (JSON): objetivo, critério de pronto, subtarefas com dependências e agente responsável, dúvidas.
- `dotcrow planejar "organize minha pasta Downloads"` chama o roteador com o perfil do Huginn e mostra o plano.
- Validar a resposta da LLM; se vier inválida, pedir de novo uma vez e depois falhar com mensagem clara.

## Critérios de aceite
- [ ] Testes com o `MockProvider` devolvendo planos válidos e inválidos.
- [ ] Nenhuma ação é executada.
