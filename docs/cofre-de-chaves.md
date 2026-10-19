# Cofre de chaves de API

## A regra

> Só o dono do computador adiciona chaves. Depois de guardada, **ninguém consegue ver a chave**, nem o dono. Dá para listar, apagar e criar outra.

Por quê: chaves de API são dinheiro. Se a interface, a CLI ou a API local pudessem mostrar a chave, qualquer coisa que conseguisse usar essas interfaces (um script, uma extensão, um agente enganado por prompt injection, alguém olhando a tela) poderia copiá-la. Sem operação de leitura, esse caminho simplesmente não existe.

## O que dá para fazer

```bash
dotcrow chaves adicionar anthropic --rotulo "conta pessoal"   # lê sem eco na tela
dotcrow chaves listar
dotcrow chaves apagar 1a2b3c4d
```

`listar` mostra:

```
1a2b3c4d  anthropic   conta pessoal        8cfd-d087-daa7-e5ef  adicionada em 2026-10-19
```

A **impressão digital** (`8cfd-d087-...`) é um hash truncado da chave com separação de domínio. Serve para você conferir qual chave é qual (por exemplo, comparando com a impressão gerada em outro computador) sem que nenhum pedaço da chave apareça.

Para **trocar** uma chave: apague a antiga e adicione a nova.

## Como funciona por dentro

- O cofre inteiro é um JSON cifrado com **XChaCha20-Poly1305** e gravado de forma atômica em `vault.bin`, com permissão `0600` no Linux e macOS.
- A **chave mestra** que cifra o cofre hoje fica em `master.key`, também `0600`, na mesma pasta. **Isso é temporário.** A issue da fase 1 move a chave mestra para o chaveiro do sistema (Keychain no macOS, Credential Manager no Windows, Secret Service no Linux).
- Na memória, as chaves ficam em `Zeroizing<String>`, apagadas quando saem de uso.
- O único acesso ao texto da chave é `Vault::lease(provedor)`, que devolve um `Lease` com `with_secret(|chave| ...)`. Ele é usado pelo adaptador do provedor para montar o cabeçalho da requisição, e só.
- `Lease` e `Vault` têm `Debug` mascarado. Há testes garantindo que a chave não aparece em `Debug`, na listagem nem no arquivo em disco.
- Chaves duplicadas (mesmo provedor e mesma impressão digital) são recusadas.

## Garantias e limites

| Garantimos | Não garantimos |
|---|---|
| Nenhum comando, tela ou API local devolve a chave. | Um programa com privilégio de administrador lendo a memória do Ninho enquanto a chave é usada. |
| A chave não fica em texto puro no disco. | Proteção se alguém copiar `vault.bin` **e** `master.key` juntos (resolvido quando a chave mestra for para o chaveiro do sistema). |
| A chave não aparece em logs e mensagens de erro. | O que o provedor de LLM faz com a chave do lado dele. |

Na fase 1, o cofre passa a viver dentro do **Ninho** (o serviço local). A CLI e o app conversam com o Ninho por um canal local autenticado cujo protocolo **não tem** verbo de leitura de chave. Isso transforma a regra "ninguém vê a chave" de convenção de código em fronteira entre processos.

## Contribuindo aqui

Mudanças no cofre são área sensível: precisam de testes de não-vazamento e duas aprovações. **Nunca** adicione um comando "mostrar chave", nem para depuração.
