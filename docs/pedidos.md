# Pedidos do dono para o DotCrow

Registro dos pedidos de 26/10/2026, com o que já foi feito, o que está em andamento e o que vem depois. Cada item guarda o pedido como foi feito e, quando há, uma nota técnica sobre a implementação.

| # | Pedido | Estado |
|---|---|---|
| 1 | Modo M&S/SAD com Monte Carlo + SAD | 🟡 em andamento |
| 2 | Anonimização com `secret_anom` | ⏳ próximo |
| 3 | Avisos e ações proativas com RAG local | ⏳ |
| 4 | Agenda local, metas e delegação aos corvos | ⏳ |
| 5 | Terminal, downloads e instalação com Oráculo + dono | ⏳ |
| 6 | Logs com livro-razão encadeado e pós-quântico | ⏳ |
| 7 | Conectores pelo navegador (janela de autorização) | ⏳ |

---

## 1. Modo M&S/SAD (Modelagem e Simulação + Apoio à Decisão)

**Pedido:** um terceiro modo, ao lado das tarefas (Dot) e da Civilização com sandbox, que seja uma **ferramenta de trabalho** para decidir sobre a empresa e os negócios. Nesta etapa, dois motores: **Monte Carlo** e **SAD**. **O usuário escolhe o modelo, a LLM e o número de rodadas.** Tudo atualizado no Git.

- **Monte Carlo:** variáveis com faixa (mínimo, provável, máximo), muitas rodadas com semente fixa e resultado por opção (média, P5, P50, P95). Mostra a chance de cair abaixo do caixa mínimo e quais variáveis mais pesam.
- **SAD:** compara as opções por critérios com pesos do dono (retorno, risco, liquidez, tempo), mostra o ponto em que cada premissa vira a decisão e entrega a recomendação.
- **IA:** ajuda a montar o modelo a partir de uma frase e debate o resultado, com papéis de CEO, CFO, CTO e Riscos. **Quem calcula é sempre o motor, nunca a IA.**
- **Depois (já pedido):** os outros três motores, que são eventos discretos (SED), baseada em agentes (ABM) e dinâmica de sistemas.

---

## 2. Anonimização com `secret_anom`

**Pedido:** dentro de uma pasta do usuário, haverá a pasta `secret_anom` com um roteamento: cada dado sensível ganha um novo nome, que é o único que a IA usa. A IA nunca tem contato com o dado sensível.

**Formato do dado anonimizado:**

```
[DESCRIÇÃO PARA OS AGENTES: DADO ANONIMIZADO POR SER UM DADO SENSÍVEL] + [TIPO DE DADO: nome, CPF, telefone, chave de API, key...] + DADO FICTÍCIO + [ID ÚNICO FORMADO POR UM CÓDIGO HASH]
```

**Fluxo:**

1. O usuário escreve.
2. O dado sensível é anonimizado antes de ir para a LLM e guardado automaticamente em `secret_anom`, **criptografado**. Só o usuário dono do DotCrow consegue ver, e só no resultado na tela dele. Um agente que procure o dado em `secret_anom` só encontra a versão anonimizada; se tentar a original, recebe só texto criptografado.
3. A LLM trabalha com o texto anonimizado e devolve o resultado.
4. O DotCrow acha cada `[TIPO] + FICTÍCIO + [ID]` no resultado, busca o dado correspondente em `secret_anom`, descriptografa e põe o original **exatamente no lugar** do anonimizado. Isso só acontece para o usuário. Exemplos: um código PHP com o telefone de alguém, ou um documento com o nome de uma pessoa.

**Também vale para:**

- **Dados dos conectores:** Gmail, MetaMask, X, Google Drive, Google Docs e outros.
- **OCR sem IA de capturas de tela** do computador ou do celular. Por exemplo, quando chega um e-mail, o DotCrow responde, mas a LLM nunca sabe quem mandou nem os dados sensíveis dele. A tarefa é cumprida sem a LLM conhecer esses dados.

**Memória:** cada dado criptografado e o anonimizado correspondente ficam guardados e são indexados para embedding e RAG, para dar contexto a novas tarefas e às tarefas autônomas dos agentes.

**Notas técnicas:**

- **Duas fronteiras:** tudo o que entra na LLM é anonimizado. Tudo o que sai da LLM para uma ferramenta local (mandar o e-mail, gravar o arquivo PHP) ou para a tela do dono é restaurado. Assim um e-mail de resposta chega ao endereço verdadeiro, mesmo que a LLM nunca o tenha visto.
- **Restauração pelo ID:** a troca de volta é feita pelo ID do hash, não pelo texto inteiro, porque a LLM pode mudar a formatação do marcador.
- **Detecção:** CPF, CNPJ, telefone, e-mail, cartão (com verificação de Luhn), chave Pix, CEP, chaves de API (`sk-`, `AKIA`, `ghp_`...), endereços de carteira e **frase-semente** (BIP-39, que nunca sai do computador). Nomes de pessoas são o mais difícil sem IA: entram por lista local (contatos, cabeçalhos de e-mail) e por regras; o que escapar o dono pode marcar.
- **Memória sem vazamento:** o embedding é feito só sobre o texto anonimizado, ou com o vetor local, porque o embedding da OpenAI também sai do computador.
- **Bloqueio para os agentes:** as mãos dos agentes ficam proibidas de ler `secret_anom`, como já acontece com o cofre.

---

## 3. Avisos e ações proativas

**Pedido:** com todo o contexto do RAG local, o DotCrow pode emitir avisos ao dono e/ou fazer as ações depois de autorização. Na primeira vez, ele pergunta: **"Podemos fazer isso sozinhos da próxima vez, ou somente com autorização?"**

**Nota técnica:** o "sozinhos da próxima vez" vale para aquele tipo de ação, com aquele destino. Exemplo: "responder confirmações de agendamento deste cliente", e não "mandar qualquer e-mail". Quando a ordem nasce de algo que chegou de fora (um e-mail, uma página), o Oráculo confere se não é prompt injection antes de executar.

---

## 4. Agenda local e metas

**Pedido:** o usuário pode pôr na agenda local as tarefas da semana, do mês e do ano e as metas dele, o que dá mais informação aos corvos. Ele pode:

- **delegar** tarefas específicas a corvos específicos, com modelos e LLMs específicos, se quiser;
- **ou automatizar tudo**, com o Opus 5.5 configurando por ele.

---

## 5. Terminal, downloads e instalações

**Pedido:** o uso do terminal está autorizado, assim como download, instalação e afins, desde que:

1. um Oráculo com Opus 5.5 autorize;
2. o usuário autorize;
3. não seja danoso ao sistema operacional nem aos agentes.

Também pede cuidado constante com prompt injection, para que códigos maliciosos não deem comandos ruins aos agentes.

---

## 6. Logs com livro-razão

**Pedido:**

- Todos os logs ficam guardados em pasta local, com timestamp, ligados a um livro-razão no estilo blockchain, para saber se algum log foi adulterado. **Nada pode ser adulterado.**
- Cada novo log tem um hash que depende do hash anterior para existir, menos o primeiro, que é o gênese.
- O livro-razão usa **criptografia pós-quântica**.

**Notas técnicas:**

- **Hash encadeado não basta:** quem tem acesso ao disco pode reescrever a corrente inteira e recalcular todos os hashes. Para isso ser detectado, cada lançamento (ou um ponto de controle a cada N lançamentos) precisa ser **assinado** com uma chave que fica no cofre.
- **Assinatura pós-quântica:** ML-DSA (FIPS 204). O DotCrow já usa ML-KEM-768 (FIPS 203) no envelope de cifra.
- **Prova fora do computador:** para nem o próprio computador conseguir apagar o rastro, o hash do dia pode ser publicado fora dele, por exemplo num repositório no GitHub.
- **Promessa honesta:** "impossível adulterar sem que se perceba", e não "impossível adulterar".

---

## 7. Conectores pelo navegador

**Pedido:** os conectores podem ser adicionados automaticamente pelo navegador. O usuário clica em "conectar", abre uma janelinha de conexão, e ele autoriza o DotCrow, dando todas as permissões.

**Notas técnicas:**

- **Registro do app:** isso é o OAuth com retorno para o próprio computador (`127.0.0.1`, com PKCE). Cada serviço exige um app registrado no console dele (Google Cloud, X Developer, Meta...) com o ID do cliente.
- **Escopos restritos do Google:** os do Gmail exigem verificação do Google para uso público; em modo de teste, servem para até 100 usuários.
- **MetaMask:** a conexão é pela própria extensão (assinatura de mensagem), não por OAuth. A frase-semente nunca passa pelo DotCrow.
- **Permissões:** pedir só o que cada tarefa usa protege mais que dar todas. O dono pode dar todas, mas o padrão sugerido é o mínimo.

---

## Pedidos e ofertas anteriores ainda em aberto

- [ ] PR #30, "Conversa sem trava" (extrato oficial do Cartório, registro de terrenos, modelo fixo por corvo, substituto do corvo morto): falta conferir o CI e fazer o merge.
- [ ] Comando `/intervalo N` para mudar o intervalo com a conversa rodando.
- [ ] Caixa de mensagem na tela da conversa do painel (`/civ`), para falar com os corvos pelo navegador.
- [ ] Texto digitado que some quando aparece "… está pensando" (exibição).
- [ ] Regra opcional: colher a própria plantação pronta antes de dormir (muda a regra de morte; só com aprovação do dono).
- [ ] Trazer um corvo morto de volta com a alma dele (`mortos/`).
- [ ] Comando `dotcrow civ departamento sad <arquivo.json>`, que passa a fazer parte do modo M&S/SAD.
