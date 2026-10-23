# Relay em PHP (`relay.php`)

Ponto de encontro do túnel do DotCrow em **PHP puro** — a alternativa ao binário Rust
`dotcrow-relay` para quem prefere só **subir um arquivo no domínio** (hospedagem PHP comum,
tipo a da Hostinger) em vez de rodar um serviço.

Faz o mesmo papel: repassa bytes entre o **computador** (host) e o **celular** (guest) que
estão na mesma sala. É **zero-conhecimento** — tudo que passa por ele já está cifrado de
ponta a ponta; o `relay.php` só vê base64 embaralhado, o id da sala e para que lado mandar.
Não guarda nem lê chave nenhuma.

## Subir no domínio

Copie `relay.php` para a pasta pública do seu site, por exemplo:

```
https://dotcrow.si/relay.php
```

Pronto. Não precisa de banco de dados nem de `composer`. Ele guarda os frames em arquivos
temporários (em `sys_get_temp_dir()/dotcrow-relay`, ou numa pasta `.relay-dados` protegida
ao lado do arquivo) e apaga cada sala depois de ~5 min sem uso.

Requisitos: PHP 7.4+ (testado no 8.3). Para o long-poll fluir bem, o servidor precisa
atender **requisições em paralelo** — Apache/nginx + php-fpm já fazem isso (não use o
`php -S` sem `PHP_CLI_SERVER_WORKERS`, que atende uma de cada vez).

Teste rápido depois de subir:

```sh
curl "https://dotcrow.si/relay.php?saude=1"
# {"ok":true,"servico":"dotcrow-relay-php"}
```

## Como a conversa acontece (HTTP long-poll)

Como PHP comum não segura um WebSocket aberto, cada lado **envia** frames e **busca** os do
outro, esperando um pouco se ainda não houver nada:

| ação | requisição |
|------|------------|
| enviar um frame | `POST relay.php?sala=<room>&de=<host\|guest>` · corpo = 1 frame em **base64** |
| buscar os frames do outro | `GET relay.php?sala=<room>&para=<host\|guest>&desde=<n>&espera=<seg>` |

- O que o **host** envia (`de=host`) vai para a caixa que o **guest** lê (`para=guest`), e
  vice-versa.
- O `GET` devolve `{"frames":[{"seq":N,"b64":"..."}],"ultimo":M}`. Guarde o `ultimo` e mande
  como `desde` na próxima busca — assim você nunca repete nem perde frame (idempotente).
- `espera` (até 25 s) segura a resposta até chegar algo; se não chegar, volta com a lista
  vazia e você busca de novo (long-poll).

A `sala` é o `room_id` (vem do QR, derivado da identidade do computador), então os dois
lados se acham sem cadastro. Os frames são exatamente os mesmos do protocolo do túnel
(`HI/HELLO/KEY/OK/CRED/DATA`, veja `docs/remoto.md`) — só que em base64 dentro do corpo HTTP
em vez de binário no WebSocket.

## E o túnel WebSocket?

O relay Rust (`dotcrow-relay`, via `wss://`) continua valendo e tem **menos latência** — é o
caminho recomendado quando dá para rodar o binário. O `relay.php` existe para hospedagem só-PHP.

> Observação: hoje o computador (`dotcrow painel --remoto`) e o app falam **WebSocket** com o
> relay. Para usarem este `relay.php` (HTTP), falta ligar o transporte HTTP nos dois lados —
> a lógica de frames é a mesma, muda só o "cano". Isso é o próximo passo; me avise que eu
> faço. O `relay.php` em si já está pronto e testado.
