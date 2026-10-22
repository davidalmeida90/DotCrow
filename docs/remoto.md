# Acesso remoto: o celular pela internet

O painel do DotCrow já funciona no celular na mesma rede Wi-Fi (`dotcrow painel --rede`).
O **modo remoto** (`dotcrow painel --remoto`) vai além: o celular acessa o computador de
**qualquer lugar**, por Wi-Fi ou dados móveis, sem estar na mesma rede e sem abrir porta
no roteador. Os dados são cifrados **de ponta a ponta**; o relay no meio só repassa bytes.

```
Celular  ──wss──▶  Relay (dotcrow.si)  ◀──wss──  Computador (dotcrow painel --remoto)
          (sai)                                   (sai)
         └──────── tudo cifrado de ponta a ponta; o relay só vê ruído ────────┘
```

Os dois lados **discam para fora**, então funciona atrás de NAT, CGNAT e dados móveis.

## Por que um relay

Celular e computador quase nunca conseguem se achar direto na internet (os dois estão
atrás de NAT). O relay é um ponto de encontro para onde os dois ligam. Ele é
**zero-conhecimento**: tudo chega cifrado com a chave de sessão, que nasce de um envelope
pós-quântico que só o computador abre. O relay nunca vê o código de acesso, o pedido nem
a resposta — só o id da sala e bytes embaralhados. Por isso não guarda nada e não tem
chave nenhuma.

## As peças

- **`dotcrow-relay`** — o servidor de encontro, roda na VPS (veja o fim deste arquivo).
- **`dotcrow painel --remoto`** — o lado computador: conecta no relay e faz o túnel da
  mesma API+SSE do painel.
- **o app (Android/iOS)** — o lado celular, que implementa o protocolo abaixo.

## Identidade e sala

O computador tem uma identidade pós-quântica em `identidade.bin` (criada na primeira vez).
A **sala** é `room_id = SHA-256("dotcrow/room/v1\0" || identidade_pública)[..16]` em hex
(32 caracteres). Como a sala vem da identidade, o celular confere que a sala que recebeu no
QR corresponde à identidade que o computador apresenta — isso barra um impostor no relay.

## Conexão ao relay

WebSocket:

```
wss://<relay>/t?sala=<room_id>&rol=host     (computador)
wss://<relay>/t?sala=<room_id>&rol=guest    (celular)
```

O relay pareia os dois lados da mesma sala e repassa cada mensagem binária de um para o
outro. Frame maior que 8 MiB derruba a conexão; 120 s sem tráfego também.

## Frames

Cada frame é `1 byte de tag || payload`:

| tag  | nome  | sentido            | payload |
|------|-------|--------------------|---------|
| 0x01 | HI    | celular → PC       | vazio |
| 0x02 | HELLO | PC → celular       | identidade pública (1216 bytes: X25519 32 + ML-KEM-768 1184) |
| 0x03 | KEY   | celular → PC       | envelope pós-quântico selado para a identidade do PC |
| 0x04 | OK    | PC → celular       | `{"ok":true}` cifrado com a chave de sessão |
| 0x10 | DATA  | ambos              | requisição/resposta JSON cifrada com a chave de sessão |
| 0x1f | ERRO  | PC → celular       | texto claro (ex.: código de acesso errado) |

## Handshake

1. **HI** — o celular entra na sala e manda `HI`.
2. **HELLO** — o computador responde com sua identidade pública (em claro; é pública).
   O celular confere `room_id(identidade) == sala`; se não bater, aborta (impostor).
3. **KEY** — o celular sorteia uma chave de sessão `K` (32 bytes) e monta o claro
   `len(código) (1 byte) || código || K (32 bytes)`, onde `código` é o **código de acesso**
   do painel (o mesmo do QR). Sela esse claro para a identidade pública do computador com o
   envelope híbrido (ML-KEM-768 + X25519, AAD = `dotcrow/remoto/v1/key`) e manda como `KEY`.
4. O computador abre o envelope com sua identidade secreta, confere o código de acesso
   (autentica o celular) e adota `K`. Se o código estiver errado, manda `ERRO`.
5. **OK** — o computador manda `{"ok":true}` cifrado com `K`. O celular só confia na sessão
   quando consegue abrir esse OK.

A partir daí, os dois usam `K` com XChaCha20-Poly1305 (AAD = `dotcrow/remoto/v1/data`) em
todos os frames `DATA`. O formato do cifrado é `versão (1) || nonce (24) || cifra+tag`.

## Falando com o painel (frames DATA)

Cada `DATA` carrega um JSON cifrado. O celular faz requisições; o computador executa a
mesma chamada no painel local e devolve a resposta.

Requisição (celular → PC):

```json
{ "id": 1, "metodo": "GET", "caminho": "/api/estado" }
{ "id": 2, "metodo": "POST", "caminho": "/api/tarefas", "corpo": { "text": "..." } }
{ "id": 3, "metodo": "GET", "caminho": "/api/tarefas/<id>/eventos", "sse": true }
```

Resposta (PC → celular), um ou vários frames com o mesmo `id`:

```json
{ "id": 1, "status": 200, "corpo": "<texto da resposta>" }     // resposta normal
{ "id": 3, "sse": "data: {...}\n\n" }                          // um pedaço do fluxo ao vivo
{ "id": 3, "fim": true, "status": 200 }                        // o fluxo acabou
{ "id": 1, "erro": "caminho inválido" }                        // deu errado
```

Para os eventos ao vivo (`sse: true`, ou qualquer caminho terminando em `/eventos`), o
computador transmite o corpo em pedaços, cada um num frame com `"sse"`, até um frame com
`"fim": true`. O app lê o texto SSE igual ao painel web: linhas `data:` com JSON de evento.
Assim o celular vê os corvos trabalhando **em tempo real**. A API inteira do painel está em
[painel.md](painel.md).

## Pareamento (QR)

O computador mostra um QR com o link:

```
dotcrow://p?r=<relay wss>&s=<room_id>&c=<código de acesso>
```

O app registra o esquema `dotcrow://`, lê o QR uma vez, guarda `relay`, `sala`, `código` e a
identidade do computador (aprendida no `HELLO` e conferida contra a sala). Depois conecta de
qualquer rede sem ler o QR de novo. Trate o QR como uma senha.

## Rodar o relay na VPS

O relay escuta `ws://` no localhost; o TLS fica no nginx (que serve `wss://dotcrow.si/t`).

Serviço systemd (`/etc/systemd/system/dotcrow-relay.service`):

```ini
[Unit]
Description=DotCrow relay
After=network.target

[Service]
Environment=DOTCROW_RELAY_BIND=127.0.0.1:8787
ExecStart=/usr/local/bin/dotcrow-relay
Restart=always
DynamicUser=yes

[Install]
WantedBy=multi-user.target
```

nginx (dentro do `server { }` de dotcrow.si, que já tem o certificado TLS):

```nginx
location /t {
    proxy_pass http://127.0.0.1:8787;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 130s;
}
```

Compile o relay com `cargo build --release -p dotcrow-relay` e copie
`target/release/dotcrow-relay` para a VPS. Ele não guarda nada e não precisa de chaves.
