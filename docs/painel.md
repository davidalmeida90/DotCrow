# Painel e API

O painel é o jeito visual de usar o DotCrow: guardar e testar as chaves de API, entregar tarefas aos corvos e acompanhar cada etapa ao vivo. Ele roda no seu computador e abre no navegador, inclusive no do celular.

```bash
dotcrow painel
```

O navegador abre sozinho. O terminal mostra o endereço e o **código de acesso**. Deixe o terminal aberto enquanto usa; `Ctrl+C` fecha o painel.

## No celular

O celular precisa estar no mesmo Wi-Fi do computador.

```bash
dotcrow painel --rede
```

O terminal mostra um QR code. Aponte a câmera do celular: o endereço já leva o código de acesso, então não precisa digitar nada. No navegador do celular, use "Adicionar à tela de início" para abrir o painel como um app.

> [!WARNING]
> Na rede local o tráfego do painel ainda não é cifrado (HTTPS no painel está no ROADMAP). Use `--rede` só em Wi-Fi de confiança, como o de casa ou do escritório.

Outras opções:

| Opção | O que faz |
|---|---|
| `--porta 7778` | Usa outra porta (o padrão é 7777) |
| `--sem-navegador` | Não abre o navegador sozinho |
| `--novo-codigo` | Gera um código de acesso novo; o antigo para de funcionar em todos os aparelhos |

## O que o painel faz hoje

- **Tarefas:** você escreve o pedido; o Huginn planeja, o Vigia revisa os riscos, quem executa entrega o resultado, o Vigia confere e a Pena resume. Cada etapa aparece ao vivo, com o provedor e o modelo que responderam e qualquer troca automática de provedor.
- **Pergunta rápida:** uma pergunta direta, sem passar pelo fluxo dos corvos. Bom para testar as chaves.
- **Chaves de API:** guardar, testar, apagar e trocar. Nenhuma tela mostra uma chave guardada.
- **Provedores:** ordem de preferência, modelo de cada um e endereço do Ollama.
- **Atividade:** as últimas 50 tarefas, enviadas de qualquer aparelho. Uma tarefa enviada pelo computador aparece no celular, e vice-versa.

Nesta fase os corvos **ainda não mexem no computador**. Em tarefas de escrita, a Pena entrega o texto pronto. Em tarefas de organizar ou operar, a Gralha e o Bico entregam os passos exatos que executariam, sem executar. As tarefas ficam na memória enquanto o painel está aberto.

## Segurança do painel

| Proteção | Como funciona |
|---|---|
| Só este computador por padrão | O painel escuta em `127.0.0.1`. Com `--rede`, aceita aparelhos da rede local. |
| Código de acesso | 16 símbolos (80 bits), guardado em `painel.token` na pasta de dados, com permissão 0600. Toda chamada a `/api/` exige o código. |
| Contra sites maliciosos | O painel confere o cabeçalho `Host` (barra *DNS rebinding*) e não libera CORS, então uma página aberta no navegador não consegue falar com ele por trás. |
| Sem vazar chaves | Nenhuma rota devolve uma chave. As chaves entram pelo painel e vão direto para o cofre cifrado. |
| Conteúdo da IA é texto | A página mostra as respostas dos modelos sempre como texto puro, nunca como HTML. A política de segurança de conteúdo (CSP) bloqueia scripts e conexões de fora. |
| Código fora da barra de endereço | Depois de abrir pelo QR code, a página guarda o código e o tira da URL. |

## API

A mesma API que a página usa serve para os seus apps e, no futuro, para a ponte com o WhatsApp. Todas as rotas abaixo de `/api/` falam JSON e exigem:

```
Authorization: Bearer <código de acesso>
```

O código pode ir com ou sem hífens. Erros sempre vêm como `{"error": "mensagem"}` com o status HTTP adequado (400 pedido inválido, 401 código errado, 403 endereço não permitido, 404 não encontrado, 502 todos os provedores falharam).

### Estado

`GET /api/estado`

```json
{
  "version": "0.1.0",
  "providers": [
    { "id": "anthropic", "name": "Anthropic (Claude)", "model": "claude-sonnet-5-5", "default_model": true,
      "catalog_model": "claude-sonnet-5-5", "custom_url": null, "needs_key": true, "has_key": true,
      "position": 1, "key_url": "https://console.anthropic.com/settings/keys" }
  ],
  "keys": [
    { "id": "8365fa75", "provider": "anthropic", "label": "conta pessoal",
      "fingerprint": "cae4-728a-25d6-b4f2", "created_at": 1792548562 }
  ],
  "order": ["anthropic"],
  "warnings": [],
  "lan": false
}
```

### Chaves

| Rota | Corpo | Resposta |
|---|---|---|
| `POST /api/chaves` | `{"provider": "anthropic", "key": "sk-...", "label": "opcional"}` | Metadados da chave (sem a chave) |
| `DELETE /api/chaves/{id}` | | Metadados da chave apagada |

Nomes populares viram o id do catálogo: `claude` → `anthropic`, `gpt` → `openai`, `google` → `gemini`.

### Provedores

`PUT /api/provedores`

```json
{ "order": ["anthropic", "gemini"], "models": { "anthropic": "claude-haiku-5-5", "gemini": null }, "urls": { "ollama": "http://192.168.0.10:11434/v1" } }
```

Todos os campos são opcionais. `null` ou texto vazio volta ao padrão. Responde com o estado atualizado.

`POST /api/testar` com `{}` testa todos os provedores com chave; com `{"provider": "gemini"}`, só um. Responde uma lista:

```json
[{ "provider": "gemini", "ok": false, "model": "gemini-3.8-flash", "ms": 412,
   "error": "chave de API recusada", "hint": "A chave foi recusada. Confira se copiou inteira..." }]
```

### Pergunta rápida

`POST /api/perguntar` com `{"text": "...", "provider": null}` responde:

```json
{ "text": "...", "provider": "anthropic", "model": "claude-sonnet-5-5", "failovers": [] }
```

### Tarefas

`POST /api/tarefas`

```json
{ "text": "escreva um e-mail para a cliente avisando o atraso", "kind": null, "provider": null }
```

`kind` pode ser `escrever`, `organizar`, `operar` (ou `null` para o Huginn decidir). Responde `201` com `{"id": "27b5d91b01"}` e a tarefa começa na hora.

| Rota | O que devolve |
|---|---|
| `GET /api/tarefas` | As últimas tarefas, da mais nova para a mais antiga: `id`, `request`, `created_at`, `status` |
| `GET /api/tarefas/{id}` | A tarefa com todos os eventos até agora |
| `GET /api/tarefas/{id}/eventos` | Os eventos como *Server-Sent Events*: primeiro os que já aconteceram, depois os novos, ao vivo, até o fim |

`status` é `running`, `finished`, `blocked` (o Vigia bloqueou) ou `failed`.

Cada evento é uma linha `data: {json}`. O campo `type` diz o que aconteceu:

| `type` | Campos | Quando |
|---|---|---|
| `started` | `request` | A tarefa começou |
| `kind_chosen` | `kind` (`write`, `organize`, `operate`) | O tipo foi definido |
| `stage_started` | `step`, `agent`, `name`, `what` | Um corvo começou |
| `stage_finished` | `step`, `agent`, `name`, `text`, `provider`, `model`, `failovers` | Um corvo terminou |
| `stage_skipped` | `step`, `agent`, `name`, `reason` | Uma etapa foi pulada |
| `blocked` | `reason` | O Vigia bloqueou o plano |
| `finished` | `summary` | Fim, com o resumo da Pena |
| `failed` | `error` | A tarefa parou por erro |

Exemplo com `curl`:

```bash
CODIGO=abcd-efgh-jkmn-pqrs
ID=$(curl -s -X POST -H "Authorization: Bearer $CODIGO" -H 'Content-Type: application/json' \
  -d '{"text":"escreva uma legenda para o Instagram sobre corvos"}' \
  http://127.0.0.1:7777/api/tarefas | jq -r .id)
curl -N -H "Authorization: Bearer $CODIGO" http://127.0.0.1:7777/api/tarefas/$ID/eventos
```

No navegador, `EventSource` não manda cabeçalhos; leia o fluxo com `fetch` e `response.body.getReader()`, como a página do painel faz.

### Celular

`GET /api/celular` responde `{"lan": true, "url": "...", "qr_svg": "<svg ...>"}` quando o painel foi aberto com `--rede`.

## Próximos passos

- **WhatsApp:** uma ponte que recebe mensagens, cria tarefas por `POST /api/tarefas` e devolve o resumo quando chega o evento `finished`.
- **Ver a tela ao vivo:** quando os corvos ganharem mãos no computador (fase 2), o painel vai mostrar a tela em tempo real, no computador e no celular, com confirmação de cada ação séria.
- **HTTPS no painel** para o modo `--rede`.
