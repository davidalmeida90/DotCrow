title: Esqueleto do Ninho com canal local autenticado
labels: ninho,seguranca
milestone: Fase 1 · Ninho
---
## Contexto
O Ninho é o serviço local que mantém o cofre aberto e executa as tarefas. A CLI e o app falam com ele. A regra "ninguém vê a chave" precisa virar fronteira entre processos: o protocolo **não pode ter** nenhum comando que devolva uma chave.

## O que fazer
- Novo crate `dotcrow-ninho` (binário `dotcrow-ninho`).
- Canal local: socket Unix (Linux/macOS) e named pipe (Windows), acessível só pelo usuário dono.
- Protocolo versionado com mensagens: adicionar chave, listar chaves, apagar chave, status, enviar tarefa (só ecoa por enquanto).
- A CLI passa a usar o Ninho quando ele estiver rodando.

## Critérios de aceite
- [ ] Teste garantindo que nenhuma resposta do protocolo contém o segredo.
- [ ] Outro usuário do sistema não consegue conectar.
- [ ] ADR descrevendo o protocolo.
